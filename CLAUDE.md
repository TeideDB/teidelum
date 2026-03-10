# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Teidelum is a compact, local-first MCP server that syncs work tools (Notion, Zulip) and connects live data sources (kdb+) into a single searchable and queryable index. Single binary, zero config, data never leaves the machine. See SPEC.md for the full specification.

## Build & Development Commands

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

## Architecture

Single crate, modules under `src/`:

| Module | Role |
|--------|------|
| `main.rs` | Entrypoint: opens `TeidelumApi`, registers relationships, serves MCP over stdio |
| `api.rs` | Unified programmatic API: `TeidelumApi` wraps catalog, search, router, graph behind thread-safe interface |
| `mcp.rs` | MCP tool definitions via `rmcp`; delegates to `TeidelumApi` for all operations |
| `router.rs` | Query router: dispatches SQL to libteide (local) or connectors (remote) |
| `search.rs` | tantivy wrapper: `SearchEngine` (BM25, fuzzy) |
| `catalog.rs` | Metadata catalog: schemas, FK relationships, local vs remote tracking |
| `graph.rs` | SQL-based graph traversal engine: BFS over catalog FK relationships |
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

### Key Design Patterns

- **Query Router** (`router.rs`): Routes queries through the metadata catalog — local tables go to libteide, remote tables go through connectors.
- **Dual Storage**: Sync modules split data into structured fields (→ libteide columnar tables for SQL) and freeform content (→ tantivy full-text index for search).
- **Incremental Sync**: Sync modules track cursors to pull only changed data on subsequent runs.
- **Catalog-Driven**: `Catalog` describes all available data, enabling the `describe` MCP tool and the query router.
- **Graph Traversal** (`graph.rs`): BFS over catalog FK relationships using SQL queries at each hop. Supports neighbor discovery and path-finding with direction and relationship-type filtering. Capped at 10 hops (`MAX_DEPTH`).
- **Unified API** (`api.rs`): `TeidelumApi` wraps all subsystems (catalog, search engine, query router, graph engine) behind a single thread-safe facade. Uses `std::sync::RwLock` for concurrent read access to catalog and graph. MCP server, tests, and future plugins all delegate through this API.
- **MCP via rmcp**: Tools are defined with `#[tool]` macro on `Teidelum` struct methods. Parameters use `schemars::JsonSchema` for auto-generated schemas. Tracing goes to stderr (stdout is the MCP transport).
- **Search Auth Filtering**: Both the `chat_search` MCP tool and `search.messages` REST endpoint filter results to only channels the caller is a member of. Results are over-fetched (3x limit) from tantivy then filtered post-query, since tantivy has no per-user access control.
- **MIME Hardening** (`chat/files.rs`): MIME type is always derived from file extension, never from client-supplied headers or DB values. Downloads include `X-Content-Type-Options: nosniff` and `Content-Disposition: attachment`.

### MCP Tools

Seventeen tools exposed to AI agents: core tools (`search`, `sql`, `describe`, `graph`, `sync`), data management tools (`create_table`, `insert_rows`, `delete_table`, `add_documents`, `delete_documents`, `add_relationship`), and chat tools (`chat_post_message`, `chat_history`, `chat_reply`, `chat_react`, `chat_list_channels`, `chat_search`). Defined in `mcp.rs`.
