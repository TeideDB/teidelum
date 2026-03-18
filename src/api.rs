use std::path::Path;
use std::sync::{Arc, RwLock};

use anyhow::{bail, Result};
use serde_json::Value as JsonValue;

use crate::catalog::{
    is_valid_identifier, Catalog, ColumnInfo, Relationship, StorageType, TableEntry,
};
use crate::connector::{ColumnSchema, QueryResult, Value};
use crate::router::QueryRouter;
use crate::search::{SearchEngine, SearchQuery, SearchResult};
use crate::sync::SearchDocument;

/// Unified programmatic API for Teidelum.
///
/// Wraps all subsystems (catalog, search, query router) behind a single
/// thread-safe interface. The MCP server and all tests delegate here.
pub struct TeidelumApi {
    catalog: RwLock<Catalog>,
    search_engine: Arc<SearchEngine>,
    query_router: Arc<QueryRouter>,
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

        Ok(Self {
            catalog: RwLock::new(catalog),
            search_engine: Arc::new(search_engine),
            query_router: Arc::new(query_router),
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

        // Insert rows in batches of 1000; rollback (DROP) if any batch fails
        if !rows.is_empty() {
            if let Err(e) = self.insert_rows(name, columns, rows) {
                let _ = self
                    .query_router
                    .query_sync(&format!("DROP TABLE IF EXISTS {name}"));
                return Err(e);
            }
        }

        // Register in catalog after successful INSERT so metadata is always accurate
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

        Ok(())
    }

    /// Insert rows into an existing table in batches.
    pub fn insert_rows(
        &self,
        name: &str,
        columns: &[ColumnSchema],
        rows: &[Vec<Value>],
    ) -> Result<()> {
        validate_identifier(name)?;
        for c in columns {
            validate_identifier(&c.name)?;
        }
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

    /// Delete a table from the SQL engine and catalog.
    pub fn delete_table(&self, name: &str) -> Result<()> {
        validate_identifier(name)?;

        // Acquire write lock once to collect graph names and remove table atomically
        let graph_names: Vec<String>;
        {
            let mut catalog = self.catalog.write().unwrap();
            graph_names = catalog
                .relationships()
                .iter()
                .filter(|r| r.from_table == name || r.to_table == name)
                .map(|r| format!("pg_{}_{}_{}", r.from_table, r.to_table, r.relation))
                .collect();
            if !catalog.remove_table(name) {
                bail!("table '{name}' not found");
            }
        }

        // Drop associated property graphs
        for graph_name in &graph_names {
            if let Err(e) = self
                .query_router
                .query_sync(&format!("DROP PROPERTY GRAPH IF EXISTS {graph_name}"))
            {
                tracing::warn!("failed to drop property graph {graph_name}: {e}");
            }
        }

        // Drop from SQL engine (ignore errors if not present in SQL — could be remote-only)
        let _ = self.query_router.drop_table(name);

        Ok(())
    }

    /// Delete documents from the search index by their IDs.
    pub fn delete_documents(&self, ids: &[String]) -> Result<usize> {
        self.search_engine.delete_documents(ids)
    }

    /// Register a pre-built table entry in the catalog (e.g. for remote connectors).
    pub fn register_table(&self, entry: TableEntry) {
        let mut catalog = self.catalog.write().unwrap();
        catalog.register_table(entry);
    }

    /// Create a property graph for a catalog relationship.
    /// Graph name: pg_{from_table}_{to_table}_{relation}
    /// Uses the first column of from_table as the source vertex identity key.
    fn create_property_graph_for_relationship(&self, rel: &Relationship) {
        let graph_name = format!("pg_{}_{}_{}", rel.from_table, rel.to_table, rel.relation);

        // Look up the identity column (first column) of the from_table
        let from_id_col = {
            let catalog = self.catalog.read().unwrap();
            catalog
                .tables()
                .iter()
                .find(|t| t.name == rel.from_table)
                .and_then(|t| t.columns.first().map(|c| c.name.clone()))
        };
        let from_id_col = match from_id_col {
            Some(col) => col,
            None => {
                tracing::warn!(
                    "skipping property graph {graph_name}: table '{}' not in catalog",
                    rel.from_table
                );
                return;
            }
        };

        let vertex_clause = if rel.from_table == rel.to_table {
            format!("{} LABEL {}", rel.from_table, rel.from_table)
        } else {
            format!(
                "{from} LABEL {from}, {to} LABEL {to}",
                from = rel.from_table,
                to = rel.to_table
            )
        };

        let sql = format!(
            "CREATE PROPERTY GRAPH {graph_name} \
             VERTEX TABLES ({vertex_clause}) \
             EDGE TABLES ({from_table} \
               SOURCE KEY ({from_id_col}) REFERENCES {from_table} ({from_id_col}) \
               DESTINATION KEY ({from_col}) REFERENCES {to_table} ({to_col}) \
               LABEL {relation})",
            from_table = rel.from_table,
            to_table = rel.to_table,
            from_id_col = from_id_col,
            from_col = rel.from_col,
            to_col = rel.to_col,
            relation = rel.relation,
        );
        if let Err(e) = self.query_router.query_sync(&sql) {
            tracing::warn!("failed to create property graph {graph_name}: {e}");
        }
    }

    /// Execute a SQL query.
    pub fn query(&self, sql: &str) -> Result<QueryResult> {
        self.query_router.query_sync(sql)
    }

    /// Run a full-text search.
    pub fn search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        self.search_engine.search(query)
    }

    /// Register a relationship and create the corresponding property graph.
    pub fn register_relationship(&self, rel: Relationship) -> Result<()> {
        let mut catalog = self.catalog.write().unwrap();
        catalog.register_relationship(rel.clone())?;
        drop(catalog);
        self.create_property_graph_for_relationship(&rel);
        Ok(())
    }

    /// Register multiple relationships in bulk, creating property graphs for each.
    ///
    /// Validates all relationships before mutating the catalog, so a validation
    /// failure in any relationship leaves the catalog unchanged.
    pub fn register_relationships(&self, rels: Vec<Relationship>) -> Result<()> {
        // Validate all identifiers upfront to avoid partial catalog mutation
        for rel in &rels {
            for (label, val) in [
                ("from_table", &rel.from_table),
                ("from_col", &rel.from_col),
                ("to_table", &rel.to_table),
                ("to_col", &rel.to_col),
                ("relation", &rel.relation),
            ] {
                if !is_valid_identifier(val) {
                    bail!("invalid identifier in relationship {label}: '{val}'");
                }
            }
        }

        let rels_clone = rels.clone();
        let mut catalog = self.catalog.write().unwrap();
        for rel in rels {
            catalog.register_relationship(rel)?;
        }
        drop(catalog);
        for rel in &rels_clone {
            self.create_property_graph_for_relationship(rel);
        }
        Ok(())
    }

    /// Access the search engine (for sync and MCP delegation).
    pub fn search_engine(&self) -> &Arc<SearchEngine> {
        &self.search_engine
    }

    /// Access the query router (for sync and MCP delegation).
    pub fn query_router(&self) -> &Arc<QueryRouter> {
        &self.query_router
    }

    /// Produce a JSON description of the catalog.
    pub fn describe(&self, source_filter: Option<&str>) -> Result<JsonValue> {
        let catalog = self.catalog.read().unwrap();
        catalog.describe(source_filter)
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

        // Collect table info first (outside catalog lock), then register all at once.
        let mut table_entries = Vec::new();

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

                    table_entries.push((name, columns, nrows));
                }
            }
        }

        // Register all tables under a single write lock.
        if !table_entries.is_empty() {
            let mut catalog = self.catalog.write().unwrap();
            for (name, columns, nrows) in table_entries {
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
    fn test_register_relationship_creates_property_graph() {
        let tmp = tempfile::tempdir().unwrap();
        let api = TeidelumApi::new(tmp.path()).unwrap();

        // Create two tables with data
        let person_cols = vec![
            ColumnSchema {
                name: "id".to_string(),
                dtype: "i64".to_string(),
            },
            ColumnSchema {
                name: "name".to_string(),
                dtype: "string".to_string(),
            },
        ];
        api.create_table(
            "persons",
            "test",
            &person_cols,
            &[
                vec![Value::Int(0), Value::String("Alice".to_string())],
                vec![Value::Int(1), Value::String("Bob".to_string())],
                vec![Value::Int(2), Value::String("Carol".to_string())],
            ],
        )
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
                name: "assignee_id".to_string(),
                dtype: "i64".to_string(),
            },
        ];
        api.create_table(
            "tasks",
            "test",
            &task_cols,
            &[
                vec![
                    Value::Int(1),
                    Value::String("Fix bug".to_string()),
                    Value::Int(0),
                ],
                vec![
                    Value::Int(2),
                    Value::String("Add feature".to_string()),
                    Value::Int(1),
                ],
            ],
        )
        .unwrap();

        // Register relationship — should auto-create property graph
        api.register_relationship(Relationship {
            from_table: "tasks".to_string(),
            from_col: "assignee_id".to_string(),
            to_table: "persons".to_string(),
            to_col: "id".to_string(),
            relation: "assigned_to".to_string(),
        })
        .unwrap();

        // Property graph should be queryable via PGQ
        let result = api.query(
            "SELECT * FROM GRAPH_TABLE (pg_tasks_persons_assigned_to \
             MATCH (t:tasks)-[:assigned_to]->(p:persons WHERE p.name = 'Alice') \
             COLUMNS (t.title AS task_title))",
        );
        assert!(result.is_ok(), "PGQ query failed: {result:?}");
        let qr = result.unwrap();
        assert_eq!(qr.rows.len(), 1);
        match &qr.rows[0][0] {
            Value::String(s) => assert_eq!(s, "Fix bug"),
            other => panic!("expected String, got {other:?}"),
        }
    }

    #[test]
    fn test_register_relationship_works_after_open() {
        let tmp = tempfile::tempdir().unwrap();

        // First session: create tables, register relationships, save
        {
            let api = TeidelumApi::new(tmp.path()).unwrap();

            let person_cols = vec![
                ColumnSchema {
                    name: "id".to_string(),
                    dtype: "i64".to_string(),
                },
                ColumnSchema {
                    name: "name".to_string(),
                    dtype: "string".to_string(),
                },
            ];
            api.create_table(
                "people",
                "test",
                &person_cols,
                &[
                    vec![Value::Int(0), Value::String("Alice".to_string())],
                    vec![Value::Int(1), Value::String("Bob".to_string())],
                ],
            )
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
                    name: "owner_id".to_string(),
                    dtype: "i64".to_string(),
                },
            ];
            api.create_table(
                "work_items",
                "test",
                &task_cols,
                &[
                    vec![
                        Value::Int(1),
                        Value::String("Fix bug".to_string()),
                        Value::Int(0),
                    ],
                    vec![
                        Value::Int(2),
                        Value::String("Add feature".to_string()),
                        Value::Int(1),
                    ],
                ],
            )
            .unwrap();

            api.register_relationship(Relationship {
                from_table: "work_items".to_string(),
                from_col: "owner_id".to_string(),
                to_table: "people".to_string(),
                to_col: "id".to_string(),
                relation: "owned_by".to_string(),
            })
            .unwrap();

            // Save tables to disk
            let tables_dir = tmp.path().join("tables");
            std::fs::create_dir_all(&tables_dir).unwrap();
            api.query_router()
                .save_table("people", &tables_dir.join("people"))
                .unwrap();
            api.query_router()
                .save_table("work_items", &tables_dir.join("work_items"))
                .unwrap();
            api.query_router()
                .save_sym(&tables_dir.join("sym"))
                .unwrap();
        }

        // Second session: open from disk — property graphs should be recreated
        // after re-registering relationships (catalog relationships aren't persisted yet)
        let api = TeidelumApi::open(tmp.path()).unwrap();
        api.register_relationship(Relationship {
            from_table: "work_items".to_string(),
            from_col: "owner_id".to_string(),
            to_table: "people".to_string(),
            to_col: "id".to_string(),
            relation: "owned_by".to_string(),
        })
        .unwrap();

        let result = api.query(
            "SELECT * FROM GRAPH_TABLE (pg_work_items_people_owned_by \
             MATCH (w:work_items)-[:owned_by]->(p:people) \
             COLUMNS (p.name AS person))",
        );
        assert!(result.is_ok(), "PGQ query after open failed: {result:?}");
        let qr = result.unwrap();
        assert_eq!(qr.rows.len(), 2); // Both work items have owners
    }

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
        api.create_table(
            "ephemeral",
            "test",
            &columns,
            &[vec![Value::Int(1), Value::String("Alice".to_string())]],
        )
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

    #[test]
    fn test_pgq_full_workflow() {
        let tmp = tempfile::tempdir().unwrap();
        let api = TeidelumApi::new(tmp.path()).unwrap();

        // Create employees table (vertex)
        api.create_table(
            "employees",
            "test",
            &[
                ColumnSchema {
                    name: "id".to_string(),
                    dtype: "i64".to_string(),
                },
                ColumnSchema {
                    name: "name".to_string(),
                    dtype: "string".to_string(),
                },
                ColumnSchema {
                    name: "manager_id".to_string(),
                    dtype: "i64".to_string(),
                },
            ],
            &[
                vec![Value::Int(0), Value::String("Alice".to_string()), Value::Int(0)],
                vec![Value::Int(1), Value::String("Bob".to_string()), Value::Int(0)],
                vec![Value::Int(2), Value::String("Carol".to_string()), Value::Int(0)],
                vec![Value::Int(3), Value::String("Dave".to_string()), Value::Int(1)],
            ],
        )
        .unwrap();

        // Register self-referencing relationship — auto-creates property graph
        api.register_relationship(Relationship {
            from_table: "employees".to_string(),
            from_col: "manager_id".to_string(),
            to_table: "employees".to_string(),
            to_col: "id".to_string(),
            relation: "managed_by".to_string(),
        })
        .unwrap();

        // Verify describe includes property graph
        let desc = api.describe(None).unwrap();
        let graphs = desc["property_graphs"].as_array().unwrap();
        assert!(graphs
            .iter()
            .any(|g| g["name"] == "pg_employees_employees_managed_by"));

        // 1-hop MATCH: who is managed by Alice (id=0)?
        // Alice(0)->Alice(0) self-loop, Bob(1)->Alice(0), Carol(2)->Alice(0)
        let result = api
            .query(
                "SELECT * FROM GRAPH_TABLE (pg_employees_employees_managed_by \
                 MATCH (e1:employees)-[:managed_by]->(e2:employees WHERE e2.name = 'Alice') \
                 COLUMNS (e1.name AS subordinate))",
            )
            .unwrap();
        assert_eq!(result.rows.len(), 3); // Alice (self), Bob, Carol

        let names: Vec<String> = result
            .rows
            .iter()
            .map(|r| match &r[0] {
                Value::String(s) => s.clone(),
                other => panic!("expected String, got {other:?}"),
            })
            .collect();
        assert!(names.contains(&"Alice".to_string()));
        assert!(names.contains(&"Bob".to_string()));
        assert!(names.contains(&"Carol".to_string()));

        // Agent can also create custom property graphs via sql tool
        // Create a separate edge table for a different graph structure
        api.create_table(
            "reports_to",
            "test",
            &[
                ColumnSchema {
                    name: "subordinate".to_string(),
                    dtype: "i64".to_string(),
                },
                ColumnSchema {
                    name: "manager".to_string(),
                    dtype: "i64".to_string(),
                },
            ],
            &[
                vec![Value::Int(1), Value::Int(0)], // Bob reports to Alice
                vec![Value::Int(2), Value::Int(0)], // Carol reports to Alice
                vec![Value::Int(3), Value::Int(1)], // Dave reports to Bob
            ],
        )
        .unwrap();

        api.query(
            "CREATE PROPERTY GRAPH org_chart \
             VERTEX TABLES (employees LABEL Employee) \
             EDGE TABLES (reports_to \
               SOURCE KEY (subordinate) REFERENCES employees (id) \
               DESTINATION KEY (manager) REFERENCES employees (id) \
               LABEL ReportsTo)",
        )
        .unwrap();

        // PageRank on custom graph
        let result = api
            .query(
                "SELECT COUNT(*) FROM GRAPH_TABLE (org_chart \
                 MATCH (e:Employee) \
                 COLUMNS (PAGERANK(org_chart, e) AS rank)) WHERE rank > 0",
            )
            .unwrap();
        match &result.rows[0][0] {
            Value::Int(n) => assert_eq!(*n, 4),
            other => panic!("expected Int, got {other:?}"),
        }
    }
}
