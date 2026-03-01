# Plan: Comprehensive Plugin/Ingestion API (`TeidelumApi`)

## Context

Currently Teidelum has no unified programmatic API. Data ingestion is scattered across `main.rs` (manual CSV→splayed tables, markdown file reading, hardcoded relationship registration). Tests directly construct internal objects (`QueryRouter`, `GraphEngine`) and generate demo data on disk. Plugins and external tools have no clean entry point.

This plan creates `src/api.rs` — a single `TeidelumApi` struct that wraps all subsystems and provides a clean, thread-safe API for creating tables, indexing documents, registering relationships, querying, and graph traversal. The MCP server and all tests delegate to this API.

## Files to Change

| File | Action |
|------|--------|
| `src/api.rs` | **New** — `TeidelumApi` struct + all methods + comprehensive tests |
| `src/lib.rs` | Add `pub mod api;` |
| `src/mcp.rs` | Refactor `Teidelum` to hold `Arc<TeidelumApi>` and delegate |
| `src/main.rs` | Simplify to `TeidelumApi::open()` + `register_relationships()` |
| `src/graph.rs` | Remove `#[cfg(test)]` from `from_relationships*` constructors; rewrite integration tests to use API |

## 1. New `src/api.rs`

```rust
pub struct TeidelumApi {
    catalog: RwLock<Catalog>,
    search_engine: Arc<SearchEngine>,
    query_router: Arc<QueryRouter>,
    graph_engine: RwLock<GraphEngine>,
}
```

**Why `RwLock`**: Catalog and graph are read-heavy, rarely written. `RwLock` allows concurrent readers.

### Methods

- `new(data_dir) -> Result<Self>` — empty instance, creates SearchEngine + QueryRouter
- `open(data_dir) -> Result<Self>` — loads existing splayed tables + indexes markdown docs
- `create_table(name, source, columns, rows) -> Result<()>` — SQL `CREATE TABLE` + `INSERT INTO VALUES` (teide supports this natively, verified in `teide-rs/tests/slt/insert.slt`)
- `add_documents(docs: &[SearchDocument]) -> Result<usize>` — indexes into tantivy
- `query(sql) -> Result<QueryResult>` — execute SQL
- `search(query) -> Result<Vec<SearchResult>>` — full-text search
- `register_table(entry)` — add to catalog
- `register_relationship(rel) -> Result<()>` — add to catalog + rebuild graph engine
- `register_relationships(rels) -> Result<()>` — bulk add, single rebuild
- `describe(source_filter) -> Result<Value>` — catalog JSON
- `neighbors(...)` / `path(...)` — graph traversal, delegates to GraphEngine under read lock
- `search_engine()` / `query_router()` — accessors for MCP delegation

### `create_table` implementation

Uses native SQL (no CSV temp files):
1. Validate identifier
2. `CREATE TABLE name (col1 INTEGER, col2 VARCHAR, ...)`
3. `INSERT INTO name VALUES (...)` in batches of 1000
4. Register in catalog

Type mapping (from `connector::ColumnSchema.dtype` to SQL):
- `bool` → `BOOLEAN`, `i32`/`i64` → `BIGINT`, `f64` → `DOUBLE`, `string` → `VARCHAR`
- `date` → `DATE`, `time` → `TIME`, `timestamp` → `TIMESTAMP`

### Helper functions (private)

- `dtype_to_sql(dtype: &str) -> &str` — maps connector dtype to SQL type name
- `row_to_sql_values(row: &[Value]) -> String` — formats a row as SQL VALUES clause
- `validate_identifier(s: &str) -> Result<()>` — reuse existing pattern from catalog.rs
- `load_splayed_tables(&self, tables_dir) -> Result<()>` — load all splayed tables from dir
- `index_markdown_dir(&self, docs_dir) -> Result<()>` — index all .md files from dir

## 2. Refactor `src/mcp.rs`

Replace individual Arc fields with `Arc<TeidelumApi>`:

```rust
pub struct Teidelum {
    api: Arc<TeidelumApi>,
    tool_router: ToolRouter<Self>,
}
```

Each `#[tool]` method delegates to `self.api.search()`, `self.api.query()`, etc.
Remove `tokio::sync::Mutex` import — catalog locking is now internal to API via `std::sync::RwLock`.

Constructor changes from:
```rust
pub fn new(catalog, search_engine, query_router, graph_engine) -> Self
```
to:
```rust
pub fn new(api: TeidelumApi) -> Self
```

## 3. Simplify `src/main.rs`

Replace ~100 lines of manual setup with:
```rust
let api = TeidelumApi::open(&data)?;
api.register_relationships(vec![
    Relationship { from_table: "project_tasks", from_col: "assignee",
                   to_table: "team_members", to_col: "name", relation: "assigned_to" },
    Relationship { from_table: "incidents", from_col: "reporter",
                   to_table: "team_members", to_col: "name", relation: "reported_by" },
])?;
let server = Teidelum::new(api);
```

Remove `index_documents()` and `load_tables()` private functions — `TeidelumApi::open()` handles both.

## 4. Modify `src/graph.rs`

- Make `GraphEngine::from_relationships()` and `from_relationships_with_columns()` public (remove `#[cfg(test)]`)
- Keep utility unit tests (escape_sql, identifier validation) unchanged
- Keep `find_relationships` unit tests unchanged (they test internal logic)
- Rewrite integration tests (`test_neighbors_*`, `test_path_*`, `test_reverse_traversal_*`) to use `TeidelumApi` instead of manual `setup_demo_router()` + `demo_engine()`

## 5. Tests in `src/api.rs`

Comprehensive test suite exercising the full API:

| Test | What it verifies |
|------|-----------------|
| `test_create_table_and_query` | Create table from Values, query back |
| `test_create_table_empty` | Empty table (DDL only, no INSERT) |
| `test_create_table_invalid_name` | SQL injection prevention |
| `test_create_table_all_types` | bool, i64, f64, string columns |
| `test_add_documents_and_search` | Index SearchDocuments + full-text search |
| `test_register_relationship_rebuilds_graph` | Create 2 tables, register FK, traverse graph |
| `test_register_relationships_bulk` | Bulk registration, single graph rebuild |
| `test_describe_catalog` | Catalog JSON output after table + relationship registration |
| `test_neighbors_via_api` | Graph neighbors through API with demo data |
| `test_path_via_api` | Graph path through API with demo data |
| `test_open_with_demo_data` | `open()` loads splayed tables + docs |

### Test helper

```rust
fn test_api() -> TeidelumApi {
    let tmp = tempfile::tempdir().unwrap();
    crate::demo::generate(tmp.path()).unwrap();
    let api = TeidelumApi::open(tmp.path()).unwrap();
    api.register_relationships(vec![/* demo FKs */]).unwrap();
    api
}
```

## Verification

```bash
cargo test                     # all tests pass
cargo clippy -- -D warnings    # no warnings
cargo fmt --check              # formatted
```
