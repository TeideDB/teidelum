# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Teidelum is a compact, local-first MCP server that syncs work tools (Notion, Zulip) and connects live data sources (kdb+) into a single searchable and queryable index. Single binary, zero config, data never leaves the machine. See SPEC.md for the full specification.

## Build & Development Commands

### Backend (Rust)

```bash
cargo build                 # build
cargo run                   # run (serves MCP over stdio)
cargo test                  # all tests
cargo test test_name        # single test
cargo check                 # type-check only
cargo clippy -- -D warnings # lint
cargo fmt --check           # format check
cargo fmt                   # format apply
```

### Frontend (ui/)

```bash
cd ui && npm install          # install frontend dependencies
cd ui && npm run dev          # dev server (proxies /api, /ws, /files to localhost:3000)
cd ui && npm run build        # production build
cd ui && npx svelte-check     # type checking
```

### Desktop (Tauri)

```bash
cd ui && npm run tauri:dev      # dev mode (launches native window with Vite dev server)
cd ui && npm run tauri:build    # production build (creates native app bundle)
```

### Production Build

```bash
cd ui && npm run build          # builds SPA to ui/build/
cargo build --release           # builds server binary
TEIDE_CHAT_SECRET=<secret-min-32-bytes> ./target/release/teidelum  # serves frontend + API on :3000
```

## Architecture

Single crate, modules under `src/`:

| Module | Role |
|--------|------|
| `main.rs` | Entrypoint: opens `TeidelumApi`, registers relationships, serves MCP over stdio |
| `api.rs` | Unified programmatic API: `TeidelumApi` wraps catalog, search, router behind thread-safe interface |
| `mcp.rs` | MCP tool definitions via `rmcp`; delegates to `TeidelumApi` for all operations |
| `router.rs` | Query router: dispatches SQL to libteide (local) or connectors (remote) |
| `search.rs` | tantivy wrapper: `SearchEngine` (BM25, fuzzy) |
| `catalog.rs` | Metadata catalog: schemas, FK relationships, local vs remote tracking |
| `connector/mod.rs` | `Connector` trait for live external queries |
| `connector/kdb.rs` | kdb+ live query adapter |
| `sync/mod.rs` | `SyncSource` trait + types (`SyncOutput`, `SearchDocument`) |
| `sync/notion.rs` | Notion incremental sync |
| `sync/zulip.rs` | Zulip incremental sync |
| `server.rs` | HTTP server setup: Axum router, CORS, optional API key auth |
| `chat/handlers.rs` | Slack-compatible REST API handlers (channels, messages, reactions, search) |
| `chat/files.rs` | File upload (multipart, MIME allowlist, extension-only MIME detection) and download with auth, nosniff headers |
| `chat/hub.rs` | WebSocket pub/sub hub for real-time event broadcasting |
| `chat/ws.rs` | WebSocket upgrade and per-connection event loop |
| `chat/auth.rs` | JWT auth, Argon2 password hashing, middleware |
| `chat/models.rs` | Chat schema DDL, FK relationships, SQL helpers |
| `chat/events.rs` | Server/client event types for WebSocket protocol |

### Desktop Client (`ui/src-tauri/`)

Tauri v2 wrapper that packages the SvelteKit SPA as a native desktop app. Connects to a remote Teidelum server (does not embed the Rust backend).

| Module | Role |
|--------|------|
| `ui/src-tauri/src/main.rs` | Tauri bootstrap, loads SvelteKit frontend in a webview |
| `ui/src-tauri/tauri.conf.json` | Window config (1200x800, min 800x600), build settings, bundle targets |

### Frontend (`ui/`)

SvelteKit SPA (SSR disabled) with TypeScript and Tailwind CSS. Uses `@sveltejs/adapter-static` (output to `ui/build/`, `fallback: 'index.html'`).

| Module | Role |
|--------|------|
| `ui/src/lib/api.ts` | Typed API client, all calls via `POST /api/slack/<method>` with Bearer token |
| `ui/src/lib/ws.ts` | WebSocket client with auto-reconnect and event dispatch to stores |
| `ui/src/lib/types.ts` | Shared TypeScript types (User, Channel, Message, WsEvent, API responses) |
| `ui/src/lib/markdown.ts` | Markdown rendering (marked + DOMPurify) with @mention highlighting |
| `ui/src/lib/stores/` | Svelte stores: auth (JWT), channels, messages (per-channel cache), users, unreads |
| `ui/src/lib/components/` | UI components: Sidebar, MessageList, MessageInput, ThreadPanel, SearchModal, etc. |

### Key Design Patterns

- **Query Router** (`router.rs`): Routes queries through the metadata catalog — local tables go to libteide, remote tables go through connectors.
- **Dual Storage**: Sync modules split data into structured fields (→ libteide columnar tables for SQL) and freeform content (→ tantivy full-text index for search).
- **Incremental Sync**: Sync modules track cursors to pull only changed data on subsequent runs.
- **Catalog-Driven**: `Catalog` describes all available data, enabling the `describe` MCP tool and the query router.
- **PGQ Property Graphs** (`api.rs`): Auto-generated property graphs created from catalog FK relationships via `CREATE PROPERTY GRAPH` DDL. Graph name convention: `pg_{from_table}_{to_table}_{relation}`. Agents use standard PGQ syntax (`GRAPH_TABLE MATCH`, `PAGERANK`, `COMMUNITY`, `COMPONENT`) through the `sql` tool.
- **Unified API** (`api.rs`): `TeidelumApi` wraps all subsystems (catalog, search engine, query router) behind a single thread-safe facade. Uses `std::sync::RwLock` for concurrent read access to catalog. MCP server, tests, and future plugins all delegate through this API.
- **MCP via rmcp**: Tools are defined with `#[tool]` macro on `Teidelum` struct methods. Parameters use `schemars::JsonSchema` for auto-generated schemas. Tracing goes to stderr (stdout is the MCP transport).
- **Search Auth Filtering**: Both the `chat_search` MCP tool and `search.messages` REST endpoint filter results to only channels the caller is a member of. Results are over-fetched (3x limit) from tantivy then filtered post-query, since tantivy has no per-user access control.
- **MIME Hardening** (`chat/files.rs`): MIME type is always derived from file extension, never from client-supplied headers or DB values. Downloads include `X-Content-Type-Options: nosniff` and `Content-Disposition: attachment`.
- **SPA Mode** (`ui/`): SvelteKit with SSR disabled, using `@sveltejs/adapter-static` (output to `ui/build/`, `fallback: 'index.html'`). Vite dev server proxies `/api`, `/ws`, and `/files` to the Rust backend at `localhost:3000`.
- **DM Channels** (`chat/handlers.rs`): DM channels use deterministic naming `dm-{min_id}-{max_id}` for lookup, avoiding multi-table JOINs that TeideDB does not support.
- **Store-Driven UI**: Svelte writable stores manage auth, channels, messages, users, and unreads. WebSocket events update stores in real time. All WS listener init functions return cleanup callbacks.
- **Unread Tracking** (`chat/handlers.rs`): `channel_reads` table stores `last_read_ts` per user per channel. Updated on `conversations.history` fetch and `conversations.markRead`. Unread count computed in `conversations.list` by counting messages after `last_read_ts`.
- **Thread Metadata**: `conversations.history` enriches parent messages with `reply_count` and `last_reply_ts` computed from the messages table. No denormalized columns — always computed fresh.
- **MCP WebSocket Broadcasting** (`mcp.rs`): MCP chat tools (`chat_post_message`, `chat_reply`, `chat_react`) broadcast WebSocket events via the `Hub` when available. The `Teidelum` struct accepts an optional `Arc<Hub>` via `new_with_hub()`, allowing MCP-originated messages to appear in real time for connected UI clients.
- **Input Validation** (`chat/handlers.rs`, `chat/auth.rs`): Passwords require minimum 8 characters. Channel names are trimmed, limited to 80 characters, and restricted to alphanumeric, hyphen, and underscore. Messages capped at 40,000 characters. JWT secrets must be at least 32 bytes. Server bails at startup if `TEIDE_CHAT_SECRET` is set but too short.
- **SQL Escaping** (`chat/models.rs`): `escape_sql()` strips null bytes and doubles single quotes. All user-supplied strings in chat SQL queries must pass through this function. For LIKE clauses, use `escape_sql_like()` which additionally escapes `%` and `_` wildcards with backslash (requires `ESCAPE '\'` clause in SQL).
- **CORS Policy** (`server.rs`): CORS allows any origin (local-first tool) but restricts methods to GET/POST/OPTIONS and headers to Authorization and Content-Type. Server warns at startup if `TEIDELUM_API_KEY` is unset.
- **Static Frontend Serving** (`server.rs`): When `ui/build/` exists, Axum serves it as a fallback after API routes. SPA routing handled via `index.html` fallback. In dev, use Vite proxy instead.

### Testing

Integration tests in `tests/chat_integration.rs` spin up an in-memory `TeidelumApi` + `ChatState` and exercise chat HTTP handlers via `tower::ServiceExt::oneshot`. Each test creates its own temp directory and registers fresh users. Covers: auth, channels, messaging, threads, unreads, DMs, presence, mentions, and reactions. Run with `--test-threads=1` to avoid TeideDB concurrency issues.

### MCP Tools

Sixteen tools exposed to AI agents: core tools (`search`, `sql`, `describe`, `sync`), data management tools (`create_table`, `insert_rows`, `delete_table`, `add_documents`, `delete_documents`, `add_relationship`), and chat tools (`chat_post_message`, `chat_history`, `chat_reply`, `chat_react`, `chat_list_channels`, `chat_search`). Defined in `mcp.rs`.
