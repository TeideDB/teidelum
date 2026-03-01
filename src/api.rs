use std::path::Path;
use std::sync::{Arc, RwLock};

use anyhow::{bail, Result};
use serde_json::Value as JsonValue;

use crate::catalog::{
    is_valid_identifier, Catalog, ColumnInfo, Relationship, StorageType, TableEntry,
};
use crate::connector::{ColumnSchema, QueryResult, Value};
use crate::graph::GraphEngine;
use crate::router::QueryRouter;
use crate::search::{SearchEngine, SearchQuery, SearchResult};
use crate::sync::SearchDocument;

/// Unified programmatic API for Teidelum.
///
/// Wraps all subsystems (catalog, search, query router, graph engine) behind
/// a single thread-safe interface. The MCP server and all tests delegate here.
pub struct TeidelumApi {
    catalog: RwLock<Catalog>,
    search_engine: Arc<SearchEngine>,
    query_router: Arc<QueryRouter>,
    graph_engine: RwLock<GraphEngine>,
}

/// Map connector dtype strings to SQL type names.
fn dtype_to_sql(dtype: &str) -> &str {
    match dtype {
        "bool" => "BOOLEAN",
        "i32" | "i64" => "BIGINT",
        "f64" => "DOUBLE",
        "string" => "VARCHAR",
        "date" => "DATE",
        "time" => "TIME",
        "timestamp" => "TIMESTAMP",
        _ => "VARCHAR",
    }
}

/// Format a row of Values as a SQL VALUES tuple: ('val1', 42, 3.14, NULL)
fn row_to_sql_values(row: &[Value]) -> String {
    let parts: Vec<String> = row
        .iter()
        .map(|v| match v {
            Value::Null => "NULL".to_string(),
            Value::Bool(b) => {
                if *b {
                    "TRUE".to_string()
                } else {
                    "FALSE".to_string()
                }
            }
            Value::Int(i) => i.to_string(),
            Value::Float(f) => {
                if f.is_finite() {
                    format!("{f}")
                } else {
                    "NULL".to_string()
                }
            }
            Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        })
        .collect();
    format!("({})", parts.join(", "))
}

/// Validate that a string is a safe SQL identifier, returning an error if not.
fn validate_identifier(s: &str) -> Result<()> {
    if !is_valid_identifier(s) {
        bail!("invalid identifier: '{s}'");
    }
    Ok(())
}

impl TeidelumApi {
    /// Create an empty instance with no data.
    pub fn new(data_dir: &Path) -> Result<Self> {
        let search_engine = SearchEngine::open(&data_dir.join("index"))?;
        let query_router = QueryRouter::new()?;
        let catalog = Catalog::new();
        let graph_engine = GraphEngine::build_from_catalog(&catalog);

        Ok(Self {
            catalog: RwLock::new(catalog),
            search_engine: Arc::new(search_engine),
            query_router: Arc::new(query_router),
            graph_engine: RwLock::new(graph_engine),
        })
    }

    /// Open an existing data directory, loading splayed tables and indexing markdown docs.
    pub fn open(data_dir: &Path) -> Result<Self> {
        let api = Self::new(data_dir)?;
        api.load_splayed_tables(&data_dir.join("tables"))?;
        api.index_markdown_dir(&data_dir.join("docs"))?;
        Ok(api)
    }

    /// Create a table from column schemas and row data via SQL DDL + INSERT.
    pub fn create_table(
        &self,
        name: &str,
        source: &str,
        columns: &[ColumnSchema],
        rows: &[Vec<Value>],
    ) -> Result<()> {
        validate_identifier(name)?;

        if columns.is_empty() {
            bail!("table must have at least one column");
        }

        // Validate row widths match column count
        for (i, row) in rows.iter().enumerate() {
            if row.len() != columns.len() {
                bail!(
                    "row {i} has {} values but {} columns defined",
                    row.len(),
                    columns.len()
                );
            }
        }

        // Build CREATE TABLE statement
        let col_defs: Vec<String> = columns
            .iter()
            .map(|c| {
                validate_identifier(&c.name)?;
                Ok(format!("{} {}", c.name, dtype_to_sql(&c.dtype)))
            })
            .collect::<Result<Vec<_>>>()?;

        let create_sql = format!(
            "CREATE TABLE {name} ({col_defs})",
            col_defs = col_defs.join(", ")
        );
        self.query_router.query_sync(&create_sql)?;

        // Register in catalog before inserting rows so the table is always visible
        let catalog_columns: Vec<ColumnInfo> = columns
            .iter()
            .map(|c| ColumnInfo {
                name: c.name.clone(),
                dtype: c.dtype.clone(),
            })
            .collect();

        {
            let mut catalog = self.catalog.write().unwrap();
            catalog.register_table(TableEntry {
                name: name.to_string(),
                source: source.to_string(),
                storage: StorageType::Local,
                columns: catalog_columns,
                row_count: Some(rows.len() as u64),
            });
        }

        // Insert rows in batches of 1000
        if !rows.is_empty() {
            for chunk in rows.chunks(1000) {
                let values: Vec<String> = chunk.iter().map(|row| row_to_sql_values(row)).collect();
                let col_names: Vec<&str> = columns.iter().map(|c| c.name.as_str()).collect();
                let insert_sql = format!(
                    "INSERT INTO {name} ({cols}) VALUES {vals}",
                    cols = col_names.join(", "),
                    vals = values.join(", "),
                );
                self.query_router.query_sync(&insert_sql)?;
            }
        }

        Ok(())
    }

    /// Index search documents into the full-text search engine.
    pub fn add_documents(&self, docs: &[SearchDocument]) -> Result<usize> {
        let tuples: Vec<(String, String, String, String)> = docs
            .iter()
            .map(|d| {
                (
                    d.id.clone(),
                    d.source.clone(),
                    d.title.clone(),
                    d.body.clone(),
                )
            })
            .collect();
        self.search_engine.index_documents(&tuples)
    }

    /// Execute a SQL query.
    pub fn query(&self, sql: &str) -> Result<QueryResult> {
        self.query_router.query_sync(sql)
    }

    /// Run a full-text search.
    pub fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        self.search_engine.search(query)
    }

    /// Register a relationship and rebuild the graph engine.
    pub fn register_relationship(&self, rel: Relationship) -> Result<()> {
        let mut catalog = self.catalog.write().unwrap();
        catalog.register_relationship(rel)?;
        let graph = GraphEngine::build_from_catalog(&catalog);
        *self.graph_engine.write().unwrap() = graph;
        Ok(())
    }

    /// Register multiple relationships in bulk, rebuilding the graph engine once.
    pub fn register_relationships(&self, rels: Vec<Relationship>) -> Result<()> {
        let mut catalog = self.catalog.write().unwrap();
        for rel in rels {
            catalog.register_relationship(rel)?;
        }
        let graph = GraphEngine::build_from_catalog(&catalog);
        *self.graph_engine.write().unwrap() = graph;
        Ok(())
    }

    /// Produce a JSON description of the catalog.
    pub fn describe(&self, source_filter: Option<&str>) -> Result<JsonValue> {
        let catalog = self.catalog.read().unwrap();
        catalog.describe(source_filter)
    }

    /// Find neighbors of a node in the graph.
    #[allow(clippy::too_many_arguments)]
    pub fn neighbors(
        &self,
        table: &str,
        key_col: &str,
        key_value: &str,
        depth: usize,
        direction: &str,
        rel_types: Option<&[String]>,
    ) -> Result<JsonValue> {
        let graph = self.graph_engine.read().unwrap();
        graph.neighbors(
            table,
            key_col,
            key_value,
            depth,
            direction,
            rel_types,
            &self.query_router,
        )
    }

    /// Find a path between two nodes in the graph.
    #[allow(clippy::too_many_arguments)]
    pub fn path(
        &self,
        table: &str,
        key_col: &str,
        key_value: &str,
        to_table: &str,
        to_key_col: &str,
        to_key: &str,
        depth: usize,
        direction: &str,
        rel_types: Option<&[String]>,
    ) -> Result<JsonValue> {
        let graph = self.graph_engine.read().unwrap();
        graph.path(
            table,
            key_col,
            key_value,
            to_table,
            to_key_col,
            to_key,
            depth,
            direction,
            rel_types,
            &self.query_router,
        )
    }

    /// Load all splayed tables from a directory.
    fn load_splayed_tables(&self, tables_dir: &Path) -> Result<()> {
        if !tables_dir.exists() {
            return Ok(());
        }

        let sym_path = tables_dir.join("sym");
        let sym = if sym_path.exists() {
            Some(sym_path.as_path())
        } else {
            None
        };

        for entry in std::fs::read_dir(tables_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() && path.join(".d").exists() {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                if validate_identifier(&name).is_err() {
                    tracing::warn!("skipping directory with invalid name: {name}");
                    continue;
                }
                self.query_router.load_splayed(&name, &path, sym)?;

                if let Some((nrows, _ncols)) = self.query_router.table_info(&name) {
                    let result = self
                        .query_router
                        .query_sync(&format!("SELECT * FROM {name} LIMIT 1"))?;
                    let columns = result
                        .columns
                        .iter()
                        .map(|c| ColumnInfo {
                            name: c.name.clone(),
                            dtype: c.dtype.clone(),
                        })
                        .collect::<Vec<_>>();

                    let mut catalog = self.catalog.write().unwrap();
                    catalog.register_table(TableEntry {
                        name: name.clone(),
                        source: "demo".to_string(),
                        storage: StorageType::Local,
                        columns,
                        row_count: Some(nrows as u64),
                    });

                    tracing::info!("registered table: {name} ({nrows} rows)");
                }
            }
        }

        // Rebuild graph engine after loading tables
        let catalog = self.catalog.read().unwrap();
        let graph = GraphEngine::build_from_catalog(&catalog);
        drop(catalog);
        *self.graph_engine.write().unwrap() = graph;

        Ok(())
    }

    /// Index all markdown files from a directory.
    fn index_markdown_dir(&self, docs_dir: &Path) -> Result<()> {
        if !docs_dir.exists() {
            return Ok(());
        }

        let mut documents = Vec::new();

        for entry in std::fs::read_dir(docs_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "md") {
                let content = std::fs::read_to_string(&path)?;
                let filename = path.file_stem().unwrap().to_string_lossy().to_string();

                let title = content
                    .lines()
                    .find(|l| l.starts_with("# "))
                    .map(|l| l.trim_start_matches("# ").to_string())
                    .unwrap_or_else(|| filename.clone());

                let source = if content.contains("zulip")
                    || filename.contains("zulip")
                    || filename.contains("standup")
                    || filename.contains("incident")
                {
                    "zulip"
                } else {
                    "notion"
                };

                documents.push((filename, source.to_string(), title, content));
            }
        }

        let count = self.search_engine.index_documents(&documents)?;

        tracing::info!("indexed {count} documents for full-text search");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_api(tmp: &Path) -> TeidelumApi {
        crate::demo::generate(tmp).unwrap();
        let api = TeidelumApi::open(tmp).unwrap();
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
        ])
        .unwrap();
        api
    }

    #[test]
    fn test_create_table_and_query() {
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
        let rows = vec![
            vec![Value::Int(1), Value::String("Alice".to_string())],
            vec![Value::Int(2), Value::String("Bob".to_string())],
        ];

        api.create_table("users", "test", &columns, &rows).unwrap();

        let result = api.query("SELECT name FROM users WHERE id = 1").unwrap();
        assert_eq!(result.rows.len(), 1);
        match &result.rows[0][0] {
            Value::String(s) => assert_eq!(s, "Alice"),
            other => panic!("expected string, got {other:?}"),
        }
    }

    #[test]
    fn test_create_table_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let api = TeidelumApi::new(tmp.path()).unwrap();

        let columns = vec![ColumnSchema {
            name: "id".to_string(),
            dtype: "i64".to_string(),
        }];

        api.create_table("empty_table", "test", &columns, &[])
            .unwrap();

        let result = api.query("SELECT * FROM empty_table").unwrap();
        assert_eq!(result.rows.len(), 0);
    }

    #[test]
    fn test_create_table_invalid_name() {
        let tmp = tempfile::tempdir().unwrap();
        let api = TeidelumApi::new(tmp.path()).unwrap();

        let columns = vec![ColumnSchema {
            name: "id".to_string(),
            dtype: "i64".to_string(),
        }];

        let result = api.create_table("'; DROP TABLE x;--", "test", &columns, &[]);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("invalid identifier"));
    }

    #[test]
    fn test_create_table_all_types() {
        let tmp = tempfile::tempdir().unwrap();
        let api = TeidelumApi::new(tmp.path()).unwrap();

        let columns = vec![
            ColumnSchema {
                name: "b".to_string(),
                dtype: "bool".to_string(),
            },
            ColumnSchema {
                name: "i".to_string(),
                dtype: "i64".to_string(),
            },
            ColumnSchema {
                name: "f".to_string(),
                dtype: "f64".to_string(),
            },
            ColumnSchema {
                name: "s".to_string(),
                dtype: "string".to_string(),
            },
        ];
        let rows = vec![vec![
            Value::Bool(true),
            Value::Int(42),
            Value::Float(3.14),
            Value::String("hello".to_string()),
        ]];

        api.create_table("typed", "test", &columns, &rows).unwrap();

        let result = api.query("SELECT * FROM typed").unwrap();
        assert_eq!(result.rows.len(), 1);

        let row = &result.rows[0];
        match &row[0] {
            Value::Bool(b) => assert!(b, "expected true"),
            other => panic!("expected Bool, got {other:?}"),
        }
        match &row[1] {
            Value::Int(i) => assert_eq!(*i, 42),
            other => panic!("expected Int, got {other:?}"),
        }
        match &row[2] {
            Value::Float(f) => assert!((f - 3.14).abs() < 0.001),
            other => panic!("expected Float, got {other:?}"),
        }
        match &row[3] {
            Value::String(s) => assert_eq!(s, "hello"),
            other => panic!("expected String, got {other:?}"),
        }
    }

    #[test]
    fn test_add_documents_and_search() {
        let tmp = tempfile::tempdir().unwrap();
        let api = TeidelumApi::new(tmp.path()).unwrap();

        let docs = vec![
            SearchDocument {
                id: "doc1".to_string(),
                source: "test".to_string(),
                title: "Authentication Guide".to_string(),
                body: "This document covers JWT authentication and token management".to_string(),
                metadata: serde_json::Map::new(),
            },
            SearchDocument {
                id: "doc2".to_string(),
                source: "test".to_string(),
                title: "Database Guide".to_string(),
                body: "This document covers database migrations and schema design".to_string(),
                metadata: serde_json::Map::new(),
            },
        ];

        let count = api.add_documents(&docs).unwrap();
        assert_eq!(count, 2);

        let results = api
            .search(&SearchQuery {
                text: "authentication JWT".to_string(),
                sources: None,
                limit: 10,
                date_from: None,
                date_to: None,
            })
            .unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].id, "doc1");
    }

    #[test]
    fn test_register_relationship_rebuilds_graph() {
        let tmp = tempfile::tempdir().unwrap();
        let api = TeidelumApi::new(tmp.path()).unwrap();

        // Create two tables
        let user_cols = vec![
            ColumnSchema {
                name: "id".to_string(),
                dtype: "i64".to_string(),
            },
            ColumnSchema {
                name: "name".to_string(),
                dtype: "string".to_string(),
            },
        ];
        let user_rows = vec![vec![Value::Int(1), Value::String("Alice".to_string())]];
        api.create_table("people", "test", &user_cols, &user_rows)
            .unwrap();

        let task_cols = vec![
            ColumnSchema {
                name: "id".to_string(),
                dtype: "i64".to_string(),
            },
            ColumnSchema {
                name: "title".to_string(),
                dtype: "string".to_string(),
            },
            ColumnSchema {
                name: "owner".to_string(),
                dtype: "string".to_string(),
            },
        ];
        let task_rows = vec![vec![
            Value::Int(1),
            Value::String("Fix bug".to_string()),
            Value::String("Alice".to_string()),
        ]];
        api.create_table("tasks", "test", &task_cols, &task_rows)
            .unwrap();

        // Register relationship
        api.register_relationship(Relationship {
            from_table: "tasks".to_string(),
            from_col: "owner".to_string(),
            to_table: "people".to_string(),
            to_col: "name".to_string(),
            relation: "owned_by".to_string(),
        })
        .unwrap();

        // Graph traversal should work
        let result = api
            .neighbors("tasks", "title", "Fix bug", 1, "forward", None)
            .unwrap();
        let edges = result["edges"].as_array().unwrap();
        assert!(!edges.is_empty());
    }

    #[test]
    fn test_register_relationships_bulk() {
        let tmp = tempfile::tempdir().unwrap();
        let api = TeidelumApi::new(tmp.path()).unwrap();

        api.register_relationships(vec![
            Relationship {
                from_table: "tasks".to_string(),
                from_col: "owner".to_string(),
                to_table: "people".to_string(),
                to_col: "name".to_string(),
                relation: "owned_by".to_string(),
            },
            Relationship {
                from_table: "bugs".to_string(),
                from_col: "reporter".to_string(),
                to_table: "people".to_string(),
                to_col: "name".to_string(),
                relation: "reported_by".to_string(),
            },
        ])
        .unwrap();

        let desc = api.describe(None).unwrap();
        let rels = desc["relationships"].as_array().unwrap();
        assert_eq!(rels.len(), 2);
    }

    #[test]
    fn test_describe_catalog() {
        let tmp = tempfile::tempdir().unwrap();
        let api = TeidelumApi::new(tmp.path()).unwrap();

        let columns = vec![ColumnSchema {
            name: "id".to_string(),
            dtype: "i64".to_string(),
        }];
        api.create_table("my_table", "test_source", &columns, &[])
            .unwrap();

        api.register_relationship(Relationship {
            from_table: "my_table".to_string(),
            from_col: "id".to_string(),
            to_table: "other_table".to_string(),
            to_col: "ref_id".to_string(),
            relation: "references".to_string(),
        })
        .unwrap();

        let desc = api.describe(None).unwrap();
        let tables = desc["tables"].as_array().unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0]["name"], "my_table");

        let rels = desc["relationships"].as_array().unwrap();
        assert_eq!(rels.len(), 1);

        // Filter by source
        let desc_filtered = api.describe(Some("test_source")).unwrap();
        let tables = desc_filtered["tables"].as_array().unwrap();
        assert_eq!(tables.len(), 1);

        let desc_empty = api.describe(Some("nonexistent")).unwrap();
        let tables = desc_empty["tables"].as_array().unwrap();
        assert!(tables.is_empty());
    }

    #[test]
    fn test_neighbors_via_api() {
        let tmp = tempfile::tempdir().unwrap();
        let api = test_api(tmp.path());

        let result = api
            .neighbors("team_members", "name", "Alice Chen", 1, "both", None)
            .unwrap();

        let nodes = result["nodes"].as_array().unwrap();
        let edges = result["edges"].as_array().unwrap();

        assert!(nodes.len() >= 2, "should find neighbors for Alice Chen");
        assert!(!edges.is_empty(), "should have edges");

        let has_alice = nodes.iter().any(|n| n["key"] == "Alice Chen");
        assert!(has_alice, "starting node should be in results");
    }

    #[test]
    fn test_path_via_api() {
        let tmp = tempfile::tempdir().unwrap();
        let api = test_api(tmp.path());

        // Find a task assigned to Alice Chen
        let result = api
            .query("SELECT title FROM project_tasks WHERE assignee = 'Alice Chen' LIMIT 1")
            .unwrap();
        if result.rows.is_empty() {
            return; // Alice has no tasks in this random seed
        }
        let task_title = match &result.rows[0][0] {
            Value::String(s) => s.clone(),
            _ => return,
        };

        let result = api
            .path(
                "project_tasks",
                "title",
                &task_title,
                "team_members",
                "name",
                "Alice Chen",
                5,
                "both",
                None,
            )
            .unwrap();

        assert_eq!(result["found"], true);
        let path = result["path"].as_array().unwrap();
        assert!(path.len() >= 2, "path should have at least 2 nodes");
        assert_eq!(result["hops"], 1);
    }

    #[test]
    fn test_open_with_demo_data() {
        let tmp = tempfile::tempdir().unwrap();
        crate::demo::generate(tmp.path()).unwrap();

        let api = TeidelumApi::open(tmp.path()).unwrap();

        // Tables should be loaded
        let desc = api.describe(None).unwrap();
        let tables = desc["tables"].as_array().unwrap();
        assert!(tables.len() >= 3, "should have at least 3 demo tables");

        // Search should work
        let results = api
            .search(&SearchQuery {
                text: "authentication".to_string(),
                sources: None,
                limit: 10,
                date_from: None,
                date_to: None,
            })
            .unwrap();
        assert!(
            !results.is_empty(),
            "should find documents about authentication"
        );

        // SQL should work
        let result = api.query("SELECT count(*) FROM team_members").unwrap();
        assert!(!result.rows.is_empty());
    }
}
