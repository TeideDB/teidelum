use anyhow::Result;
use async_trait::async_trait;
use teidelum_connector_core::{ColumnSchema, Connector, QueryResult};

pub struct KdbConnector {
    // TODO: connection config (host, port)
}

impl KdbConnector {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Connector for KdbConnector {
    fn name(&self) -> &str {
        "kdb"
    }

    async fn list_tables(&self) -> Result<Vec<(String, Vec<ColumnSchema>)>> {
        // TODO: query kdb+ for table schemas
        Ok(Vec::new())
    }

    async fn query(&self, _sql: &str) -> Result<QueryResult> {
        // TODO: translate SQL to q, execute against kdb+ server
        anyhow::bail!("kdb+ connector not yet implemented")
    }
}
