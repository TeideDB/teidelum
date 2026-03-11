# Teidelum — Product Design Spec

**One server replaces Slack + Elasticsearch + Neo4j + Redis + analytics.**

Teidelum is a self-hosted team communication and knowledge platform. Real-time chat, full-text search, SQL analytics, graph traversal, and AI agents — all in a single binary. No external dependencies. Powered by TeideDB.

## Why Teidelum Exists

Modern teams run Slack ($8-12/user/mo) + Elasticsearch (search) + a graph DB (relationships) + Redis (real-time) + an analytics warehouse + AI integrations (extra cost per tool). That's 5-6 systems, 5-6 bills, 5-6 ops burdens.

Teidelum collapses all of this into one self-contained server:

| Capability | Traditional Stack | Teidelum |
|-----------|------------------|----------|
| Chat & messaging | Slack / Zulip | Built-in |
| Full-text search | Elasticsearch | Built-in (embedded) |
| SQL analytics | Data warehouse | TeideDB (embedded columnar) |
| Graph queries | Neo4j | Catalog FK traversal |
| Real-time pub/sub | Redis | In-process WebSocket hub |
| AI agent access | Custom integrations | Native MCP |
| Ops complexity | 5-6 services | 1 binary |

**Result:** 10-50x cheaper. Zero ops. Self-hosted. Your data stays yours.

## Target Users

- Small-to-medium teams (5-500 people) who want Slack-level chat without Slack-level cost
- Teams that want their chat data queryable (SQL, search, graph) — not locked in a vendor
- Organizations that need AI agents as first-class team members, not bolted-on integrations
- Privacy-conscious teams that require self-hosting

## Licensing & Business Model

**Apache 2.0 with Commons Clause** (or similar source-available license):
- Free to self-host, modify, and use for any internal purpose
- Cannot be sold as a hosted service by third parties without a commercial license

### Tiers

**Community (Free)**
- Unlimited users, channels, messages, history
- Full-text search, SQL analytics, graph queries
- WebSocket real-time, file sharing
- MCP for AI agents
- Community support (GitHub)

**Pro (Paid, per-server license)**
- Everything in Community
- SAML/OIDC SSO
- Audit logs and compliance exports
- Data retention policies
- Priority support
- Custom branding
- Advanced admin dashboard
- Backup/restore tooling

**Enterprise (Contact sales)**
- Everything in Pro
- Multi-server federation
- Dedicated support engineer
- Custom integrations
- SLA guarantees

---

## Architecture

Single Rust binary. Zero external dependencies. All state in a data directory.

```
teidelum
├── TeideDB engine         — Columnar SQL (microsecond queries)
├── Search index            — Full-text search (BM25, fuzzy)
├── Catalog                — Schema registry, FK relationships
├── Graph engine           — BFS/DFS over FK relationships
├── Chat                   — Channels, messages, threads, reactions
├── WebSocket hub          — Real-time broadcast, presence, typing
├── Auth                   — JWT sessions, argon2 passwords
├── File storage           — Local disk, MIME-hardened
├── MCP server             — AI agent protocol (stdio + HTTP)
├── Axum HTTP server       — REST API + WebSocket + file serving
└── SvelteKit frontend     — Static SPA served by Axum
```

### Data Directory Layout

```
data/
├── tables/          — TeideDB columnar storage
├── docs/            — Full-text search index
├── files/           — Uploaded files (uuid/filename)
└── config.toml      — Server configuration (Pro)
```

### Why This Works

TeideDB executes analytical queries in microseconds-to-milliseconds on columnar storage. The embedded search engine provides full-text search with BM25 ranking and fuzzy matching. The WebSocket hub is an in-process `HashMap<user_id, broadcast::Sender>` — no Redis needed. Everything shares one process, one memory space, zero serialization overhead between components.

---

## Data Model

All data stored as TeideDB tables. FK relationships registered in the Catalog enable graph traversal.

### Core Tables

**users** — Human and bot accounts

| Column | Type | Notes |
|--------|------|-------|
| id | i64 | PK, monotonic |
| username | string | unique |
| display_name | string | |
| email | string | unique |
| password_hash | string | argon2, never exposed via API |
| avatar_url | string | |
| status | string | online/away/dnd/offline |
| is_bot | bool | true for MCP agents |
| created_at | string | unix timestamp |

**channels** — Public channels, private channels, DMs

| Column | Type | Notes |
|--------|------|-------|
| id | i64 | PK |
| name | string | unique |
| kind | string | public/private/dm |
| topic | string | |
| created_by | i64 | FK -> users.id |
| created_at | string | |

**channel_members** — Who belongs to which channel

| Column | Type | Notes |
|--------|------|-------|
| channel_id | i64 | FK -> channels.id |
| user_id | i64 | FK -> users.id |
| role | string | owner/admin/member |
| joined_at | string | |

**messages** — All chat messages

| Column | Type | Notes |
|--------|------|-------|
| id | i64 | PK, time-ordered |
| channel_id | i64 | FK -> channels.id |
| user_id | i64 | FK -> users.id |
| thread_id | i64 | 0 = top-level, else FK -> messages.id |
| content | string | |
| deleted_at | string | nullable, soft delete |
| edited_at | string | nullable |
| created_at | string | |

**reactions**, **mentions**, **channel_reads**, **files** — Supporting tables for reactions, @mentions, read tracking, and file metadata.

### ID Generation

`(unix_millis << 16) | atomic_counter` — Monotonic, time-ordered, supports ~65k IDs/ms, no external coordination.

### Uniqueness

TeideDB has no UNIQUE constraints. Enforced at application layer with SELECT-before-INSERT.

---

## API

Slack-compatible method-based endpoints. All POST with JSON body. Response: `{"ok": true, ...}` or `{"ok": false, "error": "..."}`.

### Auth (public)
- `auth.register` — Create account
- `auth.login` — Get JWT token

### Chat (authenticated)
- `chat.postMessage` — Send message (auto-indexes in search)
- `chat.update` — Edit message (author only)
- `chat.delete` — Soft-delete message (author only)

### Channels (authenticated)
- `conversations.create` — Create channel
- `conversations.list` — List user's channels
- `conversations.info` — Channel details
- `conversations.history` — Paginated message history
- `conversations.replies` — Thread replies
- `conversations.join` — Join public channel
- `conversations.leave` — Leave channel
- `conversations.invite` — Invite user
- `conversations.members` — List members
- `conversations.open` — Open/create DM

### Users (authenticated)
- `users.list` — All users
- `users.info` — User profile
- `users.setPresence` — Set status

### Reactions (authenticated)
- `reactions.add` / `reactions.remove`

### Search (authenticated)
- `search.messages` — Full-text search (filtered by membership)

### Files (authenticated)
- `files.upload` — Multipart upload
- `GET /files/:id/:filename` — Download (auth + membership check)

---

## Security

### Authentication
- **Passwords:** argon2 hashing
- **Sessions:** JWT signed with `TEIDE_CHAT_SECRET`, 24h expiry
- **JWT payload:** `{user_id, username, is_bot, exp}`
- **WebSocket:** JWT in `?token=` query param on upgrade

### Authorization
- Every operation checks `channel_members` before read/write
- Private channels/DMs only accessible to members
- Message edit/delete restricted to author
- Search results filtered by caller's channel membership
- File download requires membership in the file's channel

### File Security
- MIME type derived from file extension only (never client headers)
- `X-Content-Type-Options: nosniff` on all downloads
- `Content-Disposition: attachment` to prevent inline execution
- 10MB limit, allowlisted extensions
- UUID-based storage paths prevent traversal

### Deployment
- Run behind reverse proxy (nginx/caddy) with TLS
- `TEIDE_CHAT_SECRET` — Required, minimum 32 bytes, cryptographically random
- `TEIDELUM_API_KEY` — Optional, protects `/api/v1/*` and `/mcp`

---

## Real-Time

### WebSocket Protocol
- Connect: `GET /ws?token=<jwt>`
- Server sends `{"type": "hello"}` on connect
- Events: `message`, `message_changed`, `message_deleted`, `reaction_added`, `reaction_removed`, `typing`, `presence_change`, `member_joined_channel`, `member_left_channel`
- Client sends: `typing {channel}`, `ping`

### Hub Design
- `HashMap<user_id, broadcast::Sender>` — supports multiple tabs
- Channel membership cached in-memory, invalidated on join/leave
- Presence = active connection count
- Typing throttled: 1 event per user per channel per 3 seconds

---

## Search & Analytics

### Full-Text Search
- Every message indexed on post
- Source: "chat", title: "#channel-name", body: message content
- `search.messages` API filters by channel membership
- Same index used by MCP `search` tool

### SQL Analytics
- All chat data is in TeideDB tables — directly queryable
- Examples:
  - `SELECT channel_id, COUNT(*) FROM messages GROUP BY channel_id ORDER BY COUNT(*) DESC` — most active channels
  - `SELECT user_id, COUNT(*) FROM messages WHERE created_at > '...' GROUP BY user_id` — user activity
  - `SELECT * FROM messages WHERE content LIKE '%deploy%'` — keyword search via SQL
- AI agents can run arbitrary analytics via MCP `sql` tool

### Graph Queries
- FK relationships enable traversal: "who mentioned whom", "thread trees", "user's channels"
- MCP `graph` tool for AI agents to explore relationships
- Example: find all users who reacted to messages in a specific thread

---

## MCP Integration

AI agents are first-class team members, not add-ons.

### Chat Tools
- `chat_post_message` — Send message as bot
- `chat_history` — Read recent messages
- `chat_reply` — Reply to thread
- `chat_react` — Add reaction
- `chat_list_channels` — List bot's channels
- `chat_search` — Search messages (filtered by bot's membership)

### Knowledge Tools
- `sql` — Run SQL queries on any table
- `search` — Full-text search across all indexed content
- `graph` — Traverse FK relationships
- `describe` — Inspect table schemas
- `create_table`, `insert_rows`, `delete_table` — Data management
- `add_documents`, `delete_documents` — Search index management
- `sync` — Pull data from external sources (Notion, Zulip)

### Agent Flow
1. Create bot user (`is_bot: true`)
2. Invite bot to channels
3. Connect via MCP (stdio or `/mcp` HTTP)
4. Agent reads context with `chat_history`, responds with `chat_post_message`
5. Agent queries knowledge base with `sql`, `search`, `graph`
6. Bot messages appear in UI with bot badge

### Why This Matters
No other chat platform gives AI agents direct SQL access to all data, full-text search, and graph traversal — alongside real-time chat participation. Teidelum agents can answer "what did the team discuss about X last week?" by searching messages, correlate with structured data via SQL, and trace relationships via graph — all through one protocol.

---

## Frontend

SvelteKit SPA in `ui/`, served statically by Axum. Dark theme.

### Layout
Three-column Slack-style: Sidebar | Messages | Thread Panel

### Features
- Login / Register
- Channel list with unread badges
- Message list with infinite scroll
- Threaded replies
- Reactions (emoji picker)
- @mention highlighting
- Markdown rendering (sanitized)
- Typing indicators
- Presence indicators (online/away/offline)
- File upload and preview
- Search modal (Cmd+K)
- WebSocket auto-reconnect with exponential backoff
- Optimistic UI on send

### Tech
- SvelteKit (SSR disabled, SPA mode)
- TypeScript
- Tailwind CSS
- Svelte stores for state management
- Vite dev server proxies to backend

---

## Slack-Compatible API

Teidelum implements a Slack-compatible API surface. Same method names, same JSON format, same `{"ok": true/false}` pattern. This is a strategic choice:

- **Zero learning curve** for developers who know Slack's API
- **Existing Slack bots and integrations** can point at Teidelum with minimal code changes
- **Migration path** from Slack: export data, import into TeideDB tables, existing tooling works
- **Familiar client libraries** in every language already exist for this API pattern

This is not a clone — it's API compatibility where it makes sense, extended with capabilities Slack can't offer (SQL queries, graph traversal, MCP).

---

## Competitive Positioning

| | Slack | Zulip | Teidelum |
|---|---|---|---|
| Price | $8-12/user/mo | Free (self-host) | Free (self-host) |
| Dependencies | SaaS only | PostgreSQL, Redis, RabbitMQ, memcached | None |
| Search | Limited, paid tier | Requires Elasticsearch | Built-in |
| SQL on chat data | No | No | Yes (TeideDB) |
| Graph queries | No | No | Yes (Catalog FK) |
| AI agents | Third-party, extra cost | Limited | Native MCP, first-class |
| Self-hosted | No | Yes (complex) | Yes (single binary) |
| Setup time | N/A | Hours | Minutes |

---

## Implementation Status

### Completed (Plan 1)
- Chat backend core: auth, channels, messages, reactions, WebSocket hub
- Slack-compatible API (18 endpoints)
- JWT + argon2 auth
- Integration tests

### In Progress (Plan 2)
- Message search indexing
- File upload/download
- MCP chat tools (6 new tools)

### Planned (Plan 3)
- SvelteKit frontend (in `ui/`)

### Future
- Pro tier features (SSO, audit logs, retention policies)
- Admin dashboard
- Backup/restore
- Multi-server federation (Enterprise)
- Tauri desktop app (wrap SvelteKit build)
- Mobile apps
