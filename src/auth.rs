//! Opaque, revocable sessions and invitation enrollment. No bearer secrets in SQLite.
use crate::{ApiError, App, conversations, internal, store};
use anyhow::{Context, Result, bail};
use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use axum::{
    Json,
    extract::{Extension, Request, State},
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::Semaphore;

#[derive(Clone)]
pub struct Auth {
    pub required: bool,
    pub origin: Option<String>,
    pub secure: bool,
    pub db: String,
    pub users_dir: PathBuf,
    pub hashing: Arc<Semaphore>,
    dummy_hash: String,
}
#[derive(Clone, Serialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub admin: bool,
    #[serde(skip)]
    pub session: String,
}
pub fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn secret() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}
pub(crate) fn now() -> i64 {
    (conversations::now() / 1000) as i64
}
fn error(status: StatusCode, message: &str) -> ApiError {
    ApiError(status, message.into())
}
fn unauthorized() -> ApiError {
    error(StatusCode::UNAUTHORIZED, "Please sign in to continue.")
}
pub(crate) fn username(name: &str) -> Result<String> {
    let name = name.trim().to_lowercase();
    if !(3..=40).contains(&name.len())
        || !name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
    {
        bail!("Use a username of 3–40 letters, numbers, dots, hyphens or underscores.");
    }
    Ok(name)
}
fn hash(password: &str) -> Result<String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|_| anyhow::anyhow!("Password hashing failed"))
}
pub fn init(path: &str) -> Result<()> {
    store::open(path)?.execute_batch("CREATE TABLE IF NOT EXISTS users (id TEXT PRIMARY KEY, username TEXT NOT NULL UNIQUE, password TEXT, admin INTEGER NOT NULL DEFAULT 0, active INTEGER NOT NULL DEFAULT 1);
    CREATE TABLE IF NOT EXISTS sessions (digest TEXT PRIMARY KEY, user_id TEXT NOT NULL, expires INTEGER NOT NULL, touched INTEGER NOT NULL);
    CREATE TABLE IF NOT EXISTS invitations (digest TEXT PRIMARY KEY, user_id TEXT NOT NULL UNIQUE, expires INTEGER NOT NULL);
    CREATE TABLE IF NOT EXISTS rate_limits (key TEXT NOT NULL, bucket INTEGER NOT NULL, count INTEGER NOT NULL, PRIMARY KEY(key,bucket));")?;
    crate::control::init(&store::open(path)?)?;
    Ok(())
}
impl Auth {
    pub fn configured(library: &str) -> Result<Self> {
        let mode = std::env::var("AUTH_MODE").unwrap_or_else(|_| "local".into());
        if !["local", "required"].contains(&mode.as_str()) {
            bail!("AUTH_MODE must be local or required");
        }
        let required = mode == "required";
        let origin = std::env::var("APP_ORIGIN").ok();
        let production = std::env::var("APP_ENV").is_ok_and(|v| v == "production");
        if production && !required {
            bail!("Production requires AUTH_MODE=required");
        }
        let mut secure = false;
        if let Some(value) = &origin {
            let url = reqwest::Url::parse(value)?;
            let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
            if url.origin().ascii_serialization() != *value
                || !(url.scheme() == "https" || (url.scheme() == "http" && loopback && !production))
            {
                bail!(
                    "APP_ORIGIN must be an exact HTTPS origin (HTTP loopback is allowed only outside production)"
                );
            }
            secure = url.scheme() == "https";
        } else if required {
            bail!("Authenticated mode requires APP_ORIGIN");
        }
        if !required && origin.is_some() {
            bail!("APP_ORIGIN requires authenticated mode");
        }
        let root = std::path::Path::new(library)
            .parent()
            .unwrap_or(std::path::Path::new("data"));
        let db = root.join("auth.sqlite").to_string_lossy().to_string();
        let users_dir = root.join("users");
        if required {
            init(&db)?;
            std::fs::create_dir_all(&users_dir)?;
        }
        Ok(Self {
            required,
            origin,
            secure,
            db,
            users_dir,
            hashing: Arc::new(Semaphore::new(2)),
            dummy_hash: if required {
                hash(&secret())?
            } else {
                String::new()
            },
        })
    }
    fn cookie_name(&self) -> &str {
        if self.secure {
            "__Host-ordo_session"
        } else {
            "ordo_session"
        }
    }
    fn cookie(&self, value: &str, age: i64) -> String {
        format!(
            "{}={}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}{}",
            self.cookie_name(),
            value,
            age,
            if self.secure { "; Secure" } else { "" }
        )
    }
    fn token(&self, headers: &HeaderMap) -> Option<String> {
        let prefix = format!("{}=", self.cookie_name());
        headers
            .get(header::COOKIE)?
            .to_str()
            .ok()?
            .split(';')
            .find_map(|part| part.trim().strip_prefix(&prefix))
            .filter(|v| v.len() == 64 && v.bytes().all(|c| c.is_ascii_hexdigit()))
            .map(digest)
    }
    pub fn current(&self, headers: &HeaderMap) -> Result<Option<User>> {
        let Some(token) = self.token(headers) else {
            return Ok(None);
        };
        self.by_session(&token)
    }
    fn by_session(&self, token: &str) -> Result<Option<User>> {
        let db = store::open(&self.db)?;
        let user=db.query_row("SELECT u.id,u.username,u.admin FROM sessions s JOIN users u ON u.id=s.user_id WHERE s.digest=?1 AND s.expires>?2 AND s.touched>?3 AND u.active=1",params![token,now(),now()-3600],|row|Ok(User{id:row.get(0)?,username:row.get(1)?,admin:row.get(2)?,session:token.into()})).optional()?;
        if user.is_some() {
            db.execute(
                "UPDATE sessions SET touched=?2 WHERE digest=?1 AND touched<?3",
                params![token, now(), now() - 60],
            )?;
        }
        Ok(user)
    }
    pub fn valid(&self, user: Option<&User>) -> bool {
        !self.required
            || user.is_some_and(|u| self.by_session(&u.session).is_ok_and(|u| u.is_some()))
    }
    pub async fn guard<T>(
        &self,
        user: Option<&User>,
        work: impl std::future::Future<Output = Result<T>>,
    ) -> Result<T> {
        if !self.valid(user) {
            bail!("Session ended");
        }
        tokio::pin!(work);
        loop {
            tokio::select! {
                result=&mut work=>return result,
                _=tokio::time::sleep(std::time::Duration::from_secs(2))=>{if !self.valid(user){bail!("Session ended");}}
            }
        }
    }
    pub fn rate(&self, key: &str, seconds: i64, limit: i64) -> Result<(), ApiError> {
        let mut db = store::open(&self.db).map_err(internal)?;
        let tx = db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(internal)?;
        let bucket = (now() / seconds) * seconds;
        tx.execute(
            "DELETE FROM rate_limits WHERE bucket < ?1",
            [now() - 172800],
        )
        .map_err(internal)?;
        tx.execute(
            "DELETE FROM rate_limits WHERE key=?1 AND bucket<?2",
            params![key, bucket],
        )
        .map_err(internal)?;
        tx.execute("INSERT INTO rate_limits VALUES(?1,?2,1) ON CONFLICT(key,bucket) DO UPDATE SET count=count+1",params![key,bucket]).map_err(internal)?;
        let count: i64 = tx
            .query_row(
                "SELECT count FROM rate_limits WHERE key=?1 AND bucket=?2",
                params![key, bucket],
                |r| r.get(0),
            )
            .map_err(internal)?;
        tx.commit().map_err(internal)?;
        if count > limit {
            return Err(error(
                StatusCode::TOO_MANY_REQUESTS,
                "Request limit reached. Please try again later.",
            ));
        }
        Ok(())
    }
    pub fn invite(&self, name: &str, admin: bool) -> Result<String> {
        let mut db = store::open(&self.db)?;
        let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let url = self.invite_tx(&tx, name, admin)?;
        crate::control::audit(
            &tx,
            "operator",
            "user.invited",
            &json!({"username":username(name)?,"admin":admin}),
        )?;
        tx.commit()?;
        Ok(url)
    }
    pub(crate) fn invite_tx(
        &self,
        db: &rusqlite::Connection,
        name: &str,
        admin: bool,
    ) -> Result<String> {
        let name = username(name)?;
        let count: i64 = db.query_row("SELECT count(*) FROM users", [], |r| r.get(0))?;
        if count >= 100 {
            bail!("This instance is limited to 100 accounts");
        }
        let id = conversations::id();
        db.execute(
            "INSERT INTO users(id,username,admin) VALUES(?1,?2,?3)",
            params![id, name, admin],
        )
        .context("Username already exists")?;
        let token = secret();
        db.execute(
            "INSERT INTO invitations VALUES(?1,?2,?3)",
            params![digest(&token), id, now() + 86400],
        )?;
        Ok(format!(
            "{}#invite={token}",
            self.origin.as_deref().unwrap_or("")
        ))
    }
}
pub async fn boundary(State(app): State<App>, mut request: Request, next: Next) -> Response {
    let result: Result<(), ApiError> = (|| {
        let path = request.uri().path();
        let host = request
            .headers()
            .get(header::HOST)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| reqwest::Url::parse(&format!("http://{v}")).ok());
        let local = host
            .as_ref()
            .is_some_and(|u| matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")));
        if app.auth.required {
            let origin = app.auth.origin.as_ref().unwrap();
            let expected = reqwest::Url::parse(origin).map_err(internal)?;
            let actual = request
                .headers()
                .get(header::HOST)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            let authority = &expected.as_str()[url_authority_range(&expected)];
            if actual != authority && !(path == "/api/health" && local) {
                return Err(error(StatusCode::FORBIDDEN, "Untrusted host"));
            }
            if let Some(value) = request.headers().get(header::ORIGIN) {
                if value.to_str().ok() != Some(origin) {
                    return Err(error(StatusCode::FORBIDDEN, "Cross-origin access denied"));
                }
            } else if !matches!(
                *request.method(),
                axum::http::Method::GET | axum::http::Method::HEAD | axum::http::Method::OPTIONS
            ) {
                return Err(error(StatusCode::FORBIDDEN, "Same-origin request required"));
            }
            if path.starts_with("/api/")
                && path != "/api/health"
                && !["/api/auth/session", "/api/auth/login", "/api/auth/enroll"].contains(&path)
            {
                let user = app
                    .auth
                    .current(request.headers())
                    .map_err(internal)?
                    .ok_or_else(unauthorized)?;
                app.auth.rate(&format!("api:{}", user.id), 60, 120)?;
                if path == "/api/chat" {
                    app.auth.rate(&format!("chat:{}", user.id), 86400, 50)?;
                    app.auth.rate("chat:instance", 86400, 500)?;
                }
                if path == "/api/attachments" && request.method() == axum::http::Method::POST {
                    app.auth.rate(&format!("upload:{}", user.id), 86400, 50)?;
                }
                let mut scoped = app.clone();
                scoped.db = app
                    .auth
                    .users_dir
                    .join(format!("{}.sqlite", user.id))
                    .to_string_lossy()
                    .to_string();
                scoped.user = Some(user);
                crate::control::authorize_route(&scoped, path, request.method())?;
                conversations::init(&store::open(&scoped.db).map_err(internal)?)
                    .map_err(internal)?;
                request.extensions_mut().insert(scoped);
            }
        } else {
            if !local {
                return Err(error(StatusCode::FORBIDDEN, "Loopback host required"));
            }
            if let Some(value) = request.headers().get(header::ORIGIN) {
                let origin = value
                    .to_str()
                    .ok()
                    .and_then(|v| reqwest::Url::parse(v).ok());
                if !origin.is_some_and(|u| {
                    u.scheme() == "http"
                        && matches!(u.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
                        && (u.port_or_known_default()
                            == host.as_ref().and_then(|u| u.port_or_known_default())
                            || u.port() == Some(5173))
                }) {
                    return Err(error(StatusCode::FORBIDDEN, "Cross-origin access denied"));
                }
            }
            request.extensions_mut().insert(app.clone());
        }
        Ok(())
    })();
    let control_request = request.uri().path().starts_with("/api/control/");
    let actor = request
        .extensions()
        .get::<App>()
        .and_then(|a| a.user.as_ref())
        .map(|u| u.username.clone());
    let mut response = match result {
        Ok(()) => next.run(request).await,
        Err(e) => e.into_response(),
    };
    if control_request
        && response.status().as_u16() >= 400
        && let Some(actor) = actor
        && let Ok(db) = store::open(&app.auth.db)
    {
        let _ = db.execute(
            "INSERT INTO audit(created,actor,event,details) VALUES(?1,?2,'command.rejected',?3)",
            params![
                now(),
                actor,
                json!({"http_status":response.status().as_u16()}).to_string()
            ],
        );
    }
    for (name, value) in [
        ("x-content-type-options", "nosniff"),
        ("referrer-policy", "no-referrer"),
        ("x-frame-options", "DENY"),
        ("cache-control", "no-store"),
        (
            "content-security-policy",
            "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; media-src 'self' blob:; connect-src 'self'; font-src 'self'; object-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'self'",
        ),
    ] {
        response.headers_mut().insert(
            axum::http::HeaderName::from_static(name),
            value.parse().unwrap(),
        );
    }
    if app.auth.secure {
        response.headers_mut().insert(
            "strict-transport-security",
            "max-age=31536000".parse().unwrap(),
        );
    }
    response
}
fn url_authority_range(url: &reqwest::Url) -> std::ops::Range<usize> {
    let start = url.scheme().len() + 3;
    start..url.as_str().len() - 1
}
pub async fn session(State(app): State<App>, headers: HeaderMap) -> Result<Json<Value>, ApiError> {
    let user = if app.auth.required {
        app.auth.current(&headers).map_err(internal)?
    } else {
        None
    };
    Ok(Json(
        json!({"required":app.auth.required,"user":user,"capabilities":crate::control::capabilities(&app.auth,user.as_ref()).map_err(internal)?,"flags":crate::control::flags(&app.auth).map_err(internal)?}),
    ))
}
#[derive(Deserialize)]
pub struct Credentials {
    username: Option<String>,
    password: String,
    token: Option<String>,
}
async fn establish(
    app: App,
    headers: HeaderMap,
    credentials: Credentials,
    enrollment: bool,
) -> Result<Response, ApiError> {
    if !app.auth.required {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Authentication is not enabled",
        ));
    }
    app.auth.rate("login:global", 900, 60)?;
    if !(12..=128).contains(&credentials.password.chars().count()) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Use a password between 12 and 128 characters.",
        ));
    }
    let name = username(credentials.username.as_deref().unwrap_or("invalid"))
        .map_err(|_| unauthorized())?;
    if !enrollment {
        app.auth
            .rate(&format!("login:{}", digest(&name)), 900, 10)?;
    }
    let token = credentials.token.unwrap_or_default();
    let db = store::open(&app.auth.db).map_err(internal)?;
    let row: Option<(String, String, bool, Option<String>)> = if enrollment {
        if token.len() != 64 {
            return Err(unauthorized());
        }
        db.query_row("SELECT u.id,u.username,u.admin,u.password FROM invitations i JOIN users u ON u.id=i.user_id WHERE i.digest=?1 AND i.expires>?2 AND u.active=1 AND u.password IS NULL",params![digest(&token),now()],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).optional().map_err(internal)?
    } else {
        db.query_row(
            "SELECT id,username,admin,password FROM users WHERE username=?1 AND active=1",
            [name],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()
        .map_err(internal)?
    };
    drop(db);
    let old_hash = row
        .as_ref()
        .and_then(|r| r.3.clone())
        .unwrap_or_else(|| app.auth.dummy_hash.clone());
    let password = credentials.password;
    let permit = app.auth.hashing.clone().try_acquire_owned().map_err(|_| {
        error(
            StatusCode::TOO_MANY_REQUESTS,
            "Sign-in is busy. Try again shortly.",
        )
    })?;
    let encoded = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        if enrollment {
            hash(&password)
        } else {
            let parsed =
                PasswordHash::new(&old_hash).map_err(|_| anyhow::anyhow!("Invalid password"))?;
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .map_err(|_| anyhow::anyhow!("Invalid password"))?;
            Ok(old_hash)
        }
    })
    .await
    .map_err(internal)?
    .map_err(|_| unauthorized())?;
    let Some((id, name, admin, original)) = row else {
        return Err(unauthorized());
    };
    let mut db = store::open(&app.auth.db).map_err(internal)?;
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(internal)?;
    let active: bool = tx
        .query_row(
            "SELECT active=1 AND password IS ?2 FROM users WHERE id=?1",
            params![id, original],
            |r| r.get(0),
        )
        .map_err(internal)?;
    if !active {
        return Err(unauthorized());
    }
    if enrollment {
        let consumed = tx
            .execute(
                "DELETE FROM invitations WHERE digest=?1 AND user_id=?2 AND expires>?3",
                params![digest(&token), id, now()],
            )
            .map_err(internal)?;
        if consumed != 1 {
            return Err(unauthorized());
        }
        tx.execute(
            "UPDATE users SET password=?2 WHERE id=?1",
            params![id, encoded],
        )
        .map_err(internal)?;
    }
    if let Some(old) = app.auth.token(&headers) {
        tx.execute("DELETE FROM sessions WHERE digest=?1", [old])
            .map_err(internal)?;
    }
    tx.execute(
        "DELETE FROM sessions WHERE expires<=?1 OR touched<=?2",
        params![now(), now() - 3600],
    )
    .map_err(internal)?;
    // Bound stored sessions per account; older sessions remain revocable through logout/reset.
    tx.execute("DELETE FROM sessions WHERE user_id=?1 AND digest NOT IN (SELECT digest FROM sessions WHERE user_id=?1 ORDER BY touched DESC LIMIT 4)",[&id]).map_err(internal)?;
    let bearer = secret();
    tx.execute(
        "INSERT INTO sessions VALUES(?1,?2,?3,?4)",
        params![digest(&bearer), id, now() + 28800, now()],
    )
    .map_err(internal)?;
    tx.commit().map_err(internal)?;
    let mut response =
        Json(json!({"user":{"id":id,"username":name,"admin":admin}})).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        app.auth.cookie(&bearer, 28800).parse().unwrap(),
    );
    Ok(response)
}
pub async fn login(
    State(app): State<App>,
    headers: HeaderMap,
    Json(c): Json<Credentials>,
) -> Result<Response, ApiError> {
    establish(app, headers, c, false).await
}
pub async fn enroll(
    State(app): State<App>,
    headers: HeaderMap,
    Json(c): Json<Credentials>,
) -> Result<Response, ApiError> {
    establish(app, headers, c, true).await
}
pub async fn logout(
    Extension(app): Extension<App>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if let Some(token) = app.auth.token(&headers) {
        store::open(&app.auth.db)
            .map_err(internal)?
            .execute("DELETE FROM sessions WHERE digest=?1", [token])
            .map_err(internal)?;
    }
    let mut response = Json(json!({"ok":true})).into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, app.auth.cookie("", 0).parse().unwrap());
    Ok(response)
}
pub fn cli(auth: &Auth, args: &[String]) -> Result<()> {
    if !auth.required {
        bail!("Set AUTH_MODE=required and APP_ORIGIN before account administration");
    }
    match args.first().map(String::as_str) {
        Some("invite") => {
            let name = args.get(1).context("Provide a username")?;
            println!(
                "{}",
                auth.invite(name, args.iter().any(|a| a == "--admin"))?
            );
        }
        Some("reset") => {
            let name = username(args.get(1).context("Provide a username")?)?;
            let mut db = store::open(&auth.db)?;
            let tx = db.transaction()?;
            let id: String = tx
                .query_row("SELECT id FROM users WHERE username=?1", [&name], |r| {
                    r.get(0)
                })
                .context("Unknown account")?;
            tx.execute("UPDATE users SET active=1,password=NULL WHERE id=?1", [&id])?;
            tx.execute("DELETE FROM sessions WHERE user_id=?1", [&id])?;
            tx.execute("DELETE FROM invitations WHERE user_id=?1", [&id])?;
            let token = secret();
            tx.execute(
                "INSERT INTO invitations VALUES(?1,?2,?3)",
                params![digest(&token), id, now() + 86400],
            )?;
            crate::control::audit(&tx, "operator", "user.reset", &json!({"username":name}))?;
            tx.commit()?;
            println!("{}#invite={token}", auth.origin.as_deref().unwrap_or(""));
        }
        Some("disable") => {
            let name = username(args.get(1).context("Provide a username")?)?;
            let mut db = store::open(&auth.db)?;
            let tx = db.transaction()?;
            tx.execute("UPDATE users SET active=0 WHERE username=?1", [&name])?;
            tx.execute(
                "DELETE FROM sessions WHERE user_id IN (SELECT id FROM users WHERE username=?1)",
                [&name],
            )?;
            crate::control::audit(&tx, "operator", "user.disabled", &json!({"username":name}))?;
            tx.commit()?;
            println!("Account disabled and sessions revoked");
        }
        _ => bail!(
            "Usage: auth invite USERNAME [--admin] | auth disable USERNAME | auth reset USERNAME"
        ),
    }
    Ok(())
}
