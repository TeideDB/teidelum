---
title: Query Router
description: How SQL queries are dispatched to the right engine
---

The query router receives SQL queries and dispatches them to the appropriate engine based on the catalog's metadata.

## Local Queries

For tables with `StorageType::Local`, queries go to the teide columnar engine. Teide stores data in a splayed format — one file per column — optimized for analytical operations.

### Table Loading

On startup, Teidelum scans the `tables/` directory for splayed tables (directories containing a `.d` marker file). Each table is loaded into teide's in-memory engine:

```rust
pub fn load_splayed(
    &self,
    name: &str,
    dir: &Path,
    sym_path: Option<&Path>,
) -> Result<()>
```

The optional `sym_path` points to a shared symbol file used for enumerated string columns.

## Thread Safety

Teide's `Session` contains raw pointers from its C FFI layer, making it neither `Send` nor `Sync`. The router wraps it in a `Mutex` to ensure exclusive access:

```rust
pub struct QueryRouter {
    session: Mutex<teide::Session>,
}
```

All query execution goes through `query_sync`, which locks the mutex:

```rust
pub fn query_sync(&self, sql: &str) -> Result<QueryResult> {
    let mut session = self.session.lock().unwrap();
    let result = session.execute(sql)?;
    // ... convert to QueryResult
}
```

## Query Results

All queries return a uniform `QueryResult`:

```rust
pub struct QueryResult {
    pub columns: Vec<ColumnSchema>,
    pub rows: Vec<Vec<Value>>,
}
```

DDL statements (CREATE TABLE, DROP TABLE) return a single-row result with a status message.
