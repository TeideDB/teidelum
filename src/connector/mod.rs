pub mod kdb;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A column value returned from a connector query.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Value {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

impl Value {
    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Value::Null => serde_json::Value::Null,
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Int(i) => serde_json::json!(i.to_string()),
            Value::Float(f) => serde_json::json!(*f),
            Value::String(s) => serde_json::Value::String(s.clone()),
        }
    }
}

/// Schema information for a single column.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnSchema {
    pub name: String,
    pub dtype: String,
}

/// Result of a connector query: column schemas + rows of values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<ColumnSchema>,
    pub rows: Vec<Vec<Value>>,
}

/// A connector queries an external data source live, without local storage.
/// The query router dispatches to connectors for tables marked as `remote`
/// in the metadata catalog.
#[async_trait]
pub trait Connector: Send + Sync {
    /// Unique name for this connector (e.g. "kdb").
    fn name(&self) -> &str;

    /// Return the schemas of tables available through this connector.
    async fn list_tables(&self) -> Result<Vec<(String, Vec<ColumnSchema>)>>;

    /// Execute a SQL query against the remote source.
    async fn query(&self, sql: &str) -> Result<QueryResult>;
}
