# Teidelum — Design Spec

A knowledge base server with built-in chat and native MCP integration for AI agents. Combines structured data (TeideDB), full-text search (tantivy), real-time messaging, and AI tool access in a single binary.

## Vision

Teidelum is a **team knowledge hub**: a server where humans chat, share files, and organize information — while AI agents participate as first-class members via MCP. All data lives in one engine (TeideDB + tantivy), queryable by both humans and machines.

## Stack

- **Backend:** Rust, Axum, TeideDB, tantivy, rmcp
- **Frontend:** SvelteKit SPA (in `ui/`), TypeScript, Tailwind CSS
- **Real-time:** WebSockets
- **API:** Slack-compatible method-based endpoints
- **AI Integration:** MCP (Model Context Protocol)

## Feature Set

Channels, DMs, threads, reactions, mentions, search, file sharing. Web app with WebSocket real-time updates. MCP tools for AI agent participation.

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
| password_hash | string | argon2 hash, never exposed via API |
| avatar_url | string | |
| status | string | online/away/dnd/offline |
| is_bot | bool | true for MCP agent accounts |
| created_at | timestamp | |

**channels**

| Column | Type | Notes |
|--------|------|-------|
| id | i64 | PK |
| name | string | unique |
| kind | string | public/private/dm |
| topic | string | |
| created_by | i64 | FK -> users.id |
| created_at | timestamp | |

**channel_members** (unique by channel_id + user_id, enforced at application layer)

| Column | Type | Notes |
|--------|------|-------|
| channel_id | i64 | FK -> channels.id |
| user_id | i64 | FK -> users.id |
| role | string | owner/admin/member |
| joined_at | timestamp | |

**messages**

| Column | Type | Notes |
|--------|------|-------|
| id | i64 | PK |
| channel_id | i64 | FK -> channels.id |
| user_id | i64 | FK -> users.id |
| thread_id | i64 | nullable, FK -> messages.id |
| content | string | |
| deleted_at | timestamp | nullable, soft delete |
| edited_at | timestamp | nullable |
| created_at | timestamp | |

**reactions** (unique by message_id + user_id + emoji, enforced at application layer)

| Column | Type | Notes |
|--------|------|-------|
| message_id | i64 | FK -> messages.id |
| user_id | i64 | FK -> users.id |
| emoji | string | |
| created_at | timestamp | |

**mentions** (unique by message_id + user_id, enforced at application layer)

| Column | Type | Notes |
|--------|------|-------|
| message_id | i64 | FK -> messages.id |
| user_id | i64 | FK -> users.id |

**channel_reads** (unique by channel_id + user_id, enforced at application layer)

| Column | Type | Notes |
|--------|------|-------|
| channel_id | i64 | FK -> channels.id |
| user_id | i64 | FK -> users.id |
| last_read_ts | timestamp | |

**files**

| Column | Type | Notes |
|--------|------|-------|
| id | i64 | PK |
| message_id | i64 | FK -> messages.id |
| user_id | i64 | FK -> users.id |
| channel_id | i64 | FK -> channels.id |
| filename | string | |
| mime_type | string | derived from extension, never client-supplied |
| size_bytes | i64 | |
| storage_path | string | |
| created_at | timestamp | |

### Relationships (Catalog)

- `messages.user_id -> users.id` ("sent_by")
- `messages.channel_id -> channels.id` ("posted_in")
- `messages.thread_id -> messages.id` ("reply_to")
- `channel_members.user_id -> users.id` ("member")
- `channel_members.channel_id -> channels.id` ("belongs_to")
- `reactions.message_id -> messages.id` ("reacted_to")
- `mentions.message_id -> messages.id` ("mentioned_in")
- `mentions.user_id -> users.id` ("mentions")
- `channel_reads.channel_id -> channels.id` ("read_status_for")
- `channel_reads.user_id -> users.id` ("read_by")
- `files.message_id -> messages.id` ("attached_to")
- `files.user_id -> users.id` ("uploaded_by")
- `files.channel_id -> channels.id` ("uploaded_in")

### Uniqueness

TeideDB does not support UNIQUE constraints. All junction table uniqueness (channel_members, reactions, mentions, channel_reads) is enforced at the application layer with SELECT-before-INSERT checks.

### ID Generation

Timestamp-based monotonic IDs: `(unix_millis << 16) | atomic_counter`. Natural time-ordering, ~65k IDs per millisecond, no external coordination.

---

## 2. Architecture

Single Rust binary. All modules in teidelum crate.

```
teidelum binary
├── Core modules
│   ├── QueryRouter -> TeideDB (SQL engine)
│   ├── SearchEngine -> tantivy (full-text search)
│   ├── GraphEngine -> FK traversal
│   ├── Catalog -> schema registry
│   └── MCP server (stdio + streamable HTTP at /mcp)
│
├── Chat modules
│   ├── chat/auth.rs        — JWT, argon2, auth middleware
│   ├── chat/hub.rs         — WebSocket connection hub
│   ├── chat/ws.rs          — WebSocket upgrade handler
│   ├── chat/models.rs      — Table schemas, SQL helpers
│   ├── chat/handlers.rs    — Slack-compatible API handlers
│   ├── chat/events.rs      — Real-time event types
│   ├── chat/files.rs       — File upload/download
│   └── chat/slack.rs       — Response formatting
│
├── Axum router
│   ├── /api/v1/*            — Data management REST API
│   ├── /api/slack/*         — Slack-compatible chat API
│   ├── /ws                  — WebSocket endpoint
│   ├── /files/:id/:name     — File download
│   └── /mcp                 — MCP endpoint
│
└── Frontend (ui/)
    └── SvelteKit SPA served statically
```

### Slack-Compatible API

All endpoints accept POST with JSON body, return `{"ok": true, ...}` or `{"ok": false, "error": "..."}`.

**Auth (public):**
- `auth.register` — Create account
- `auth.login` — Get JWT token

**Messaging (authenticated):**
- `chat.postMessage` — Send message (indexes in tantivy)
- `chat.update` — Edit message
- `chat.delete` — Soft-delete message

**Channels (authenticated):**
- `conversations.create` — Create channel
- `conversations.list` — List user's channels
- `conversations.info` — Channel details
- `conversations.history` — Message history (paginated)
- `conversations.replies` — Thread replies
- `conversations.join` — Join public channel
- `conversations.leave` — Leave channel
- `conversations.invite` — Invite user to channel
- `conversations.members` — List channel members
- `conversations.open` — Open/create DM

**Users (authenticated):**
- `users.list` — List all users
- `users.info` — User profile
- `users.setPresence` — Set online status

**Reactions (authenticated):**
- `reactions.add` — Add reaction
- `reactions.remove` — Remove reaction

**Search (authenticated):**
- `search.messages` — Full-text search (filtered by membership)

**Files (authenticated):**
- `files.upload` — Upload file (multipart)

---

## 3. Security

### Authentication

- **Passwords:** argon2 hashing (never stored or transmitted in plaintext)
- **Sessions:** JWT signed with `TEIDE_CHAT_SECRET` env var
- **JWT payload:** `{user_id, username, is_bot, exp}`
- **Token expiry:** 24 hours
- **Request auth:** `Authorization: Bearer <jwt>` header on all authenticated endpoints
- **WebSocket auth:** JWT in `?token=` query parameter on upgrade

### Authorization

- **Channel access control:** Every read/write operation checks `channel_members` table
- **Private channels/DMs:** Only visible and accessible to members
- **Message ownership:** Only the author can edit/delete their own messages
- **Search filtering:** Results filtered post-query to only channels the caller belongs to (over-fetch 3x from tantivy, then filter)
- **File access:** Download requires valid JWT and channel membership check

### File Security

- **MIME hardening:** MIME type derived from file extension only, never from client headers or DB values
- **Download headers:** `X-Content-Type-Options: nosniff`, `Content-Disposition: attachment`
- **Upload limits:** 10MB per file, allowlisted MIME types only
- **Storage:** `data/files/<uuid>/<original_filename>` — UUID prevents path traversal

### API Security

- **Input sanitization:** SQL values escaped via `escape_sql()` (single-quote doubling)
- **Auth middleware layering:**
  - `/api/slack/auth.*` — public (register, login)
  - `/api/slack/*` — JWT auth middleware
  - `/ws` — JWT validation on upgrade
  - `/files/*` — JWT validation via query param
  - `/api/v1/*` — API key auth (existing)
  - `/mcp` — API key auth (for bot/agent access)

### Deployment

- `TEIDE_CHAT_SECRET` — Required. JWT signing key. Must be cryptographically random, minimum 32 bytes.
- `TEIDELUM_API_KEY` — Optional. Protects `/api/v1/*` and `/mcp` endpoints.
- Server should run behind a reverse proxy (nginx/caddy) with TLS termination.

---

## 4. Real-Time & Message Flow

### Send Message Flow

1. Client POST `chat.postMessage` with `{channel, text}`
2. Validate JWT, check channel membership
3. Parse `@mentions` from text -> insert into mentions table
4. Insert message row via QueryRouter
5. Index message in tantivy (source: "chat", title: "#channel-name")
6. Build WebSocket event
7. Hub broadcasts to all connected channel members
8. Return `{"ok": true, "message": {...}}`

### WebSocket

**Connection:**
1. Client connects to `/ws?token=<jwt>`
2. Server validates JWT, registers sender in Hub under user_id
3. Server sends `{"type": "hello"}`
4. Ping/pong keepalive via axum

**Events pushed to clients:**
- `message` — new message in a channel the user belongs to
- `message_changed` — message edited
- `message_deleted` — message soft-deleted
- `reaction_added` / `reaction_removed`
- `typing` — ephemeral, not stored
- `presence_change` — online/away/offline
- `member_joined_channel` / `member_left_channel`

**Hub design:**
- `HashMap<user_id, broadcast::Sender>` — tokio broadcast for multiple tabs/devices
- Channel membership cached in-memory (`HashMap<channel_id, HashSet<user_id>>`), invalidated on join/leave
- Presence derived from active connections
- Typing events throttled: max 1 per user per channel per 3 seconds

### Unread Tracking

- `channel_reads` table stores `last_read_ts` per user per channel
- Client computes unread by comparing `last_read_ts` with latest message timestamp

---

## 5. Search

- Every message indexed in tantivy on post (source: "chat", title: "#channel-name", body: message content)
- `search.messages` Slack API endpoint wraps SearchEngine, filtered by channel membership
- MCP `chat_search` tool wraps the same logic for AI agents
- Existing `search` MCP tool also finds chat messages (same tantivy index)

---

## 6. Frontend

SvelteKit SPA in `teidelum/ui/`, dark theme, Slack-familiar layout.

```
ui/
├── src/
│   ├── lib/
│   │   ├── api.ts              — Typed Slack API client
│   │   ├── ws.ts               — WebSocket client (auto-reconnect, event dispatch)
│   │   ├── types.ts            — Shared TypeScript types
│   │   ├── markdown.ts         — Markdown rendering + @mention highlighting
│   │   ├── stores/
│   │   │   ├── auth.ts         — JWT persistence, current user
│   │   │   ├── channels.ts     — Channel list, active channel
│   │   │   ├── messages.ts     — Per-channel message cache
│   │   │   ├── users.ts        — User list, presence
│   │   │   └── unreads.ts      — Unread counts
│   │   └── components/
│   │       ├── Sidebar.svelte
│   │       ├── MessageList.svelte
│   │       ├── MessageInput.svelte
│   │       ├── ThreadPanel.svelte
│   │       ├── ReactionPicker.svelte
│   │       ├── FileUpload.svelte
│   │       ├── SearchModal.svelte
│   │       └── UserPresence.svelte
│   └── routes/
│       ├── +layout.svelte      — Auth guard, WS init
│       ├── login/+page.svelte
│       ├── register/+page.svelte
│       └── (app)/[channel]/+page.svelte
```

**Key decisions:**
- Sidebar | message area | thread panel (3-column layout)
- Messages loaded via `conversations.history`, then real-time via WebSocket
- Infinite scroll up for older messages
- Optimistic UI on send
- WebSocket auto-reconnect with exponential backoff
- Markdown rendering with DOMPurify sanitization
- SPA mode (SSR disabled) — Vite dev server proxies API/WS/files to backend

---

## 7. MCP Integration

AI agents are first-class participants via MCP.

### MCP Tools

- `chat_post_message` — Send message to channel
- `chat_history` — Read recent messages
- `chat_reply` — Reply to thread
- `chat_react` — Add reaction
- `chat_list_channels` — List accessible channels
- `chat_search` — Search messages (filtered by bot's channel membership)

Plus existing tools: `sql`, `search`, `describe`, `graph`, `sync`, `create_table`, `insert_rows`, etc.

### Agent Flow

1. Create bot user (`is_bot: true`) via `auth.register` or admin
2. Add bot to channels via `conversations.invite`
3. MCP client connects via stdio or `/mcp` (authenticated with `TEIDELUM_API_KEY`)
4. Agent uses `chat_history` to read context, `chat_post_message` to respond
5. Bot messages appear in UI with bot badge
6. Agent also has full access to `sql`, `search`, `graph` tools for knowledge base queries
