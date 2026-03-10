# Teide Chat — Design Spec

A Slack competitor for small teams and startups, powered by TeideDB, with native MCP integration for AI agents. 10x cheaper than Slack through single-engine architecture and lean solo ownership.

## Stack

- **Backend:** Rust, extending teidelum (Axum + TeideDB + tantivy + MCP)
- **Frontend:** SvelteKit (web app first, Tauri desktop later)
- **Real-time:** WebSockets
- **API:** Slack-compatible method-based endpoints

## MVP Feature Set

Channels, DMs, threads, reactions, mentions, notifications, search, file sharing. Web app only.

---

## 1. Data Model

All data stored as TeideDB tables with FK relationships registered in the Catalog.

### Tables

**users**

| Column | Type | Notes |
|--------|------|-------|
| id | i64 | PK |
| username | string | unique |
| display_name | string | |
| email | string | unique |
| password_hash | string | |
| avatar_url | string | |
| status | string | online/away/dnd/offline |
| is_bot | bool | |
| created_at | timestamp | |

**channels**

| Column | Type | Notes |
|--------|------|-------|
| id | i64 | PK |
| name | string | unique |
| kind | string | public/private/dm |
| topic | string | |
| created_by | i64 | FK → users.id |
| created_at | timestamp | |

**channel_members** (unique by channel_id + user_id, enforced at application layer)

| Column | Type | Notes |
|--------|------|-------|
| channel_id | i64 | FK → channels.id |
| user_id | i64 | FK → users.id |
| role | string | owner/admin/member |
| joined_at | timestamp | |

**messages**

| Column | Type | Notes |
|--------|------|-------|
| id | i64 | PK |
| channel_id | i64 | FK → channels.id |
| user_id | i64 | FK → users.id |
| thread_id | i64 | nullable, FK → messages.id |
| content | string | |
| deleted_at | timestamp | nullable, soft delete |
| edited_at | timestamp | nullable |
| created_at | timestamp | |

**reactions** (unique by message_id + user_id + emoji, enforced at application layer)

| Column | Type | Notes |
|--------|------|-------|
| message_id | i64 | FK → messages.id |
| user_id | i64 | FK → users.id |
| emoji | string | |
| created_at | timestamp | |

**mentions** (unique by message_id + user_id, enforced at application layer)

| Column | Type | Notes |
|--------|------|-------|
| message_id | i64 | FK → messages.id |
| user_id | i64 | FK → users.id |

**channel_reads** (unique by channel_id + user_id, enforced at application layer via upsert)

| Column | Type | Notes |
|--------|------|-------|
| channel_id | i64 | FK → channels.id |
| user_id | i64 | FK → users.id |
| last_read_ts | timestamp | |

**files**

| Column | Type | Notes |
|--------|------|-------|
| id | i64 | PK |
| message_id | i64 | FK → messages.id |
| user_id | i64 | FK → users.id |
| channel_id | i64 | FK → channels.id |
| filename | string | |
| mime_type | string | |
| size_bytes | i64 | |
| storage_path | string | |
| created_at | timestamp | |

### Relationships (Catalog)

- `messages.user_id → users.id` ("sent_by")
- `messages.channel_id → channels.id` ("posted_in")
- `messages.thread_id → messages.id` ("reply_to")
- `channel_members.user_id → users.id` ("member")
- `channel_members.channel_id → channels.id` ("belongs_to")
- `reactions.message_id → messages.id` ("reacted_to")
- `mentions.message_id → messages.id` ("mentioned_in")
- `mentions.user_id → users.id` ("mentions")
- `channel_reads.channel_id → channels.id` ("read_status_for")
- `channel_reads.user_id → users.id` ("read_by")
- `files.message_id → messages.id` ("attached_to")
- `files.user_id → users.id` ("uploaded_by")
- `files.channel_id → channels.id` ("uploaded_in")

### Uniqueness

TeideDB does not support UNIQUE constraints. All junction table uniqueness (channel_members, reactions, mentions, channel_reads) is enforced at the application layer with SELECT-before-INSERT checks in `chat/models.rs`.

### ID Generation

All tables use i64 PKs generated via timestamp-based monotonic IDs: `(unix_millis << 16) | atomic_counter`. This gives natural time-ordering (newest messages have highest IDs), supports ~65k inserts per millisecond, and requires no external coordination.

### TeideDB Advantages

- `sql` tool queries messages directly
- `search` tool finds messages via tantivy full-text index
- `graph` tool available for FK traversal (threads, mentions, membership) — useful for agents exploring data, not primary chat navigation

---

## 2. Architecture

Extend teidelum with chat modules. Single binary, single process.

### Concurrency Note

The existing QueryRouter wraps `teide::Session` in a `Mutex`, serializing all SQL operations. For the MVP targeting small teams (<50 concurrent users), this is acceptable — TeideDB executes queries in microseconds to low milliseconds, so lock contention is negligible at this scale. If scaling beyond this, the Mutex can be replaced with a pool of Sessions or a dedicated write thread with an async channel.

```
teidelum binary
├── Existing modules (unchanged)
│   ├── MCP server (stdio + streamable HTTP at /mcp)
│   ├── QueryRouter → TeideDB
│   ├── SearchEngine → tantivy
│   ├── GraphEngine → FK traversal
│   └── Catalog → schema registry
│
├── New: Chat modules
│   ├── chat/auth.rs        — JWT sessions, registration, login
│   ├── chat/ws.rs          — WebSocket hub (connections, broadcasts)
│   ├── chat/models.rs      — Table creation, ID generation, data access
│   ├── chat/handlers.rs    — Slack-compatible API handlers
│   ├── chat/events.rs      — Real-time event types
│   └── chat/files.rs       — File upload/storage (local disk)
│
└── Axum router
    ├── /api/v1/*            — Existing teidelum REST API
    ├── /mcp                 — Existing MCP endpoint
    ├── /api/slack/*         — Slack-compatible API (new)
    └── /ws                  — WebSocket endpoint (new)
```

### Slack-Compatible API

All endpoints accept POST with JSON body, return `{"ok": true, ...}` or `{"ok": false, "error": "..."}`.

**Auth (non-Slack, required):**
- `auth.register` — Create account
- `auth.login` — Get JWT token

**Messaging:**
- `chat.postMessage` — Send message
- `chat.update` — Edit message
- `chat.delete` — Soft-delete message (sets `deleted_at`, shows as "[deleted]" in UI)

**Channels:**
- `conversations.create` — Create channel
- `conversations.list` — List channels
- `conversations.info` — Channel details
- `conversations.history` — Message history (paginated)
- `conversations.replies` — Thread replies
- `conversations.join` — Join channel
- `conversations.leave` — Leave channel
- `conversations.invite` — Invite user
- `conversations.members` — List members
- `conversations.open` — Open/resume a DM (given user IDs, return existing or create new)

**Users:**
- `users.list` — List users
- `users.info` — User profile
- `users.setPresence` — Set status

**Reactions:**
- `reactions.add` — Add reaction
- `reactions.remove` — Remove reaction

**Search:**
- `search.messages` — Full-text search (routes to tantivy)

**Files:**
- `files.upload` — Upload file (multipart)

---

## 3. Auth & Security

- **Passwords:** argon2 hashing
- **Sessions:** JWT signed with `TEIDE_CHAT_SECRET` env var, 24h expiry
- **JWT payload:** `{user_id, username, is_bot, exp}`
- **Request auth:** `Authorization: Bearer <jwt>` on all `/api/slack/*` and WebSocket upgrade
- **Bot auth:** API key via existing `TEIDELUM_API_KEY` mechanism
- **Auth middleware layering:**
  - `/api/v1/*` — API key auth (existing, unchanged)
  - `/api/slack/*` and `/ws` — JWT auth
  - `/mcp` — API key auth (bots connect here; same key as `/api/v1/*`)
- **Access control:** Every API call checks `channel_members` before read/write
- **Private channels/DMs:** Only visible to members
- **Rate limiting:** In-memory counter on auth endpoints
- **File uploads:** 10MB limit, allowed MIME types, random UUID storage paths

**Skipped for MVP:** OAuth/SSO, refresh tokens, 2FA, multi-workspace.

---

## 4. Real-Time & Message Flow

### Send Message Flow

1. Client POST `chat.postMessage` with `{channel, text}`
2. Validate JWT, check channel membership
3. Parse `@mentions` from text → insert into mentions table
4. Insert message row via QueryRouter
5. Index message in tantivy
6. Register relationships in Catalog (thread_id → parent)
7. Build WebSocket event
8. Hub broadcasts to all connected channel members
9. Return `{"ok": true, "message": {...}}`

### WebSocket

**Connection:**
1. Client connects to `/ws` with JWT in query param
2. Server validates, registers sender in Hub under user_id
3. Server sends `{"type": "hello"}`
4. Ping/pong keepalive

**Events pushed to clients:**
- `message` — new message in a channel the user belongs to
- `reaction_added` / `reaction_removed`
- `typing` — ephemeral, not stored
- `presence_change` — online/away/offline
- `member_joined_channel` / `member_left_channel`

**Hub design:**
- `HashMap<user_id, Vec<Sender>>` — supports multiple tabs/devices
- Channel membership cached in-memory (`HashMap<channel_id, HashSet<user_id>>`), invalidated on join/leave events — avoids SQL roundtrip on every broadcast
- Presence derived from active socket count
- Typing events throttled: max 1 per user per channel per 3 seconds

### Unread Tracking

- `channel_reads` table stores `last_read_ts` per user per channel
- Client computes unread state by comparing `last_read_ts` with latest message timestamp

**Skipped for MVP:** Push/email notifications, delivery receipts.

---

## 5. File Handling & Search

### Files

- Stored on disk: `data/files/<uuid>/<original_filename>`
- Metadata in `files` table
- Upload via `files.upload` (multipart), posts a message with file reference
- Download via `GET /files/<id>/<filename>` with auth check
- Limits: 10MB per file, common MIME types

### Search

- Every message indexed in tantivy as a SearchDocument (source: "chat", title: "#channel-name", body: message content)
- Existing `search` MCP tool works on messages automatically
- `search.messages` Slack API wraps SearchEngine with Slack-compatible response
- File content not indexed for MVP

---

## 6. Frontend

SvelteKit web app with Slack-familiar layout.

```
teide-chat-ui/
├── src/
│   ├── lib/
│   │   ├── api.ts              — Slack API client
│   │   ├── ws.ts               — WebSocket client (reconnect, event dispatch)
│   │   ├── stores/
│   │   │   ├── auth.ts         — JWT, current user
│   │   │   ├── channels.ts     — Channel list, active channel
│   │   │   ├── messages.ts     — Message cache per channel
│   │   │   ├── users.ts        — User list, presence
│   │   │   └── unreads.ts      — Unread state
│   │   └── components/
│   │       ├── Sidebar.svelte
│   │       ├── MessageList.svelte
│   │       ├── MessageInput.svelte
│   │       ├── ThreadPanel.svelte
│   │       ├── ReactionPicker.svelte
│   │       ├── FileUpload.svelte
│   │       ├── SearchModal.svelte
│   │       └── UserPresence.svelte
│   ├── routes/
│   │   ├── login/+page.svelte
│   │   ├── register/+page.svelte
│   │   └── (app)/[channel]/+page.svelte
│   └── app.html
```

**Key decisions:**
- Sidebar | message area | thread panel layout
- Messages loaded via `conversations.history`, then real-time via WebSocket
- Infinite scroll up for history
- Optimistic UI on send
- WebSocket auto-reconnect with exponential backoff
- Markdown rendering for message content

**Skipped for MVP:** Themes, keyboard shortcuts, drag-and-drop upload, emoji picker, formatting toolbar.

**Tauri (post-MVP):** Wrap SvelteKit static build with `cargo tauri build`. No code changes needed.

---

## 7. MCP Integration for AI Agents

AI agents are first-class chat participants via the existing MCP server.

### New MCP Tools

- `chat.postMessage` — Send message to channel
- `chat.history` — Read recent messages
- `chat.reply` — Reply to thread
- `chat.react` — Add reaction
- `chat.listChannels` — List accessible channels
- `chat.search` — Search messages (wraps existing search)

### Agent Flow

1. Create bot user (`is_bot: true`)
2. Add bot to channels via `conversations.invite`
3. MCP client connects via stdio or `/mcp`
4. Agent reads context with `chat.history`, responds with `chat.postMessage`
5. Bot messages appear in UI with bot badge
6. Agent can also use `sql`, `search`, `graph` tools for deeper data access

### Differentiator vs Slack

| | Slack | Teide Chat |
|---|---|---|
| Protocol | Custom Events API + webhooks | MCP (open standard) |
| Data access | Limited API, rate-limited | Full SQL + search + graph |
| Setup | App manifest, OAuth, events | Add bot user, connect MCP |
| AI integration | Third-party, extra cost | Native, same binary |

**Skipped for MVP:** Agent-to-agent protocols, tool approval flows, streaming responses.
