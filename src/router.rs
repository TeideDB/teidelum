use std::ffi::CString;
use std::path::Path;
use std::sync::Mutex;

use anyhow::Result;

use crate::connector::{ColumnSchema, QueryResult, Value};

/// The query router executes SQL against local teide tables.
/// Teide's Session contains raw pointers (not Send/Sync), but we
/// guarantee exclusive access via Mutex so this is safe.
pub struct QueryRouter {
    session: Mutex<teide::Session>,
}

// SAFETY: QueryRouter protects all Session access behind a Mutex,
// ensuring only one thread accesses the C engine at a time.
unsafe impl Send for QueryRouter {}
unsafe impl Sync for QueryRouter {}

impl QueryRouter {
    /// Create a new router with an empty teide session.
    pub fn new() -> Result<Self> {
        let session = teide::Session::new()?;
        Ok(Self {
            session: Mutex::new(session),
        })
    }

    /// Load a splayed table from disk and register it with the given name.
    pub fn load_splayed(&self, name: &str, dir: &Path, sym_path: Option<&Path>) -> Result<()> {
        let mut session = self.session.lock().unwrap();
        let sql = match sym_path {
            Some(p) => format!(
                "CREATE TABLE {} AS SELECT * FROM read_splayed('{}', '{}')",
                name,
                dir.display(),
                p.display(),
            ),
            None => format!(
                "CREATE TABLE {} AS SELECT * FROM read_splayed('{}')",
                name,
                dir.display(),
            ),
        };
        session.execute(&sql)?;
        tracing::info!("loaded splayed table: {name}");
        Ok(())
    }

    /// List registered table names.
    pub fn table_names(&self) -> Vec<String> {
        let session = self.session.lock().unwrap();
        session
            .table_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect()
    }

    /// Get table info (rows, cols) for a registered table.
    pub fn table_info(&self, name: &str) -> Option<(i64, usize)> {
        let session = self.session.lock().unwrap();
        session.table_info(name)
    }

    /// Execute a SQL query and return results.
    pub fn query_sync(&self, sql: &str) -> Result<QueryResult> {
        let mut session = self.session.lock().unwrap();
        let result = session.execute(sql)?;

        match result {
            teide::ExecResult::Query(q) => {
                let ncols = q.columns.len();
                let nrows = q.table.nrows() as usize;

                let columns: Vec<ColumnSchema> = q
                    .columns
                    .iter()
                    .enumerate()
                    .map(|(i, name)| {
                        let dtype = col_type_name(q.table.col_type(i));
                        ColumnSchema {
                            name: name.clone(),
                            dtype: dtype.to_string(),
                        }
                    })
                    .collect();

                let mut rows = Vec::with_capacity(nrows);
                for row_idx in 0..nrows {
                    let mut row = Vec::with_capacity(ncols);
                    for col_idx in 0..ncols {
                        let val = read_value(&q.table, col_idx, row_idx);
                        row.push(val);
                    }
                    rows.push(row);
                }

                Ok(QueryResult { columns, rows })
            }
            teide::ExecResult::Ddl(msg) => Ok(QueryResult {
                columns: vec![ColumnSchema {
                    name: "status".to_string(),
                    dtype: "string".to_string(),
                }],
                rows: vec![vec![Value::String(msg)]],
            }),
        }
    }

    /// Drop a table from the teide session.
    pub fn drop_table(&self, name: &str) -> Result<()> {
        if !crate::catalog::is_valid_identifier(name) {
            anyhow::bail!("invalid identifier: '{name}'");
        }
        self.query_sync(&format!("DROP TABLE IF EXISTS {name}"))?;
        Ok(())
    }

    /// Save a table to disk as a splayed directory.
    pub fn save_table(&self, name: &str, dir: &Path) -> Result<()> {
        let mut session = self.session.lock().unwrap();
        let result = session.execute(&format!("SELECT * FROM {name}"))?;
        if let teide::ExecResult::Query(q) = result {
            std::fs::create_dir_all(dir)?;
            let c_dir = CString::new(dir.to_str().unwrap())?;
            let err = unsafe {
                teide::ffi::td_splay_save(q.table.as_raw(), c_dir.as_ptr(), std::ptr::null())
            };
            if err != teide::ffi::td_err_t::TD_OK {
                anyhow::bail!("td_splay_save failed for {name}: {err:?}");
            }
        }
        Ok(())
    }

    /// Save the global symbol table to disk atomically.
    ///
    /// Writes to a temporary file first, then renames over the target.
    /// This prevents SIGKILL during the write from leaving a truncated
    /// sym file that corrupts subsequent `read_splayed` calls.
    pub fn save_sym(&self, sym_path: &Path) -> Result<()> {
        let tmp_path = sym_path.with_extension("sym.tmp");
        let c_tmp = CString::new(tmp_path.to_str().unwrap())?;
        let err = unsafe { teide::ffi::td_sym_save(c_tmp.as_ptr()) };
        if err != teide::ffi::td_err_t::TD_OK {
            let _ = std::fs::remove_file(&tmp_path);
            anyhow::bail!("td_sym_save failed: {err:?}");
        }
        std::fs::rename(&tmp_path, sym_path)?;
        Ok(())
    }

    /// Async wrapper.
    pub async fn query(&self, sql: &str) -> Result<QueryResult> {
        self.query_sync(sql)
    }
}

fn col_type_name(type_tag: i8) -> &'static str {
    match type_tag {
        1 => "bool",
        5 => "i32",
        6 => "i64",
        7 => "f64",
        9 => "date",
        10 => "time",
        11 => "timestamp",
        20 => "string",
        _ => "unknown",
    }
}

fn read_value(table: &teide::Table, col: usize, row: usize) -> Value {
    let type_tag = table.col_type(col);
    match type_tag {
        1 => match table.get_i64(col, row) {
            Some(v) => Value::Bool(v != 0),
            None => Value::Null,
        },
        5 | 6 => match table.get_i64(col, row) {
            Some(v) => Value::Int(v),
            None => Value::Null,
        },
        7 => match table.get_f64(col, row) {
            Some(v) => Value::Float(v),
            None => Value::Null,
        },
        20 | 9 | 10 | 11 => match table.get_str(col, row) {
            Some(v) => Value::String(v),
            None => Value::Null,
        },
        _ => Value::Null,
    }
}

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

    #[test]
    fn test_create_and_select() {
        let router = QueryRouter::new().unwrap();
        router
            .query_sync("CREATE TABLE people (id BIGINT, name VARCHAR, score DOUBLE)")
            .unwrap();
        router
            .query_sync(
                "INSERT INTO people (id, name, score) VALUES (1, 'Alice', 9.5), (2, 'Bob', 7.2)",
            )
            .unwrap();

        let result = router.query_sync("SELECT * FROM people").unwrap();
        assert_eq!(result.columns.len(), 3);
        assert_eq!(result.rows.len(), 2);

        assert_eq!(result.columns[0].name, "id");
        assert_eq!(result.columns[1].name, "name");
        assert_eq!(result.columns[2].name, "score");

        match &result.rows[0][0] {
            Value::Int(i) => assert_eq!(*i, 1),
            other => panic!("expected Int, got {other:?}"),
        }
        match &result.rows[0][1] {
            Value::String(s) => assert_eq!(s, "Alice"),
            other => panic!("expected String, got {other:?}"),
        }
        match &result.rows[0][2] {
            Value::Float(f) => assert!((f - 9.5).abs() < 0.001),
            other => panic!("expected Float, got {other:?}"),
        }
    }

    #[test]
    fn test_multiple_inserts() {
        let router = QueryRouter::new().unwrap();
        router.query_sync("CREATE TABLE items (id BIGINT)").unwrap();
        router
            .query_sync("INSERT INTO items (id) VALUES (1), (2), (3)")
            .unwrap();
        router
            .query_sync("INSERT INTO items (id) VALUES (4), (5)")
            .unwrap();

        let result = router.query_sync("SELECT * FROM items").unwrap();
        assert_eq!(result.rows.len(), 5);
    }

    #[test]
    fn test_select_with_where() {
        let router = QueryRouter::new().unwrap();
        router
            .query_sync("CREATE TABLE nums (id BIGINT, val BIGINT)")
            .unwrap();
        router
            .query_sync("INSERT INTO nums (id, val) VALUES (1, 10), (2, 20), (3, 30)")
            .unwrap();

        let result = router
            .query_sync("SELECT * FROM nums WHERE val > 15")
            .unwrap();
        assert_eq!(result.rows.len(), 2);
    }

    #[test]
    fn test_select_with_order_by() {
        let router = QueryRouter::new().unwrap();
        router
            .query_sync("CREATE TABLE sorted (name VARCHAR, score BIGINT)")
            .unwrap();
        router
            .query_sync("INSERT INTO sorted (name, score) VALUES ('c', 3), ('a', 1), ('b', 2)")
            .unwrap();

        let result = router
            .query_sync("SELECT name FROM sorted ORDER BY score")
            .unwrap();
        assert_eq!(result.rows.len(), 3);
        let names: Vec<String> = result
            .rows
            .iter()
            .map(|r| match &r[0] {
                Value::String(s) => s.clone(),
                other => panic!("expected String, got {other:?}"),
            })
            .collect();
        assert_eq!(names, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_select_with_aggregation() {
        let router = QueryRouter::new().unwrap();
        router.query_sync("CREATE TABLE agg (val BIGINT)").unwrap();
        router
            .query_sync("INSERT INTO agg (val) VALUES (10), (20), (30)")
            .unwrap();

        let result = router
            .query_sync("SELECT count(*) as cnt, sum(val) as total FROM agg")
            .unwrap();
        assert_eq!(result.rows.len(), 1);
        match &result.rows[0][0] {
            Value::Int(i) => assert_eq!(*i, 3),
            other => panic!("expected Int for count, got {other:?}"),
        }
        match &result.rows[0][1] {
            Value::Int(i) => assert_eq!(*i, 60),
            other => panic!("expected Int for sum, got {other:?}"),
        }
    }

    #[test]
    fn test_all_column_types() {
        let router = QueryRouter::new().unwrap();
        router
            .query_sync("CREATE TABLE typed (b BOOLEAN, i BIGINT, f DOUBLE, s VARCHAR)")
            .unwrap();
        router
            .query_sync("INSERT INTO typed (b, i, f, s) VALUES (TRUE, 42, 3.14, 'hello')")
            .unwrap();

        let result = router.query_sync("SELECT * FROM typed").unwrap();
        assert_eq!(result.rows.len(), 1);

        assert_eq!(result.columns[0].dtype, "bool");
        assert_eq!(result.columns[1].dtype, "i64");
        assert_eq!(result.columns[2].dtype, "f64");
        assert_eq!(result.columns[3].dtype, "string");

        match &result.rows[0][0] {
            Value::Bool(b) => assert!(*b),
            other => panic!("expected Bool, got {other:?}"),
        }
        match &result.rows[0][1] {
            Value::Int(i) => assert_eq!(*i, 42),
            other => panic!("expected Int, got {other:?}"),
        }
        match &result.rows[0][2] {
            Value::Float(f) => assert!((f - 3.14).abs() < 0.001),
            other => panic!("expected Float, got {other:?}"),
        }
        match &result.rows[0][3] {
            Value::String(s) => assert_eq!(s, "hello"),
            other => panic!("expected String, got {other:?}"),
        }
    }

    #[test]
    fn test_null_values() {
        let router = QueryRouter::new().unwrap();
        router
            .query_sync("CREATE TABLE nullable (id BIGINT, name VARCHAR)")
            .unwrap();
        router
            .query_sync("INSERT INTO nullable (id, name) VALUES (1, NULL)")
            .unwrap();

        let result = router.query_sync("SELECT * FROM nullable").unwrap();
        assert_eq!(result.rows.len(), 1);
        // teide represents NULL strings as empty strings
        match &result.rows[0][1] {
            Value::Null => {}
            Value::String(s) => assert!(s.is_empty(), "NULL should be empty string, got: {s}"),
            other => panic!("expected Null or empty String for NULL value, got {other:?}"),
        }
    }

    #[test]
    fn test_table_names() {
        let router = QueryRouter::new().unwrap();
        assert!(router.table_names().is_empty());

        router.query_sync("CREATE TABLE alpha (id BIGINT)").unwrap();
        router.query_sync("CREATE TABLE beta (id BIGINT)").unwrap();

        let names = router.table_names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"alpha".to_string()));
        assert!(names.contains(&"beta".to_string()));
    }

    #[test]
    fn test_table_info() {
        let router = QueryRouter::new().unwrap();
        router
            .query_sync("CREATE TABLE info_test (id BIGINT, name VARCHAR)")
            .unwrap();
        router
            .query_sync("INSERT INTO info_test (id, name) VALUES (1, 'a'), (2, 'b'), (3, 'c')")
            .unwrap();

        let (rows, cols) = router.table_info("info_test").unwrap();
        assert_eq!(rows, 3);
        assert_eq!(cols, 2);
    }

    #[test]
    fn test_table_info_nonexistent() {
        let router = QueryRouter::new().unwrap();
        assert!(router.table_info("ghost").is_none());
    }

    #[test]
    fn test_query_nonexistent_table() {
        let router = QueryRouter::new().unwrap();
        let result = router.query_sync("SELECT * FROM ghost");
        assert!(result.is_err());
    }

    #[test]
    fn test_drop_table_invalid_identifier() {
        let router = QueryRouter::new().unwrap();
        let result = router.drop_table("'; DROP TABLE x;--");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("invalid identifier"));
    }

    #[test]
    fn test_ddl_result_format() {
        let router = QueryRouter::new().unwrap();
        let result = router
            .query_sync("CREATE TABLE ddl_test (id BIGINT)")
            .unwrap();
        assert_eq!(result.columns.len(), 1);
        assert_eq!(result.columns[0].name, "status");
        match &result.rows[0][0] {
            Value::String(s) => assert!(!s.is_empty()),
            other => panic!("expected String status, got {other:?}"),
        }
    }

    #[test]
    fn test_pgq_create_property_graph() {
        let router = QueryRouter::new().unwrap();
        router
            .query_sync("CREATE TABLE persons (id BIGINT, name VARCHAR)")
            .unwrap();
        router
            .query_sync(
                "INSERT INTO persons (id, name) VALUES (0, 'Alice'), (1, 'Bob'), (2, 'Carol')",
            )
            .unwrap();
        router
            .query_sync("CREATE TABLE knows (src BIGINT, dst BIGINT)")
            .unwrap();
        router
            .query_sync("INSERT INTO knows (src, dst) VALUES (0, 1), (1, 2), (0, 2)")
            .unwrap();

        let result = router.query_sync(
            "CREATE PROPERTY GRAPH social \
             VERTEX TABLES (persons LABEL Person) \
             EDGE TABLES (knows SOURCE KEY (src) REFERENCES persons (id) \
             DESTINATION KEY (dst) REFERENCES persons (id) LABEL Knows)",
        );
        assert!(result.is_ok(), "CREATE PROPERTY GRAPH failed: {result:?}");

        // DDL should return a status message
        let qr = result.unwrap();
        assert_eq!(qr.columns[0].name, "status");
    }

    #[test]
    fn test_pgq_match_query() {
        let router = QueryRouter::new().unwrap();
        router
            .query_sync("CREATE TABLE persons (id BIGINT, name VARCHAR)")
            .unwrap();
        router
            .query_sync(
                "INSERT INTO persons (id, name) VALUES (0, 'Alice'), (1, 'Bob'), (2, 'Carol')",
            )
            .unwrap();
        router
            .query_sync("CREATE TABLE knows (src BIGINT, dst BIGINT)")
            .unwrap();
        router
            .query_sync("INSERT INTO knows (src, dst) VALUES (0, 1), (1, 2), (0, 2)")
            .unwrap();
        router
            .query_sync(
                "CREATE PROPERTY GRAPH social \
                 VERTEX TABLES (persons LABEL Person) \
                 EDGE TABLES (knows SOURCE KEY (src) REFERENCES persons (id) \
                 DESTINATION KEY (dst) REFERENCES persons (id) LABEL Knows)",
            )
            .unwrap();

        // 1-hop: Alice's direct knows connections
        let result = router
            .query_sync(
                "SELECT * FROM GRAPH_TABLE (social \
                 MATCH (p:Person WHERE p.name = 'Alice')-[:Knows]->(q:Person) \
                 COLUMNS (q.name AS friend))",
            )
            .unwrap();

        assert_eq!(result.columns.len(), 1);
        assert_eq!(result.columns[0].name, "friend");
        assert_eq!(result.rows.len(), 2); // Alice knows Bob and Carol

        let names: Vec<String> = result
            .rows
            .iter()
            .map(|r| match &r[0] {
                Value::String(s) => s.clone(),
                other => panic!("expected String, got {other:?}"),
            })
            .collect();
        assert!(names.contains(&"Bob".to_string()));
        assert!(names.contains(&"Carol".to_string()));
    }

    #[test]
    fn test_pgq_algorithms() {
        let router = QueryRouter::new().unwrap();
        router
            .query_sync("CREATE TABLE persons (id BIGINT, name VARCHAR)")
            .unwrap();
        router
            .query_sync(
                "INSERT INTO persons (id, name) VALUES (0, 'Alice'), (1, 'Bob'), (2, 'Carol'), (3, 'Dave'), (4, 'Eve')",
            )
            .unwrap();
        router
            .query_sync("CREATE TABLE knows (src BIGINT, dst BIGINT)")
            .unwrap();
        router
            .query_sync(
                "INSERT INTO knows (src, dst) VALUES (0, 1), (0, 2), (1, 3), (2, 3), (3, 4)",
            )
            .unwrap();
        router
            .query_sync(
                "CREATE PROPERTY GRAPH social \
                 VERTEX TABLES (persons LABEL Person) \
                 EDGE TABLES (knows SOURCE KEY (src) REFERENCES persons (id) \
                 DESTINATION KEY (dst) REFERENCES persons (id) LABEL Knows)",
            )
            .unwrap();

        // PageRank: all 5 nodes should get positive ranks
        let result = router
            .query_sync(
                "SELECT COUNT(*) FROM GRAPH_TABLE (social \
                 MATCH (p:Person) \
                 COLUMNS (PAGERANK(social, p) AS rank)) WHERE rank > 0",
            )
            .unwrap();
        match &result.rows[0][0] {
            Value::Int(n) => assert_eq!(*n, 5, "all 5 nodes should have positive pagerank"),
            other => panic!("expected Int, got {other:?}"),
        }

        // Connected components: single connected graph = 1 component
        let result = router
            .query_sync(
                "SELECT COUNT(DISTINCT component) FROM GRAPH_TABLE (social \
                 MATCH (p:Person) \
                 COLUMNS (COMPONENT(social, p) AS component))",
            )
            .unwrap();
        match &result.rows[0][0] {
            Value::Int(n) => assert_eq!(*n, 1, "fully connected graph should have 1 component"),
            other => panic!("expected Int, got {other:?}"),
        }

        // Louvain community detection: all nodes should get non-negative community IDs
        let result = router
            .query_sync(
                "SELECT COUNT(*) FROM GRAPH_TABLE (social \
                 MATCH (p:Person) \
                 COLUMNS (COMMUNITY(social, p) AS community)) WHERE community >= 0",
            )
            .unwrap();
        match &result.rows[0][0] {
            Value::Int(n) => assert_eq!(*n, 5),
            other => panic!("expected Int, got {other:?}"),
        }
    }
}
