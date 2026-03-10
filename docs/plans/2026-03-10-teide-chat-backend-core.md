# Teide Chat Backend Core — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend teidelum with chat backend: data model, auth (JWT), Slack-compatible API, and WebSocket real-time hub.

**Architecture:** New `chat/` module inside teidelum with sub-modules for auth, models, handlers, WebSocket hub, and events. Integrated into the existing Axum server with separate route groups and auth middleware. All data stored as TeideDB tables via the existing QueryRouter.

**Tech Stack:** Rust, Axum 0.8, TeideDB (via teide-rs), argon2 (password hashing), jsonwebtoken (JWT), tokio-tungstenite (WebSocket)

**Spec:** `docs/superpowers/specs/2026-03-10-teide-chat-design.md`

---

## File Structure

```
teidelum/src/
├── chat/
│   ├── mod.rs          — Module declarations
│   ├── id.rs           — Timestamp-based monotonic ID generator
│   ├── models.rs       — Table schema creation, CRUD helpers (SELECT-before-INSERT uniqueness)
│   ├── auth.rs         — JWT creation/validation, password hashing, auth middleware
│   ├── events.rs       — WebSocket event types (message, typing, presence, etc.)
│   ├── hub.rs          — WebSocket connection hub (broadcast, membership cache, presence)
│   ├── ws.rs           — WebSocket upgrade handler, connection lifecycle
│   ├── handlers.rs     — Slack-compatible API handlers (all /api/slack/* endpoints)
│   └── slack.rs        — Slack response formatting helpers (ok/error wrappers)
├── lib.rs              — Add `pub mod chat;`
├── server.rs           — Add chat routes + JWT middleware + WS endpoint
└── main.rs             — Initialize chat tables on startup
```

**Existing files modified:**
- `src/lib.rs` — add `pub mod chat;`
- `src/server.rs` — add chat routes, JWT middleware layer, WS endpoint
- `src/main.rs` — call chat table initialization on startup
- `Cargo.toml` — add `argon2`, `jsonwebtoken`, `uuid` dependencies

---

## Chunk 1: Foundation (ID generator, models, table init)

### Task 1: Add dependencies to Cargo.toml

**Files:**
- Modify: `teidelum/Cargo.toml`

- [ ] **Step 1: Add new dependencies**

Add to `[dependencies]` section:

```toml
argon2 = "0.5"
jsonwebtoken = "9"
uuid = { version = "1", features = ["v4"] }
```

- [ ] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum && cargo check`
Expected: compiles with no errors

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat(chat): add argon2, jsonwebtoken, uuid dependencies"
```

---

### Task 2: ID generator

**Files:**
- Create: `teidelum/src/chat/mod.rs`
- Create: `teidelum/src/chat/id.rs`
- Modify: `teidelum/src/lib.rs`

- [ ] **Step 1: Create chat module declaration**

Create `src/chat/mod.rs`:

```rust
pub mod id;
```

- [ ] **Step 2: Add chat module to lib.rs**

Add `pub mod chat;` to `src/lib.rs` after the existing module declarations.

- [ ] **Step 3: Write the failing test for ID generator**

Create `src/chat/id.rs`:

```rust
use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU16 = AtomicU16::new(0);

/// Generate a timestamp-based monotonic ID: (unix_millis << 16) | counter.
/// Naturally time-ordered, supports ~65k IDs per millisecond.
pub fn next_id() -> i64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed) as i64;
    (millis << 16) | (seq & 0xFFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_monotonically_increasing() {
        let a = next_id();
        let b = next_id();
        let c = next_id();
        assert!(b > a, "b={b} should be > a={a}");
        assert!(c > b, "c={c} should be > b={b}");
    }

    #[test]
    fn ids_are_unique() {
        let ids: Vec<i64> = (0..1000).map(|_| next_id()).collect();
        let mut deduped = ids.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(ids.len(), deduped.len(), "all IDs should be unique");
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum && cargo test chat::id --lib`
Expected: 2 tests pass

- [ ] **Step 5: Commit**

```bash
git add src/chat/mod.rs src/chat/id.rs src/lib.rs
git commit -m "feat(chat): add timestamp-based monotonic ID generator"
```

---

### Task 3: Chat models — table schema and creation

**Files:**
- Create: `teidelum/src/chat/models.rs`
- Modify: `teidelum/src/chat/mod.rs`

- [ ] **Step 1: Add module declaration**

Add `pub mod models;` to `src/chat/mod.rs`.

- [ ] **Step 2: Write table schema definitions and init function**

Create `src/chat/models.rs`:

```rust
use crate::api::TeidelumApi;
use crate::catalog::Relationship;
use anyhow::Result;

/// SQL statements to create all chat tables.
const CREATE_TABLES: &[&str] = &[
    "CREATE TABLE users (
        id BIGINT, username VARCHAR, display_name VARCHAR, email VARCHAR,
        password_hash VARCHAR, avatar_url VARCHAR, status VARCHAR,
        is_bot BOOLEAN, created_at VARCHAR
    )",
    "CREATE TABLE channels (
        id BIGINT, name VARCHAR, kind VARCHAR, topic VARCHAR,
        created_by BIGINT, created_at VARCHAR
    )",
    "CREATE TABLE channel_members (
        channel_id BIGINT, user_id BIGINT, role VARCHAR, joined_at VARCHAR
    )",
    "CREATE TABLE messages (
        id BIGINT, channel_id BIGINT, user_id BIGINT, thread_id BIGINT,
        content VARCHAR, deleted_at VARCHAR, edited_at VARCHAR, created_at VARCHAR
    )",
    "CREATE TABLE reactions (
        message_id BIGINT, user_id BIGINT, emoji VARCHAR, created_at VARCHAR
    )",
    "CREATE TABLE mentions (
        message_id BIGINT, user_id BIGINT
    )",
    "CREATE TABLE channel_reads (
        channel_id BIGINT, user_id BIGINT, last_read_ts VARCHAR
    )",
    "CREATE TABLE files (
        id BIGINT, message_id BIGINT, user_id BIGINT, channel_id BIGINT,
        filename VARCHAR, mime_type VARCHAR, size_bytes BIGINT,
        storage_path VARCHAR, created_at VARCHAR
    )",
];

/// All FK relationships for the chat data model.
fn chat_relationships() -> Vec<Relationship> {
    vec![
        Relationship {
            from_table: "messages".into(), from_col: "user_id".into(),
            to_table: "users".into(), to_col: "id".into(),
            relation: "sent_by".into(),
        },
        Relationship {
            from_table: "messages".into(), from_col: "channel_id".into(),
            to_table: "channels".into(), to_col: "id".into(),
            relation: "posted_in".into(),
        },
        Relationship {
            from_table: "messages".into(), from_col: "thread_id".into(),
            to_table: "messages".into(), to_col: "id".into(),
            relation: "reply_to".into(),
        },
        Relationship {
            from_table: "channel_members".into(), from_col: "user_id".into(),
            to_table: "users".into(), to_col: "id".into(),
            relation: "member".into(),
        },
        Relationship {
            from_table: "channel_members".into(), from_col: "channel_id".into(),
            to_table: "channels".into(), to_col: "id".into(),
            relation: "belongs_to".into(),
        },
        Relationship {
            from_table: "reactions".into(), from_col: "message_id".into(),
            to_table: "messages".into(), to_col: "id".into(),
            relation: "reacted_to".into(),
        },
        Relationship {
            from_table: "mentions".into(), from_col: "message_id".into(),
            to_table: "messages".into(), to_col: "id".into(),
            relation: "mentioned_in".into(),
        },
        Relationship {
            from_table: "mentions".into(), from_col: "user_id".into(),
            to_table: "users".into(), to_col: "id".into(),
            relation: "mentions".into(),
        },
        Relationship {
            from_table: "channel_reads".into(), from_col: "channel_id".into(),
            to_table: "channels".into(), to_col: "id".into(),
            relation: "read_status_for".into(),
        },
        Relationship {
            from_table: "channel_reads".into(), from_col: "user_id".into(),
            to_table: "users".into(), to_col: "id".into(),
            relation: "read_by".into(),
        },
        Relationship {
            from_table: "files".into(), from_col: "message_id".into(),
            to_table: "messages".into(), to_col: "id".into(),
            relation: "attached_to".into(),
        },
        Relationship {
            from_table: "files".into(), from_col: "user_id".into(),
            to_table: "users".into(), to_col: "id".into(),
            relation: "uploaded_by".into(),
        },
        Relationship {
            from_table: "files".into(), from_col: "channel_id".into(),
            to_table: "channels".into(), to_col: "id".into(),
            relation: "uploaded_in".into(),
        },
    ]
}

/// Initialize all chat tables and register relationships.
/// Safe to call if tables already exist — will skip existing ones.
pub fn init_chat_tables(api: &TeidelumApi) -> Result<()> {
    let router = api.query_router();

    for sql in CREATE_TABLES {
        // Ignore "table already exists" errors
        match router.query_sync(sql) {
            Ok(_) => {}
            Err(e) if e.to_string().contains("already exists") => {}
            Err(e) => return Err(e),
        }
    }

    api.register_relationships(chat_relationships())?;

    Ok(())
}

/// Escape a string value for SQL (double single quotes).
pub fn escape_sql(s: &str) -> String {
    s.replace('\'', "''")
}

/// Format an optional string as SQL NULL or quoted value.
pub fn sql_str_or_null(v: &Option<String>) -> String {
    match v {
        Some(s) => format!("'{}'", escape_sql(s)),
        None => "NULL".to_string(),
    }
}

/// Get current timestamp as ISO 8601 string.
pub fn now_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    // Simple ISO-ish format: seconds since epoch as string
    // TeideDB stores timestamps as VARCHAR
    format!("{}", now.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_sql() {
        assert_eq!(escape_sql("hello"), "hello");
        assert_eq!(escape_sql("it's"), "it''s");
        assert_eq!(escape_sql("a''b"), "a''''b");
    }

    #[test]
    fn test_sql_str_or_null() {
        assert_eq!(sql_str_or_null(&None), "NULL");
        assert_eq!(sql_str_or_null(&Some("test".into())), "'test'");
        assert_eq!(sql_str_or_null(&Some("it's".into())), "'it''s'");
    }

    #[test]
    fn test_chat_relationships_valid() {
        let rels = chat_relationships();
        assert_eq!(rels.len(), 13);
        // All identifiers should be valid
        for rel in &rels {
            assert!(crate::catalog::is_valid_identifier(&rel.from_table), "invalid: {}", rel.from_table);
            assert!(crate::catalog::is_valid_identifier(&rel.from_col), "invalid: {}", rel.from_col);
            assert!(crate::catalog::is_valid_identifier(&rel.to_table), "invalid: {}", rel.to_table);
            assert!(crate::catalog::is_valid_identifier(&rel.to_col), "invalid: {}", rel.to_col);
        }
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum && cargo test chat::models --lib`
Expected: 3 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/chat/models.rs src/chat/mod.rs
git commit -m "feat(chat): add table schemas, relationships, and SQL helpers"
```

---

### Task 4: Wire up table init in main.rs

**Files:**
- Modify: `teidelum/src/main.rs`
- Modify: `teidelum/src/api.rs` (expose query_router getter if not already public)

- [ ] **Step 1: Add query_router accessor to TeidelumApi if needed**

Check if `api.query_router()` is already public in `src/api.rs`. If not, add:

```rust
/// Access the query router for direct SQL execution.
pub fn query_router(&self) -> &QueryRouter {
    &self.query_router
}
```

Note: `query_router` is stored as `Arc<QueryRouter>`, so the return type should match. Adjust to return `&Arc<QueryRouter>` or `&QueryRouter` based on how it's stored.

- [ ] **Step 2: Add chat init call in main.rs**

In `main.rs`, after `api.register_relationships(...)` and before the server start, add:

```rust
// Initialize chat tables
teidelum::chat::models::init_chat_tables(&api)?;
tracing::info!("chat tables initialized");
```

- [ ] **Step 3: Verify build and startup**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum && cargo build`
Expected: compiles with no errors

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum && cargo run -- --data /tmp/teidelum-test --port 3001 &`
Wait 2 seconds, then kill.
Expected: logs show "chat tables initialized"

- [ ] **Step 4: Commit**

```bash
git add src/main.rs src/api.rs
git commit -m "feat(chat): initialize chat tables on startup"
```

---

## Chunk 2: Auth (password hashing, JWT, middleware)

### Task 5: Auth module — password hashing and JWT

**Files:**
- Create: `teidelum/src/chat/auth.rs`
- Modify: `teidelum/src/chat/mod.rs`

- [ ] **Step 1: Add module declaration**

Add `pub mod auth;` to `src/chat/mod.rs`.

- [ ] **Step 2: Write auth module with tests**

Create `src/chat/auth.rs`:

```rust
use anyhow::{bail, Result};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// JWT claims payload.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub user_id: i64,
    pub username: String,
    pub is_bot: bool,
    pub exp: u64,
}

/// Hash a password with argon2.
pub fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("password hashing failed: {e}"))?;
    Ok(hash.to_string())
}

/// Verify a password against a hash.
pub fn verify_password(password: &str, hash: &str) -> Result<bool> {
    let parsed = PasswordHash::new(hash)
        .map_err(|e| anyhow::anyhow!("invalid password hash: {e}"))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// Create a JWT token for a user.
pub fn create_token(secret: &str, user_id: i64, username: &str, is_bot: bool) -> Result<String> {
    if secret.is_empty() {
        bail!("JWT secret cannot be empty");
    }

    let exp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 86400; // 24 hours

    let claims = Claims {
        user_id,
        username: username.to_string(),
        is_bot,
        exp,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?;

    Ok(token)
}

/// Validate a JWT token and return claims.
pub fn validate_token(secret: &str, token: &str) -> Result<Claims> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;

    Ok(token_data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hash_and_verify() {
        let hash = hash_password("mysecret").unwrap();
        assert!(verify_password("mysecret", &hash).unwrap());
        assert!(!verify_password("wrongpass", &hash).unwrap());
    }

    #[test]
    fn test_jwt_roundtrip() {
        let secret = "test-secret-key";
        let token = create_token(secret, 42, "alice", false).unwrap();
        let claims = validate_token(secret, &token).unwrap();
        assert_eq!(claims.user_id, 42);
        assert_eq!(claims.username, "alice");
        assert!(!claims.is_bot);
    }

    #[test]
    fn test_jwt_invalid_secret() {
        let token = create_token("secret1", 1, "bob", false).unwrap();
        assert!(validate_token("secret2", &token).is_err());
    }

    #[test]
    fn test_jwt_empty_secret_rejected() {
        assert!(create_token("", 1, "bob", false).is_err());
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum && cargo test chat::auth --lib`
Expected: 4 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/chat/auth.rs src/chat/mod.rs
git commit -m "feat(chat): add password hashing (argon2) and JWT auth"
```

---

### Task 6: JWT auth middleware for Axum

**Files:**
- Modify: `teidelum/src/chat/auth.rs`

- [ ] **Step 1: Add Axum JWT middleware extractor**

Append to `src/chat/auth.rs`:

```rust
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Axum middleware that validates JWT from Authorization header.
/// Injects Claims into request extensions on success.
pub async fn jwt_middleware(mut request: Request, next: Next) -> Response {
    let secret = match std::env::var("TEIDE_CHAT_SECRET") {
        Ok(s) if !s.is_empty() => s,
        _ => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"ok": false, "error": "server_misconfigured"})),
            )
                .into_response();
        }
    };

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    let token = match auth_header {
        Some(h) if h.starts_with("Bearer ") => &h[7..],
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({"ok": false, "error": "not_authed"})),
            )
                .into_response();
        }
    };

    match validate_token(&secret, token) {
        Ok(claims) => {
            request.extensions_mut().insert(claims);
            next.run(request).await
        }
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"ok": false, "error": "invalid_auth"})),
        )
            .into_response(),
    }
}
```

- [ ] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum && cargo check`
Expected: compiles

- [ ] **Step 3: Commit**

```bash
git add src/chat/auth.rs
git commit -m "feat(chat): add JWT auth middleware for Axum"
```

---

## Chunk 3: Slack response format and event types

### Task 7: Slack response helpers

**Files:**
- Create: `teidelum/src/chat/slack.rs`
- Modify: `teidelum/src/chat/mod.rs`

- [ ] **Step 1: Add module declaration**

Add `pub mod slack;` to `src/chat/mod.rs`.

- [ ] **Step 2: Write Slack response helpers**

Create `src/chat/slack.rs`:

```rust
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};

/// Slack-compatible success response: {"ok": true, ...extra_fields}
pub fn ok(data: Value) -> Response {
    let mut obj = match data {
        Value::Object(map) => map,
        _ => {
            let mut map = serde_json::Map::new();
            map.insert("data".into(), data);
            map
        }
    };
    obj.insert("ok".into(), json!(true));
    (StatusCode::OK, Json(Value::Object(obj))).into_response()
}

/// Slack-compatible created response: {"ok": true, ...extra_fields}
pub fn created(data: Value) -> Response {
    let mut obj = match data {
        Value::Object(map) => map,
        _ => {
            let mut map = serde_json::Map::new();
            map.insert("data".into(), data);
            map
        }
    };
    obj.insert("ok".into(), json!(true));
    (StatusCode::CREATED, Json(Value::Object(obj))).into_response()
}

/// Slack-compatible error response: {"ok": false, "error": "error_code"}
pub fn err(error: &str) -> Response {
    (
        StatusCode::OK, // Slack returns 200 even for app-level errors
        Json(json!({"ok": false, "error": error})),
    )
        .into_response()
}

/// Slack-compatible error with HTTP status code (for transport-level errors).
pub fn http_err(status: StatusCode, error: &str) -> Response {
    (status, Json(json!({"ok": false, "error": error}))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ok_response_merges_fields() {
        // Just verify it builds without panic
        let _resp = ok(json!({"channel": "general", "ts": "123"}));
    }

    #[test]
    fn test_err_response() {
        let _resp = err("channel_not_found");
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum && cargo test chat::slack --lib`
Expected: 2 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/chat/slack.rs src/chat/mod.rs
git commit -m "feat(chat): add Slack-compatible response helpers"
```

---

### Task 8: WebSocket event types

**Files:**
- Create: `teidelum/src/chat/events.rs`
- Modify: `teidelum/src/chat/mod.rs`

- [ ] **Step 1: Add module declaration**

Add `pub mod events;` to `src/chat/mod.rs`.

- [ ] **Step 2: Write event types**

Create `src/chat/events.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Events sent FROM server TO client over WebSocket.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum ServerEvent {
    #[serde(rename = "hello")]
    Hello,

    #[serde(rename = "message")]
    Message {
        channel: String,
        user: String,
        text: String,
        ts: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        thread_ts: Option<String>,
    },

    #[serde(rename = "message_changed")]
    MessageChanged {
        channel: String,
        message: MessagePayload,
    },

    #[serde(rename = "message_deleted")]
    MessageDeleted {
        channel: String,
        ts: String,
    },

    #[serde(rename = "reaction_added")]
    ReactionAdded {
        channel: String,
        user: String,
        reaction: String,
        item_ts: String,
    },

    #[serde(rename = "reaction_removed")]
    ReactionRemoved {
        channel: String,
        user: String,
        reaction: String,
        item_ts: String,
    },

    #[serde(rename = "typing")]
    Typing {
        channel: String,
        user: String,
    },

    #[serde(rename = "presence_change")]
    PresenceChange {
        user: String,
        presence: String,
    },

    #[serde(rename = "member_joined_channel")]
    MemberJoinedChannel {
        channel: String,
        user: String,
    },

    #[serde(rename = "member_left_channel")]
    MemberLeftChannel {
        channel: String,
        user: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct MessagePayload {
    pub user: String,
    pub text: String,
    pub ts: String,
    pub edited_ts: String,
}

/// Events sent FROM client TO server over WebSocket.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClientEvent {
    #[serde(rename = "typing")]
    Typing { channel: String },

    #[serde(rename = "ping")]
    Ping,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_event_serialization() {
        let event = ServerEvent::Message {
            channel: "5".into(),
            user: "3".into(),
            text: "hello".into(),
            ts: "1710000000".into(),
            thread_ts: None,
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"message\""));
        assert!(json.contains("\"text\":\"hello\""));
        // thread_ts should be absent when None
        assert!(!json.contains("thread_ts"));
    }

    #[test]
    fn test_client_event_deserialization() {
        let json = r#"{"type": "typing", "channel": "5"}"#;
        let event: ClientEvent = serde_json::from_str(json).unwrap();
        match event {
            ClientEvent::Typing { channel } => assert_eq!(channel, "5"),
            _ => panic!("expected Typing event"),
        }
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum && cargo test chat::events --lib`
Expected: 2 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/chat/events.rs src/chat/mod.rs
git commit -m "feat(chat): add WebSocket event types (server + client)"
```

---

## Chunk 4: WebSocket Hub

### Task 9: WebSocket Hub — connection management and broadcast

**Files:**
- Create: `teidelum/src/chat/hub.rs`
- Modify: `teidelum/src/chat/mod.rs`

- [ ] **Step 1: Add module declaration**

Add `pub mod hub;` to `src/chat/mod.rs`.

- [ ] **Step 2: Write hub implementation**

Create `src/chat/hub.rs`:

```rust
use crate::chat::events::ServerEvent;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{broadcast, RwLock};

/// Maximum broadcast channel capacity.
const BROADCAST_CAPACITY: usize = 1024;

/// A connected user's sender handle.
#[derive(Clone)]
pub struct UserSender {
    pub tx: broadcast::Sender<Arc<String>>,
}

/// WebSocket connection hub. Manages connected users, channel membership cache,
/// presence state, and typing throttle.
pub struct Hub {
    /// Connected users: user_id → broadcast sender (supports multiple tabs via broadcast)
    connections: RwLock<HashMap<i64, UserSender>>,
    /// Channel membership cache: channel_id → set of user_ids
    membership: RwLock<HashMap<i64, HashSet<i64>>>,
    /// Typing throttle: (user_id, channel_id) → last typing event time
    typing_throttle: RwLock<HashMap<(i64, i64), Instant>>,
}

impl Hub {
    pub fn new() -> Self {
        Self {
            connections: RwLock::new(HashMap::new()),
            membership: RwLock::new(HashMap::new()),
            typing_throttle: RwLock::new(HashMap::new()),
        }
    }

    /// Register a user connection. Returns a broadcast receiver for events.
    pub async fn connect(&self, user_id: i64) -> broadcast::Receiver<Arc<String>> {
        let mut conns = self.connections.write().await;
        if let Some(sender) = conns.get(&user_id) {
            sender.tx.subscribe()
        } else {
            let (tx, rx) = broadcast::channel(BROADCAST_CAPACITY);
            conns.insert(user_id, UserSender { tx });
            rx
        }
    }

    /// Remove a user connection.
    pub async fn disconnect(&self, user_id: i64) {
        let mut conns = self.connections.write().await;
        // Only remove if no receivers are left
        if let Some(sender) = conns.get(&user_id) {
            if sender.tx.receiver_count() <= 1 {
                conns.remove(&user_id);
            }
        }
    }

    /// Check if a user is connected (online).
    pub async fn is_online(&self, user_id: i64) -> bool {
        let conns = self.connections.read().await;
        conns.contains_key(&user_id)
    }

    /// Get all online user IDs.
    pub async fn online_users(&self) -> Vec<i64> {
        let conns = self.connections.read().await;
        conns.keys().copied().collect()
    }

    /// Set channel membership (full replace).
    pub async fn set_channel_members(&self, channel_id: i64, members: HashSet<i64>) {
        let mut mem = self.membership.write().await;
        mem.insert(channel_id, members);
    }

    /// Add a member to a channel's cached membership.
    pub async fn add_channel_member(&self, channel_id: i64, user_id: i64) {
        let mut mem = self.membership.write().await;
        mem.entry(channel_id).or_default().insert(user_id);
    }

    /// Remove a member from a channel's cached membership.
    pub async fn remove_channel_member(&self, channel_id: i64, user_id: i64) {
        let mut mem = self.membership.write().await;
        if let Some(members) = mem.get_mut(&channel_id) {
            members.remove(&user_id);
        }
    }

    /// Broadcast an event to all members of a channel who are connected.
    pub async fn broadcast_to_channel(&self, channel_id: i64, event: &ServerEvent) {
        let json = match serde_json::to_string(event) {
            Ok(j) => Arc::new(j),
            Err(_) => return,
        };

        let mem = self.membership.read().await;
        let conns = self.connections.read().await;

        if let Some(members) = mem.get(&channel_id) {
            for &user_id in members {
                if let Some(sender) = conns.get(&user_id) {
                    let _ = sender.tx.send(json.clone());
                }
            }
        }
    }

    /// Send an event to a specific user.
    pub async fn send_to_user(&self, user_id: i64, event: &ServerEvent) {
        let json = match serde_json::to_string(event) {
            Ok(j) => Arc::new(j),
            Err(_) => return,
        };

        let conns = self.connections.read().await;
        if let Some(sender) = conns.get(&user_id) {
            let _ = sender.tx.send(json);
        }
    }

    /// Check typing throttle. Returns true if typing event should be broadcast.
    /// Enforces max 1 typing event per user per channel per 3 seconds.
    pub async fn should_broadcast_typing(&self, user_id: i64, channel_id: i64) -> bool {
        let now = Instant::now();
        let key = (user_id, channel_id);

        let mut throttle = self.typing_throttle.write().await;
        if let Some(last) = throttle.get(&key) {
            if now.duration_since(*last).as_secs() < 3 {
                return false;
            }
        }
        throttle.insert(key, now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_connect_and_disconnect() {
        let hub = Hub::new();
        let _rx = hub.connect(1).await;
        assert!(hub.is_online(1).await);
        hub.disconnect(1).await;
        assert!(!hub.is_online(1).await);
    }

    #[tokio::test]
    async fn test_channel_membership() {
        let hub = Hub::new();
        hub.set_channel_members(10, HashSet::from([1, 2, 3])).await;
        hub.add_channel_member(10, 4).await;
        hub.remove_channel_member(10, 2).await;

        let mem = hub.membership.read().await;
        let members = mem.get(&10).unwrap();
        assert!(members.contains(&1));
        assert!(!members.contains(&2));
        assert!(members.contains(&3));
        assert!(members.contains(&4));
    }

    #[tokio::test]
    async fn test_broadcast_to_channel() {
        let hub = Hub::new();
        let mut rx1 = hub.connect(1).await;
        let mut rx2 = hub.connect(2).await;
        let _rx3 = hub.connect(3).await; // not in channel

        hub.set_channel_members(10, HashSet::from([1, 2])).await;

        let event = ServerEvent::Message {
            channel: "10".into(),
            user: "1".into(),
            text: "hello".into(),
            ts: "123".into(),
            thread_ts: None,
        };
        hub.broadcast_to_channel(10, &event).await;

        // User 1 and 2 should receive
        assert!(rx1.try_recv().is_ok());
        assert!(rx2.try_recv().is_ok());
    }

    #[tokio::test]
    async fn test_typing_throttle() {
        let hub = Hub::new();
        assert!(hub.should_broadcast_typing(1, 10).await);
        assert!(!hub.should_broadcast_typing(1, 10).await); // too soon
        assert!(hub.should_broadcast_typing(1, 20).await); // different channel OK
        assert!(hub.should_broadcast_typing(2, 10).await); // different user OK
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum && cargo test chat::hub --lib`
Expected: 4 tests pass

- [ ] **Step 4: Commit**

```bash
git add src/chat/hub.rs src/chat/mod.rs
git commit -m "feat(chat): add WebSocket hub with membership cache and typing throttle"
```

---

## Chunk 5: Slack API Handlers — Auth & Users

### Task 10: Auth handlers (register, login)

**Files:**
- Create: `teidelum/src/chat/handlers.rs`
- Modify: `teidelum/src/chat/mod.rs`

- [ ] **Step 1: Add module declaration**

Add `pub mod handlers;` to `src/chat/mod.rs`.

- [ ] **Step 2: Write auth handlers**

Create `src/chat/handlers.rs`:

```rust
use crate::api::TeidelumApi;
use crate::chat::auth::{self, Claims};
use crate::chat::id::next_id;
use crate::chat::models::{escape_sql, now_timestamp};
use crate::chat::slack;
use axum::{extract::State, response::Response, Extension, Json};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

pub type AppState = Arc<ChatState>;

pub struct ChatState {
    pub api: Arc<TeidelumApi>,
    pub hub: Arc<crate::chat::hub::Hub>,
}

// ── Auth ──

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
    pub email: String,
    #[serde(default)]
    pub display_name: Option<String>,
}

pub async fn auth_register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Response {
    if req.username.is_empty() || req.password.is_empty() || req.email.is_empty() {
        return slack::err("invalid_arguments");
    }

    // Check if username already exists
    let check_sql = format!(
        "SELECT id FROM users WHERE username = '{}'",
        escape_sql(&req.username)
    );
    match state.api.query_router().query_sync(&check_sql) {
        Ok(result) if !result.rows.is_empty() => {
            return slack::err("username_taken");
        }
        Err(e) => {
            tracing::error!("register check failed: {e}");
            return slack::err("internal_error");
        }
        _ => {}
    }

    // Check if email already exists
    let check_email = format!(
        "SELECT id FROM users WHERE email = '{}'",
        escape_sql(&req.email)
    );
    match state.api.query_router().query_sync(&check_email) {
        Ok(result) if !result.rows.is_empty() => {
            return slack::err("email_taken");
        }
        Err(e) => {
            tracing::error!("register email check failed: {e}");
            return slack::err("internal_error");
        }
        _ => {}
    }

    let password_hash = match auth::hash_password(&req.password) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("password hash failed: {e}");
            return slack::err("internal_error");
        }
    };

    let id = next_id();
    let display_name = req.display_name.unwrap_or_else(|| req.username.clone());
    let now = now_timestamp();

    let insert_sql = format!(
        "INSERT INTO users (id, username, display_name, email, password_hash, avatar_url, status, is_bot, created_at) \
         VALUES ({id}, '{username}', '{display}', '{email}', '{hash}', '', 'offline', false, '{now}')",
        username = escape_sql(&req.username),
        display = escape_sql(&display_name),
        email = escape_sql(&req.email),
        hash = escape_sql(&password_hash),
    );

    if let Err(e) = state.api.query_router().query_sync(&insert_sql) {
        tracing::error!("user insert failed: {e}");
        return slack::err("internal_error");
    }

    let secret = match std::env::var("TEIDE_CHAT_SECRET") {
        Ok(s) if !s.is_empty() => s,
        _ => return slack::err("server_misconfigured"),
    };

    let token = match auth::create_token(&secret, id, &req.username, false) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("token creation failed: {e}");
            return slack::err("internal_error");
        }
    };

    slack::created(json!({
        "user_id": id.to_string(),
        "username": req.username,
        "token": token,
    }))
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

pub async fn auth_login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Response {
    let sql = format!(
        "SELECT id, username, password_hash, is_bot FROM users WHERE username = '{}'",
        escape_sql(&req.username)
    );

    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("login query failed: {e}");
            return slack::err("internal_error");
        }
    };

    if result.rows.is_empty() {
        return slack::err("invalid_credentials");
    }

    let row = &result.rows[0];
    let user_id = match &row[0] {
        crate::connector::Value::Int(v) => *v,
        _ => return slack::err("internal_error"),
    };
    let username = match &row[1] {
        crate::connector::Value::String(v) => v.clone(),
        _ => return slack::err("internal_error"),
    };
    let password_hash = match &row[2] {
        crate::connector::Value::String(v) => v.clone(),
        _ => return slack::err("internal_error"),
    };
    let is_bot = match &row[3] {
        crate::connector::Value::Bool(v) => *v,
        _ => false,
    };

    match auth::verify_password(&req.password, &password_hash) {
        Ok(true) => {}
        Ok(false) => return slack::err("invalid_credentials"),
        Err(e) => {
            tracing::error!("password verify failed: {e}");
            return slack::err("internal_error");
        }
    }

    let secret = match std::env::var("TEIDE_CHAT_SECRET") {
        Ok(s) if !s.is_empty() => s,
        _ => return slack::err("server_misconfigured"),
    };

    let token = match auth::create_token(&secret, user_id, &username, is_bot) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("token creation failed: {e}");
            return slack::err("internal_error");
        }
    };

    slack::ok(json!({
        "user_id": user_id.to_string(),
        "username": username,
        "token": token,
    }))
}

// ── Users ──

pub async fn users_list(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
) -> Response {
    let sql = "SELECT id, username, display_name, email, avatar_url, status, is_bot FROM users";
    let result = match state.api.query_router().query_sync(sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("users list failed: {e}");
            return slack::err("internal_error");
        }
    };

    let members: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            json!({
                "id": row[0].to_json(),
                "username": row[1].to_json(),
                "display_name": row[2].to_json(),
                "email": row[3].to_json(),
                "avatar_url": row[4].to_json(),
                "status": row[5].to_json(),
                "is_bot": row[6].to_json(),
            })
        })
        .collect();

    slack::ok(json!({"members": members}))
}

pub async fn users_info(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(req): Json<UserInfoRequest>,
) -> Response {
    let sql = format!(
        "SELECT id, username, display_name, email, avatar_url, status, is_bot \
         FROM users WHERE id = {}",
        req.user
    );
    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("users info failed: {e}");
            return slack::err("internal_error");
        }
    };

    if result.rows.is_empty() {
        return slack::err("user_not_found");
    }

    let row = &result.rows[0];
    slack::ok(json!({
        "user": {
            "id": row[0].to_json(),
            "username": row[1].to_json(),
            "display_name": row[2].to_json(),
            "email": row[3].to_json(),
            "avatar_url": row[4].to_json(),
            "status": row[5].to_json(),
            "is_bot": row[6].to_json(),
        }
    }))
}

#[derive(Deserialize)]
pub struct UserInfoRequest {
    pub user: i64,
}

pub async fn users_set_presence(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<SetPresenceRequest>,
) -> Response {
    let valid = ["online", "away", "dnd", "offline"];
    if !valid.contains(&req.presence.as_str()) {
        return slack::err("invalid_presence");
    }

    let sql = format!(
        "UPDATE users SET status = '{}' WHERE id = {}",
        escape_sql(&req.presence),
        claims.user_id
    );

    // Note: TeideDB may not support UPDATE. If not, we'll need DELETE + INSERT.
    // For now, attempt the UPDATE and handle errors.
    if let Err(e) = state.api.query_router().query_sync(&sql) {
        tracing::error!("set presence failed: {e}");
        return slack::err("internal_error");
    }

    // Broadcast presence change
    let event = crate::chat::events::ServerEvent::PresenceChange {
        user: claims.user_id.to_string(),
        presence: req.presence,
    };
    let online = state.hub.online_users().await;
    for uid in online {
        state.hub.send_to_user(uid, &event).await;
    }

    slack::ok(json!({}))
}

#[derive(Deserialize)]
pub struct SetPresenceRequest {
    pub presence: String,
}
```

- [ ] **Step 3: Add `to_json()` helper on Value**

Check if `crate::connector::Value` already has a JSON serialization method. If not, add a `to_json()` method to it in `src/connector/mod.rs`:

```rust
impl Value {
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Int(i) => serde_json::json!(i.to_string()),
            Value::Float(f) => serde_json::json!(*f),
            Value::String(s) => serde_json::Value::String(s.clone()),
        }
    }
}
```

Note: Int serialized as string to match Slack convention (IDs are strings in Slack API).

- [ ] **Step 4: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum && cargo check`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add src/chat/handlers.rs src/chat/mod.rs src/connector/mod.rs
git commit -m "feat(chat): add auth (register/login) and user handlers"
```

---

### Task 11: Channel and messaging handlers

**Files:**
- Modify: `teidelum/src/chat/handlers.rs`

- [ ] **Step 1: Add channel handlers**

Append to `src/chat/handlers.rs`:

```rust
// ── Conversations ──

#[derive(Deserialize)]
pub struct ConversationsCreateRequest {
    pub name: String,
    #[serde(default = "default_channel_kind")]
    pub kind: String,
    #[serde(default)]
    pub topic: Option<String>,
}

fn default_channel_kind() -> String { "public".to_string() }

pub async fn conversations_create(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ConversationsCreateRequest>,
) -> Response {
    let valid_kinds = ["public", "private", "dm"];
    if !valid_kinds.contains(&req.kind.as_str()) {
        return slack::err("invalid_kind");
    }

    // Check if channel name already exists
    let check = format!(
        "SELECT id FROM channels WHERE name = '{}'",
        escape_sql(&req.name)
    );
    match state.api.query_router().query_sync(&check) {
        Ok(r) if !r.rows.is_empty() => return slack::err("name_taken"),
        Err(e) => {
            tracing::error!("channel check failed: {e}");
            return slack::err("internal_error");
        }
        _ => {}
    }

    let id = next_id();
    let now = now_timestamp();
    let topic = req.topic.unwrap_or_default();

    let insert = format!(
        "INSERT INTO channels (id, name, kind, topic, created_by, created_at) \
         VALUES ({id}, '{name}', '{kind}', '{topic}', {created_by}, '{now}')",
        name = escape_sql(&req.name),
        kind = escape_sql(&req.kind),
        topic = escape_sql(&topic),
        created_by = claims.user_id,
    );

    if let Err(e) = state.api.query_router().query_sync(&insert) {
        tracing::error!("channel create failed: {e}");
        return slack::err("internal_error");
    }

    // Add creator as owner
    let member_insert = format!(
        "INSERT INTO channel_members (channel_id, user_id, role, joined_at) \
         VALUES ({id}, {user_id}, 'owner', '{now}')",
        user_id = claims.user_id,
    );
    if let Err(e) = state.api.query_router().query_sync(&member_insert) {
        tracing::error!("channel member insert failed: {e}");
        return slack::err("internal_error");
    }

    // Update hub membership cache
    let mut members = std::collections::HashSet::new();
    members.insert(claims.user_id);
    state.hub.set_channel_members(id, members).await;

    slack::created(json!({
        "channel": {
            "id": id.to_string(),
            "name": req.name,
            "kind": req.kind,
        }
    }))
}

pub async fn conversations_list(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    // List channels the user is a member of
    let sql = format!(
        "SELECT c.id, c.name, c.kind, c.topic, c.created_at \
         FROM channels c \
         JOIN channel_members cm ON c.id = cm.channel_id \
         WHERE cm.user_id = {}",
        claims.user_id
    );
    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("conversations list failed: {e}");
            return slack::err("internal_error");
        }
    };

    let channels: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            json!({
                "id": row[0].to_json(),
                "name": row[1].to_json(),
                "kind": row[2].to_json(),
                "topic": row[3].to_json(),
                "created_at": row[4].to_json(),
            })
        })
        .collect();

    slack::ok(json!({"channels": channels}))
}

pub async fn conversations_info(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ChannelIdRequest>,
) -> Response {
    // Check membership
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    let sql = format!(
        "SELECT id, name, kind, topic, created_by, created_at \
         FROM channels WHERE id = {}",
        req.channel
    );
    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("conversations info failed: {e}");
            return slack::err("internal_error");
        }
    };

    if result.rows.is_empty() {
        return slack::err("channel_not_found");
    }

    let row = &result.rows[0];
    slack::ok(json!({
        "channel": {
            "id": row[0].to_json(),
            "name": row[1].to_json(),
            "kind": row[2].to_json(),
            "topic": row[3].to_json(),
            "created_by": row[4].to_json(),
            "created_at": row[5].to_json(),
        }
    }))
}

#[derive(Deserialize)]
pub struct HistoryRequest {
    pub channel: i64,
    #[serde(default = "default_history_limit")]
    pub limit: usize,
    #[serde(default)]
    pub before: Option<String>,
}

fn default_history_limit() -> usize { 50 }

pub async fn conversations_history(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<HistoryRequest>,
) -> Response {
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    let limit = req.limit.min(200);
    let before_clause = match &req.before {
        Some(ts) => format!(" AND m.id < {}", escape_sql(ts)),
        None => String::new(),
    };

    let sql = format!(
        "SELECT m.id, m.channel_id, m.user_id, m.thread_id, m.content, \
         m.deleted_at, m.edited_at, m.created_at, u.username \
         FROM messages m \
         JOIN users u ON m.user_id = u.id \
         WHERE m.channel_id = {} AND m.thread_id = 0{} \
         ORDER BY m.id DESC LIMIT {}",
        req.channel, before_clause, limit
    );

    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("history failed: {e}");
            return slack::err("internal_error");
        }
    };

    let messages: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            let deleted = !matches!(&row[5], crate::connector::Value::Null);
            json!({
                "ts": row[0].to_json(),
                "channel": row[1].to_json(),
                "user": row[2].to_json(),
                "thread_ts": row[3].to_json(),
                "text": if deleted { serde_json::Value::String("[deleted]".into()) } else { row[4].to_json() },
                "edited_ts": row[6].to_json(),
                "created_at": row[7].to_json(),
                "username": row[8].to_json(),
            })
        })
        .collect();

    slack::ok(json!({"messages": messages, "has_more": messages.len() == limit}))
}

pub async fn conversations_replies(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<RepliesRequest>,
) -> Response {
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    let sql = format!(
        "SELECT m.id, m.channel_id, m.user_id, m.thread_id, m.content, \
         m.deleted_at, m.edited_at, m.created_at, u.username \
         FROM messages m \
         JOIN users u ON m.user_id = u.id \
         WHERE m.channel_id = {} AND (m.id = {} OR m.thread_id = {}) \
         ORDER BY m.id ASC",
        req.channel, req.ts, req.ts
    );

    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("replies failed: {e}");
            return slack::err("internal_error");
        }
    };

    let messages: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            let deleted = !matches!(&row[5], crate::connector::Value::Null);
            json!({
                "ts": row[0].to_json(),
                "channel": row[1].to_json(),
                "user": row[2].to_json(),
                "thread_ts": row[3].to_json(),
                "text": if deleted { serde_json::Value::String("[deleted]".into()) } else { row[4].to_json() },
                "edited_ts": row[6].to_json(),
                "created_at": row[7].to_json(),
                "username": row[8].to_json(),
            })
        })
        .collect();

    slack::ok(json!({"messages": messages}))
}

#[derive(Deserialize)]
pub struct RepliesRequest {
    pub channel: i64,
    pub ts: i64,
}

pub async fn conversations_join(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ChannelIdRequest>,
) -> Response {
    // Check channel exists and is public
    let sql = format!("SELECT kind FROM channels WHERE id = {}", req.channel);
    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("join check failed: {e}");
            return slack::err("internal_error");
        }
    };

    if result.rows.is_empty() {
        return slack::err("channel_not_found");
    }

    let kind = match &result.rows[0][0] {
        crate::connector::Value::String(s) => s.clone(),
        _ => return slack::err("internal_error"),
    };

    if kind != "public" {
        return slack::err("method_not_allowed_for_channel_type");
    }

    // Check if already a member
    if is_channel_member(&state, req.channel, claims.user_id) {
        return slack::ok(json!({"already_in_channel": true}));
    }

    let now = now_timestamp();
    let insert = format!(
        "INSERT INTO channel_members (channel_id, user_id, role, joined_at) \
         VALUES ({}, {}, 'member', '{now}')",
        req.channel, claims.user_id
    );

    if let Err(e) = state.api.query_router().query_sync(&insert) {
        tracing::error!("join insert failed: {e}");
        return slack::err("internal_error");
    }

    state.hub.add_channel_member(req.channel, claims.user_id).await;

    // Broadcast member joined
    let event = crate::chat::events::ServerEvent::MemberJoinedChannel {
        channel: req.channel.to_string(),
        user: claims.user_id.to_string(),
    };
    state.hub.broadcast_to_channel(req.channel, &event).await;

    slack::ok(json!({"channel": {"id": req.channel.to_string()}}))
}

pub async fn conversations_leave(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ChannelIdRequest>,
) -> Response {
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("not_in_channel");
    }

    let sql = format!(
        "DELETE FROM channel_members WHERE channel_id = {} AND user_id = {}",
        req.channel, claims.user_id
    );

    if let Err(e) = state.api.query_router().query_sync(&sql) {
        tracing::error!("leave failed: {e}");
        return slack::err("internal_error");
    }

    state.hub.remove_channel_member(req.channel, claims.user_id).await;

    let event = crate::chat::events::ServerEvent::MemberLeftChannel {
        channel: req.channel.to_string(),
        user: claims.user_id.to_string(),
    };
    state.hub.broadcast_to_channel(req.channel, &event).await;

    slack::ok(json!({}))
}

pub async fn conversations_invite(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<InviteRequest>,
) -> Response {
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    // Check if target user exists
    let check = format!("SELECT id FROM users WHERE id = {}", req.user);
    match state.api.query_router().query_sync(&check) {
        Ok(r) if r.rows.is_empty() => return slack::err("user_not_found"),
        Err(e) => {
            tracing::error!("invite user check failed: {e}");
            return slack::err("internal_error");
        }
        _ => {}
    }

    // Check if already a member
    if is_channel_member(&state, req.channel, req.user) {
        return slack::err("already_in_channel");
    }

    let now = now_timestamp();
    let insert = format!(
        "INSERT INTO channel_members (channel_id, user_id, role, joined_at) \
         VALUES ({}, {}, 'member', '{now}')",
        req.channel, req.user
    );

    if let Err(e) = state.api.query_router().query_sync(&insert) {
        tracing::error!("invite insert failed: {e}");
        return slack::err("internal_error");
    }

    state.hub.add_channel_member(req.channel, req.user).await;

    let event = crate::chat::events::ServerEvent::MemberJoinedChannel {
        channel: req.channel.to_string(),
        user: req.user.to_string(),
    };
    state.hub.broadcast_to_channel(req.channel, &event).await;

    slack::ok(json!({}))
}

#[derive(Deserialize)]
pub struct InviteRequest {
    pub channel: i64,
    pub user: i64,
}

pub async fn conversations_members(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ChannelIdRequest>,
) -> Response {
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    let sql = format!(
        "SELECT cm.user_id, u.username, cm.role \
         FROM channel_members cm \
         JOIN users u ON cm.user_id = u.id \
         WHERE cm.channel_id = {}",
        req.channel
    );

    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("members list failed: {e}");
            return slack::err("internal_error");
        }
    };

    let members: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            json!({
                "id": row[0].to_json(),
                "username": row[1].to_json(),
                "role": row[2].to_json(),
            })
        })
        .collect();

    slack::ok(json!({"members": members}))
}

pub async fn conversations_open(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ConversationsOpenRequest>,
) -> Response {
    let other_user = req.users.first().copied().unwrap_or(0);
    if other_user == 0 {
        return slack::err("invalid_arguments");
    }

    // Look for existing DM between these two users
    let sql = format!(
        "SELECT c.id, c.name FROM channels c \
         JOIN channel_members cm1 ON c.id = cm1.channel_id \
         JOIN channel_members cm2 ON c.id = cm2.channel_id \
         WHERE c.kind = 'dm' AND cm1.user_id = {} AND cm2.user_id = {}",
        claims.user_id, other_user
    );

    match state.api.query_router().query_sync(&sql) {
        Ok(r) if !r.rows.is_empty() => {
            let row = &r.rows[0];
            return slack::ok(json!({
                "channel": {
                    "id": row[0].to_json(),
                    "name": row[1].to_json(),
                    "kind": "dm",
                },
                "already_open": true,
            }));
        }
        Err(e) => {
            tracing::error!("dm check failed: {e}");
            return slack::err("internal_error");
        }
        _ => {}
    }

    // Create new DM channel
    let id = next_id();
    let now = now_timestamp();
    let dm_name = format!("dm-{}-{}", claims.user_id.min(other_user), claims.user_id.max(other_user));

    let insert_channel = format!(
        "INSERT INTO channels (id, name, kind, topic, created_by, created_at) \
         VALUES ({id}, '{name}', 'dm', '', {created_by}, '{now}')",
        name = escape_sql(&dm_name),
        created_by = claims.user_id,
    );

    if let Err(e) = state.api.query_router().query_sync(&insert_channel) {
        tracing::error!("dm channel create failed: {e}");
        return slack::err("internal_error");
    }

    // Add both users as members
    for uid in [claims.user_id, other_user] {
        let member_sql = format!(
            "INSERT INTO channel_members (channel_id, user_id, role, joined_at) \
             VALUES ({id}, {uid}, 'member', '{now}')"
        );
        if let Err(e) = state.api.query_router().query_sync(&member_sql) {
            tracing::error!("dm member insert failed: {e}");
            return slack::err("internal_error");
        }
    }

    let mut members = std::collections::HashSet::new();
    members.insert(claims.user_id);
    members.insert(other_user);
    state.hub.set_channel_members(id, members).await;

    slack::created(json!({
        "channel": {
            "id": id.to_string(),
            "name": dm_name,
            "kind": "dm",
        },
        "already_open": false,
    }))
}

#[derive(Deserialize)]
pub struct ConversationsOpenRequest {
    pub users: Vec<i64>,
}

#[derive(Deserialize)]
pub struct ChannelIdRequest {
    pub channel: i64,
}

// ── Messaging ──

pub async fn chat_post_message(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<PostMessageRequest>,
) -> Response {
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    let id = next_id();
    let now = now_timestamp();
    let thread_id = req.thread_ts.unwrap_or(0);

    let insert = format!(
        "INSERT INTO messages (id, channel_id, user_id, thread_id, content, deleted_at, edited_at, created_at) \
         VALUES ({id}, {channel}, {user}, {thread}, '{text}', NULL, NULL, '{now}')",
        channel = req.channel,
        user = claims.user_id,
        thread = thread_id,
        text = escape_sql(&req.text),
    );

    if let Err(e) = state.api.query_router().query_sync(&insert) {
        tracing::error!("post message failed: {e}");
        return slack::err("internal_error");
    }

    // Parse and store mentions
    let mention_regex = regex_lite::Regex::new(r"@(\w+)").unwrap();
    for cap in mention_regex.captures_iter(&req.text) {
        let mentioned_username = &cap[1];
        let user_sql = format!(
            "SELECT id FROM users WHERE username = '{}'",
            escape_sql(mentioned_username)
        );
        if let Ok(r) = state.api.query_router().query_sync(&user_sql) {
            if let Some(row) = r.rows.first() {
                if let crate::connector::Value::Int(uid) = &row[0] {
                    // Check uniqueness before inserting
                    let check = format!(
                        "SELECT message_id FROM mentions WHERE message_id = {} AND user_id = {}",
                        id, uid
                    );
                    if let Ok(existing) = state.api.query_router().query_sync(&check) {
                        if existing.rows.is_empty() {
                            let mention_sql = format!(
                                "INSERT INTO mentions (message_id, user_id) VALUES ({}, {})",
                                id, uid
                            );
                            let _ = state.api.query_router().query_sync(&mention_sql);
                        }
                    }
                }
            }
        }
    }

    // Broadcast message event
    let event = crate::chat::events::ServerEvent::Message {
        channel: req.channel.to_string(),
        user: claims.user_id.to_string(),
        text: req.text.clone(),
        ts: id.to_string(),
        thread_ts: if thread_id != 0 { Some(thread_id.to_string()) } else { None },
    };
    state.hub.broadcast_to_channel(req.channel, &event).await;

    slack::ok(json!({
        "message": {
            "ts": id.to_string(),
            "channel": req.channel.to_string(),
            "user": claims.user_id.to_string(),
            "text": req.text,
        }
    }))
}

#[derive(Deserialize)]
pub struct PostMessageRequest {
    pub channel: i64,
    pub text: String,
    #[serde(default)]
    pub thread_ts: Option<i64>,
}

pub async fn chat_update(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ChatUpdateRequest>,
) -> Response {
    // Verify the message exists and belongs to the user
    let check = format!(
        "SELECT user_id, channel_id FROM messages WHERE id = {}",
        req.ts
    );
    let result = match state.api.query_router().query_sync(&check) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("chat update check failed: {e}");
            return slack::err("internal_error");
        }
    };

    if result.rows.is_empty() {
        return slack::err("message_not_found");
    }

    let msg_user = match &result.rows[0][0] {
        crate::connector::Value::Int(v) => *v,
        _ => return slack::err("internal_error"),
    };
    let channel_id = match &result.rows[0][1] {
        crate::connector::Value::Int(v) => *v,
        _ => return slack::err("internal_error"),
    };

    if msg_user != claims.user_id {
        return slack::err("cant_update_message");
    }

    let now = now_timestamp();

    // TeideDB may not support UPDATE. Use DELETE + INSERT if needed.
    let update_sql = format!(
        "UPDATE messages SET content = '{}', edited_at = '{}' WHERE id = {}",
        escape_sql(&req.text), now, req.ts
    );

    if let Err(e) = state.api.query_router().query_sync(&update_sql) {
        tracing::error!("chat update failed: {e}");
        return slack::err("internal_error");
    }

    // Broadcast message_changed
    let event = crate::chat::events::ServerEvent::MessageChanged {
        channel: channel_id.to_string(),
        message: crate::chat::events::MessagePayload {
            user: claims.user_id.to_string(),
            text: req.text.clone(),
            ts: req.ts.to_string(),
            edited_ts: now,
        },
    };
    state.hub.broadcast_to_channel(channel_id, &event).await;

    slack::ok(json!({
        "message": {
            "ts": req.ts.to_string(),
            "text": req.text,
        }
    }))
}

#[derive(Deserialize)]
pub struct ChatUpdateRequest {
    pub ts: i64,
    pub text: String,
}

pub async fn chat_delete(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ChatDeleteRequest>,
) -> Response {
    let check = format!(
        "SELECT user_id, channel_id FROM messages WHERE id = {}",
        req.ts
    );
    let result = match state.api.query_router().query_sync(&check) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("chat delete check failed: {e}");
            return slack::err("internal_error");
        }
    };

    if result.rows.is_empty() {
        return slack::err("message_not_found");
    }

    let msg_user = match &result.rows[0][0] {
        crate::connector::Value::Int(v) => *v,
        _ => return slack::err("internal_error"),
    };
    let channel_id = match &result.rows[0][1] {
        crate::connector::Value::Int(v) => *v,
        _ => return slack::err("internal_error"),
    };

    if msg_user != claims.user_id {
        return slack::err("cant_delete_message");
    }

    let now = now_timestamp();
    let soft_delete = format!(
        "UPDATE messages SET deleted_at = '{}' WHERE id = {}",
        now, req.ts
    );

    if let Err(e) = state.api.query_router().query_sync(&soft_delete) {
        tracing::error!("chat delete failed: {e}");
        return slack::err("internal_error");
    }

    let event = crate::chat::events::ServerEvent::MessageDeleted {
        channel: channel_id.to_string(),
        ts: req.ts.to_string(),
    };
    state.hub.broadcast_to_channel(channel_id, &event).await;

    slack::ok(json!({}))
}

#[derive(Deserialize)]
pub struct ChatDeleteRequest {
    pub ts: i64,
}

// ── Reactions ──

pub async fn reactions_add(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ReactionRequest>,
) -> Response {
    // Get channel from message
    let check = format!("SELECT channel_id FROM messages WHERE id = {}", req.timestamp);
    let result = match state.api.query_router().query_sync(&check) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("reaction check failed: {e}");
            return slack::err("internal_error");
        }
    };

    if result.rows.is_empty() {
        return slack::err("message_not_found");
    }

    let channel_id = match &result.rows[0][0] {
        crate::connector::Value::Int(v) => *v,
        _ => return slack::err("internal_error"),
    };

    if !is_channel_member(&state, channel_id, claims.user_id) {
        return slack::err("channel_not_found");
    }

    // Check uniqueness
    let dup_check = format!(
        "SELECT message_id FROM reactions WHERE message_id = {} AND user_id = {} AND emoji = '{}'",
        req.timestamp, claims.user_id, escape_sql(&req.name)
    );
    if let Ok(r) = state.api.query_router().query_sync(&dup_check) {
        if !r.rows.is_empty() {
            return slack::err("already_reacted");
        }
    }

    let now = now_timestamp();
    let insert = format!(
        "INSERT INTO reactions (message_id, user_id, emoji, created_at) \
         VALUES ({}, {}, '{}', '{now}')",
        req.timestamp, claims.user_id, escape_sql(&req.name)
    );

    if let Err(e) = state.api.query_router().query_sync(&insert) {
        tracing::error!("reaction insert failed: {e}");
        return slack::err("internal_error");
    }

    let event = crate::chat::events::ServerEvent::ReactionAdded {
        channel: channel_id.to_string(),
        user: claims.user_id.to_string(),
        reaction: req.name,
        item_ts: req.timestamp.to_string(),
    };
    state.hub.broadcast_to_channel(channel_id, &event).await;

    slack::ok(json!({}))
}

pub async fn reactions_remove(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ReactionRequest>,
) -> Response {
    let check = format!("SELECT channel_id FROM messages WHERE id = {}", req.timestamp);
    let result = match state.api.query_router().query_sync(&check) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("reaction remove check failed: {e}");
            return slack::err("internal_error");
        }
    };

    if result.rows.is_empty() {
        return slack::err("message_not_found");
    }

    let channel_id = match &result.rows[0][0] {
        crate::connector::Value::Int(v) => *v,
        _ => return slack::err("internal_error"),
    };

    let delete = format!(
        "DELETE FROM reactions WHERE message_id = {} AND user_id = {} AND emoji = '{}'",
        req.timestamp, claims.user_id, escape_sql(&req.name)
    );

    if let Err(e) = state.api.query_router().query_sync(&delete) {
        tracing::error!("reaction delete failed: {e}");
        return slack::err("internal_error");
    }

    let event = crate::chat::events::ServerEvent::ReactionRemoved {
        channel: channel_id.to_string(),
        user: claims.user_id.to_string(),
        reaction: req.name,
        item_ts: req.timestamp.to_string(),
    };
    state.hub.broadcast_to_channel(channel_id, &event).await;

    slack::ok(json!({}))
}

#[derive(Deserialize)]
pub struct ReactionRequest {
    pub name: String,
    pub timestamp: i64,
}

// ── Helpers ──

fn is_channel_member(state: &AppState, channel_id: i64, user_id: i64) -> bool {
    let sql = format!(
        "SELECT channel_id FROM channel_members WHERE channel_id = {} AND user_id = {}",
        channel_id, user_id
    );
    match state.api.query_router().query_sync(&sql) {
        Ok(r) => !r.rows.is_empty(),
        Err(_) => false,
    }
}
```

- [ ] **Step 2: Add `regex-lite` dependency**

Add to `Cargo.toml` under `[dependencies]`:

```toml
regex-lite = "0.1"
```

- [ ] **Step 3: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum && cargo check`
Expected: compiles

- [ ] **Step 4: Commit**

```bash
git add src/chat/handlers.rs Cargo.toml Cargo.lock
git commit -m "feat(chat): add channel, messaging, and reaction handlers"
```

---

## Chunk 6: WebSocket handler and route wiring

### Task 12: WebSocket upgrade handler

**Files:**
- Create: `teidelum/src/chat/ws.rs`
- Modify: `teidelum/src/chat/mod.rs`

- [ ] **Step 1: Add module declaration**

Add `pub mod ws;` to `src/chat/mod.rs`.

- [ ] **Step 2: Write WebSocket handler**

Create `src/chat/ws.rs`:

```rust
use crate::chat::auth;
use crate::chat::events::{ClientEvent, ServerEvent};
use crate::chat::handlers::AppState;
use axum::{
    extract::{Query, State, WebSocketUpgrade},
    response::Response,
};
use axum::extract::ws::{Message, WebSocket};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct WsQuery {
    pub token: String,
}

pub async fn ws_upgrade(
    State(state): State<AppState>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    let secret = match std::env::var("TEIDE_CHAT_SECRET") {
        Ok(s) if !s.is_empty() => s,
        _ => {
            return axum::http::Response::builder()
                .status(401)
                .body(axum::body::Body::empty())
                .unwrap()
                .into_response();
        }
    };

    let claims = match auth::validate_token(&secret, &query.token) {
        Ok(c) => c,
        Err(_) => {
            return axum::http::Response::builder()
                .status(401)
                .body(axum::body::Body::empty())
                .unwrap()
                .into_response();
        }
    };

    ws.on_upgrade(move |socket| handle_socket(state, claims, socket))
}

async fn handle_socket(state: AppState, claims: auth::Claims, socket: WebSocket) {
    let user_id = claims.user_id;
    let mut rx = state.hub.connect(user_id).await;

    let (mut ws_sink, mut ws_stream) = socket.split();

    // Send hello
    let hello = serde_json::to_string(&ServerEvent::Hello).unwrap();
    if ws_sink.send(Message::Text(hello.into())).await.is_err() {
        return;
    }

    // Broadcast presence online
    let presence = ServerEvent::PresenceChange {
        user: user_id.to_string(),
        presence: "online".to_string(),
    };
    let online = state.hub.online_users().await;
    for uid in online {
        state.hub.send_to_user(uid, &presence).await;
    }

    let state_clone = state.clone();
    let user_id_clone = user_id;

    // Task: forward hub events to WebSocket
    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if ws_sink.send(Message::Text((*msg).clone().into())).await.is_err() {
                break;
            }
        }
    });

    // Task: process incoming client messages
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_stream.next().await {
            match msg {
                Message::Text(text) => {
                    if let Ok(event) = serde_json::from_str::<ClientEvent>(&text) {
                        match event {
                            ClientEvent::Typing { channel } => {
                                if let Ok(ch_id) = channel.parse::<i64>() {
                                    if state_clone.hub.should_broadcast_typing(user_id_clone, ch_id).await {
                                        let typing_event = ServerEvent::Typing {
                                            channel: channel.clone(),
                                            user: user_id_clone.to_string(),
                                        };
                                        state_clone.hub.broadcast_to_channel(ch_id, &typing_event).await;
                                    }
                                }
                            }
                            ClientEvent::Ping => {
                                // Pong handled by axum automatically
                            }
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    // Wait for either task to finish
    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }

    // Cleanup
    state.hub.disconnect(user_id).await;

    // Broadcast offline if no more connections
    if !state.hub.is_online(user_id).await {
        let offline = ServerEvent::PresenceChange {
            user: user_id.to_string(),
            presence: "offline".to_string(),
        };
        let online = state.hub.online_users().await;
        for uid in online {
            state.hub.send_to_user(uid, &offline).await;
        }
    }
}
```

- [ ] **Step 3: Add `futures-util` dependency**

Add to `Cargo.toml`:

```toml
futures-util = "0.3"
```

- [ ] **Step 4: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum && cargo check`
Expected: compiles

- [ ] **Step 5: Commit**

```bash
git add src/chat/ws.rs src/chat/mod.rs Cargo.toml Cargo.lock
git commit -m "feat(chat): add WebSocket upgrade handler and connection lifecycle"
```

---

### Task 13: Wire chat routes into server

**Files:**
- Modify: `teidelum/src/server.rs`
- Modify: `teidelum/src/chat/mod.rs` (final module list)

- [ ] **Step 1: Update chat/mod.rs with all modules**

Ensure `src/chat/mod.rs` has:

```rust
pub mod auth;
pub mod events;
pub mod handlers;
pub mod hub;
pub mod id;
pub mod models;
pub mod slack;
pub mod ws;
```

- [ ] **Step 2: Add chat route builder**

Add a function to `src/chat/handlers.rs` (or a new `src/chat/routes.rs` if preferred) that builds the Slack API router:

```rust
use axum::{middleware, Router};

pub fn chat_routes(state: AppState) -> Router {
    let authed = Router::new()
        // Conversations
        .route("/conversations.create", axum::routing::post(conversations_create))
        .route("/conversations.list", axum::routing::post(conversations_list))
        .route("/conversations.info", axum::routing::post(conversations_info))
        .route("/conversations.history", axum::routing::post(conversations_history))
        .route("/conversations.replies", axum::routing::post(conversations_replies))
        .route("/conversations.join", axum::routing::post(conversations_join))
        .route("/conversations.leave", axum::routing::post(conversations_leave))
        .route("/conversations.invite", axum::routing::post(conversations_invite))
        .route("/conversations.members", axum::routing::post(conversations_members))
        .route("/conversations.open", axum::routing::post(conversations_open))
        // Chat
        .route("/chat.postMessage", axum::routing::post(chat_post_message))
        .route("/chat.update", axum::routing::post(chat_update))
        .route("/chat.delete", axum::routing::post(chat_delete))
        // Users
        .route("/users.list", axum::routing::post(users_list))
        .route("/users.info", axum::routing::post(users_info))
        .route("/users.setPresence", axum::routing::post(users_set_presence))
        // Reactions
        .route("/reactions.add", axum::routing::post(reactions_add))
        .route("/reactions.remove", axum::routing::post(reactions_remove))
        .layer(middleware::from_fn(crate::chat::auth::jwt_middleware))
        .with_state(state.clone());

    let public = Router::new()
        .route("/auth.register", axum::routing::post(auth_register))
        .route("/auth.login", axum::routing::post(auth_login))
        .with_state(state);

    Router::new()
        .nest("/api/slack", authed)
        .nest("/api/slack", public)
}
```

- [ ] **Step 3: Modify server.rs to include chat routes and WS endpoint**

In `src/server.rs`, modify `build_router` to accept a `ChatState` and add chat routes:

```rust
use crate::chat::handlers::{AppState as ChatAppState, ChatState, chat_routes};
use crate::chat::hub::Hub;
use crate::chat::ws::ws_upgrade;

pub fn build_router(
    api: Arc<TeidelumApi>,
    hub: Arc<Hub>,
    ct: CancellationToken,
) -> Router {
    let chat_state = Arc::new(ChatState {
        api: api.clone(),
        hub: hub.clone(),
    });

    let mut app = Router::new()
        .merge(routes::api_routes())
        .with_state(api.clone())
        .merge(chat_routes(chat_state.clone()))
        .route("/ws", axum::routing::get(ws_upgrade).with_state(chat_state))
        .layer(CorsLayer::permissive());

    // ... rest of existing auth + MCP setup unchanged ...
}
```

- [ ] **Step 4: Update main.rs to create Hub and pass it**

In `src/main.rs`, create the Hub before starting the server:

```rust
let hub = Arc::new(teidelum::chat::hub::Hub::new());

// Load channel membership cache from DB
// (optional: populate hub membership from existing channel_members table)

// Pass hub to server::start()
```

- [ ] **Step 5: Verify build and test startup**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum && cargo build`
Expected: compiles

- [ ] **Step 6: Commit**

```bash
git add src/chat/ src/server.rs src/main.rs
git commit -m "feat(chat): wire Slack API routes and WebSocket into Axum server"
```

---

## Chunk 7: Integration test

### Task 14: End-to-end integration test

**Files:**
- Create: `teidelum/tests/chat_integration.rs`

- [ ] **Step 1: Write integration test**

Create `tests/chat_integration.rs`:

```rust
//! Integration tests for the chat API.
//! Tests the full flow: register → login → create channel → post message → history.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

/// Helper to build a POST request with JSON body.
fn post_json(uri: &str, body: Value, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .uri(uri)
        .method("POST")
        .header("content-type", "application/json");

    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }

    builder.body(Body::from(serde_json::to_string(&body).unwrap())).unwrap()
}

/// Parse response body as JSON.
async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn test_chat_flow() {
    // Set JWT secret for tests
    std::env::set_var("TEIDE_CHAT_SECRET", "test-secret-key-12345");

    // Create temp data dir
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path();

    // Initialize API and chat tables
    let api = teidelum::api::TeidelumApi::open(data_dir).unwrap();
    teidelum::chat::models::init_chat_tables(&api).unwrap();

    let api = std::sync::Arc::new(api);
    let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());

    let state = std::sync::Arc::new(teidelum::chat::handlers::ChatState {
        api: api.clone(),
        hub: hub.clone(),
    });

    let app = teidelum::chat::handlers::chat_routes(state);

    // 1. Register user
    let resp = app.clone().oneshot(
        post_json("/api/slack/auth.register", json!({
            "username": "alice",
            "password": "secret123",
            "email": "alice@example.com"
        }), None)
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let token = body["token"].as_str().unwrap().to_string();

    // 2. Login
    let resp = app.clone().oneshot(
        post_json("/api/slack/auth.login", json!({
            "username": "alice",
            "password": "secret123"
        }), None)
    ).await.unwrap();

    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    assert!(body["token"].is_string());

    // 3. Create channel
    let resp = app.clone().oneshot(
        post_json("/api/slack/conversations.create", json!({
            "name": "general"
        }), Some(&token))
    ).await.unwrap();

    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let channel_id: i64 = body["channel"]["id"].as_str().unwrap().parse().unwrap();

    // 4. Post message
    let resp = app.clone().oneshot(
        post_json("/api/slack/chat.postMessage", json!({
            "channel": channel_id,
            "text": "Hello world!"
        }), Some(&token))
    ).await.unwrap();

    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    assert_eq!(body["message"]["text"], "Hello world!");

    // 5. Get history
    let resp = app.clone().oneshot(
        post_json("/api/slack/conversations.history", json!({
            "channel": channel_id
        }), Some(&token))
    ).await.unwrap();

    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["text"], "Hello world!");

    // 6. Duplicate username rejected
    let resp = app.clone().oneshot(
        post_json("/api/slack/auth.register", json!({
            "username": "alice",
            "password": "other",
            "email": "other@example.com"
        }), None)
    ).await.unwrap();

    let body = body_json(resp).await;
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"], "username_taken");
}
```

- [ ] **Step 2: Run integration test**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum && cargo test test_chat_flow -- --nocapture`
Expected: test passes

- [ ] **Step 3: Commit**

```bash
git add tests/chat_integration.rs
git commit -m "test(chat): add end-to-end integration test for chat API flow"
```

---

## Summary

This plan covers the backend core:

| Chunk | Tasks | What it delivers |
|-------|-------|------------------|
| 1: Foundation | Tasks 1-4 | Dependencies, ID generator, table schemas, startup init |
| 2: Auth | Tasks 5-6 | Password hashing, JWT creation/validation, Axum middleware |
| 3: Response/Events | Tasks 7-8 | Slack response helpers, WebSocket event types |
| 4: WebSocket Hub | Task 9 | Connection management, broadcast, membership cache, typing throttle |
| 5: Handlers | Tasks 10-11 | All Slack API handlers (auth, users, channels, messages, reactions) |
| 6: Wiring | Tasks 12-13 | WebSocket handler, route registration, server integration |
| 7: Integration | Task 14 | End-to-end test: register → login → channel → message → history |

**Next plans (separate documents):**
- Plan 2: Search indexing, file uploads, MCP chat tools
- Plan 3: SvelteKit frontend
