use anyhow::Result;

use crate::catalog::Catalog;
use crate::connector::{Connector, QueryResult};

/// The query router checks the metadata catalog to determine
/// how to serve each SQL query — local tables go to libteide,
/// remote tables go through the appropriate connector.
#[allow(dead_code)]
pub struct QueryRouter {
    catalog: Catalog,
    connectors: Vec<Box<dyn Connector>>,
}

#[allow(dead_code)]
impl QueryRouter {
    pub fn new(catalog: Catalog) -> Self {
        Self {
            catalog,
            connectors: Vec::new(),
        }
    }

    pub fn register_connector(&mut self, connector: Box<dyn Connector>) {
        self.connectors.push(connector);
    }

    /// Route a SQL query to the appropriate engine.
    pub async fn query(&self, _sql: &str) -> Result<QueryResult> {
        // TODO: parse SQL to extract table names
        // TODO: look up tables in catalog
        // TODO: if all tables are local → execute via libteide
        // TODO: if any table is remote → dispatch to connector
        // TODO: handle mixed local/remote queries

        anyhow::bail!("query routing not yet implemented")
    }
}
