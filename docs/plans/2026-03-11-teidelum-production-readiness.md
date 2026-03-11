# Teidelum Production Readiness Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Take the existing Teidelum implementation (chat backend, search, files, MCP tools, SvelteKit frontend) from feature branches to a merged, production-ready state with all design spec gaps closed.

**Architecture:** All work happens on the `teide-chat-frontend` branch (the most advanced branch containing all features). Tasks fill gaps identified between the current implementation and the design spec: frontend production build pipeline, static file serving from Axum, unread tracking, thread metadata, and test coverage.

**Tech Stack:** Rust (Axum, TeideDB, rmcp), SvelteKit (adapter-static, TypeScript, Tailwind CSS), WebSocket

---

## Current State

**Implemented:**
- Chat backend: auth, channels, messages, reactions, mentions, DMs, presence, WebSocket hub
- Search: full-text indexing on post, `search.messages` endpoint
- Files: multipart upload, download with auth + MIME hardening
- MCP: 17 tools including 6 chat tools
- Frontend: full SvelteKit SPA (login, register, sidebar, messages, threads, reactions, search, file upload, presence)

**Gaps to close:**
1. Frontend uses `adapter-auto` — needs `adapter-static` for production SPA build
2. Axum doesn't serve static frontend files — dev proxy only
3. `channel_reads` table exists but no read tracking logic
4. No thread reply count / last_reply_ts in API responses
5. Test coverage ~40% — missing tests for mentions, threads, presence, DMs, files, WebSocket

**Key code patterns (reference for all tasks):**
- SQL queries: `state.api.query_router().query_sync(&sql)` returns `Result<QueryResult>` where `QueryResult` has `.rows: Vec<Vec<Value>>`
- Handler signature: `async fn handler(State(state): State<AppState>, Extension(claims): Extension<Claims>, Json(req): Json<SomeStruct>) -> Response`
- Responses: `slack::ok(json!({...}))` and `slack::err("error_code")`
- Column access: `row[0].to_json()` returns `serde_json::Value`
- Membership check: `is_channel_member(&state, channel_id, user_id)` returns `bool`
- Route registration: `.route("/endpoint.name", axum::routing::post(handler_fn))` inside `chat_routes()`, authed routes go before `.layer(middleware::from_fn(crate::chat::auth::jwt_middleware))`

---

## Chunk 1: Frontend Production Build Pipeline

### Task 1: Switch SvelteKit to adapter-static

**Files:**
- Modify: `ui/package.json`
- Modify: `ui/svelte.config.js`
- Modify: `ui/src/routes/+layout.ts`

- [x] **Step 1: Install adapter-static**

```bash
cd ui && npm install -D @sveltejs/adapter-static && npm uninstall @sveltejs/adapter-auto
```

- [x] **Step 2: Update svelte.config.js**

Replace `ui/svelte.config.js` with:

```javascript
import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	kit: {
		adapter: adapter({
			pages: 'build',
			assets: 'build',
			fallback: 'index.html',
			precompress: false,
			strict: true
		})
	}
};

export default config;
```

The `fallback: 'index.html'` is critical — it generates a catch-all page so SPA client-side routing works.

- [x] **Step 3: Ensure SPA prerender is disabled**

`ui/src/routes/+layout.ts` must contain:

```typescript
export const prerender = false;
export const ssr = false;
```

- [x] **Step 4: Build and verify**

```bash
cd ui && npm run build
ls -la build/index.html
```

Expected: `build/` directory with `index.html`, `_app/` directory with JS/CSS assets.

- [x] **Step 5: Commit**

```bash
git add ui/package.json ui/package-lock.json ui/svelte.config.js ui/src/routes/+layout.ts
git commit -m "feat(ui): switch to adapter-static for production SPA build"
```

---

### Task 2: Serve static frontend from Axum

**Files:**
- Modify: `src/server.rs`
- Modify: `Cargo.toml`

- [x] **Step 1: Add tower-http `fs` feature**

In `Cargo.toml`, change:

```toml
tower-http = { version = "0.6", features = ["cors"] }
```

to:

```toml
tower-http = { version = "0.6", features = ["cors", "fs"] }
```

- [x] **Step 2: Add static file serving to server.rs**

Add import at top of `server.rs`:

```rust
use tower_http::services::{ServeDir, ServeFile};
```

In `build_router()`, after `.layer(CorsLayer::permissive())` and before the API key middleware check, add:

```rust
    // Serve SvelteKit static build — fallback after API routes
    let ui_dir = std::path::Path::new("ui/build");
    if ui_dir.exists() {
        let serve_dir = ServeDir::new(ui_dir)
            .not_found_service(ServeFile::new(ui_dir.join("index.html")));
        app = app.fallback_service(serve_dir);
    }
```

API routes take priority because they're merged first. The fallback catches everything else and serves static files or falls back to `index.html` for SPA routing.

- [x] **Step 3: Build and verify**

```bash
cd ui && npm run build && cd ..
cargo check
```

Expected: compiles without errors.

- [x] **Step 4: Verify end-to-end**

```bash
TEIDE_CHAT_SECRET=testsecret123456789012345678901234 cargo run &
sleep 2
curl -s http://localhost:3000/ | head -5
curl -s http://localhost:3000/api/slack/auth.login | head -3
kill %1
```

Expected: Root path returns SvelteKit HTML. API path returns JSON error (missing body).

- [x] **Step 5: Commit**

```bash
git add Cargo.toml src/server.rs
git commit -m "feat: serve SvelteKit static build from Axum"
```

---

## Chunk 2: Unread Tracking

### Task 3: Implement channel_reads update on message view

**Files:**
- Modify: `src/chat/handlers.rs`
- Modify: `tests/chat_integration.rs`

The `channel_reads` table already exists (created in `models.rs`). We need:
1. Update `last_read_ts` when a user fetches `conversations.history` (implicit read)
2. Add unread info to `conversations.list` response

- [x] **Step 1: Write failing test for unread tracking**

Add to `tests/chat_integration.rs`:

```rust
#[tokio::test]
async fn test_unread_tracking() {
    std::env::set_var("TEIDE_CHAT_SECRET", "test-secret-key-12345");
    let tmp = tempfile::tempdir().unwrap();
    let api = teidelum::api::TeidelumApi::open(tmp.path()).unwrap();
    teidelum::chat::models::init_chat_tables(&api).unwrap();
    let api = std::sync::Arc::new(api);
    let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());
    let state = std::sync::Arc::new(teidelum::chat::handlers::ChatState {
        api: api.clone(),
        hub: hub.clone(),
    });
    let app = teidelum::chat::handlers::chat_routes(state);

    // Register alice
    let resp = app.clone().oneshot(post_json(
        "/api/slack/auth.register",
        json!({"username": "alice", "password": "secret123", "email": "alice@test.com"}),
        None,
    )).await.unwrap();
    let body = body_json(resp).await;
    let token_a = body["token"].as_str().unwrap().to_string();

    // Register bob
    let resp = app.clone().oneshot(post_json(
        "/api/slack/auth.register",
        json!({"username": "bob", "password": "secret123", "email": "bob@test.com"}),
        None,
    )).await.unwrap();
    let body = body_json(resp).await;
    let token_b = body["token"].as_str().unwrap().to_string();

    // Alice creates channel
    let resp = app.clone().oneshot(post_json(
        "/api/slack/conversations.create",
        json!({"name": "unread-test"}),
        Some(&token_a),
    )).await.unwrap();
    let body = body_json(resp).await;
    let ch_id: i64 = body["channel"]["id"].as_str().unwrap().parse().unwrap();

    // Bob joins channel
    let _ = app.clone().oneshot(post_json(
        "/api/slack/conversations.join",
        json!({"channel": ch_id}),
        Some(&token_b),
    )).await.unwrap();

    // Alice posts a message
    let _ = app.clone().oneshot(post_json(
        "/api/slack/chat.postMessage",
        json!({"channel": ch_id, "text": "hello bob"}),
        Some(&token_a),
    )).await.unwrap();

    // Bob lists channels — should see unread > 0
    let resp = app.clone().oneshot(post_json(
        "/api/slack/conversations.list",
        json!({}),
        Some(&token_b),
    )).await.unwrap();
    let body = body_json(resp).await;
    let channels = body["channels"].as_array().unwrap();
    let ch = channels.iter().find(|c| c["id"].as_str().unwrap() == ch_id.to_string()).unwrap();
    assert!(ch["unread_count"].as_i64().unwrap() > 0, "expected unread > 0");

    // Bob reads history — implicitly marks as read
    let _ = app.clone().oneshot(post_json(
        "/api/slack/conversations.history",
        json!({"channel": ch_id}),
        Some(&token_b),
    )).await.unwrap();

    // Bob lists channels again — unread should be 0
    let resp = app.clone().oneshot(post_json(
        "/api/slack/conversations.list",
        json!({}),
        Some(&token_b),
    )).await.unwrap();
    let body = body_json(resp).await;
    let channels = body["channels"].as_array().unwrap();
    let ch = channels.iter().find(|c| c["id"].as_str().unwrap() == ch_id.to_string()).unwrap();
    assert_eq!(ch["unread_count"].as_i64().unwrap(), 0, "expected unread = 0 after reading history");
}
```

- [x] **Step 2: Run test to verify it fails**

```bash
cargo test test_unread_tracking -- --nocapture
```

Expected: FAIL — no `unread_count` field in conversations.list response.

- [x] **Step 3: Update conversations.history to record read timestamp**

In `handlers.rs`, in `conversations_history()`, just before the final `slack::ok(...)` return line, add:

```rust
    // Update channel_reads for this user
    let now = now_timestamp();
    let read_check = format!(
        "SELECT user_id FROM channel_reads WHERE channel_id = {} AND user_id = {}",
        req.channel, claims.user_id
    );
    let has_existing = state.api.query_router().query_sync(&read_check)
        .map(|r| !r.rows.is_empty())
        .unwrap_or(false);

    if has_existing {
        let update_sql = format!(
            "UPDATE channel_reads SET last_read_ts = '{}' WHERE channel_id = {} AND user_id = {}",
            escape_sql(&now), req.channel, claims.user_id
        );
        let _ = state.api.query_router().query_sync(&update_sql);
    } else {
        let insert_sql = format!(
            "INSERT INTO channel_reads (channel_id, user_id, last_read_ts) VALUES ({}, {}, '{}')",
            req.channel, claims.user_id, escape_sql(&now)
        );
        let _ = state.api.query_router().query_sync(&insert_sql);
    }
```

- [x] **Step 4: Add unread_count to conversations.list response**

In `conversations_list()`, replace the `.map(|row| { json!({...}) })` closure to compute unread count per channel:

```rust
    let channels: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            let ch_id_val = &row[0];
            let ch_id_str = match ch_id_val {
                crate::connector::Value::Int(n) => n.to_string(),
                _ => ch_id_val.to_json().as_str().unwrap_or("0").to_string(),
            };

            // Get last_read_ts for this channel
            let read_sql = format!(
                "SELECT last_read_ts FROM channel_reads WHERE channel_id = {} AND user_id = {}",
                ch_id_str, claims.user_id
            );
            let last_read_ts = state.api.query_router().query_sync(&read_sql).ok()
                .and_then(|r| r.rows.first().map(|row| row[0].to_json()))
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .unwrap_or_default();

            // Count unread messages
            let unread_sql = if last_read_ts.is_empty() {
                format!(
                    "SELECT COUNT(*) AS cnt FROM messages WHERE channel_id = {} AND deleted_at = ''",
                    ch_id_str
                )
            } else {
                format!(
                    "SELECT COUNT(*) AS cnt FROM messages WHERE channel_id = {} AND created_at > '{}' AND deleted_at = ''",
                    ch_id_str, escape_sql(&last_read_ts)
                )
            };
            let unread_count = state.api.query_router().query_sync(&unread_sql).ok()
                .and_then(|r| r.rows.first().map(|row| row[0].to_json()))
                .and_then(|v| v.as_str().and_then(|s| s.parse::<i64>().ok()))
                .unwrap_or(0);

            json!({
                "id": row[0].to_json(),
                "name": row[1].to_json(),
                "kind": row[2].to_json(),
                "topic": row[3].to_json(),
                "created_at": row[4].to_json(),
                "unread_count": unread_count,
            })
        })
        .collect();
```

- [x] **Step 5: Run test to verify it passes**

```bash
cargo test test_unread_tracking -- --nocapture
```

Expected: PASS

- [x] **Step 6: Commit**

```bash
git add src/chat/handlers.rs tests/chat_integration.rs
git commit -m "feat(chat): implement unread tracking via channel_reads table"
```

---

### Task 4: Add conversations.markRead endpoint

**Files:**
- Modify: `src/chat/handlers.rs`
- Modify: `tests/chat_integration.rs`

The frontend needs an explicit way to mark a channel as read (not just on history fetch).

- [x] **Step 1: Write failing test**

Add to `tests/chat_integration.rs`:

```rust
#[tokio::test]
async fn test_conversations_mark_read() {
    std::env::set_var("TEIDE_CHAT_SECRET", "test-secret-key-12345");
    let tmp = tempfile::tempdir().unwrap();
    let api = teidelum::api::TeidelumApi::open(tmp.path()).unwrap();
    teidelum::chat::models::init_chat_tables(&api).unwrap();
    let api = std::sync::Arc::new(api);
    let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());
    let state = std::sync::Arc::new(teidelum::chat::handlers::ChatState {
        api: api.clone(),
        hub: hub.clone(),
    });
    let app = teidelum::chat::handlers::chat_routes(state);

    // Register and login
    let resp = app.clone().oneshot(post_json(
        "/api/slack/auth.register",
        json!({"username": "carol", "password": "secret123", "email": "carol@test.com"}),
        None,
    )).await.unwrap();
    let token = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Create channel
    let resp = app.clone().oneshot(post_json(
        "/api/slack/conversations.create",
        json!({"name": "mark-read-test"}),
        Some(&token),
    )).await.unwrap();
    let ch_id: i64 = body_json(resp).await["channel"]["id"].as_str().unwrap().parse().unwrap();

    // Post message
    let _ = app.clone().oneshot(post_json(
        "/api/slack/chat.postMessage",
        json!({"channel": ch_id, "text": "test message"}),
        Some(&token),
    )).await.unwrap();

    // Mark as read
    let resp = app.clone().oneshot(post_json(
        "/api/slack/conversations.markRead",
        json!({"channel": ch_id}),
        Some(&token),
    )).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // List channels — should have 0 unread
    let resp = app.clone().oneshot(post_json(
        "/api/slack/conversations.list",
        json!({}),
        Some(&token),
    )).await.unwrap();
    let body = body_json(resp).await;
    let channels = body["channels"].as_array().unwrap();
    let ch = channels.iter().find(|c| c["id"].as_str().unwrap() == ch_id.to_string()).unwrap();
    assert_eq!(ch["unread_count"].as_i64().unwrap(), 0);
}
```

- [x] **Step 2: Run test to verify it fails**

```bash
cargo test test_conversations_mark_read -- --nocapture
```

Expected: FAIL — 404 on `/api/slack/conversations.markRead`.

- [x] **Step 3: Add request struct and handler**

In `handlers.rs`, add the request struct:

```rust
#[derive(Deserialize)]
pub struct MarkReadRequest {
    pub channel: i64,
    #[serde(default)]
    pub ts: Option<String>,
}
```

Add the handler function:

```rust
pub async fn conversations_mark_read(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<MarkReadRequest>,
) -> Response {
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    let ts = req.ts.unwrap_or_else(now_timestamp);

    // Upsert channel_reads
    let check_sql = format!(
        "SELECT user_id FROM channel_reads WHERE channel_id = {} AND user_id = {}",
        req.channel, claims.user_id
    );
    let has_existing = state.api.query_router().query_sync(&check_sql)
        .map(|r| !r.rows.is_empty())
        .unwrap_or(false);

    if has_existing {
        let sql = format!(
            "UPDATE channel_reads SET last_read_ts = '{}' WHERE channel_id = {} AND user_id = {}",
            escape_sql(&ts), req.channel, claims.user_id
        );
        if let Err(e) = state.api.query_router().query_sync(&sql) {
            tracing::error!("mark read update failed: {e}");
            return slack::err("internal_error");
        }
    } else {
        let sql = format!(
            "INSERT INTO channel_reads (channel_id, user_id, last_read_ts) VALUES ({}, {}, '{}')",
            req.channel, claims.user_id, escape_sql(&ts)
        );
        if let Err(e) = state.api.query_router().query_sync(&sql) {
            tracing::error!("mark read insert failed: {e}");
            return slack::err("internal_error");
        }
    }

    slack::ok(json!({}))
}
```

Register route in `chat_routes()` inside the authed router, before the `.layer(middleware::from_fn(...))` line:

```rust
        .route(
            "/conversations.markRead",
            axum::routing::post(conversations_mark_read),
        )
```

- [x] **Step 4: Run tests**

```bash
cargo test test_conversations_mark_read -- --nocapture
```

Expected: PASS

- [x] **Step 5: Commit**

```bash
git add src/chat/handlers.rs tests/chat_integration.rs
git commit -m "feat(chat): add conversations.markRead endpoint"
```

---

## Chunk 3: Thread Metadata

### Task 5: Add reply_count and last_reply_ts to message responses

**Files:**
- Modify: `src/chat/handlers.rs`
- Modify: `tests/chat_integration.rs`

When returning messages in `conversations.history`, parent messages should include `reply_count` and `last_reply_ts` so the frontend can show "N replies" badges.

- [x] **Step 1: Write failing test**

Add to `tests/chat_integration.rs`:

```rust
#[tokio::test]
async fn test_thread_metadata() {
    std::env::set_var("TEIDE_CHAT_SECRET", "test-secret-key-12345");
    let tmp = tempfile::tempdir().unwrap();
    let api = teidelum::api::TeidelumApi::open(tmp.path()).unwrap();
    teidelum::chat::models::init_chat_tables(&api).unwrap();
    let api = std::sync::Arc::new(api);
    let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());
    let state = std::sync::Arc::new(teidelum::chat::handlers::ChatState {
        api: api.clone(),
        hub: hub.clone(),
    });
    let app = teidelum::chat::handlers::chat_routes(state);

    // Register
    let resp = app.clone().oneshot(post_json(
        "/api/slack/auth.register",
        json!({"username": "dave", "password": "secret123", "email": "dave@test.com"}),
        None,
    )).await.unwrap();
    let token = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Create channel
    let resp = app.clone().oneshot(post_json(
        "/api/slack/conversations.create",
        json!({"name": "thread-test"}),
        Some(&token),
    )).await.unwrap();
    let ch_id: i64 = body_json(resp).await["channel"]["id"].as_str().unwrap().parse().unwrap();

    // Post parent message
    let resp = app.clone().oneshot(post_json(
        "/api/slack/chat.postMessage",
        json!({"channel": ch_id, "text": "parent message"}),
        Some(&token),
    )).await.unwrap();
    let parent_ts = body_json(resp).await["message"]["ts"].as_str().unwrap().to_string();
    let parent_id: i64 = parent_ts.parse().unwrap();

    // Post 3 replies
    for i in 0..3 {
        let _ = app.clone().oneshot(post_json(
            "/api/slack/chat.postMessage",
            json!({"channel": ch_id, "text": format!("reply {i}"), "thread_ts": parent_id}),
            Some(&token),
        )).await.unwrap();
    }

    // Fetch history — parent should have reply_count: 3
    let resp = app.clone().oneshot(post_json(
        "/api/slack/conversations.history",
        json!({"channel": ch_id}),
        Some(&token),
    )).await.unwrap();
    let body = body_json(resp).await;
    let messages = body["messages"].as_array().unwrap();

    // History returns top-level messages only (thread_id == 0), so parent should be there
    let parent = messages.iter().find(|m| m["ts"].as_str().unwrap() == parent_ts).unwrap();
    assert_eq!(parent["reply_count"].as_i64().unwrap(), 3, "expected 3 replies");
    assert!(parent["last_reply_ts"].is_string(), "expected last_reply_ts to be set");
}
```

- [x] **Step 2: Run test to verify it fails**

```bash
cargo test test_thread_metadata -- --nocapture
```

Expected: FAIL — no `reply_count` field on parent message.

- [x] **Step 3: Enrich parent messages with reply metadata in conversations.history**

In `conversations_history()`, change the `messages` variable to `mut`:

```rust
    let mut messages: Vec<serde_json::Value> = result
```

Then, after the `.collect()` and before the `channel_reads` update code (added in Task 3), add:

```rust
    // Enrich parent messages with reply metadata
    for msg in messages.iter_mut() {
        let msg_id_str = msg["ts"].as_str().unwrap_or("0");
        let msg_id: i64 = msg_id_str.parse().unwrap_or(0);
        if msg_id == 0 {
            continue;
        }
        let reply_sql = format!(
            "SELECT COUNT(*) AS cnt FROM messages WHERE thread_id = {} AND deleted_at = ''",
            msg_id
        );
        if let Ok(reply_result) = state.api.query_router().query_sync(&reply_sql) {
            if let Some(row) = reply_result.rows.first() {
                let count_str = row[0].to_json();
                let count: i64 = count_str.as_str()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                if count > 0 {
                    msg.as_object_mut().unwrap().insert(
                        "reply_count".to_string(),
                        serde_json::json!(count),
                    );
                    // Get last reply timestamp
                    let last_sql = format!(
                        "SELECT MAX(created_at) AS last_reply FROM messages WHERE thread_id = {} AND deleted_at = ''",
                        msg_id
                    );
                    if let Ok(last_result) = state.api.query_router().query_sync(&last_sql) {
                        if let Some(last_row) = last_result.rows.first() {
                            msg.as_object_mut().unwrap().insert(
                                "last_reply_ts".to_string(),
                                last_row[0].to_json(),
                            );
                        }
                    }
                }
            }
        }
    }
```

- [x] **Step 4: Run test to verify it passes**

```bash
cargo test test_thread_metadata -- --nocapture
```

Expected: PASS

- [x] **Step 5: Commit**

```bash
git add src/chat/handlers.rs tests/chat_integration.rs
git commit -m "feat(chat): add reply_count and last_reply_ts to message history"
```

---

## Chunk 4: Test Coverage

### Task 6: Add integration tests for DMs, presence, and file operations

**Files:**
- Modify: `tests/chat_integration.rs`

- [x] **Step 1: Add DM (conversations.open) test**

```rust
#[tokio::test]
async fn test_dm_conversation() {
    std::env::set_var("TEIDE_CHAT_SECRET", "test-secret-key-12345");
    let tmp = tempfile::tempdir().unwrap();
    let api = teidelum::api::TeidelumApi::open(tmp.path()).unwrap();
    teidelum::chat::models::init_chat_tables(&api).unwrap();
    let api = std::sync::Arc::new(api);
    let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());
    let state = std::sync::Arc::new(teidelum::chat::handlers::ChatState {
        api: api.clone(),
        hub: hub.clone(),
    });
    let app = teidelum::chat::handlers::chat_routes(state);

    // Register alice
    let resp = app.clone().oneshot(post_json(
        "/api/slack/auth.register",
        json!({"username": "alice_dm", "password": "secret123", "email": "alice_dm@test.com"}),
        None,
    )).await.unwrap();
    let body_a = body_json(resp).await;
    let token_a = body_a["token"].as_str().unwrap().to_string();
    let user_a: i64 = body_a["user"]["id"].as_str().unwrap().parse().unwrap();

    // Register bob
    let resp = app.clone().oneshot(post_json(
        "/api/slack/auth.register",
        json!({"username": "bob_dm", "password": "secret123", "email": "bob_dm@test.com"}),
        None,
    )).await.unwrap();
    let body_b = body_json(resp).await;
    let token_b = body_b["token"].as_str().unwrap().to_string();
    let user_b: i64 = body_b["user"]["id"].as_str().unwrap().parse().unwrap();

    // Open DM
    let resp = app.clone().oneshot(post_json(
        "/api/slack/conversations.open",
        json!({"users": [user_a, user_b]}),
        Some(&token_a),
    )).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let dm_id: i64 = body["channel"]["id"].as_str().unwrap().parse().unwrap();

    // Alice posts in DM
    let resp = app.clone().oneshot(post_json(
        "/api/slack/chat.postMessage",
        json!({"channel": dm_id, "text": "hey bob, DM!"}),
        Some(&token_a),
    )).await.unwrap();
    assert_eq!(body_json(resp).await["ok"], true);

    // Bob reads DM history
    let resp = app.clone().oneshot(post_json(
        "/api/slack/conversations.history",
        json!({"channel": dm_id}),
        Some(&token_b),
    )).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["text"], "hey bob, DM!");

    // Open again — should return same channel
    let resp = app.clone().oneshot(post_json(
        "/api/slack/conversations.open",
        json!({"users": [user_a, user_b]}),
        Some(&token_a),
    )).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    assert_eq!(body["already_open"], true);
    let dm_id_2: i64 = body["channel"]["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(dm_id, dm_id_2, "should return same DM channel");
}
```

- [x] **Step 2: Add presence test**

```rust
#[tokio::test]
async fn test_presence_update() {
    std::env::set_var("TEIDE_CHAT_SECRET", "test-secret-key-12345");
    let tmp = tempfile::tempdir().unwrap();
    let api = teidelum::api::TeidelumApi::open(tmp.path()).unwrap();
    teidelum::chat::models::init_chat_tables(&api).unwrap();
    let api = std::sync::Arc::new(api);
    let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());
    let state = std::sync::Arc::new(teidelum::chat::handlers::ChatState {
        api: api.clone(),
        hub: hub.clone(),
    });
    let app = teidelum::chat::handlers::chat_routes(state);

    // Register
    let resp = app.clone().oneshot(post_json(
        "/api/slack/auth.register",
        json!({"username": "eve", "password": "secret123", "email": "eve@test.com"}),
        None,
    )).await.unwrap();
    let token = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Set presence to away
    let resp = app.clone().oneshot(post_json(
        "/api/slack/users.setPresence",
        json!({"presence": "away"}),
        Some(&token),
    )).await.unwrap();
    assert_eq!(body_json(resp).await["ok"], true);

    // Verify via users.list
    let resp = app.clone().oneshot(post_json(
        "/api/slack/users.list",
        json!({}),
        Some(&token),
    )).await.unwrap();
    let body = body_json(resp).await;
    let users = body["members"].as_array().unwrap();
    let eve = users.iter().find(|u| u["name"].as_str() == Some("eve")).unwrap();
    assert_eq!(eve["status"].as_str(), Some("away"));

    // Set invalid presence — should error
    let resp = app.clone().oneshot(post_json(
        "/api/slack/users.setPresence",
        json!({"presence": "invisible"}),
        Some(&token),
    )).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"], "invalid_presence");
}
```

- [x] **Step 3: Run all tests**

```bash
cargo test -- --nocapture
```

Expected: All tests pass.

- [x] **Step 4: Commit**

```bash
git add tests/chat_integration.rs
git commit -m "test: add integration tests for DMs and presence"
```

---

### Task 7: Add integration tests for mentions and reactions

**Files:**
- Modify: `tests/chat_integration.rs`

- [x] **Step 1: Add mention extraction test**

```rust
#[tokio::test]
async fn test_mention_extraction() {
    std::env::set_var("TEIDE_CHAT_SECRET", "test-secret-key-12345");
    let tmp = tempfile::tempdir().unwrap();
    let api = teidelum::api::TeidelumApi::open(tmp.path()).unwrap();
    teidelum::chat::models::init_chat_tables(&api).unwrap();
    let api = std::sync::Arc::new(api);
    let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());
    let state = std::sync::Arc::new(teidelum::chat::handlers::ChatState {
        api: api.clone(),
        hub: hub.clone(),
    });
    let app = teidelum::chat::handlers::chat_routes(state.clone());

    // Register alice and bob
    let resp = app.clone().oneshot(post_json(
        "/api/slack/auth.register",
        json!({"username": "alice_m", "password": "secret123", "email": "alice_m@test.com"}),
        None,
    )).await.unwrap();
    let token_a = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app.clone().oneshot(post_json(
        "/api/slack/auth.register",
        json!({"username": "bob_m", "password": "secret123", "email": "bob_m@test.com"}),
        None,
    )).await.unwrap();
    let _token_b = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Alice creates channel
    let resp = app.clone().oneshot(post_json(
        "/api/slack/conversations.create",
        json!({"name": "mention-test"}),
        Some(&token_a),
    )).await.unwrap();
    let ch_id: i64 = body_json(resp).await["channel"]["id"].as_str().unwrap().parse().unwrap();

    // Alice posts message mentioning bob
    let _ = app.clone().oneshot(post_json(
        "/api/slack/chat.postMessage",
        json!({"channel": ch_id, "text": "hey @bob_m check this out"}),
        Some(&token_a),
    )).await.unwrap();

    // Verify mention was stored by querying mentions table directly
    let sql = "SELECT message_id, user_id FROM mentions";
    let result = state.api.query_router().query_sync(sql).unwrap();
    assert!(!result.rows.is_empty(), "expected mention rows");

    // Post message without mentions
    let before_count = result.rows.len();
    let _ = app.clone().oneshot(post_json(
        "/api/slack/chat.postMessage",
        json!({"channel": ch_id, "text": "no mentions here"}),
        Some(&token_a),
    )).await.unwrap();

    let result = state.api.query_router().query_sync(sql).unwrap();
    assert_eq!(result.rows.len(), before_count, "no new mentions should be added");
}
```

- [x] **Step 2: Add reaction lifecycle test**

```rust
#[tokio::test]
async fn test_reaction_lifecycle() {
    std::env::set_var("TEIDE_CHAT_SECRET", "test-secret-key-12345");
    let tmp = tempfile::tempdir().unwrap();
    let api = teidelum::api::TeidelumApi::open(tmp.path()).unwrap();
    teidelum::chat::models::init_chat_tables(&api).unwrap();
    let api = std::sync::Arc::new(api);
    let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());
    let state = std::sync::Arc::new(teidelum::chat::handlers::ChatState {
        api: api.clone(),
        hub: hub.clone(),
    });
    let app = teidelum::chat::handlers::chat_routes(state);

    // Register
    let resp = app.clone().oneshot(post_json(
        "/api/slack/auth.register",
        json!({"username": "frank", "password": "secret123", "email": "frank@test.com"}),
        None,
    )).await.unwrap();
    let token = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Create channel and post message
    let resp = app.clone().oneshot(post_json(
        "/api/slack/conversations.create",
        json!({"name": "reaction-test"}),
        Some(&token),
    )).await.unwrap();
    let ch_id: i64 = body_json(resp).await["channel"]["id"].as_str().unwrap().parse().unwrap();

    let resp = app.clone().oneshot(post_json(
        "/api/slack/chat.postMessage",
        json!({"channel": ch_id, "text": "react to this"}),
        Some(&token),
    )).await.unwrap();
    let msg_ts = body_json(resp).await["message"]["ts"].as_str().unwrap().to_string();
    let msg_id: i64 = msg_ts.parse().unwrap();

    // Add reaction
    let resp = app.clone().oneshot(post_json(
        "/api/slack/reactions.add",
        json!({"channel": ch_id, "timestamp": msg_id, "name": "thumbsup"}),
        Some(&token),
    )).await.unwrap();
    assert_eq!(body_json(resp).await["ok"], true);

    // Add same reaction again — should not create duplicate
    let resp = app.clone().oneshot(post_json(
        "/api/slack/reactions.add",
        json!({"channel": ch_id, "timestamp": msg_id, "name": "thumbsup"}),
        Some(&token),
    )).await.unwrap();
    let body = body_json(resp).await;
    // Should either succeed silently or return already_reacted
    assert!(body["ok"] == true || body["error"] == "already_reacted");

    // Remove reaction
    let resp = app.clone().oneshot(post_json(
        "/api/slack/reactions.remove",
        json!({"channel": ch_id, "timestamp": msg_id, "name": "thumbsup"}),
        Some(&token),
    )).await.unwrap();
    assert_eq!(body_json(resp).await["ok"], true);

    // Remove again — should fail (not found)
    let resp = app.clone().oneshot(post_json(
        "/api/slack/reactions.remove",
        json!({"channel": ch_id, "timestamp": msg_id, "name": "thumbsup"}),
        Some(&token),
    )).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false);
}
```

- [x] **Step 3: Run all tests**

```bash
cargo test -- --nocapture
```

Expected: All tests pass.

- [x] **Step 4: Commit**

```bash
git add tests/chat_integration.rs
git commit -m "test: add integration tests for mentions and reactions"
```

---

## Chunk 5: Production Hardening

### Task 8: Add .gitignore for frontend build artifacts

**Files:**
- Create: `ui/.gitignore` (if doesn't exist, or modify if it does)

- [x] **Step 1: Ensure ui/.gitignore includes build artifacts**

`ui/.gitignore` must contain:

```
build/
.svelte-kit/
node_modules/
```

The `build/` directory should not be committed — it's generated by `npm run build`.

- [x] **Step 2: Commit**

```bash
git add ui/.gitignore
git commit -m "chore: add ui/.gitignore for build artifacts"
```

---

### Task 9: Update CLAUDE.md

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Add production build instructions to CLAUDE.md**

Add to the Frontend section of CLAUDE.md:

```markdown
### Production Build

```bash
cd ui && npm run build          # builds SPA to ui/build/
cargo build --release           # builds server binary
TEIDE_CHAT_SECRET=<secret> ./target/release/teidelum  # serves frontend + API on :3000
```
```

- [ ] **Step 2: Add new feature documentation to CLAUDE.md**

Under Key Design Patterns, add:

```markdown
- **Unread Tracking** (`chat/handlers.rs`): `channel_reads` table stores `last_read_ts` per user per channel. Updated on `conversations.history` fetch and `conversations.markRead`. Unread count computed in `conversations.list` by counting messages after `last_read_ts`.
- **Thread Metadata**: `conversations.history` enriches parent messages with `reply_count` and `last_reply_ts` computed from the messages table. No denormalized columns — always computed fresh.
- **Static Frontend Serving** (`server.rs`): When `ui/build/` exists, Axum serves it as a fallback after API routes. SPA routing handled via `index.html` fallback. In dev, use Vite proxy instead.
```

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: update CLAUDE.md with production build and new features"
```

---

### Task 10: Merge to master

**Files:** None (git operations only)

- [ ] **Step 1: Run full test suite**

```bash
cargo test
cargo clippy -- -D warnings
cargo fmt --check
cd ui && npm run build && npx svelte-check
```

All must pass.

- [ ] **Step 2: Merge teide-chat-frontend into master**

```bash
git checkout master
git merge teide-chat-frontend --no-ff -m "merge: teidelum production readiness — frontend build, unread tracking, thread metadata, tests"
```

- [ ] **Step 3: Verify post-merge**

```bash
cargo test
```

Expected: All tests pass.

- [ ] **Step 4: Push**

```bash
git push origin master
```

- [ ] **Step 5: Clean up feature branches**

```bash
git branch -d teide-chat-frontend
git branch -d teide-chat-search-files-mcp
git branch -d api-layer
git branch -d graph-layer
```

---

## Chunk 6: Tauri Desktop Client

### Task 11: Scaffold Tauri project wrapping the SvelteKit frontend

**Files:**
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/icons/` (default icons)
- Modify: `ui/package.json` (add tauri scripts)

Tauri wraps the same `ui/build/` SPA into a native desktop app. The app connects to a remote Teidelum server — it does NOT embed the Rust server. It's a thin native shell around the web frontend.

- [ ] **Step 1: Install Tauri CLI**

```bash
cd ui && npm install -D @tauri-apps/cli@^2
```

- [ ] **Step 2: Initialize Tauri project**

```bash
cd ui && npx tauri init
```

When prompted:
- App name: `Teidelum`
- Window title: `Teidelum`
- Frontend dev URL: `http://localhost:5173` (Vite dev server)
- Frontend dist directory: `../ui/build` (relative to src-tauri)
- Frontend dev command: `npm run dev`
- Frontend build command: `npm run build`

This creates `src-tauri/` inside the `ui/` directory.

- [ ] **Step 3: Configure tauri.conf.json**

Update `ui/src-tauri/tauri.conf.json` to set the correct build paths and window configuration:

```json
{
  "$schema": "https://raw.githubusercontent.com/nicbarker/tauri/v2/crates/tauri-cli/config.schema.json",
  "productName": "Teidelum",
  "version": "0.1.0",
  "identifier": "com.teidelum.app",
  "build": {
    "frontendDist": "../build",
    "devUrl": "http://localhost:5173",
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build"
  },
  "app": {
    "title": "Teidelum",
    "windows": [
      {
        "title": "Teidelum",
        "width": 1200,
        "height": 800,
        "minWidth": 800,
        "minHeight": 600,
        "resizable": true
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ]
  }
}
```

- [ ] **Step 4: Update Tauri's Rust entry point**

Replace `ui/src-tauri/src/main.rs` with:

```rust
// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running Teidelum");
}
```

- [ ] **Step 5: Add server URL configuration to the frontend**

The desktop app needs to know which Teidelum server to connect to. Update `ui/src/lib/api.ts` to support a configurable base URL:

In `api.ts`, ensure the base URL is configurable (not hardcoded to relative paths). If the app detects it's running inside Tauri (via `window.__TAURI__`), it should use a stored server URL; otherwise, use relative paths (web mode):

```typescript
function getBaseUrl(): string {
    // In Tauri, use configured server URL; in browser, use relative paths
    if (typeof window !== 'undefined' && '__TAURI__' in window) {
        return localStorage.getItem('teidelum_server_url') || 'http://localhost:3000';
    }
    return '';
}
```

Prefix all fetch calls in the API client with `getBaseUrl()`.

- [ ] **Step 6: Add tauri scripts to package.json**

Add to `ui/package.json` scripts:

```json
{
  "scripts": {
    "tauri": "tauri",
    "tauri:dev": "tauri dev",
    "tauri:build": "tauri build"
  }
}
```

- [ ] **Step 7: Verify Tauri dev mode**

```bash
cd ui && npm run tauri dev
```

Expected: Native window opens showing the Teidelum login page (requires backend running on localhost:3000).

- [ ] **Step 8: Build Tauri release**

```bash
cd ui && npm run tauri build
```

Expected: Platform-specific installer created in `ui/src-tauri/target/release/bundle/`.

- [ ] **Step 9: Add src-tauri to .gitignore exceptions**

Update `ui/.gitignore` to exclude Tauri build artifacts but include config:

```
# Tauri
src-tauri/target/
```

- [ ] **Step 10: Commit**

```bash
git add ui/src-tauri/ ui/package.json ui/package-lock.json ui/src/lib/api.ts ui/.gitignore
git commit -m "feat: add Tauri desktop client wrapping SvelteKit frontend"
```

---

## Summary

| Task | Description | Effort |
|------|-------------|--------|
| 1 | Switch to adapter-static | Small |
| 2 | Axum static file serving | Small |
| 3 | Unread tracking (channel_reads) | Medium |
| 4 | conversations.markRead endpoint | Small |
| 5 | Thread reply_count metadata | Medium |
| 6 | Tests: DMs, presence | Medium |
| 7 | Tests: mentions, reactions | Small |
| 8 | .gitignore for build | Trivial |
| 9 | Update CLAUDE.md | Small |
| 10 | Merge to master | Small |
| 11 | Tauri desktop client scaffold | Medium |
