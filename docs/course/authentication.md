# Accounts, capabilities, and private data

Ordo has two explicit modes. `AUTH_MODE=local` is the original loopback-only learning setup. `AUTH_MODE=required` enables invitation enrollment, password login and account-scoped storage. `APP_ENV=production` refuses to start unless authentication is required and `APP_ORIGIN` is an exact HTTPS origin. Public deployments use the production profile; changing a Docker port does not enable authentication.

## Follow an invitation into a session

```mermaid
sequenceDiagram
  participant O as Owner
  participant A as Rust application
  participant D as Authentication SQLite
  participant S as Student browser
  O->>A: Review and confirm users.invite
  A->>D: Reserve username; store invitation hash and expiry
  A-->>O: Private single-use URL (24 hours)
  O-->>S: Share URL through a private channel
  S->>A: Enrollment token + chosen password
  A->>D: Consume invitation, hash password, create session digest
  A-->>S: HttpOnly session cookie
  S->>A: Request with cookie
  A->>D: Check active user, session expiry and capabilities
  A-->>S: Only that account's data
```

The browser removes the invitation fragment from its address bar on load. URL fragments are not sent in HTTP requests. Enrollment sends the token in a same-origin POST body. The token is never placed in an access log, model prompt or audit result. Only its SHA-256 digest is retained. Invitation links are credentials: share them only with their intended recipient, through your institution's approved private channel. There is no automatic email sender.

Passwords must contain 12–128 characters. RustCrypto Argon2id generates a salted password hash. Hashing runs on Tokio's blocking pool behind a two-permit semaphore so expensive password work cannot create an unlimited queue. Incorrect and nonexistent-user logins both perform password verification. The system never recovers a student's original password.

Session tokens use two random UUIDv4 values (244 random bits), independent of user IDs. SQLite stores the SHA-256 digest, account ID, absolute expiry and last touch time. A session expires after eight hours, or after one hour without activity. At most five sessions per account are retained. HTTPS cookies use the `__Host-` prefix, `Secure`, `HttpOnly`, `SameSite=Strict`, and `Path=/`, without a Domain attribute. Browser JavaScript cannot read this cookie. There are no bearer tokens in localStorage.

Logout removes the session row and clears the cookie. Disable/reset removes every session for the account. Active model streams and preprocessing check session validity every two seconds. Closing or switching the UI aborts its request. This is cancellation, not a durable background job or replayable event log.

## Authentication is different from authorization

Authentication answers “which active account owns this request?” Authorization answers “may that account perform this action on this target?” A valid cookie is necessary but does not grant administration.

`src/auth.rs` performs identity/session checks and creates a request-scoped `App` with a server-generated user database path. `src/control.rs` owns capability policy. Every protected control command checks current database state; neither the browser's displayed role nor a model-generated role is authoritative.

| Capability | Member default | Owner default | Explicit member override |
| --- | --- | --- | --- |
| `conversation:write` | allow | allow | allow/deny |
| `chat:use` | allow | allow | allow/deny |
| `research:use` | allow | allow | allow/deny |
| `media:use` | allow | allow | allow/deny |
| `user:read` | deny | allow | allow/deny |
| `user:invite` | deny | allow | allow/deny |
| `usage:read` | deny | allow | allow/deny |
| `user:manage` | deny | allow | cannot delegate |
| `policy:manage` | deny | allow | cannot delegate |
| `audit:read` | deny | allow | cannot delegate |

Owners cannot change owner privileges through these web tools. Owner creation and recovery are deliberate operator CLI actions. An owner may grant `user:invite` to a teaching assistant without granting feature-policy management. An explicit deny can pause a member's chat capability while preserving access to their existing conversations. An inactive account is denied regardless of grants.

The MVP has one instance-wide course library and private account workspaces. It does not implement organizations, teams, cross-account shared conversations, delegated external agents, or arbitrary scope expressions. “Instance” in the registry means this one installation; “own” data is isolated by the account database. Do not market this as enterprise multitenancy.

## Storage and privacy boundaries

```text
data/
  rag.sqlite              shared, instructor-imported knowledge library
  auth.sqlite             users, session digests, invitations, grants,
                          flags, command previews, administrative audit
  users/<server UUID>.sqlite
                          one account's conversations, original files,
                          derived images, transcripts and cached metadata
```

Usernames, cookies and request-supplied IDs never become filesystem paths. All conversation, original-download, range-download, derived-asset and fork operations use the authenticated account's database. A guessed UUID from another account therefore cannot resolve to its resource. Administration can list account metadata, but the web tool registry has no command for reading another account's conversations or files. The host operator still has filesystem access: this is server-enforced separation, not end-to-end encryption.

For Groq production, relevant message context and images go to Groq's configured models, and normalized audio goes to its transcription endpoint. SQLite and CPU embeddings remain on the host. PDF extraction and video decoding also remain local. Use synthetic or permitted course material; account privacy within Ordo does not imply that a cloud provider receives no content.

## Browser boundaries

For authenticated mode, the Host must match `APP_ORIGIN`. Mutating requests require that exact Origin; cross-origin requests are rejected even with a valid cookie. GET/HEAD may omit Origin, allowing normal same-origin downloads, but a supplied hostile Origin is rejected. An internal loopback `/api/health` exception supports the Docker health check and reveals no account data.

All responses use `no-store`, `nosniff`, no-referrer, a restrictive Content Security Policy and frame denial. HTTPS adds HSTS. The app is served behind the existing Traefik proxy, which terminates TLS and preserves the public Host. Do not replace Origin checks with wildcard CORS, trust arbitrary forwarded identity headers, or publish the internal app port.

## Bounds and recovery

| Bound | Current implementation |
| --- | --- |
| Accounts | 100 reserved/enrolled accounts per instance |
| Login/enrollment | 60 attempts per 15-minute window globally; login also 10 per normalized username |
| Protected API | 120 requests per minute per account |
| Chat | 50 admitted attempts per account/day; 500 per instance/day, UTC fixed windows |
| Uploads | 50 admitted attempts per account/day; at most four files per message, 12 MiB each |
| Administrative planning | 100 proposals per account/day |
| Conversations | 500 per account, 200 messages per conversation |
| Original + derived media | production 100 MiB per account; local default 500 MiB |
| Authenticated account database | 128 MiB hard SQLite page-count limit |
| Inference concurrency | one shared model run at a time |

Counts are admission attempts, including failed work after admission. They are not billable token usage, purchased credits, or guaranteed capacity. Provider quotas apply separately and may be much smaller. Fixed-window limits can permit a burst across a boundary. There is no distributed limiter because this deployment runs one app process. Avoid multiple replicas against these SQLite files.

The CLI works with the same environment and data path as the server:

```sh
# Native local authentication exercise (use a fresh test directory)
mkdir -p data/auth-lab
export AUTH_MODE=required APP_ORIGIN=http://127.0.0.1:3732
export DATABASE_PATH=data/auth-lab/rag.sqlite PORT=3732
cargo run -- auth invite instructor --admin
cargo run -- serve
```

Use the emitted private URL to choose a password in the browser. For production, an authorized operator runs the command inside the running app container and captures its output privately:

```sh
cd /opt/ordo
sudo docker compose exec -T app is373-rag auth invite instructor --admin
sudo docker compose exec -T app is373-rag auth reset student373
sudo docker compose exec -T app is373-rag auth disable student373
```

`reset` revokes all sessions, removes the old password hash and issues a new single-use enrollment link while preserving private data. It can also replace an expired invitation. Verify the person's identity through your established course process before resetting access. No email reset, MFA, SSO, WebAuthn, self-service account deletion or automated retention scheduler is implemented yet. Archive hides a conversation; it does not erase it. These are explicit product backlog items.

## Student investigation

Run `cargo build --locked && python3 scripts/check_auth.py`. It starts a disposable server and a deterministic model fixture, creates synthetic accounts and checks enrollment replay, password/session hashing, Origin/Host rules, private conversation/file/asset access, capability-filtered prompts, stale previews, repeated execution, feature flags, revocation and redacted audit records. No instructor key is needed.

Read `boundary`, `establish`, `allowed`, `propose` and `execute` in source. Trace why an administrator's valid session cannot execute a preview created by another account or by a different session. Explain why keeping a capability out of a prompt is useful but insufficient without the execution-time policy check.

Further reading: [OWASP session management](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html), [OWASP password storage](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html), [RustCrypto Argon2](https://docs.rs/argon2/latest/argon2/), and [SQLite WAL](https://sqlite.org/wal.html).
