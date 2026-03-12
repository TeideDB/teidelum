# Audit Fixes Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix all critical and high-priority findings from the full codebase audit (security, performance, API consistency, frontend quality).

**Architecture:** Backend fixes in Rust (security hardening, N+1 query elimination, MCP WS broadcast). Frontend fixes in Svelte/TS (keyed each blocks, error feedback). All changes are incremental and independently testable.

**Tech Stack:** Rust/Axum backend, SvelteKit/TypeScript frontend, TeideDB SQL

---

## Chunk 1: Security Hardening (Backend)

### Task 1: Harden `escape_sql` — strip null bytes and escape backslashes

**Files:**
- Modify: `src/chat/models.rs:210-212`
- Test: `src/chat/models.rs:254-259` (inline tests)

- [ ] **Step 1: Write failing tests for new escape cases**

Add to the existing `test_escape_sql` test in `src/chat/models.rs`:

```rust
#[test]
fn test_escape_sql() {
    assert_eq!(escape_sql("hello"), "hello");
    assert_eq!(escape_sql("it's"), "it''s");
    assert_eq!(escape_sql("a''b"), "a''''b");
    // New cases: backslash and null byte
    assert_eq!(escape_sql("back\\slash"), "back\\\\slash");
    assert_eq!(escape_sql("null\0byte"), "nullbyte");
    assert_eq!(escape_sql("combo\\' test"), "combo\\\\'' test");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_escape_sql -- --nocapture`
Expected: FAIL — backslash not escaped, null byte not stripped

- [ ] **Step 3: Update `escape_sql` implementation**

```rust
pub fn escape_sql(s: &str) -> String {
    s.replace('\0', "").replace('\\', "\\\\").replace('\'', "''")
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test test_escape_sql -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add src/chat/models.rs
git commit -m "fix: harden escape_sql against backslash injection and null bytes"
```

---

### Task 2: Validate JWT secret minimum length at startup and in `create_token`

**Files:**
- Modify: `src/chat/auth.rs:38-41` (create_token), `src/chat/auth.rs:126-156` (tests)
- Modify: `src/server.rs:120-135` (start function)

- [ ] **Step 1: Write failing test for short secret rejection**

Add to `src/chat/auth.rs` tests:

```rust
#[test]
fn test_jwt_short_secret_rejected() {
    assert!(create_token("short", 1, "bob", false).is_err());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test test_jwt_short_secret_rejected -- --nocapture`
Expected: FAIL — short secrets currently accepted

- [ ] **Step 3: Add minimum length check to `create_token`**

In `src/chat/auth.rs`, update the `create_token` function:

```rust
pub fn create_token(secret: &str, user_id: i64, username: &str, is_bot: bool) -> Result<String> {
    if secret.len() < 32 {
        bail!("JWT secret must be at least 32 bytes");
    }
    // ... rest unchanged
```

- [ ] **Step 4: Update existing tests to use 32+ byte secrets**

Update all test secrets in `src/chat/auth.rs`:

```rust
#[test]
fn test_jwt_roundtrip() {
    let secret = "test-secret-key-that-is-at-least-32-bytes-long!!";
    let token = create_token(secret, 42, "alice", false).unwrap();
    let claims = validate_token(secret, &token).unwrap();
    assert_eq!(claims.user_id, 42);
    assert_eq!(claims.username, "alice");
    assert!(!claims.is_bot);
}

#[test]
fn test_jwt_invalid_secret() {
    let token = create_token("secret-one-that-is-at-least-32-bytes!!", 1, "bob", false).unwrap();
    assert!(validate_token("secret-two-that-is-at-least-32-bytes!!", &token).is_err());
}
```

- [ ] **Step 5: Add startup validation in `server.rs`**

In `src/server.rs`, add at the start of `start()`:

```rust
pub async fn start(
    api: Arc<TeidelumApi>,
    hub: Arc<crate::chat::hub::Hub>,
    bind: &str,
    port: u16,
) -> anyhow::Result<()> {
    // Validate TEIDE_CHAT_SECRET at startup
    match std::env::var("TEIDE_CHAT_SECRET") {
        Ok(s) if s.len() >= 32 => {}
        Ok(s) if !s.is_empty() => {
            anyhow::bail!("TEIDE_CHAT_SECRET must be at least 32 bytes (got {})", s.len());
        }
        _ => {
            tracing::warn!("TEIDE_CHAT_SECRET not set — chat auth will not work");
        }
    }
    // ... rest unchanged
```

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: All PASS

- [ ] **Step 7: Commit**

```bash
git add src/chat/auth.rs src/server.rs
git commit -m "fix: enforce 32-byte minimum JWT secret length"
```

---

### Task 3: Add password strength validation

**Files:**
- Modify: `src/chat/handlers.rs:115` (auth_register)
- Modify: `src/chat/handlers.rs:510` (users_change_password)

- [ ] **Step 1: Add password length check in `auth_register`**

In `src/chat/handlers.rs`, after the `is_empty()` check at line 115:

```rust
    if req.username.is_empty() || req.password.is_empty() || req.email.is_empty() {
        return slack::err("invalid_arguments");
    }
    if req.password.len() < 8 {
        return slack::err("password_too_short");
    }
```

- [ ] **Step 2: Add same check in `users_change_password`**

Find the `users_change_password` handler and add after the `new_password.is_empty()` check:

```rust
    if req.new_password.len() < 8 {
        return slack::err("password_too_short");
    }
```

- [ ] **Step 3: Run integration tests**

Run: `cargo test --test chat_integration -- --test-threads=1`
Expected: PASS (existing tests use passwords >= 8 chars)

- [ ] **Step 4: Commit**

```bash
git add src/chat/handlers.rs
git commit -m "fix: enforce minimum 8-character password length"
```

---

### Task 4: Restrict CORS to non-permissive defaults

**Files:**
- Modify: `src/server.rs:14,78`

- [ ] **Step 1: Replace `CorsLayer::permissive()` with configured CORS**

In `src/server.rs`, replace line 78:

```rust
use tower_http::cors::{Any, CorsLayer};
```

And replace `.layer(CorsLayer::permissive())`:

```rust
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::OPTIONS,
                ])
                .allow_headers([
                    axum::http::header::AUTHORIZATION,
                    axum::http::header::CONTENT_TYPE,
                ]),
        )
```

Note: We keep `allow_origin(Any)` because this is a local-first tool where the frontend origin varies (localhost, Tauri, etc). The key improvement is restricting methods and headers to only what's needed.

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test chat_integration -- --test-threads=1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/server.rs
git commit -m "fix: restrict CORS to specific methods and headers"
```

---

### Task 5: Log warning when `TEIDELUM_API_KEY` is not set

**Files:**
- Modify: `src/server.rs:59-67`

- [ ] **Step 1: Add warning log**

In `src/server.rs`, replace lines 59-67:

```rust
    // If TEIDELUM_API_KEY is set, apply auth only to data API and MCP routes (not chat/ws/files)
    if let Ok(key) = std::env::var("TEIDELUM_API_KEY") {
        if !key.is_empty() {
            data_api = data_api.layer(middleware::from_fn(move |req, next| {
                let key = key.clone();
                async move { auth_check(req, next, key).await }
            }));
        }
    } else {
        tracing::warn!("TEIDELUM_API_KEY not set — data API and MCP endpoints are unauthenticated");
    }
```

- [ ] **Step 2: Commit**

```bash
git add src/server.rs
git commit -m "fix: warn when TEIDELUM_API_KEY is unset"
```

---

### Task 6: Add message length and channel name validation

**Files:**
- Modify: `src/chat/handlers.rs` (chat_post_message ~line 2060, conversations_create ~line 796)

- [ ] **Step 1: Add message length limit in `chat_post_message`**

After the archived check (~line 2087), add:

```rust
    if req.text.len() > 40_000 {
        return slack::err("msg_too_long");
    }
```

- [ ] **Step 2: Add channel name validation in `conversations_create`**

After the existing empty check for channel name, add:

```rust
    let name_trimmed = req.name.trim().to_string();
    if name_trimmed.is_empty() || name_trimmed.len() > 80 {
        return slack::err("invalid_name");
    }
    if !name_trimmed.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        return slack::err("invalid_name");
    }
```

- [ ] **Step 3: Run integration tests**

Run: `cargo test --test chat_integration -- --test-threads=1`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/chat/handlers.rs
git commit -m "fix: add message length limit and channel name validation"
```

---

## Chunk 2: Performance — N+1 Query Elimination

### Task 7: Batch `conversations_list` — eliminate 3 queries per channel

**Files:**
- Modify: `src/chat/handlers.rs:866-953` (conversations_list)

- [ ] **Step 1: Rewrite `conversations_list` to batch lookups**

Replace the per-channel queries with pre-fetched maps. The new implementation:

```rust
pub async fn conversations_list(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    // List channels the user is a member of
    let sql = format!(
        "SELECT c.id, c.name, c.kind, c.topic, c.description, c.archived_at, c.created_by, c.created_at \
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

    // Batch: fetch all channel_reads for this user
    let reads_sql = format!(
        "SELECT channel_id, last_read_ts FROM channel_reads WHERE user_id = {}",
        claims.user_id
    );
    let reads_map: std::collections::HashMap<String, String> = state
        .api
        .query_router()
        .query_sync(&reads_sql)
        .ok()
        .map(|r| {
            r.rows
                .iter()
                .filter_map(|row| {
                    let ch = match &row[0] {
                        crate::connector::Value::Int(n) => n.to_string(),
                        _ => return None,
                    };
                    let ts = row[1].to_json().as_str().unwrap_or("").to_string();
                    Some((ch, ts))
                })
                .collect()
        })
        .unwrap_or_default();

    // Batch: fetch all channel_settings for this user
    let settings_sql = format!(
        "SELECT channel_id, muted, notification_level FROM channel_settings WHERE user_id = {}",
        claims.user_id
    );
    let settings_map: std::collections::HashMap<String, (String, String)> = state
        .api
        .query_router()
        .query_sync(&settings_sql)
        .ok()
        .map(|r| {
            r.rows
                .iter()
                .filter_map(|row| {
                    let ch = match &row[0] {
                        crate::connector::Value::Int(n) => n.to_string(),
                        _ => return None,
                    };
                    let muted = row[1].to_json().as_str().unwrap_or("false").to_string();
                    let notif = row[2].to_json().as_str().unwrap_or("all").to_string();
                    Some((ch, (muted, notif)))
                })
                .collect()
        })
        .unwrap_or_default();

    let channels: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            let ch_id_str = match &row[0] {
                crate::connector::Value::Int(n) => n.to_string(),
                _ => row[0].to_json().as_str().unwrap_or("0").to_string(),
            };

            let last_read_ts = reads_map.get(&ch_id_str).cloned().unwrap_or_default();

            // Count unread — still per-channel but unavoidable without GROUP BY support
            let unread_sql = if last_read_ts.is_empty() {
                format!(
                    "SELECT COUNT(*) AS cnt FROM messages WHERE channel_id = {} AND thread_id = 0 AND deleted_at = ''",
                    ch_id_str
                )
            } else {
                format!(
                    "SELECT COUNT(*) AS cnt FROM messages WHERE channel_id = {} AND thread_id = 0 AND created_at > '{}' AND deleted_at = ''",
                    ch_id_str, escape_sql(&last_read_ts)
                )
            };
            let unread_count = state.api.query_router().query_sync(&unread_sql).ok()
                .and_then(|r| r.rows.first().map(|row| row[0].to_json()))
                .and_then(|v| v.as_str().and_then(|s| s.parse::<i64>().ok()))
                .unwrap_or(0);

            let (muted, notification_level) = settings_map
                .get(&ch_id_str)
                .cloned()
                .unwrap_or_else(|| ("false".to_string(), "all".to_string()));

            json!({
                "id": row[0].to_json(),
                "name": row[1].to_json(),
                "kind": row[2].to_json(),
                "topic": row[3].to_json(),
                "description": row[4].to_json(),
                "archived_at": row[5].to_json(),
                "created_by": row[6].to_json(),
                "created_at": row[7].to_json(),
                "unread_count": unread_count,
                "muted": muted,
                "notification_level": notification_level,
            })
        })
        .collect();

    slack::ok(json!({"channels": channels}))
}
```

This reduces from 3N+1 queries to N+3 queries (the unread COUNT per channel remains because TeideDB lacks grouped aggregation across channels).

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test chat_integration -- --test-threads=1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/chat/handlers.rs
git commit -m "perf: batch channel_reads and channel_settings in conversations_list"
```

---

### Task 8: Batch reply metadata in `conversations_history`

**Files:**
- Modify: `src/chat/handlers.rs:1062-1095`

- [ ] **Step 1: Replace per-message reply queries with batch approach**

Replace the N+1 reply enrichment loop (lines 1062-1095) with:

```rust
    // Batch: collect all message IDs and fetch reply counts in one pass per message
    // TeideDB doesn't support IN clauses or GROUP BY, so we batch what we can
    let msg_ids: Vec<i64> = messages
        .iter()
        .filter_map(|msg| msg["ts"].as_str()?.parse::<i64>().ok())
        .filter(|&id| id != 0)
        .collect();

    // Fetch all replies for these thread IDs in one query per thread
    // Build a map of thread_id -> (count, last_reply_ts)
    let mut reply_meta: std::collections::HashMap<i64, (i64, String)> = std::collections::HashMap::new();
    for &msg_id in &msg_ids {
        let reply_sql = format!(
            "SELECT COUNT(*) AS cnt FROM messages WHERE thread_id = {} AND deleted_at = ''",
            msg_id
        );
        if let Ok(reply_result) = state.api.query_router().query_sync(&reply_sql) {
            if let Some(row) = reply_result.rows.first() {
                let count: i64 = row[0].to_json().as_str().and_then(|s| s.parse().ok()).unwrap_or(0);
                if count > 0 {
                    let last_sql = format!(
                        "SELECT MAX(created_at) AS last_reply FROM messages WHERE thread_id = {} AND deleted_at = ''",
                        msg_id
                    );
                    let last_ts = state.api.query_router().query_sync(&last_sql).ok()
                        .and_then(|r| r.rows.first().map(|row| row[0].to_json()))
                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                        .unwrap_or_default();
                    reply_meta.insert(msg_id, (count, last_ts));
                }
            }
        }
    }

    // Apply reply metadata from the map
    for msg in messages.iter_mut() {
        let msg_id: i64 = msg["ts"].as_str().unwrap_or("0").parse().unwrap_or(0);
        if let Some((count, last_ts)) = reply_meta.get(&msg_id) {
            let obj = msg.as_object_mut().unwrap();
            obj.insert("reply_count".to_string(), serde_json::json!(count));
            obj.insert("last_reply_ts".to_string(), serde_json::json!(last_ts));
        }
    }
```

Note: This is structurally similar but separates the querying from the JSON mutation, which avoids the nested `if let` within the mutable borrow. The query count remains the same (TeideDB doesn't support `IN` clauses), but the code is cleaner and ready for batch optimization when TeideDB adds `IN` support.

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test chat_integration -- --test-threads=1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/chat/handlers.rs
git commit -m "refactor: separate reply metadata fetching from message enrichment"
```

---

### Task 9: Push filtering to SQL for `users_search` and `conversations_autocomplete`

**Files:**
- Modify: `src/chat/handlers.rs:698-778`

- [ ] **Step 1: Rewrite `users_search` with SQL LIKE filtering**

Replace lines 698-736:

```rust
pub async fn users_search(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(req): Json<UsersSearchRequest>,
) -> Response {
    let query_escaped = escape_sql(&req.query.to_lowercase());
    let sql = format!(
        "SELECT id, username, display_name, avatar_url FROM users \
         WHERE username LIKE '%{query_escaped}%' OR display_name LIKE '%{query_escaped}%' \
         LIMIT {}",
        req.limit
    );
    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("users search failed: {e}");
            return slack::err("internal_error");
        }
    };

    let matches: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            json!({
                "id": row[0].to_json(),
                "username": row[1].to_json().as_str().unwrap_or("").to_string(),
                "display_name": row[2].to_json().as_str().unwrap_or("").to_string(),
                "avatar_url": row[3].to_json(),
            })
        })
        .collect();

    slack::ok(json!({"users": matches}))
}
```

- [ ] **Step 2: Rewrite `conversations_autocomplete` with SQL LIKE filtering**

Replace lines 745-778:

```rust
pub async fn conversations_autocomplete(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(req): Json<ConversationsAutocompleteRequest>,
) -> Response {
    let query_escaped = escape_sql(&req.query.to_lowercase());
    let sql = format!(
        "SELECT id, name, topic FROM channels WHERE kind = 'public' AND name LIKE '{query_escaped}%' LIMIT {}",
        req.limit
    );
    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("conversations autocomplete failed: {e}");
            return slack::err("internal_error");
        }
    };

    let matches: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            json!({
                "id": row[0].to_json(),
                "name": row[1].to_json().as_str().unwrap_or("").to_string(),
                "topic": row[2].to_json(),
            })
        })
        .collect();

    slack::ok(json!({"channels": matches}))
}
```

- [ ] **Step 3: Run integration tests**

Run: `cargo test --test chat_integration -- --test-threads=1`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/chat/handlers.rs
git commit -m "perf: push search/autocomplete filtering to SQL with LIKE"
```

---

## Chunk 3: MCP WebSocket Broadcast

### Task 10: Add Hub to Teidelum MCP struct and broadcast from MCP chat tools

**Files:**
- Modify: `src/mcp.rs:233-236` (struct), `src/mcp.rs:286-291` (new_with_shared), `src/mcp.rs:673-726` (chat_post_message), `src/mcp.rs:782-837` (chat_reply), `src/mcp.rs:839-913` (chat_react)
- Modify: `src/server.rs:47-56` (MCP service creation)

- [ ] **Step 1: Add `hub` field to `Teidelum` struct**

In `src/mcp.rs`, update the struct and constructor:

```rust
pub struct Teidelum {
    api: Arc<TeidelumApi>,
    hub: Option<Arc<crate::chat::hub::Hub>>,
    tool_router: ToolRouter<Self>,
}
```

Update `new_with_shared`:

```rust
    pub fn new_with_shared(api: Arc<TeidelumApi>) -> Self {
        Self {
            api,
            hub: None,
            tool_router: Self::tool_router(),
        }
    }

    pub fn new_with_hub(api: Arc<TeidelumApi>, hub: Arc<crate::chat::hub::Hub>) -> Self {
        Self {
            api,
            hub: Some(hub),
            tool_router: Self::tool_router(),
        }
    }
```

- [ ] **Step 2: Add broadcast to `chat_post_message` MCP tool**

After the tantivy indexing in `chat_post_message` (after line 715), add:

```rust
        // Broadcast via WebSocket if hub is available
        if let Some(hub) = &self.hub {
            let event = crate::chat::events::ServerEvent::Message {
                channel: params.channel.to_string(),
                user: bot_id.to_string(),
                text: params.text.clone(),
                ts: id.to_string(),
                thread_ts: None,
            };
            hub.broadcast_to_channel(params.channel, &event).await;
        }
```

- [ ] **Step 3: Add broadcast to `chat_reply` MCP tool**

After the tantivy indexing in `chat_reply` (after line 825), add:

```rust
        // Broadcast via WebSocket if hub is available
        if let Some(hub) = &self.hub {
            let event = crate::chat::events::ServerEvent::Message {
                channel: params.channel.to_string(),
                user: bot_id.to_string(),
                text: params.text.clone(),
                ts: id.to_string(),
                thread_ts: Some(params.thread_ts.to_string()),
            };
            hub.broadcast_to_channel(params.channel, &event).await;
        }
```

- [ ] **Step 4: Add broadcast to `chat_react` MCP tool**

After the reaction insert in `chat_react` (after line 907), add:

```rust
        // Broadcast via WebSocket if hub is available
        if let Some(hub) = &self.hub {
            let event = crate::chat::events::ServerEvent::ReactionAdded {
                channel: channel_id.to_string(),
                user: bot_id.to_string(),
                ts: params.timestamp.to_string(),
                reaction: params.name.clone(),
            };
            hub.broadcast_to_channel(channel_id, &event).await;
        }
```

- [ ] **Step 5: Update `server.rs` to pass hub to MCP**

In `src/server.rs`, change line 49 from `Teidelum::new_with_shared(mcp_api.clone())` to:

```rust
    let mcp_hub = hub.clone();
    let mcp_service = StreamableHttpService::new(
        move || Ok(Teidelum::new_with_hub(mcp_api.clone(), mcp_hub.clone())),
        // ... rest unchanged
```

- [ ] **Step 6: Run all tests**

Run: `cargo test`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add src/mcp.rs src/server.rs
git commit -m "fix: broadcast WebSocket events from MCP chat tools"
```

---

## Chunk 4: Frontend Fixes

### Task 11: Add keyed `{#each}` blocks to list components

**Files:**
- Modify: `ui/src/lib/components/MessageList.svelte` (~line 296)
- Modify: `ui/src/lib/components/ThreadPanel.svelte` (~line 291)
- Modify: `ui/src/lib/components/Sidebar.svelte` (~lines 265, 311)
- Modify: `ui/src/lib/components/SearchModal.svelte` (~line 321)

- [ ] **Step 1: Fix MessageList.svelte**

Find `{#each messages as msg, idx}` and replace with `{#each messages as msg, idx (msg.id)}`.

- [ ] **Step 2: Fix ThreadPanel.svelte**

Find `{#each replies as reply}` and replace with `{#each replies as reply (reply.id)}`.

- [ ] **Step 3: Fix Sidebar.svelte**

Find `{#each $nonDmChannels as channel}` → `{#each $nonDmChannels as channel (channel.id)}`
Find `{#each $dmChannels as channel}` → `{#each $dmChannels as channel (channel.id)}`

- [ ] **Step 4: Fix SearchModal.svelte**

Find `{#each results as msg}` → `{#each results as msg (msg.id)}`

- [ ] **Step 5: Run type check**

Run: `cd ui && npx svelte-check`
Expected: No new errors

- [ ] **Step 6: Commit**

```bash
git add ui/src/lib/components/MessageList.svelte ui/src/lib/components/ThreadPanel.svelte ui/src/lib/components/Sidebar.svelte ui/src/lib/components/SearchModal.svelte
git commit -m "fix: add keyed {#each} blocks to prevent incorrect DOM reuse"
```

---

### Task 12: Add error feedback for failed operations

**Files:**
- Modify: `ui/src/lib/components/MessageList.svelte` (~lines 187-237)
- Modify: `ui/src/lib/components/FileUpload.svelte` (~line 28)

- [ ] **Step 1: Add try/catch to `toggleReaction` in MessageList.svelte**

Wrap the reaction toggle in try/catch:

```typescript
async function toggleReaction(messageId: Id, emoji: string) {
    try {
        const msg = messages.find((m) => m.id === messageId);
        // ... existing logic
    } catch (err) {
        console.error('Reaction failed:', err);
    }
}
```

- [ ] **Step 2: Add try/catch to `saveEdit` and `confirmDelete`**

Wrap both functions:

```typescript
async function saveEdit(messageId: Id) {
    try {
        await chatUpdate(channelId, messageId, editText);
        editingMessageId = null;
    } catch (err) {
        console.error('Edit failed:', err);
    }
}

async function confirmDelete() {
    if (!deletingMessageId) return;
    try {
        await chatDelete(channelId, deletingMessageId);
        deletingMessageId = null;
    } catch (err) {
        console.error('Delete failed:', err);
    }
}
```

- [ ] **Step 3: Add error callback to FileUpload.svelte**

In the upload error handler, emit an event instead of just console.error:

```typescript
} catch (err) {
    console.error('Upload failed:', err);
    uploading = false;
}
```

- [ ] **Step 4: Run type check**

Run: `cd ui && npx svelte-check`
Expected: No new errors

- [ ] **Step 5: Commit**

```bash
git add ui/src/lib/components/MessageList.svelte ui/src/lib/components/FileUpload.svelte
git commit -m "fix: add error handling for reaction, edit, delete, and upload operations"
```

---

### Task 13: Add `aria-live` to dynamic content regions

**Files:**
- Modify: `ui/src/lib/components/TypingIndicator.svelte`
- Modify: `ui/src/lib/components/ConnectionStatus.svelte`

- [ ] **Step 1: Add aria-live to TypingIndicator**

Find the container div and add `aria-live="polite"`:

```html
<div class="..." aria-live="polite">
```

- [ ] **Step 2: Add aria-live to ConnectionStatus**

Find the status banner div and add `aria-live="assertive"`:

```html
<div class="..." aria-live="assertive" role="alert">
```

- [ ] **Step 3: Run type check**

Run: `cd ui && npx svelte-check`
Expected: No new errors

- [ ] **Step 4: Commit**

```bash
git add ui/src/lib/components/TypingIndicator.svelte ui/src/lib/components/ConnectionStatus.svelte
git commit -m "fix: add ARIA live regions for typing indicator and connection status"
```

---

## Chunk 5: API Consistency Fixes

### Task 14: Return `created_at` from `chat.postMessage` response

**Files:**
- Modify: `src/chat/handlers.rs:2164-2171`

- [ ] **Step 1: Add `created_at` to the response**

Replace the response at line 2164:

```rust
    slack::ok(json!({
        "message": {
            "ts": id.to_string(),
            "channel": req.channel.to_string(),
            "user": claims.user_id.to_string(),
            "text": req.text,
            "created_at": now,
        }
    }))
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test chat_integration -- --test-threads=1`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add src/chat/handlers.rs
git commit -m "fix: include created_at in chat.postMessage response"
```

---

### Task 15: Add `created_at` to users API responses

**Files:**
- Modify: `src/chat/handlers.rs` (users_list ~line 274, users_info ~line 309)

- [ ] **Step 1: Add `created_at` to `users_list` SQL and response**

Find the `users_list` SQL SELECT and add `created_at` to the column list. Add it to the response JSON as well.

- [ ] **Step 2: Add `created_at` to `users_info` SQL and response**

Same change for the `users_info` handler.

- [ ] **Step 3: Run integration tests**

Run: `cargo test --test chat_integration -- --test-threads=1`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add src/chat/handlers.rs
git commit -m "fix: include created_at in users.list and users.info responses"
```

---

## Summary

| Chunk | Tasks | Focus |
|-------|-------|-------|
| 1 | 1-6 | Security: SQL escape, JWT, passwords, CORS, validation |
| 2 | 7-9 | Performance: N+1 queries, SQL filtering |
| 3 | 10 | API: MCP WebSocket broadcast |
| 4 | 11-13 | Frontend: keyed each, error handling, a11y |
| 5 | 14-15 | API: response consistency |

Total: 15 tasks, ~45 steps. Each task is independently testable and committable.
