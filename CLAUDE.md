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
| `main.rs` | Entrypoint: initializes components, serves MCP over stdio |
| `mcp.rs` | MCP tool definitions (search, sql, describe, sync) via `rmcp` |
| `router.rs` | Query router: dispatches SQL to libteide (local) or connectors (remote) |
| `search.rs` | tantivy wrapper: `SearchEngine` (BM25, fuzzy) |
| `catalog.rs` | Metadata catalog: schemas, FK relationships, local vs remote tracking |
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
- **MCP via rmcp**: Tools are defined with `#[tool]` macro on `Teidelum` struct methods. Parameters use `schemars::JsonSchema` for auto-generated schemas. Tracing goes to stderr (stdout is the MCP transport).

### MCP Tools

Four tools exposed to AI agents: `search` (full-text), `sql` (analytical queries), `describe` (schema/catalog), `sync` (trigger data pull). Defined in `mcp.rs`.
