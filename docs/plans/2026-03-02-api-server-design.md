# Teidelum API Server & MCP Write Tools — Design Document

**Date**: 2026-03-02
**Status**: Approved

## Overview

Add an HTTP REST API server and MCP write tools to Teidelum so it can be used by both applications (via REST) and AI agents (via MCP). The same `TeidelumApi` backs both interfaces. Supports local-first use (no auth, localhost) and cloud deployment (API key auth, public binding).

## Architecture

Single binary, dual transport:

```
teidelum [--port 8080] [--bind 127.0.0.1]
│
├── MCP stdio (always on, backward compatible)
│
└── HTTP server (opt-in via --port)
    ├── /api/v1/*     REST API (axum handlers)
    └── /mcp          MCP Streamable HTTP (rmcp transport)
```

Both transports share a single `Arc<TeidelumApi>` instance.

## Module Layout

```
src/
├── main.rs          # CLI args (clap), starts stdio MCP + optional HTTP server
├── api.rs           # TeidelumApi (add delete_table, delete_documents methods)
├── mcp.rs           # MCP tools (add 6 write tools alongside existing 5)
├── server.rs        # NEW: axum server setup, routing, auth middleware
├── routes.rs        # NEW: REST endpoint handlers (delegate to TeidelumApi)
├── catalog.rs       # Unchanged
├── search.rs        # Add delete_documents method
├── router.rs        # Add drop_table method
├── graph.rs         # Unchanged
├── connector/       # Unchanged
├── sync/            # Unchanged
└── demo.rs          # Unchanged
```

## Dependencies

Add to Cargo.toml:

```toml
axum = "0.8"
tower-http = { version = "0.6", features = ["cors"] }
clap = { version = "4", features = ["derive"] }
```

Update rmcp features:

```toml
rmcp = { version = "0.16", features = ["server", "transport-io", "transport-streamable-http-server", "macros"] }
```

## CLI Arguments

```
teidelum [OPTIONS]

Options:
  --port <PORT>      Enable HTTP server on this port
  --bind <ADDR>      Bind address [default: 127.0.0.1]
  --data <DIR>       Data directory [default: ./data, env: TEIDELUM_DATA]
```

When `--port` is not set: stdio-only mode (current behavior, backward compatible).
When `--port` is set: starts HTTP server alongside stdio MCP.

## Authentication

- If `TEIDELUM_API_KEY` env var is set: axum middleware requires `Authorization: Bearer <key>` on all HTTP requests. Returns 401 on mismatch.
- If unset: no authentication. Local-first default.
- stdio MCP: never authenticated (local process, trusted).

## REST API Endpoints

All under `/api/v1/`. JSON request/response.

### Read Operations

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/search` | Full-text search |
| `POST` | `/api/v1/sql` | Execute SQL query |
| `GET` | `/api/v1/describe` | List all tables/schemas/relationships |
| `GET` | `/api/v1/describe/:source` | Filter catalog by source |
| `POST` | `/api/v1/graph/neighbors` | Find neighbors |
| `POST` | `/api/v1/graph/path` | Find path between nodes |

### Write Operations

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/api/v1/tables` | Create table with schema and rows |
| `POST` | `/api/v1/tables/:name/rows` | Insert rows into existing table |
| `DELETE` | `/api/v1/tables/:name` | Drop table and remove from catalog |
| `POST` | `/api/v1/documents` | Index search documents |
| `DELETE` | `/api/v1/documents/:id` | Remove document from search index |
| `POST` | `/api/v1/relationships` | Register FK relationship |

### MCP Endpoint

| Method | Path | Description |
|--------|------|-------------|
| `POST` | `/mcp` | MCP Streamable HTTP (rmcp handles internally) |

## Request/Response Formats

### Create Table

```json
POST /api/v1/tables
{
  "name": "customers",
  "source": "app",
  "columns": [
    {"name": "id", "type": "int"},
    {"name": "name", "type": "varchar"},
    {"name": "email", "type": "varchar"}
  ],
  "rows": [
    [1, "Alice", "alice@example.com"],
    [2, "Bob", "bob@example.com"]
  ]
}
```

Response: `201 {"table": "customers", "rows_inserted": 2}`

### Insert Rows

```json
POST /api/v1/tables/customers/rows
{
  "rows": [
    [3, "Charlie", "charlie@example.com"]
  ]
}
```

Response: `200 {"table": "customers", "rows_inserted": 1}`

### Add Documents

```json
POST /api/v1/documents
{
  "documents": [
    {
      "id": "doc-001",
      "source": "notion",
      "title": "Auth Migration Plan",
      "body": "We are migrating from session-based auth to JWT..."
    }
  ]
}
```

Response: `201 {"documents_indexed": 1}`

### Add Relationship

```json
POST /api/v1/relationships
{
  "from_table": "customers",
  "from_col": "id",
  "to_table": "orders",
  "to_col": "customer_id",
  "relation": "has_orders"
}
```

Response: `201 {"relationship": "customers.id -> orders.customer_id"}`

### Search

```json
POST /api/v1/search
{
  "query": "JWT token rotation",
  "sources": ["notion"],
  "limit": 5
}
```

Response: `200` with search results array.

### SQL

```json
POST /api/v1/sql
{
  "query": "SELECT * FROM customers LIMIT 10"
}
```

Response: `200` with query result.

### Error Responses

```json
400 {"error": "table 'customers' already exists"}
401 {"error": "invalid or missing API key"}
500 {"error": "internal error: ..."}
```

## MCP Write Tools

Add 6 new MCP tools (total: 11):

| Tool | Description | Key Parameters |
|------|-------------|----------------|
| `create_table` | Create new table | name, source, columns, rows |
| `insert_rows` | Insert into existing table | table, rows |
| `delete_table` | Drop table from catalog | table |
| `add_documents` | Index search documents | documents array |
| `delete_documents` | Remove from search index | ids array |
| `add_relationship` | Register FK relationship | from_table, from_col, to_table, to_col, relation |

Available on both stdio and HTTP MCP transports.

## Required API Changes

### SearchEngine (search.rs)

Add `delete_documents(ids: &[String])` — delete tantivy docs by id field, commit, reload reader.

### QueryRouter (router.rs)

Add `drop_table(name: &str)` — execute `DROP TABLE name` in teide session.

### TeidelumApi (api.rs)

Add:
- `delete_table(name: &str)` — calls router.drop_table + catalog.remove_table + rebuild graph
- `delete_documents(ids: &[String])` — calls search_engine.delete_documents

### Catalog (catalog.rs)

Add `remove_table(name: &str)` — remove table entry and any relationships referencing it.
