use std::sync::Arc;

use rmcp::{
    handler::server::tool::ToolRouter, handler::server::wrapper::Parameters, model::*, schemars,
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};
use tokio::sync::Mutex;

use crate::catalog::Catalog;
use crate::graph::GraphEngine;
use crate::router::QueryRouter;
use crate::search::{SearchEngine, SearchQuery};

/// Tool parameter types — derive JsonSchema for automatic MCP schema generation.

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchParams {
    /// Full-text search query string.
    pub query: String,
    /// Filter results to specific sources (e.g. ["notion", "zulip"]).
    #[serde(default)]
    pub sources: Option<Vec<String>>,
    /// Maximum number of results to return.
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Filter results from this date (ISO 8601).
    #[serde(default)]
    pub date_from: Option<String>,
    /// Filter results up to this date (ISO 8601).
    #[serde(default)]
    pub date_to: Option<String>,
}

fn default_limit() -> usize {
    10
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SqlParams {
    /// SQL query to execute against local or remote tables.
    pub query: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DescribeParams {
    /// Filter catalog to a specific source (e.g. "notion", "zulip", "kdb").
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SyncParams {
    /// Sync a specific source, or omit to sync all.
    #[serde(default)]
    pub source: Option<String>,
}

fn default_operation() -> String {
    "neighbors".to_string()
}

fn default_depth() -> usize {
    2
}

fn default_direction() -> String {
    "both".to_string()
}

fn default_key_col() -> String {
    "name".to_string()
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GraphParams {
    /// Starting node table (e.g. "team_members", "project_tasks").
    pub table: String,
    /// Node identifier value (e.g. "Alice Chen", "Implement JWT token rotation").
    pub key: String,
    /// Key column name to identify the node (default: "name").
    #[serde(default = "default_key_col")]
    pub key_col: String,
    /// Graph operation: "neighbors" or "path".
    #[serde(default = "default_operation")]
    pub operation: String,
    /// Maximum traversal depth in hops (default: 2).
    #[serde(default = "default_depth")]
    pub depth: usize,
    /// Traversal direction: "forward", "reverse", or "both" (default: "both").
    #[serde(default = "default_direction")]
    pub direction: String,
    /// Filter to specific relationship types (e.g. ["assigned_to", "reported_by"]).
    #[serde(default)]
    pub rel_types: Option<Vec<String>>,
    /// Target table for "path" operation.
    #[serde(default)]
    pub to_table: Option<String>,
    /// Target key value for "path" operation.
    #[serde(default)]
    pub to_key: Option<String>,
    /// Target key column for "path" operation (default: same as key_col).
    #[serde(default)]
    pub to_key_col: Option<String>,
}

/// The Teidelum MCP server — exposes search, sql, describe, graph, and sync tools.
#[derive(Clone)]
pub struct Teidelum {
    catalog: Arc<Mutex<Catalog>>,
    search_engine: Arc<SearchEngine>,
    query_router: Arc<QueryRouter>,
    graph_engine: Arc<GraphEngine>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl Teidelum {
    pub fn new(
        catalog: Catalog,
        search_engine: SearchEngine,
        query_router: QueryRouter,
        graph_engine: GraphEngine,
    ) -> Self {
        Self {
            catalog: Arc::new(Mutex::new(catalog)),
            search_engine: Arc::new(search_engine),
            query_router: Arc::new(query_router),
            graph_engine: Arc::new(graph_engine),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Full-text search across all connected sources")]
    async fn search(
        &self,
        Parameters(params): Parameters<SearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let query = SearchQuery {
            text: params.query,
            sources: params.sources,
            limit: params.limit,
            date_from: params.date_from,
            date_to: params.date_to,
        };

        let results = self
            .search_engine
            .search(&query)
            .map_err(|e| McpError::internal_error(format!("search failed: {e}"), None))?;

        let json = serde_json::to_string_pretty(&results)
            .map_err(|e| McpError::internal_error(format!("serialization failed: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Run analytical queries over structured data from all sources")]
    async fn sql(
        &self,
        Parameters(params): Parameters<SqlParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = self
            .query_router
            .query(&params.query)
            .await
            .map_err(|e| McpError::internal_error(format!("query failed: {e}"), None))?;

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(format!("serialization failed: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "List available tables, schemas, and relationships")]
    async fn describe(
        &self,
        Parameters(params): Parameters<DescribeParams>,
    ) -> Result<CallToolResult, McpError> {
        let catalog = self.catalog.lock().await;
        let description = catalog
            .describe(params.source.as_deref())
            .map_err(|e| McpError::internal_error(format!("describe failed: {e}"), None))?;

        let json = serde_json::to_string_pretty(&description)
            .map_err(|e| McpError::internal_error(format!("serialization failed: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Trigger incremental sync for connected sources")]
    async fn sync(
        &self,
        Parameters(_params): Parameters<SyncParams>,
    ) -> Result<CallToolResult, McpError> {
        // TODO: dispatch to registered SyncSource implementations
        let result = serde_json::json!({
            "status": "not_implemented",
            "message": "sync sources not yet configured"
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap(),
        )]))
    }

    #[tool(description = "Traverse relationships between entities (neighbors, paths)")]
    async fn graph(
        &self,
        Parameters(params): Parameters<GraphParams>,
    ) -> Result<CallToolResult, McpError> {
        let result = match params.operation.as_str() {
            "neighbors" => self.graph_engine.neighbors(
                &params.table,
                &params.key_col,
                &params.key,
                params.depth,
                &params.direction,
                params.rel_types.as_deref(),
                &self.query_router,
            ),
            "path" => {
                let to_table = params.to_table.as_deref().ok_or_else(|| {
                    McpError::invalid_params("'to_table' is required for path operation", None)
                })?;
                let to_key = params.to_key.as_deref().ok_or_else(|| {
                    McpError::invalid_params("'to_key' is required for path operation", None)
                })?;
                let to_key_col = params.to_key_col.as_deref().unwrap_or(&params.key_col);
                self.graph_engine.path(
                    &params.table,
                    &params.key_col,
                    &params.key,
                    to_table,
                    to_key_col,
                    to_key,
                    params.depth,
                    &params.direction,
                    params.rel_types.as_deref(),
                    &self.query_router,
                )
            }
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown graph operation: '{other}'. Use 'neighbors' or 'path'"),
                    None,
                ));
            }
        };

        let result = result
            .map_err(|e| McpError::internal_error(format!("graph operation failed: {e}"), None))?;

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(format!("serialization failed: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}

#[tool_handler]
impl ServerHandler for Teidelum {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "teidelum".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                title: None,
                description: None,
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "Teidelum indexes Notion, Zulip, and live data sources into a single \
                 searchable index. Use 'describe' to see available tables, 'search' for \
                 full-text queries, 'sql' for analytical queries, 'graph' to traverse \
                 relationships between entities, and 'sync' to refresh data."
                    .into(),
            ),
        }
    }
}
