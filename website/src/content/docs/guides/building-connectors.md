---
title: Building Connectors
description: Create live query adapters for external data sources
---

Connectors query external data sources in real time, without storing data locally. The query router dispatches to connectors for tables marked as `remote` in the catalog.

## The Connector Trait

```rust
#[async_trait]
pub trait Connector: Send + Sync {
    /// Unique name for this connector (e.g. "kdb").
    fn name(&self) -> &str;

    /// Return the schemas of tables available through this connector.
    async fn list_tables(&self) -> Result<Vec<(String, Vec<ColumnSchema>)>>;

    /// Execute a SQL query against the remote source.
    async fn query(&self, sql: &str) -> Result<QueryResult>;
}
```

## Implementing a Connector

A connector needs to:

1. **List available tables** with their column schemas
2. **Translate SQL** to the source's native query language
3. **Execute queries** and return results as `QueryResult`

### QueryResult Format

```rust
pub struct QueryResult {
    pub columns: Vec<ColumnSchema>,
    pub rows: Vec<Vec<Value>>,
}
```

Where `Value` is one of:

```rust
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}
```

### ColumnSchema

```rust
pub struct ColumnSchema {
    pub name: String,
    pub dtype: String,  // "bool", "i64", "f64", "string", etc.
}
```

## Registering a Connector

After implementing the trait, register the connector's tables in the catalog with `StorageType::Remote`. The query router will then dispatch matching SQL queries to your connector instead of the local engine.
