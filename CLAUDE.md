# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Teidelum is a compact, local-first MCP server that syncs work tools (Notion, Zulip) and connects live data sources (kdb+) into a single searchable and queryable index. Single binary, zero config, data never leaves the machine. See SPEC.md for the full specification.

This is a greenfield Rust project — code is being built from the spec.

## Build & Development Commands

```bash
# Build all crates
cargo build

# Build with specific features (connectors/syncs are feature-flagged)
cargo build --release --features notion,zulip,kdb

# Run tests
cargo test

# Run a single test
cargo test test_name

# Run tests for a specific crate
cargo test -p teidelum-server
cargo test -p teidelum-search

# Check without building
cargo check

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt --check   # check only
cargo fmt           # apply
```

## Architecture

Rust workspace under `crates/`:

| Crate | Role |
|-------|------|
| `server` | tokio-based MCP protocol server, query router, binary entrypoint |
| `search` | tantivy wrapper implementing `SearchEngine` trait (BM25, fuzzy) |
| `catalog` | Metadata catalog: schemas, FK relationships, local vs remote tracking |
| `connector-core` | Trait for querying external sources live (no local storage) |
| `connector-kdb` | kdb+ live query adapter (feature-gated: `kdb`) |
| `sync-core` | Trait for pull → transform → store locally |
| `sync-notion` | Notion incremental sync (feature-gated: `notion`) |
| `sync-zulip` | Zulip incremental sync (feature-gated: `zulip`) |

### Dependency Flow

`server` is the root crate depending on everything else. Connectors and sync modules are feature-gated — they compile in only when their feature flag is enabled. `teide-rs` is an external crate from `../teide-rs` providing the libteide columnar SQL engine.

### Key Design Patterns

- **Query Router**: The server routes queries through the metadata catalog — local tables go to libteide, remote tables go through connectors. This is the central dispatch point.
- **Dual Storage**: Sync modules split data into structured fields (→ libteide columnar tables for SQL) and freeform content (→ tantivy full-text index for search).
- **Incremental Sync**: Sync modules track cursors to pull only changed data on subsequent runs.
- **Catalog-Driven**: The `_catalog_tables` and `_catalog_relationships` tables describe all available data, enabling the `describe` MCP tool and the query router.

### MCP Tools

Four tools exposed to AI agents: `search` (full-text), `sql` (analytical queries), `describe` (schema/catalog), `sync` (trigger data pull).
