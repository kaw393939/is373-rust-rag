//! Durable local conversations and bounded, inert multimedia attachments.
use crate::{ApiError, App, internal, store};
use anyhow::{Result, bail};
use axum::{
    Json,
    extract::{Extension, Multipart, Path},
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub fn storage_limit() -> usize {
    std::env::var("ATTACHMENT_QUOTA_MB")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(500)
        .clamp(24, 500)
        * 1024
        * 1024
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
pub fn id() -> String {
    uuid::Uuid::new_v4().to_string()
}
#[derive(Clone, Serialize, Deserialize, Default)]
pub struct Attachment {
    pub id: String,
    pub name: String,
    pub mime: String,
    pub size: usize,
    #[serde(default)]
    pub input_state: String,
    #[serde(default)]
    pub input_note: Option<String>,
    #[serde(default)]
    pub transcript: Option<crate::providers::Transcript>,
    #[serde(default)]
    pub transcription_key: Option<String>,
    #[serde(default)]
    pub processing_key: Option<String>,
    #[serde(default)]
    pub document: Option<crate::media::DocumentRecord>,
    #[serde(default)]
    pub video: Option<crate::media::VideoRecord>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: String,
    pub content: String,
    pub created: u64,
    #[serde(default)]
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub evidence: Vec<store::Chunk>,
    pub outcome: String,
    pub model: String,
    #[serde(default)]
    pub mode: String,
    pub notice: Option<String>,
}
impl Message {
    pub fn new(role: &str, content: String) -> Self {
        Self {
            id: id(),
            role: role.into(),
            content,
            created: now(),
            attachments: vec![],
            evidence: vec![],
            outcome: "complete".into(),
            model: String::new(),
            mode: "chat".into(),
            notice: None,
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub pinned: bool,
    pub archived: bool,
    pub created: u64,
    pub updated: u64,
    pub messages: Vec<Message>,
}
pub fn init(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE IF NOT EXISTS conversations (id TEXT PRIMARY KEY, value TEXT NOT NULL);
        CREATE TABLE IF NOT EXISTS attachments (id TEXT PRIMARY KEY, meta TEXT NOT NULL, data BLOB NOT NULL);
        CREATE TABLE IF NOT EXISTS media_assets (id TEXT PRIMARY KEY, attachment_id TEXT NOT NULL, data BLOB NOT NULL);")?;
    Ok(())
}
fn open(path: &str) -> Result<Connection> {
    let db = store::open(path)?;
    init(&db)?;
    Ok(db)
}
pub fn load(path: &str, id: &str) -> Result<Conversation> {
    let raw: String =
        open(path)?.query_row("SELECT value FROM conversations WHERE id=?1", [id], |r| {
            r.get(0)
        })?;
    Ok(serde_json::from_str(&raw)?)
}
fn insert(path: &str, c: &Conversation) -> Result<()> {
    let mut db = open(path)?;
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let count: i64 = tx.query_row("SELECT count(*) FROM conversations", [], |r| r.get(0))?;
    if count >= 500 {
        anyhow::bail!("Conversation limit reached (500)");
    }
    tx.execute(
        "INSERT INTO conversations VALUES (?1,?2)",
        params![c.id, serde_json::to_string(c)?],
    )?;
    tx.commit()?;
    Ok(())
}
pub fn update(path: &str, id: &str, f: impl FnOnce(&mut Conversation)) -> Result<()> {
    let mut db = open(path)?;
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let raw: String = tx.query_row("SELECT value FROM conversations WHERE id=?1", [id], |r| {
        r.get(0)
    })?;
    let mut c: Conversation = serde_json::from_str(&raw)?;
    f(&mut c);
    c.updated = now();
    tx.execute(
        "UPDATE conversations SET value=?2 WHERE id=?1",
        params![id, serde_json::to_string(&c)?],
    )?;
    tx.commit()?;
    Ok(())
}
pub async fn list(Extension(app): Extension<App>) -> Result<Json<Value>, ApiError> {
    let db = open(&app.db).map_err(internal)?;
    let mut stmt = db
        .prepare("SELECT value FROM conversations")
        .map_err(internal)?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(internal)?;
    let mut result = vec![];
    for raw in rows {
        let c: Conversation = serde_json::from_str(&raw.map_err(internal)?).map_err(internal)?;
        result.push(json!({"id":c.id,"title":c.title,"pinned":c.pinned,"archived":c.archived,"created":c.created,"updated":c.updated,
            "message_count":c.messages.len(),"preview":c.messages.iter().rev().find(|m|m.role=="user").map(|m|m.content.chars().take(120).collect::<String>())}));
    }
    result.sort_by_key(|v| std::cmp::Reverse(v["updated"].as_u64().unwrap_or(0)));
    Ok(Json(json!(result)))
}
pub async fn create(Extension(app): Extension<App>) -> Result<Json<Conversation>, ApiError> {
    let c = Conversation {
        id: id(),
        title: "New conversation".into(),
        pinned: false,
        archived: false,
        created: now(),
        updated: now(),
        messages: vec![],
    };
    insert(&app.db, &c).map_err(internal)?;
    Ok(Json(c))
}
pub async fn get(
    Extension(app): Extension<App>,
    Path(id): Path<String>,
) -> Result<Json<Conversation>, ApiError> {
    Ok(Json(load(&app.db, &id).map_err(|_| {
        ApiError(StatusCode::NOT_FOUND, "Conversation not found.".into())
    })?))
}
#[derive(Deserialize)]
pub struct Patch {
    title: Option<String>,
    pinned: Option<bool>,
    archived: Option<bool>,
}
pub async fn patch(
    Extension(app): Extension<App>,
    Path(id): Path<String>,
    Json(p): Json<Patch>,
) -> Result<Json<Value>, ApiError> {
    if p.title
        .as_ref()
        .is_some_and(|s| s.trim().is_empty() || s.chars().count() > 100)
    {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            "Title must be 1–100 characters.".into(),
        ));
    }
    update(&app.db, &id, |c| {
        if let Some(t) = p.title {
            c.title = t.trim().into();
        }
        if let Some(v) = p.pinned {
            c.pinned = v;
        }
        if let Some(v) = p.archived {
            c.archived = v;
        }
    })
    .map_err(internal)?;
    Ok(Json(json!({"ok":true})))
}
#[derive(Deserialize)]
pub struct Fork {
    before: String,
}
pub async fn fork(
    Extension(app): Extension<App>,
    Path(id): Path<String>,
    Json(f): Json<Fork>,
) -> Result<Json<Conversation>, ApiError> {
    let mut c = load(&app.db, &id).map_err(internal)?;
    let index = c
        .messages
        .iter()
        .position(|m| m.id == f.before)
        .ok_or_else(|| ApiError(StatusCode::BAD_REQUEST, "Message not found.".into()))?;
    c.id = self::id();
    c.title = format!("{} · branch", c.title.chars().take(70).collect::<String>());
    c.messages.truncate(index);
    c.created = now();
    c.updated = now();
    c.pinned = false;
    c.archived = false;
    insert(&app.db, &c).map_err(internal)?;
    Ok(Json(c))
}

/// Flush on completion, on bounded intervals and when the stream is dropped.
pub struct RunRecord {
    pub path: String,
    pub conversation: Option<String>,
    pub message: Message,
    pub last_flush: std::time::Instant,
}
impl RunRecord {
    pub fn flush(&mut self) -> Result<()> {
        if let Some(id) = &self.conversation {
            let message = self.message.clone();
            update(&self.path, id, |c| {
                if let Some(m) = c.messages.iter_mut().find(|m| m.id == message.id) {
                    *m = message;
                }
            })?;
        }
        self.last_flush = std::time::Instant::now();
        Ok(())
    }
    pub fn finish(&mut self, outcome: &str, notice: Option<&str>) -> Result<()> {
        self.message.outcome = outcome.into();
        self.message.notice = notice.map(str::to_string);
        self.flush()
    }
}
impl Drop for RunRecord {
    fn drop(&mut self) {
        if self.message.outcome == "streaming" {
            self.message.outcome = "interrupted".into();
            self.message.notice = Some(
                "The connection ended. Partial text was saved; retry to create a new attempt."
                    .into(),
            );
        }
        if let Err(e) = self.flush() {
            eprintln!("Save interrupted run: {e}");
        }
    }
}
pub fn recover(path: &str) -> Result<()> {
    let db = open(path)?;
    let mut stmt = db.prepare("SELECT value FROM conversations")?;
    let rows = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for raw in rows {
        let c: Conversation = serde_json::from_str(&raw)?;
        if c.messages.iter().any(|m| m.outcome == "streaming") {
            update(path, &c.id, |c| {
                for m in &mut c.messages {
                    if m.outcome == "streaming" {
                        m.outcome = "interrupted".into();
                        m.notice = Some(
                            "Server restarted during generation. Saved partial text is preserved."
                                .into(),
                        );
                    }
                }
            })?;
        }
    }
    Ok(())
}

fn media_type(name: &str, bytes: &[u8]) -> Result<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok("image/png");
    }
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return Ok("image/jpeg");
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Ok("image/gif");
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return Ok("image/webp");
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE") {
        return Ok("audio/wav");
    }
    if bytes.starts_with(b"ID3")
        || bytes.starts_with(&[0xff, 0xfb])
        || bytes.starts_with(&[0xff, 0xf3])
    {
        return Ok("audio/mpeg");
    }
    if bytes.starts_with(b"OggS") {
        return Ok("audio/ogg");
    }
    if bytes.starts_with(b"fLaC") {
        return Ok("audio/flac");
    }
    if bytes.get(4..8) == Some(b"ftyp") {
        return Ok(if name.to_lowercase().ends_with(".m4a") {
            "audio/mp4"
        } else {
            "video/mp4"
        });
    }
    if bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        return Ok(if name.to_lowercase().ends_with(".weba") {
            "audio/webm"
        } else {
            "video/webm"
        });
    }
    if bytes.starts_with(b"%PDF-") {
        return Ok("application/pdf");
    }
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    if ["txt", "md", "csv", "json"].contains(&ext.as_str())
        && std::str::from_utf8(bytes).is_ok()
        && !bytes.contains(&0)
    {
        return Ok("text/plain");
    }
    bail!(
        "Unsupported file. Use PNG, JPEG, WebP, GIF, MP3, WAV, OGG, MP4, WebM, PDF or UTF-8 text."
    )
}
pub async fn upload(
    Extension(app): Extension<App>,
    mut multipart: Multipart,
) -> Result<Json<Attachment>, ApiError> {
    let field = multipart
        .next_field()
        .await
        .map_err(|_| ApiError(StatusCode::BAD_REQUEST, "Invalid upload.".into()))?
        .ok_or_else(|| ApiError(StatusCode::BAD_REQUEST, "Choose a file.".into()))?;
    let name = field
        .file_name()
        .unwrap_or("attachment")
        .chars()
        .filter(|c| !c.is_control() && *c != '/' && *c != '\\')
        .take(180)
        .collect::<String>();
    let declared = field.content_type().unwrap_or("").to_owned();
    let bytes = field
        .bytes()
        .await
        .map_err(|_| ApiError(StatusCode::PAYLOAD_TOO_LARGE, "File exceeds 12 MB.".into()))?;
    if bytes.is_empty() || bytes.len() > 12 * 1024 * 1024 {
        return Err(ApiError(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Choose a nonempty file up to 12 MB.".into(),
        ));
    }
    let mime = media_type(&name, &bytes)
        .map_err(|e| ApiError(StatusCode::UNSUPPORTED_MEDIA_TYPE, e.to_string()))?;
    let mime = match (mime, declared.split(';').next().unwrap_or("")) {
        ("video/webm", "audio/webm") => "audio/webm",
        ("video/mp4", "audio/mp4") => "audio/mp4",
        _ => mime,
    };
    if mime == "text/plain" && bytes.len() > 24_000 {
        return Err(ApiError(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Text attachments are limited to 24 KB for local context.".into(),
        ));
    }
    let meta = Attachment {
        id: id(),
        name,
        mime: mime.into(),
        size: bytes.len(),
        ..Default::default()
    };
    let mut db = open(&app.db).map_err(internal)?;
    let tx = db
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(internal)?;
    let used: i64 = tx
        .query_row(
            "SELECT (SELECT COALESCE(SUM(length(data)),0) FROM attachments) + (SELECT COALESCE(SUM(length(data)),0) FROM media_assets)",
            [],
            |r| r.get(0),
        )
        .map_err(internal)?;
    if used + bytes.len() as i64 > storage_limit() as i64 {
        return Err(ApiError(
            StatusCode::INSUFFICIENT_STORAGE,
            "Your attachment storage limit has been reached.".into(),
        ));
    }
    tx.execute(
        "INSERT INTO attachments VALUES (?1,?2,?3)",
        params![
            meta.id,
            serde_json::to_string(&meta).map_err(internal)?,
            bytes.as_ref()
        ],
    )
    .map_err(internal)?;
    tx.commit().map_err(internal)?;
    Ok(Json(meta))
}
pub fn attachment(path: &str, id: &str) -> Result<(Attachment, Vec<u8>)> {
    let (meta, bytes): (String, Vec<u8>) =
        open(path)?.query_row("SELECT meta,data FROM attachments WHERE id=?1", [id], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })?;
    Ok((serde_json::from_str(&meta)?, bytes))
}
pub fn save_attachment(path: &str, meta: &Attachment) -> Result<()> {
    open(path)?.execute(
        "UPDATE attachments SET meta=?2 WHERE id=?1",
        params![meta.id, serde_json::to_string(meta)?],
    )?;
    Ok(())
}

pub fn save_processed(path: &str, meta: &Attachment, assets: &[(String, Vec<u8>)]) -> Result<()> {
    let mut db = open(path)?;
    let tx = db.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let used:usize=tx.query_row("SELECT (SELECT COALESCE(SUM(length(data)),0) FROM attachments) + (SELECT COALESCE(SUM(length(data)),0) FROM media_assets)",[],|r|r.get(0))?;
    if used + assets.iter().map(|(_, bytes)| bytes.len()).sum::<usize>() > storage_limit() {
        bail!("Your attachment and processed-media storage limit has been reached");
    }
    for (id, bytes) in assets {
        tx.execute(
            "INSERT INTO media_assets VALUES (?1,?2,?3)",
            params![id, meta.id, bytes],
        )?;
    }
    tx.execute(
        "UPDATE attachments SET meta=?2 WHERE id=?1",
        params![meta.id, serde_json::to_string(meta)?],
    )?;
    tx.commit()?;
    Ok(())
}
pub fn asset(path: &str, attachment_id: &str, asset_id: &str) -> Result<Vec<u8>> {
    Ok(open(path)?.query_row(
        "SELECT data FROM media_assets WHERE id=?1 AND attachment_id=?2",
        params![asset_id, attachment_id],
        |r| r.get(0),
    )?)
}
pub async fn download_asset(
    Extension(app): Extension<App>,
    Path((parent, id)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let bytes = asset(&app.db, &parent, &id)
        .map_err(|_| ApiError(StatusCode::NOT_FOUND, "Processed image not found.".into()))?;
    Ok((
        [
            (header::CONTENT_TYPE, "image/jpeg"),
            (
                header::CACHE_CONTROL,
                "private, max-age=31536000, immutable",
            ),
        ],
        bytes,
    )
        .into_response())
}

fn range(value: &str, len: usize) -> Option<(usize, usize)> {
    let (start, end) = value.strip_prefix("bytes=")?.split_once('-')?;
    if start.is_empty() {
        let suffix: usize = end.parse().ok()?;
        if suffix == 0 {
            return None;
        }
        return Some((len.saturating_sub(suffix), len - 1));
    }
    let start: usize = start.parse().ok()?;
    let end = if end.is_empty() {
        len - 1
    } else {
        end.parse::<usize>().ok()?.min(len - 1)
    };
    if start >= len || start > end {
        None
    } else {
        Some((start, end))
    }
}
pub async fn download(
    Extension(app): Extension<App>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let (meta, bytes) = attachment(&app.db, &id)
        .map_err(|_| ApiError(StatusCode::NOT_FOUND, "Attachment not found.".into()))?;
    let len = bytes.len();
    let mut status = StatusCode::OK;
    let mut out = HeaderMap::new();
    out.insert(header::CONTENT_TYPE, meta.mime.parse().unwrap());
    out.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
    out.insert(header::X_CONTENT_TYPE_OPTIONS, "nosniff".parse().unwrap());
    out.insert(
        header::CONTENT_SECURITY_POLICY,
        "sandbox; default-src 'none'".parse().unwrap(),
    );
    let safe_name = meta
        .name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '-' | '_'))
        .collect::<String>();
    out.insert(
        header::CONTENT_DISPOSITION,
        format!(
            "{}; filename=\"{}\"",
            if meta.mime == "application/pdf" || meta.mime == "text/plain" {
                "attachment"
            } else {
                "inline"
            },
            safe_name
        )
        .parse()
        .unwrap(),
    );
    let bytes = if let Some(value) = headers.get(header::RANGE) {
        let (start, end) = value
            .to_str()
            .ok()
            .and_then(|v| range(v, len))
            .ok_or_else(|| {
                ApiError(
                    StatusCode::RANGE_NOT_SATISFIABLE,
                    "Invalid byte range.".into(),
                )
            })?;
        status = StatusCode::PARTIAL_CONTENT;
        out.insert(
            header::CONTENT_RANGE,
            format!("bytes {start}-{end}/{len}").parse().unwrap(),
        );
        bytes[start..=end].to_vec()
    } else {
        bytes
    };
    Ok((status, out, bytes).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_active_content_and_honors_ranges() {
        assert!(media_type("x.svg", b"<svg onload='alert(1)'/>").is_err());
        assert!(media_type("x.png", b"<html>bad</html>").is_err());
        assert_eq!(range("bytes=2-5", 10), Some((2, 5)));
        assert_eq!(range("bytes=-3", 10), Some((7, 9)));
        assert_eq!(range("bytes=10-", 10), None);
    }
    #[test]
    fn saves_partial_run_on_disconnect() {
        let path = std::env::temp_dir()
            .join(format!("chat-{}.sqlite", id()))
            .to_string_lossy()
            .into_owned();
        let c = Conversation {
            id: id(),
            title: "Test".into(),
            pinned: false,
            archived: false,
            created: now(),
            updated: now(),
            messages: vec![],
        };
        insert(&path, &c).unwrap();
        let mut m = Message::new("assistant", "partial".into());
        m.outcome = "streaming".into();
        update(&path, &c.id, |c| c.messages.push(m.clone())).unwrap();
        {
            let _run = RunRecord {
                path: path.clone(),
                conversation: Some(c.id.clone()),
                message: m,
                last_flush: std::time::Instant::now(),
            };
        }
        let saved = load(&path, &c.id).unwrap();
        assert_eq!(saved.messages[0].content, "partial");
        assert_eq!(saved.messages[0].outcome, "interrupted");
        std::fs::remove_file(path).unwrap();
    }
}
