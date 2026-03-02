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
        self.query_sync(&format!("DROP TABLE IF EXISTS {name}"))?;
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
}
