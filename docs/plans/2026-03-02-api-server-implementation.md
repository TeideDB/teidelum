# API Server & MCP Write Tools — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add HTTP REST API server and 6 MCP write tools to Teidelum, so both applications (via REST) and AI agents (via MCP) can read/write data.

**Architecture:** Single binary, dual transport. `TeidelumApi` backs both MCP (stdio + HTTP) and REST endpoints. axum serves REST under `/api/v1/` and rmcp handles MCP Streamable HTTP at `/mcp`. Optional API key auth via `TEIDELUM_API_KEY` env var.

**Tech Stack:** Rust, axum 0.8, tower-http 0.6 (CORS), clap 4 (CLI args), rmcp 0.16 (transport-streamable-http-server), tantivy 0.22, teide (local SQL).

**Design Doc:** `docs/plans/2026-03-02-api-server-design.md`

---

### Task 1: Add Dependencies

**Files:**
- Modify: `Cargo.toml`

**Step 1: Update Cargo.toml**

Add these lines to `[dependencies]`:

```toml
axum = "0.8"
tower-http = { version = "0.6", features = ["cors"] }
clap = { version = "4", features = ["derive"] }
```

Update the existing `rmcp` line to:

```toml
rmcp = { version = "0.16", features = ["server", "transport-io", "transport-streamable-http-server", "macros"] }
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles with no errors (new deps unused for now — that's fine)

**Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "deps: add axum, tower-http, clap; enable rmcp streamable HTTP"
```

---

### Task 2: Add `remove_table` to Catalog

**Files:**
- Modify: `src/catalog.rs:56-100` (add method to `impl Catalog`)

**Step 1: Write the failing test**

Add at the bottom of `src/catalog.rs`, inside a new `#[cfg(test)] mod tests { ... }` block:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_table() {
        let mut catalog = Catalog::new();
        catalog.register_table(TableEntry {
            name: "users".to_string(),
            source: "test".to_string(),
            storage: StorageType::Local,
            columns: vec![ColumnInfo {
                name: "id".to_string(),
                dtype: "i64".to_string(),
            }],
            row_count: Some(10),
        });
        catalog.register_table(TableEntry {
            name: "orders".to_string(),
            source: "test".to_string(),
            storage: StorageType::Local,
            columns: vec![ColumnInfo {
                name: "id".to_string(),
                dtype: "i64".to_string(),
            }],
            row_count: Some(5),
        });
        catalog
            .register_relationship(Relationship {
                from_table: "orders".to_string(),
                from_col: "user_id".to_string(),
                to_table: "users".to_string(),
                to_col: "id".to_string(),
                relation: "belongs_to".to_string(),
            })
            .unwrap();

        assert!(catalog.remove_table("users"));

        // Table gone
        assert!(catalog.lookup_table("users").is_none());
        assert_eq!(catalog.tables().len(), 1);
        // Relationships referencing "users" also removed
        assert!(catalog.relationships().is_empty());
    }

    #[test]
    fn test_remove_table_nonexistent() {
        let mut catalog = Catalog::new();
        assert!(!catalog.remove_table("ghost"));
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib catalog::tests::test_remove_table -- --nocapture`
Expected: FAIL — `remove_table` method doesn't exist

**Step 3: Implement `remove_table`**

Add to `impl Catalog` block (after `register_relationship`, around line 88):

```rust
    /// Remove a table and any relationships referencing it. Returns true if the table existed.
    pub fn remove_table(&mut self, name: &str) -> bool {
        let before = self.tables.len();
        self.tables.retain(|t| t.name != name);
        if self.tables.len() == before {
            return false;
        }
        self.relationships
            .retain(|r| r.from_table != name && r.to_table != name);
        true
    }
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib catalog::tests -- --nocapture`
Expected: 2 tests PASS

**Step 5: Commit**

```bash
git add src/catalog.rs
git commit -m "feat(catalog): add remove_table method"
```

---

### Task 3: Add `delete_documents` to SearchEngine

**Files:**
- Modify: `src/search.rs:42-102` (add method to `impl SearchEngine`)

**Step 1: Write the failing test**

Add at the bottom of `src/search.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delete_documents() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = SearchEngine::open(tmp.path()).unwrap();

        // Index 3 docs
        let docs = vec![
            ("d1".to_string(), "test".to_string(), "Alpha".to_string(), "alpha content".to_string()),
            ("d2".to_string(), "test".to_string(), "Beta".to_string(), "beta content".to_string()),
            ("d3".to_string(), "test".to_string(), "Gamma".to_string(), "gamma content".to_string()),
        ];
        engine.index_documents(&docs).unwrap();

        // Delete d1 and d3
        let deleted = engine.delete_documents(&["d1".to_string(), "d3".to_string()]).unwrap();
        assert_eq!(deleted, 2);

        // Search should only find d2
        let results = engine
            .search(&SearchQuery {
                text: "content".to_string(),
                sources: None,
                limit: 10,
                date_from: None,
                date_to: None,
            })
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "d2");
    }

    #[test]
    fn test_delete_documents_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = SearchEngine::open(tmp.path()).unwrap();

        let deleted = engine.delete_documents(&["ghost".to_string()]).unwrap();
        assert_eq!(deleted, 0);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib search::tests::test_delete_documents -- --nocapture`
Expected: FAIL — `delete_documents` method doesn't exist

**Step 3: Implement `delete_documents`**

Add to `impl SearchEngine` block (after `index_documents`, around line 102):

```rust
    /// Delete documents by their id field. Returns the number of delete operations issued.
    pub fn delete_documents(&self, ids: &[String]) -> Result<usize> {
        let mut writer: IndexWriter = self.index.writer(50_000_000)?;
        let mut count = 0;

        for id in ids {
            let term = tantivy::Term::from_field_text(self.f_id, id);
            writer.delete_term(term);
            count += 1;
        }

        writer.commit()?;
        self.reader.reload()?;

        Ok(count)
    }
```

Note: `delete_term` marks all docs matching that term for deletion. Tantivy handles this at merge time. We return the number of delete operations issued, not the number of docs actually removed (we can't know that without searching first).

However, the test expects `deleted == 2` for 2 IDs. Since we return `ids.len()` for the operations issued, and the test deletes d1 and d3 (both exist), this is correct. For the nonexistent test, we still issue 1 delete operation but the term matches nothing — let's change the return to be consistent: we return the number of IDs we were asked to delete. Update the nonexistent test expectation:

Actually, `delete_term` is a fire-and-forget operation — it doesn't tell us how many docs matched. We should count the IDs we process. So the nonexistent test should expect 1 (we issued 1 delete term), not 0. Let me adjust:

```rust
    #[test]
    fn test_delete_documents_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let engine = SearchEngine::open(tmp.path()).unwrap();

        // delete_documents returns count of delete operations, not matched docs
        let deleted = engine.delete_documents(&["ghost".to_string()]).unwrap();
        assert_eq!(deleted, 1);
    }
```

**Step 4: Run tests to verify they pass**

Run: `cargo test --lib search::tests -- --nocapture`
Expected: 2 tests PASS

**Step 5: Commit**

```bash
git add src/search.rs
git commit -m "feat(search): add delete_documents method"
```

---

### Task 4: Add `drop_table` to QueryRouter

**Files:**
- Modify: `src/router.rs:20-115` (add method to `impl QueryRouter`)

**Step 1: Write the failing test**

Add at the bottom of `src/router.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drop_table() {
        let router = QueryRouter::new().unwrap();

        // Create a table first
        router
            .query_sync("CREATE TABLE test_drop (id BIGINT, name VARCHAR)")
            .unwrap();
        router
            .query_sync("INSERT INTO test_drop (id, name) VALUES (1, 'Alice')")
            .unwrap();

        // Verify it exists
        let result = router.query_sync("SELECT * FROM test_drop").unwrap();
        assert_eq!(result.rows.len(), 1);

        // Drop it
        router.drop_table("test_drop").unwrap();

        // Verify it's gone (query should fail)
        let result = router.query_sync("SELECT * FROM test_drop");
        assert!(result.is_err());
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --lib router::tests::test_drop_table -- --nocapture`
Expected: FAIL — `drop_table` method doesn't exist

**Step 3: Implement `drop_table`**

Add to `impl QueryRouter` block (after `query`, around line 114):

```rust
    /// Drop a table from the teide session.
    pub fn drop_table(&self, name: &str) -> Result<()> {
        self.query_sync(&format!("DROP TABLE IF EXISTS {name}"))?;
        Ok(())
    }
```

**Step 4: Run test to verify it passes**

Run: `cargo test --lib router::tests -- --nocapture`
Expected: PASS

**Step 5: Commit**

```bash
git add src/router.rs
git commit -m "feat(router): add drop_table method"
```

---

### Task 5: Add `delete_table` and `delete_documents` to TeidelumApi

**Files:**
- Modify: `src/api.rs:76-335` (add methods to `impl TeidelumApi`)

**Step 1: Make `insert_rows` public**

Change line 186 from:

```rust
    fn insert_rows(&self, name: &str, columns: &[ColumnSchema], rows: &[Vec<Value>]) -> Result<()> {
```

to:

```rust
    pub fn insert_rows(&self, name: &str, columns: &[ColumnSchema], rows: &[Vec<Value>]) -> Result<()> {
```

**Step 2: Write failing tests**

Add to the existing `mod tests` block in `src/api.rs`:

```rust
    #[test]
    fn test_delete_table() {
        let tmp = tempfile::tempdir().unwrap();
        let api = TeidelumApi::new(tmp.path()).unwrap();

        let columns = vec![
            ColumnSchema {
                name: "id".to_string(),
                dtype: "i64".to_string(),
            },
            ColumnSchema {
                name: "name".to_string(),
                dtype: "string".to_string(),
            },
        ];
        api.create_table("ephemeral", "test", &columns, &[vec![
            Value::Int(1),
            Value::String("Alice".to_string()),
        ]])
        .unwrap();

        // Verify it exists
        assert!(api.query("SELECT * FROM ephemeral").is_ok());
        let desc = api.describe(None).unwrap();
        assert_eq!(desc["tables"].as_array().unwrap().len(), 1);

        // Delete it
        api.delete_table("ephemeral").unwrap();

        // Table gone from SQL engine
        assert!(api.query("SELECT * FROM ephemeral").is_err());

        // Table gone from catalog
        let desc = api.describe(None).unwrap();
        assert!(desc["tables"].as_array().unwrap().is_empty());
    }

    #[test]
    fn test_delete_table_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let api = TeidelumApi::new(tmp.path()).unwrap();

        let result = api.delete_table("ghost");
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_documents() {
        let tmp = tempfile::tempdir().unwrap();
        let api = TeidelumApi::new(tmp.path()).unwrap();

        let docs = vec![
            SearchDocument {
                id: "d1".to_string(),
                source: "test".to_string(),
                title: "First".to_string(),
                body: "first document content".to_string(),
                metadata: serde_json::Map::new(),
            },
            SearchDocument {
                id: "d2".to_string(),
                source: "test".to_string(),
                title: "Second".to_string(),
                body: "second document content".to_string(),
                metadata: serde_json::Map::new(),
            },
        ];
        api.add_documents(&docs).unwrap();

        api.delete_documents(&["d1".to_string()]).unwrap();

        let results = api
            .search(&SearchQuery {
                text: "document content".to_string(),
                sources: None,
                limit: 10,
                date_from: None,
                date_to: None,
            })
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "d2");
    }
```

**Step 3: Run tests to verify they fail**

Run: `cargo test --lib api::tests::test_delete_table -- --nocapture`
Expected: FAIL — `delete_table` method doesn't exist

**Step 4: Implement `delete_table` and `delete_documents`**

Add to `impl TeidelumApi` block (after `add_documents`, around line 214):

```rust
    /// Delete a table from the SQL engine, catalog, and rebuild graph.
    pub fn delete_table(&self, name: &str) -> Result<()> {
        validate_identifier(name)?;

        // Remove from catalog first to check it exists
        let mut catalog = self.catalog.write().unwrap();
        if !catalog.remove_table(name) {
            bail!("table '{name}' not found");
        }

        // Drop from SQL engine (ignore errors if not present in SQL — could be remote-only)
        let _ = self.query_router.drop_table(name);

        // Rebuild graph
        self.rebuild_graph_locked(&catalog);

        Ok(())
    }

    /// Delete documents from the search index by their IDs.
    pub fn delete_documents(&self, ids: &[String]) -> Result<usize> {
        self.search_engine.delete_documents(ids)
    }
```

**Step 5: Run all api tests to verify they pass**

Run: `cargo test --lib api::tests -- --nocapture`
Expected: ALL PASS (existing + 3 new)

**Step 6: Commit**

```bash
git add src/api.rs
git commit -m "feat(api): add delete_table, delete_documents; make insert_rows public"
```

---

### Task 6: Add 6 MCP Write Tools

**Files:**
- Modify: `src/mcp.rs` (add param types + tool methods)

**Step 1: Add param type structs**

Add after the existing `GraphParams` struct (around line 101), before `pub struct Teidelum`:

```rust
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct CreateTableParams {
    /// Table name (alphanumeric + underscores).
    pub name: String,
    /// Source identifier (e.g. "app", "import").
    pub source: String,
    /// Column definitions.
    pub columns: Vec<ColumnDef>,
    /// Rows to insert (each row is a JSON array matching column order).
    #[serde(default)]
    pub rows: Vec<Vec<serde_json::Value>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ColumnDef {
    /// Column name.
    pub name: String,
    /// Column type: "int", "varchar", "double", "boolean", "date", "time", "timestamp".
    #[serde(rename = "type")]
    pub dtype: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct InsertRowsParams {
    /// Target table name.
    pub table: String,
    /// Rows to insert (each row is a JSON array matching table column order).
    pub rows: Vec<Vec<serde_json::Value>>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DeleteTableParams {
    /// Table name to delete.
    pub table: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AddDocumentsParams {
    /// Documents to index for full-text search.
    pub documents: Vec<DocumentInput>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DocumentInput {
    /// Unique document ID.
    pub id: String,
    /// Source identifier (e.g. "notion", "app").
    pub source: String,
    /// Document title.
    pub title: String,
    /// Full text content for indexing.
    pub body: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DeleteDocumentsParams {
    /// Document IDs to remove from the search index.
    pub ids: Vec<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct AddRelationshipParams {
    /// Source table name.
    pub from_table: String,
    /// Source column name.
    pub from_col: String,
    /// Target table name.
    pub to_table: String,
    /// Target column name.
    pub to_col: String,
    /// Relationship label (e.g. "has_orders", "assigned_to").
    pub relation: String,
}
```

**Step 2: Add JSON-to-Value conversion helper**

Add a helper function before the `impl Teidelum` block:

```rust
use crate::connector::{ColumnSchema, Value};
use crate::catalog::Relationship;
use crate::sync::SearchDocument;

/// Convert a JSON value to a connector Value based on column type.
fn json_to_value(v: &serde_json::Value, dtype: &str) -> Result<Value, McpError> {
    match v {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(b) => Ok(Value::Bool(*b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Int(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Float(f))
            } else {
                Err(McpError::invalid_params(format!("unsupported number: {n}"), None))
            }
        }
        serde_json::Value::String(s) => Ok(Value::String(s.clone())),
        _ => Err(McpError::invalid_params(
            format!("unsupported value type for column type '{dtype}': {v}"),
            None,
        )),
    }
}

/// Map MCP column type names to internal dtype strings.
fn mcp_type_to_dtype(t: &str) -> &str {
    match t {
        "int" | "integer" | "bigint" => "i64",
        "varchar" | "text" | "string" => "string",
        "double" | "float" | "real" => "f64",
        "boolean" | "bool" => "bool",
        "date" => "date",
        "time" => "time",
        "timestamp" | "datetime" => "timestamp",
        other => other,
    }
}
```

**Step 3: Add 6 tool methods to `impl Teidelum`**

Add inside the `#[tool_router] impl Teidelum` block, after the `graph` method:

```rust
    #[tool(description = "Create a new table with schema and optional initial rows")]
    async fn create_table(
        &self,
        Parameters(params): Parameters<CreateTableParams>,
    ) -> Result<CallToolResult, McpError> {
        let columns: Vec<ColumnSchema> = params
            .columns
            .iter()
            .map(|c| ColumnSchema {
                name: c.name.clone(),
                dtype: mcp_type_to_dtype(&c.dtype).to_string(),
            })
            .collect();

        let rows: Vec<Vec<Value>> = params
            .rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                if row.len() != columns.len() {
                    return Err(McpError::invalid_params(
                        format!("row {i} has {} values but {} columns", row.len(), columns.len()),
                        None,
                    ));
                }
                row.iter()
                    .zip(columns.iter())
                    .map(|(v, c)| json_to_value(v, &c.dtype))
                    .collect()
            })
            .collect::<Result<Vec<_>, _>>()?;

        self.api
            .create_table(&params.name, &params.source, &columns, &rows)
            .map_err(|e| McpError::internal_error(format!("create_table failed: {e}"), None))?;

        let result = serde_json::json!({
            "table": params.name,
            "rows_inserted": rows.len(),
        });
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap(),
        )]))
    }

    #[tool(description = "Insert rows into an existing table")]
    async fn insert_rows(
        &self,
        Parameters(params): Parameters<InsertRowsParams>,
    ) -> Result<CallToolResult, McpError> {
        // Look up column schemas from catalog
        let desc = self
            .api
            .describe(None)
            .map_err(|e| McpError::internal_error(format!("describe failed: {e}"), None))?;

        let tables = desc["tables"].as_array().ok_or_else(|| {
            McpError::internal_error("unexpected catalog format".to_string(), None)
        })?;

        let table_entry = tables
            .iter()
            .find(|t| t["name"].as_str() == Some(&params.table))
            .ok_or_else(|| {
                McpError::invalid_params(format!("table '{}' not found", params.table), None)
            })?;

        let columns: Vec<ColumnSchema> = table_entry["columns"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .map(|c| ColumnSchema {
                name: c["name"].as_str().unwrap_or("").to_string(),
                dtype: c["dtype"].as_str().unwrap_or("string").to_string(),
            })
            .collect();

        let rows: Vec<Vec<Value>> = params
            .rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                if row.len() != columns.len() {
                    return Err(McpError::invalid_params(
                        format!("row {i} has {} values but {} columns", row.len(), columns.len()),
                        None,
                    ));
                }
                row.iter()
                    .zip(columns.iter())
                    .map(|(v, c)| json_to_value(v, &c.dtype))
                    .collect()
            })
            .collect::<Result<Vec<_>, _>>()?;

        self.api
            .insert_rows(&params.table, &columns, &rows)
            .map_err(|e| McpError::internal_error(format!("insert_rows failed: {e}"), None))?;

        let result = serde_json::json!({
            "table": params.table,
            "rows_inserted": rows.len(),
        });
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap(),
        )]))
    }

    #[tool(description = "Delete a table and remove it from the catalog")]
    async fn delete_table(
        &self,
        Parameters(params): Parameters<DeleteTableParams>,
    ) -> Result<CallToolResult, McpError> {
        self.api
            .delete_table(&params.table)
            .map_err(|e| McpError::internal_error(format!("delete_table failed: {e}"), None))?;

        let result = serde_json::json!({"deleted": params.table});
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap(),
        )]))
    }

    #[tool(description = "Index documents for full-text search")]
    async fn add_documents(
        &self,
        Parameters(params): Parameters<AddDocumentsParams>,
    ) -> Result<CallToolResult, McpError> {
        let docs: Vec<SearchDocument> = params
            .documents
            .into_iter()
            .map(|d| SearchDocument {
                id: d.id,
                source: d.source,
                title: d.title,
                body: d.body,
                metadata: serde_json::Map::new(),
            })
            .collect();

        let count = self
            .api
            .add_documents(&docs)
            .map_err(|e| McpError::internal_error(format!("add_documents failed: {e}"), None))?;

        let result = serde_json::json!({"documents_indexed": count});
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap(),
        )]))
    }

    #[tool(description = "Remove documents from the search index by ID")]
    async fn delete_documents(
        &self,
        Parameters(params): Parameters<DeleteDocumentsParams>,
    ) -> Result<CallToolResult, McpError> {
        let count = self
            .api
            .delete_documents(&params.ids)
            .map_err(|e| McpError::internal_error(format!("delete_documents failed: {e}"), None))?;

        let result = serde_json::json!({"delete_operations": count});
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap(),
        )]))
    }

    #[tool(description = "Register a foreign key relationship between two tables")]
    async fn add_relationship(
        &self,
        Parameters(params): Parameters<AddRelationshipParams>,
    ) -> Result<CallToolResult, McpError> {
        self.api
            .register_relationship(Relationship {
                from_table: params.from_table.clone(),
                from_col: params.from_col.clone(),
                to_table: params.to_table.clone(),
                to_col: params.to_col.clone(),
                relation: params.relation,
            })
            .map_err(|e| McpError::internal_error(format!("add_relationship failed: {e}"), None))?;

        let result = serde_json::json!({
            "relationship": format!("{}.{} -> {}.{}", params.from_table, params.from_col, params.to_table, params.to_col),
        });
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap(),
        )]))
    }
```

**Step 4: Update imports at top of mcp.rs**

Add the missing imports. The existing imports are:

```rust
use crate::api::TeidelumApi;
use crate::search::SearchQuery;
```

Add:

```rust
use crate::catalog::Relationship;
use crate::connector::{ColumnSchema, Value};
use crate::sync::SearchDocument;
```

**Step 5: Verify it compiles**

Run: `cargo check`
Expected: compiles (11 tools total now)

**Step 6: Run all existing tests**

Run: `cargo test`
Expected: ALL PASS

**Step 7: Commit**

```bash
git add src/mcp.rs
git commit -m "feat(mcp): add 6 write tools (create_table, insert_rows, delete_table, add_documents, delete_documents, add_relationship)"
```

---

### Task 7: Add CLI Arguments with clap

**Files:**
- Modify: `src/main.rs`

**Step 1: Replace hardcoded data_dir with clap**

Replace the entire `src/main.rs` with:

```rust
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use rmcp::{transport::stdio, ServiceExt};
use tracing_subscriber::EnvFilter;

use teidelum::api::TeidelumApi;
use teidelum::catalog::Relationship;
use teidelum::mcp::Teidelum;

#[derive(Parser)]
#[command(name = "teidelum", about = "Local-first MCP server with REST API")]
struct Cli {
    /// Enable HTTP server on this port
    #[arg(long)]
    port: Option<u16>,

    /// Bind address for the HTTP server
    #[arg(long, default_value = "127.0.0.1")]
    bind: String,

    /// Data directory
    #[arg(long, env = "TEIDELUM_DATA", default_value = "data")]
    data: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Tracing must go to stderr — stdout is the MCP stdio transport.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let cli = Cli::parse();

    // Generate demo data if not present
    if !cli.data.join("tables").exists() || !cli.data.join("docs").exists() {
        tracing::info!("generating demo data...");
        teidelum::demo::generate(&cli.data)?;
    }

    let api = TeidelumApi::open(&cli.data)?;

    // Register FK relationships between demo tables
    api.register_relationships(vec![
        Relationship {
            from_table: "project_tasks".to_string(),
            from_col: "assignee".to_string(),
            to_table: "team_members".to_string(),
            to_col: "name".to_string(),
            relation: "assigned_to".to_string(),
        },
        Relationship {
            from_table: "incidents".to_string(),
            from_col: "reporter".to_string(),
            to_table: "team_members".to_string(),
            to_col: "name".to_string(),
            relation: "reported_by".to_string(),
        },
    ])?;

    if let Some(port) = cli.port {
        tracing::info!("starting HTTP server on {}:{}", cli.bind, port);
        // TODO: start HTTP server (Task 9)
        todo!("HTTP server not yet implemented");
    } else {
        tracing::info!("teidelum ready — serving MCP over stdio");
        let server = Teidelum::new(api);
        let service = server.serve(stdio()).await.inspect_err(|e| {
            tracing::error!("serving error: {:?}", e);
        })?;
        service.waiting().await?;
    }

    Ok(())
}
```

**Step 2: Verify it compiles**

Run: `cargo check`
Expected: compiles. The `todo!` in the HTTP branch is intentional — it will be replaced in Task 9.

**Step 3: Verify existing behavior is preserved**

Run: `cargo test`
Expected: ALL PASS (main.rs has no tests, but this checks the binary compiles)

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat(cli): add clap CLI args (--port, --bind, --data)"
```

---

### Task 8: Create HTTP Server Module

**Files:**
- Create: `src/server.rs`
- Modify: `src/lib.rs` (add module declaration)

**Step 1: Register module in lib.rs**

Add to `src/lib.rs`:

```rust
pub mod server;
```

**Step 2: Create `src/server.rs`**

```rust
use std::sync::Arc;

use axum::{
    extract::Request,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    Router,
};
use http::StatusCode;
use tower_http::cors::CorsLayer;

use crate::api::TeidelumApi;
use crate::routes;

/// Build the axum router with all routes, CORS, and optional auth.
pub fn build_router(api: Arc<TeidelumApi>) -> Router {
    let mut app = Router::new()
        .merge(routes::api_routes())
        .with_state(api)
        .layer(CorsLayer::permissive());

    // If TEIDELUM_API_KEY is set, wrap all routes with auth middleware
    if std::env::var("TEIDELUM_API_KEY").is_ok() {
        app = app.layer(middleware::from_fn(auth_middleware));
    }

    app
}

/// Auth middleware: requires `Authorization: Bearer <key>` matching TEIDELUM_API_KEY.
async fn auth_middleware(request: Request, next: Next) -> Response {
    let expected = match std::env::var("TEIDELUM_API_KEY") {
        Ok(key) if !key.is_empty() => key,
        _ => return next.run(request).await,
    };

    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(header) if header.starts_with("Bearer ") => {
            let token = &header[7..];
            if token == expected {
                next.run(request).await
            } else {
                (
                    StatusCode::UNAUTHORIZED,
                    axum::Json(serde_json::json!({"error": "invalid or missing API key"})),
                )
                    .into_response()
            }
        }
        _ => (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({"error": "invalid or missing API key"})),
        )
            .into_response(),
    }
}

/// Start the HTTP server on the given address.
pub async fn start(api: Arc<TeidelumApi>, bind: &str, port: u16) -> anyhow::Result<()> {
    let app = build_router(api);
    let addr = format!("{bind}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("HTTP server listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: FAIL — `crate::routes` doesn't exist yet. That's expected. We'll fix in next task.

**Step 4: Commit (partial — will finish after routes)**

Don't commit yet — wait until routes module is done.

---

### Task 9: Create REST Routes Module

**Files:**
- Create: `src/routes.rs`
- Modify: `src/lib.rs` (add module declaration)

**Step 1: Register module in lib.rs**

Add to `src/lib.rs`:

```rust
pub mod routes;
```

**Step 2: Create `src/routes.rs`**

```rust
use std::sync::Arc;

use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
use http::StatusCode;
use serde::Deserialize;

use crate::api::TeidelumApi;
use crate::catalog::Relationship;
use crate::connector::{ColumnSchema, Value};
use crate::search::SearchQuery;
use crate::sync::SearchDocument;

type AppState = Arc<TeidelumApi>;

/// Build the API routes under /api/v1/.
pub fn api_routes() -> Router<AppState> {
    Router::new()
        // Read
        .route("/api/v1/search", post(search_handler))
        .route("/api/v1/sql", post(sql_handler))
        .route("/api/v1/describe", get(describe_handler))
        .route("/api/v1/describe/{source}", get(describe_source_handler))
        .route("/api/v1/graph/neighbors", post(neighbors_handler))
        .route("/api/v1/graph/path", post(path_handler))
        // Write
        .route("/api/v1/tables", post(create_table_handler))
        .route("/api/v1/tables/{name}/rows", post(insert_rows_handler))
        .route("/api/v1/tables/{name}", delete(delete_table_handler))
        .route("/api/v1/documents", post(add_documents_handler))
        .route("/api/v1/documents/{id}", delete(delete_document_handler))
        .route("/api/v1/relationships", post(add_relationship_handler))
}

// --- Request types ---

#[derive(Deserialize)]
struct SearchRequest {
    query: String,
    #[serde(default)]
    sources: Option<Vec<String>>,
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    10
}

#[derive(Deserialize)]
struct SqlRequest {
    query: String,
}

#[derive(Deserialize)]
struct NeighborsRequest {
    table: String,
    key: String,
    #[serde(default = "default_key_col")]
    key_col: String,
    #[serde(default = "default_depth")]
    depth: usize,
    #[serde(default = "default_direction")]
    direction: String,
    #[serde(default)]
    rel_types: Option<Vec<String>>,
}

fn default_key_col() -> String {
    "name".to_string()
}
fn default_depth() -> usize {
    2
}
fn default_direction() -> String {
    "both".to_string()
}

#[derive(Deserialize)]
struct PathRequest {
    table: String,
    key: String,
    #[serde(default = "default_key_col")]
    key_col: String,
    to_table: String,
    to_key: String,
    #[serde(default)]
    to_key_col: Option<String>,
    #[serde(default = "default_depth")]
    depth: usize,
    #[serde(default = "default_direction")]
    direction: String,
    #[serde(default)]
    rel_types: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct CreateTableRequest {
    name: String,
    source: String,
    columns: Vec<ColumnDefRequest>,
    #[serde(default)]
    rows: Vec<Vec<serde_json::Value>>,
}

#[derive(Deserialize)]
struct ColumnDefRequest {
    name: String,
    #[serde(rename = "type")]
    dtype: String,
}

#[derive(Deserialize)]
struct InsertRowsRequest {
    rows: Vec<Vec<serde_json::Value>>,
}

#[derive(Deserialize)]
struct AddDocumentsRequest {
    documents: Vec<DocumentRequest>,
}

#[derive(Deserialize)]
struct DocumentRequest {
    id: String,
    source: String,
    title: String,
    body: String,
}

#[derive(Deserialize)]
struct AddRelationshipRequest {
    from_table: String,
    from_col: String,
    to_table: String,
    to_col: String,
    relation: String,
}

// --- Handlers ---

async fn search_handler(
    State(api): State<AppState>,
    Json(req): Json<SearchRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let query = SearchQuery {
        text: req.query,
        sources: req.sources,
        limit: req.limit,
        date_from: None,
        date_to: None,
    };
    let results = api.search(&query).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(results).unwrap()))
}

async fn sql_handler(
    State(api): State<AppState>,
    Json(req): Json<SqlRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let result = api.query(&req.query).map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

async fn describe_handler(
    State(api): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let desc = api.describe(None).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(desc))
}

async fn describe_source_handler(
    State(api): State<AppState>,
    Path(source): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let desc = api
        .describe(Some(&source))
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(desc))
}

async fn neighbors_handler(
    State(api): State<AppState>,
    Json(req): Json<NeighborsRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let result = api
        .neighbors(
            &req.table,
            &req.key_col,
            &req.key,
            req.depth,
            &req.direction,
            req.rel_types.as_deref(),
        )
        .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    Ok(Json(result))
}

async fn path_handler(
    State(api): State<AppState>,
    Json(req): Json<PathRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let to_key_col = req.to_key_col.as_deref().unwrap_or(&req.key_col);
    let result = api
        .path(
            &req.table,
            &req.key_col,
            &req.key,
            &req.to_table,
            to_key_col,
            &req.to_key,
            req.depth,
            &req.direction,
            req.rel_types.as_deref(),
        )
        .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    Ok(Json(result))
}

async fn create_table_handler(
    State(api): State<AppState>,
    Json(req): Json<CreateTableRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let columns: Vec<ColumnSchema> = req
        .columns
        .iter()
        .map(|c| ColumnSchema {
            name: c.name.clone(),
            dtype: map_dtype(&c.dtype).to_string(),
        })
        .collect();

    let rows: Vec<Vec<Value>> = req
        .rows
        .iter()
        .map(|row| row.iter().zip(columns.iter()).map(|(v, c)| json_to_value(v, &c.dtype)).collect())
        .collect();

    let row_count = rows.len();

    api.create_table(&req.name, &req.source, &columns, &rows)
        .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({"table": req.name, "rows_inserted": row_count})),
    ))
}

async fn insert_rows_handler(
    State(api): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<InsertRowsRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Look up columns from catalog
    let desc = api.describe(None).map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let tables = desc["tables"].as_array().ok_or_else(|| {
        err(
            StatusCode::INTERNAL_SERVER_ERROR,
            anyhow::anyhow!("unexpected catalog format"),
        )
    })?;
    let table_entry = tables
        .iter()
        .find(|t| t["name"].as_str() == Some(&name))
        .ok_or_else(|| err(StatusCode::NOT_FOUND, anyhow::anyhow!("table '{name}' not found")))?;

    let columns: Vec<ColumnSchema> = table_entry["columns"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .map(|c| ColumnSchema {
            name: c["name"].as_str().unwrap_or("").to_string(),
            dtype: c["dtype"].as_str().unwrap_or("string").to_string(),
        })
        .collect();

    let rows: Vec<Vec<Value>> = req
        .rows
        .iter()
        .map(|row| row.iter().zip(columns.iter()).map(|(v, c)| json_to_value(v, &c.dtype)).collect())
        .collect();

    let row_count = rows.len();

    api.insert_rows(&name, &columns, &rows)
        .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;

    Ok(Json(
        serde_json::json!({"table": name, "rows_inserted": row_count}),
    ))
}

async fn delete_table_handler(
    State(api): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    api.delete_table(&name)
        .map_err(|e| err(StatusCode::NOT_FOUND, e))?;
    Ok(Json(serde_json::json!({"deleted": name})))
}

async fn add_documents_handler(
    State(api): State<AppState>,
    Json(req): Json<AddDocumentsRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let docs: Vec<SearchDocument> = req
        .documents
        .into_iter()
        .map(|d| SearchDocument {
            id: d.id,
            source: d.source,
            title: d.title,
            body: d.body,
            metadata: serde_json::Map::new(),
        })
        .collect();

    let count = api
        .add_documents(&docs)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({"documents_indexed": count})),
    ))
}

async fn delete_document_handler(
    State(api): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    api.delete_documents(&[id.clone()])
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::json!({"deleted": id})))
}

async fn add_relationship_handler(
    State(api): State<AppState>,
    Json(req): Json<AddRelationshipRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let desc = format!(
        "{}.{} -> {}.{}",
        req.from_table, req.from_col, req.to_table, req.to_col
    );

    api.register_relationship(Relationship {
        from_table: req.from_table,
        from_col: req.from_col,
        to_table: req.to_table,
        to_col: req.to_col,
        relation: req.relation,
    })
    .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({"relationship": desc})),
    ))
}

// --- Helpers ---

fn err(
    status: StatusCode,
    e: anyhow::Error,
) -> (StatusCode, Json<serde_json::Value>) {
    (status, Json(serde_json::json!({"error": e.to_string()})))
}

fn json_to_value(v: &serde_json::Value, dtype: &str) -> Value {
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if dtype == "f64" || dtype == "double" || dtype == "float" {
                Value::Float(n.as_f64().unwrap_or(0.0))
            } else {
                Value::Int(n.as_i64().unwrap_or(0))
            }
        }
        serde_json::Value::String(s) => Value::String(s.clone()),
        _ => Value::String(v.to_string()),
    }
}

fn map_dtype(t: &str) -> &str {
    match t {
        "int" | "integer" | "bigint" => "i64",
        "varchar" | "text" | "string" => "string",
        "double" | "float" | "real" => "f64",
        "boolean" | "bool" => "bool",
        "date" => "date",
        "time" => "time",
        "timestamp" | "datetime" => "timestamp",
        other => other,
    }
}
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: compiles

**Step 4: Commit**

```bash
git add src/lib.rs src/server.rs src/routes.rs
git commit -m "feat: add HTTP server module and REST API routes"
```

---

### Task 10: Wire Up HTTP Server in main.rs

**Files:**
- Modify: `src/main.rs` (replace `todo!` with actual server startup)

**Step 1: Replace the `todo!` block**

In `src/main.rs`, replace:

```rust
    if let Some(port) = cli.port {
        tracing::info!("starting HTTP server on {}:{}", cli.bind, port);
        // TODO: start HTTP server (Task 9)
        todo!("HTTP server not yet implemented");
    } else {
```

with:

```rust
    if let Some(port) = cli.port {
        // HTTP mode: run REST API + stdio MCP in parallel
        let api = Arc::new(api);

        let server = Teidelum::new_with_shared(api.clone());

        let http_handle = tokio::spawn({
            let api = api.clone();
            let bind = cli.bind.clone();
            async move {
                teidelum::server::start(api, &bind, port).await
            }
        });

        tracing::info!("teidelum ready — serving MCP over stdio + HTTP on {}:{}", cli.bind, port);

        let mcp_handle = tokio::spawn(async move {
            let service = server.serve(stdio()).await.inspect_err(|e| {
                tracing::error!("MCP serving error: {:?}", e);
            })?;
            service.waiting().await?;
            Ok::<_, anyhow::Error>(())
        });

        tokio::select! {
            r = http_handle => r??,
            r = mcp_handle => r??,
        }
    } else {
```

This also requires adding `Arc` import and a `new_with_shared` method to `Teidelum`.

Add to the top of main.rs:

```rust
use std::sync::Arc;
```

**Step 2: Add `new_with_shared` to Teidelum (mcp.rs)**

In `src/mcp.rs`, inside the `#[tool_router] impl Teidelum` block, add after `new`:

```rust
    pub fn new_with_shared(api: Arc<TeidelumApi>) -> Self {
        Self {
            api,
            tool_router: Self::tool_router(),
        }
    }
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: compiles

**Step 4: Run all tests**

Run: `cargo test`
Expected: ALL PASS

**Step 5: Commit**

```bash
git add src/main.rs src/mcp.rs
git commit -m "feat: wire up HTTP server alongside stdio MCP in main"
```

---

### Task 11: Integration Tests for HTTP Endpoints

**Files:**
- Create: `tests/http_api.rs`

**Step 1: Create integration test file**

```rust
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use teidelum::api::TeidelumApi;
use teidelum::server::build_router;

fn test_app() -> axum::Router {
    let tmp = tempfile::tempdir().unwrap();
    let api = TeidelumApi::new(tmp.path()).unwrap();
    build_router(Arc::new(api))
}

fn json_request(method: &str, uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

#[tokio::test]
async fn test_create_table_and_query() {
    let app = test_app();

    // Create table
    let req = json_request(
        "POST",
        "/api/v1/tables",
        serde_json::json!({
            "name": "users",
            "source": "test",
            "columns": [
                {"name": "id", "type": "int"},
                {"name": "name", "type": "varchar"}
            ],
            "rows": [[1, "Alice"], [2, "Bob"]]
        }),
    );
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["table"], "users");
    assert_eq!(json["rows_inserted"], 2);

    // Query via SQL
    let req = json_request(
        "POST",
        "/api/v1/sql",
        serde_json::json!({"query": "SELECT name FROM users WHERE id = 1"}),
    );
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Describe
    let req = Request::builder()
        .uri("/api/v1/describe")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["tables"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_insert_rows() {
    let app = test_app();

    // Create table first
    let req = json_request(
        "POST",
        "/api/v1/tables",
        serde_json::json!({
            "name": "items",
            "source": "test",
            "columns": [{"name": "id", "type": "int"}, {"name": "label", "type": "varchar"}],
            "rows": [[1, "first"]]
        }),
    );
    app.clone().oneshot(req).await.unwrap();

    // Insert more rows
    let req = json_request(
        "POST",
        "/api/v1/tables/items/rows",
        serde_json::json!({"rows": [[2, "second"], [3, "third"]]}),
    );
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["rows_inserted"], 2);
}

#[tokio::test]
async fn test_delete_table() {
    let app = test_app();

    // Create then delete
    let req = json_request(
        "POST",
        "/api/v1/tables",
        serde_json::json!({
            "name": "ephemeral",
            "source": "test",
            "columns": [{"name": "id", "type": "int"}],
            "rows": []
        }),
    );
    app.clone().oneshot(req).await.unwrap();

    let req = Request::builder()
        .method("DELETE")
        .uri("/api/v1/tables/ephemeral")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_add_and_search_documents() {
    let app = test_app();

    // Add documents
    let req = json_request(
        "POST",
        "/api/v1/documents",
        serde_json::json!({
            "documents": [
                {"id": "doc1", "source": "test", "title": "Auth Guide", "body": "JWT authentication tokens"}
            ]
        }),
    );
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Search
    let req = json_request(
        "POST",
        "/api/v1/search",
        serde_json::json!({"query": "JWT authentication", "limit": 5}),
    );
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_add_relationship() {
    let app = test_app();

    let req = json_request(
        "POST",
        "/api/v1/relationships",
        serde_json::json!({
            "from_table": "orders",
            "from_col": "customer_id",
            "to_table": "customers",
            "to_col": "id",
            "relation": "belongs_to"
        }),
    );
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_auth_required_when_key_set() {
    // Set API key for this test
    std::env::set_var("TEIDELUM_API_KEY", "test-secret-key");

    let tmp = tempfile::tempdir().unwrap();
    let api = TeidelumApi::new(tmp.path()).unwrap();
    let app = build_router(Arc::new(api));

    // No auth header → 401
    let req = Request::builder()
        .uri("/api/v1/describe")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Wrong key → 401
    let req = Request::builder()
        .uri("/api/v1/describe")
        .header("authorization", "Bearer wrong-key")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Correct key → 200
    let req = Request::builder()
        .uri("/api/v1/describe")
        .header("authorization", "Bearer test-secret-key")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Clean up
    std::env::remove_var("TEIDELUM_API_KEY");
}
```

**Step 2: Add dev-dependencies**

In `Cargo.toml`, update `[dev-dependencies]`:

```toml
[dev-dependencies]
tempfile = "3"
tower = { version = "0.5", features = ["util"] }
http-body-util = "0.1"
```

**Step 3: Run integration tests**

Run: `cargo test --test http_api`
Expected: ALL PASS

Note: The auth test uses `set_var`/`remove_var` which is not thread-safe. Run with `-- --test-threads=1` if needed:
Run: `cargo test --test http_api -- --test-threads=1`

**Step 4: Commit**

```bash
git add tests/http_api.rs Cargo.toml
git commit -m "test: add HTTP API integration tests"
```

---

### Task 12: Update Website Docs — API Reference Page

**Files:**
- Create: `website/docs/api-reference.html`
- Modify: `website/docs/index.html` (add link)
- Modify all docs pages (add sidebar link)

Add a new docs page documenting the REST API endpoints with request/response examples. Follow the existing docs page pattern (use `docs.css`, `docs.js`, same sidebar structure). Add the link to the docs sidebar on all pages:

```html
<a href="api-reference.html" class="sidebar-link">API Reference</a>
```

Add to the docs index page overview section.

**Step 1: Create the page and update links**

**Step 2: Verify all links work**

Open each docs page in a browser and verify the sidebar link is present and points correctly.

**Step 3: Commit**

```bash
git add website/docs/
git commit -m "docs(website): add API reference page"
```

---

### Task 13: Final Verification

**Step 1: Run full test suite**

Run: `cargo test`
Expected: ALL PASS

**Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: no warnings

**Step 3: Check formatting**

Run: `cargo fmt --check`
Expected: no changes needed

**Step 4: Manual smoke test**

Run: `cargo run -- --port 8080 --data /tmp/teidelum-test`

In another terminal:

```bash
# Create table
curl -X POST http://localhost:8080/api/v1/tables \
  -H "Content-Type: application/json" \
  -d '{"name":"test","source":"api","columns":[{"name":"id","type":"int"},{"name":"val","type":"varchar"}],"rows":[[1,"hello"]]}'

# Query
curl -X POST http://localhost:8080/api/v1/sql \
  -H "Content-Type: application/json" \
  -d '{"query":"SELECT * FROM test"}'

# Describe
curl http://localhost:8080/api/v1/describe

# Add document
curl -X POST http://localhost:8080/api/v1/documents \
  -H "Content-Type: application/json" \
  -d '{"documents":[{"id":"d1","source":"api","title":"Test","body":"test document"}]}'

# Search
curl -X POST http://localhost:8080/api/v1/search \
  -H "Content-Type: application/json" \
  -d '{"query":"test","limit":5}'

# Delete table
curl -X DELETE http://localhost:8080/api/v1/tables/test
```

Expected: All return valid JSON responses with correct status codes.

**Step 5: Commit any fixes**

```bash
git add -A
git commit -m "chore: final cleanup for API server"
```
