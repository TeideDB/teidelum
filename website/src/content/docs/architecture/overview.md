---
title: Architecture Overview
description: How Teidelum's modules fit together
---

Teidelum is a single-crate Rust application organized into focused modules behind a unified API facade.

## Module Map

| Module | Role |
|--------|------|
| `main.rs` | Entrypoint: opens `TeidelumApi`, registers relationships, serves MCP over stdio |
| `api.rs` | Unified API: wraps catalog, search, router, graph behind thread-safe interface |
| `mcp.rs` | MCP tool definitions via `rmcp`; delegates to `TeidelumApi` |
| `router.rs` | Query router: dispatches SQL to the local columnar engine |
| `search.rs` | Tantivy wrapper: full-text search with BM25 ranking |
| `catalog.rs` | Metadata catalog: schemas, FK relationships, local vs remote tracking |
| `graph.rs` | Graph traversal: BFS over catalog FK relationships |
| `connector/` | `Connector` trait for live external queries |
| `sync/` | `SyncSource` trait for incremental data pull |
| `demo.rs` | Demo data generator for first-run experience |

## Data Flow

```
External APIs ──┐
                ├──▶ Sync Sources ──▶ Structured Records ──▶ SQL Engine (teide)
                │                  └──▶ Search Documents  ──▶ Search Index (tantivy)
                │
External DBs ───┴──▶ Connectors ──▶ Live Query Results
                                        │
                         ┌───────────────┘
                         ▼
              ┌─────────────────────┐
              │   TeidelumApi       │
              │  ┌───────┬────────┐ │
              │  │Catalog│ Search │ │
              │  ├───────┼────────┤ │
              │  │Router │ Graph  │ │
              │  └───────┴────────┘ │
              └─────────┬───────────┘
                        ▼
                 MCP Tools (stdio)
                        ▼
                    AI Agents
```

## Design Principles

- **Unified API**: All subsystems are accessed through `TeidelumApi`. MCP server, tests, and future plugins all go through this single facade.
- **Dual Storage**: Sync modules split data into structured records (columnar tables for SQL) and search documents (full-text index). This means the same data is queryable both analytically and by content.
- **Catalog-Driven**: The catalog describes all available data. The query router uses it to dispatch queries. The `describe` tool exposes it. The graph engine builds its topology from it.
- **Thread Safety**: `RwLock` for catalog and graph (concurrent reads), `Arc` for search engine and router (shared ownership), `Mutex` for the teide session (C FFI).

## Key Dependencies

| Dependency | Purpose |
|------------|---------|
| `rmcp` | MCP protocol implementation |
| `tantivy` | Full-text search engine |
| `teide` | Local columnar database engine |
| `tokio` | Async runtime |
| `serde` / `serde_json` | Serialization |
| `anyhow` / `thiserror` | Error handling |
