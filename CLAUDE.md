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

### Key Design Patterns

- **Query Router** (`router.rs`): Routes queries through the metadata catalog — local tables go to libteide, remote tables go through connectors.
- **Dual Storage**: Sync modules split data into structured fields (→ libteide columnar tables for SQL) and freeform content (→ tantivy full-text index for search).
- **Incremental Sync**: Sync modules track cursors to pull only changed data on subsequent runs.
- **Catalog-Driven**: `Catalog` describes all available data, enabling the `describe` MCP tool and the query router.
- **Graph Traversal** (`graph.rs`): BFS over catalog FK relationships using SQL queries at each hop. Supports neighbor discovery and path-finding with direction and relationship-type filtering. Capped at 10 hops (`MAX_DEPTH`).
- **Unified API** (`api.rs`): `TeidelumApi` wraps all subsystems (catalog, search engine, query router, graph engine) behind a single thread-safe facade. Uses `std::sync::RwLock` for concurrent read access to catalog and graph. MCP server, tests, and future plugins all delegate through this API.
- **MCP via rmcp**: Tools are defined with `#[tool]` macro on `Teidelum` struct methods. Parameters use `schemars::JsonSchema` for auto-generated schemas. Tracing goes to stderr (stdout is the MCP transport).

### MCP Tools

Five tools exposed to AI agents: `search` (full-text), `sql` (analytical queries), `describe` (schema/catalog), `graph` (relationship traversal), `sync` (trigger data pull). Defined in `mcp.rs`.
