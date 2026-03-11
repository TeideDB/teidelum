use std::sync::Arc;

use rmcp::{
    handler::server::tool::ToolRouter, handler::server::wrapper::Parameters, model::*, schemars,
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler,
};

use crate::api::TeidelumApi;
use crate::catalog::Relationship;
use crate::connector::{ColumnSchema, Value};
use crate::search::SearchQuery;
use crate::sync::SearchDocument;

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

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ChatPostMessageParams {
    /// Channel ID to post to.
    pub channel: i64,
    /// Message text content.
    pub text: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ChatHistoryParams {
    /// Channel ID to read history from.
    pub channel: i64,
    /// Maximum number of messages to return.
    #[serde(default = "default_history_limit")]
    pub limit: usize,
}

fn default_history_limit() -> usize {
    50
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ChatReplyParams {
    /// Channel ID containing the thread.
    pub channel: i64,
    /// Thread parent message ID (ts).
    pub thread_ts: i64,
    /// Reply text content.
    pub text: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ChatReactParams {
    /// Message ID (ts) to react to.
    pub timestamp: i64,
    /// Emoji name (e.g. "thumbsup", "heart").
    pub name: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ChatListChannelsParams {}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ChatSearchParams {
    /// Search query string.
    pub query: String,
    /// Maximum number of results.
    #[serde(default = "default_limit")]
    pub limit: usize,
}

/// The Teidelum MCP server — exposes search, sql, describe, graph, and sync tools.
#[derive(Clone)]
pub struct Teidelum {
    api: Arc<TeidelumApi>,
    tool_router: ToolRouter<Self>,
}

/// Convert a JSON value to a connector Value.
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
                Err(McpError::invalid_params(
                    format!("unsupported number: {n}"),
                    None,
                ))
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

#[tool_router]
impl Teidelum {
    pub fn new(api: TeidelumApi) -> Self {
        Self {
            api: Arc::new(api),
            tool_router: Self::tool_router(),
        }
    }

    pub fn new_with_shared(api: Arc<TeidelumApi>) -> Self {
        Self {
            api,
            tool_router: Self::tool_router(),
        }
    }

    /// Look up the first bot user. Returns (user_id, username) or error.
    fn resolve_bot_user(&self) -> Result<(i64, String), McpError> {
        let sql = "SELECT id, username FROM users WHERE is_bot = true LIMIT 1";
        let result =
            self.api.query_router().query_sync(sql).map_err(|e| {
                McpError::internal_error(format!("bot user lookup failed: {e}"), None)
            })?;

        if result.rows.is_empty() {
            return Err(McpError::internal_error(
                "no bot user found — create one with is_bot=true via auth.register".to_string(),
                None,
            ));
        }

        let user_id = match &result.rows[0][0] {
            Value::Int(v) => *v,
            _ => {
                return Err(McpError::internal_error(
                    "invalid bot user id".to_string(),
                    None,
                ))
            }
        };
        let username = match &result.rows[0][1] {
            Value::String(s) => s.clone(),
            _ => {
                return Err(McpError::internal_error(
                    "invalid bot username".to_string(),
                    None,
                ))
            }
        };

        Ok((user_id, username))
    }

    /// Check if a user is a member of a channel.
    fn is_member(&self, channel_id: i64, user_id: i64) -> bool {
        let sql = format!(
            "SELECT channel_id FROM channel_members WHERE channel_id = {} AND user_id = {}",
            channel_id, user_id
        );
        match self.api.query_router().query_sync(&sql) {
            Ok(r) => !r.rows.is_empty(),
            Err(_) => false,
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
            .api
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
            .api
            .query(&params.query)
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
        let description = self
            .api
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
            "neighbors" => self.api.neighbors(
                &params.table,
                &params.key_col,
                &params.key,
                params.depth,
                &params.direction,
                params.rel_types.as_deref(),
            ),
            "path" => {
                let to_table = params.to_table.as_deref().ok_or_else(|| {
                    McpError::invalid_params("'to_table' is required for path operation", None)
                })?;
                let to_key = params.to_key.as_deref().ok_or_else(|| {
                    McpError::invalid_params("'to_key' is required for path operation", None)
                })?;
                let to_key_col = params.to_key_col.as_deref().unwrap_or(&params.key_col);
                self.api.path(
                    &params.table,
                    &params.key_col,
                    &params.key,
                    to_table,
                    to_key_col,
                    to_key,
                    params.depth,
                    &params.direction,
                    params.rel_types.as_deref(),
                )
            }
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown graph operation: '{other}'. Use 'neighbors' or 'path'"),
                    None,
                ));
            }
        };

        let result = result.map_err(|e| {
            let msg = e.to_string();
            let is_user_error = msg.starts_with("invalid ")
                || msg.starts_with("starting node not found")
                || msg.starts_with("source node not found")
                || msg.starts_with("target node not found");
            if is_user_error {
                McpError::invalid_params(msg, None)
            } else {
                McpError::internal_error(format!("graph operation failed: {msg}"), None)
            }
        })?;

        let json = serde_json::to_string_pretty(&result)
            .map_err(|e| McpError::internal_error(format!("serialization failed: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

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
                        format!(
                            "row {i} has {} values but {} columns",
                            row.len(),
                            columns.len()
                        ),
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
                        format!(
                            "row {i} has {} values but {} columns",
                            row.len(),
                            columns.len()
                        ),
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

    #[tool(description = "Send a message to a chat channel (as bot user)")]
    async fn chat_post_message(
        &self,
        Parameters(params): Parameters<ChatPostMessageParams>,
    ) -> Result<CallToolResult, McpError> {
        let (bot_id, _) = self.resolve_bot_user()?;

        if !self.is_member(params.channel, bot_id) {
            return Err(McpError::invalid_params(
                format!("bot is not a member of channel {}", params.channel),
                None,
            ));
        }

        let id = crate::chat::id::next_id();
        let now = crate::chat::models::now_timestamp();
        let text_escaped = crate::chat::models::escape_sql(&params.text);

        let sql = format!(
            "INSERT INTO messages (id, channel_id, user_id, thread_id, content, deleted_at, edited_at, created_at) \
             VALUES ({id}, {channel}, {bot}, 0, '{text}', '', '', '{now}')",
            channel = params.channel,
            bot = bot_id,
            text = text_escaped,
        );

        self.api
            .query_router()
            .query_sync(&sql)
            .map_err(|e| McpError::internal_error(format!("post message failed: {e}"), None))?;

        // Index in tantivy
        let channel_name =
            crate::chat::models::channel_display_name(self.api.query_router(), params.channel);
        let doc = vec![(
            id.to_string(),
            "chat".to_string(),
            channel_name,
            params.text.clone(),
        )];
        if let Err(e) = self.api.search_engine().index_documents(&doc) {
            tracing::warn!("MCP chat message indexing failed: {e}");
        }

        let result = serde_json::json!({
            "ok": true,
            "ts": id.to_string(),
            "channel": params.channel.to_string(),
            "text": params.text,
        });
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap(),
        )]))
    }

    #[tool(description = "Read recent message history from a chat channel")]
    async fn chat_history(
        &self,
        Parameters(params): Parameters<ChatHistoryParams>,
    ) -> Result<CallToolResult, McpError> {
        let (bot_id, _) = self.resolve_bot_user()?;

        if !self.is_member(params.channel, bot_id) {
            return Err(McpError::invalid_params(
                format!("bot is not a member of channel {}", params.channel),
                None,
            ));
        }

        let limit = params.limit.min(200);
        let sql = format!(
            "SELECT m.id, m.user_id, m.thread_id, m.content, m.created_at, u.username \
             FROM messages m \
             JOIN users u ON m.user_id = u.id \
             WHERE m.channel_id = {} AND m.thread_id = 0 AND m.deleted_at = '' \
             ORDER BY m.id DESC LIMIT {}",
            params.channel, limit
        );

        let result =
            self.api.query_router().query_sync(&sql).map_err(|e| {
                McpError::internal_error(format!("history query failed: {e}"), None)
            })?;

        let messages: Vec<serde_json::Value> = result
            .rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "ts": row[0].to_json(),
                    "user": row[1].to_json(),
                    "thread_ts": row[2].to_json(),
                    "text": row[3].to_json(),
                    "created_at": row[4].to_json(),
                    "username": row[5].to_json(),
                })
            })
            .collect();

        let output = serde_json::json!({
            "ok": true,
            "channel": params.channel.to_string(),
            "messages": messages,
        });
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&output).unwrap(),
        )]))
    }

    #[tool(description = "Reply to a thread in a chat channel (as bot user)")]
    async fn chat_reply(
        &self,
        Parameters(params): Parameters<ChatReplyParams>,
    ) -> Result<CallToolResult, McpError> {
        let (bot_id, _) = self.resolve_bot_user()?;

        if !self.is_member(params.channel, bot_id) {
            return Err(McpError::invalid_params(
                format!("bot is not a member of channel {}", params.channel),
                None,
            ));
        }

        let id = crate::chat::id::next_id();
        let now = crate::chat::models::now_timestamp();
        let text_escaped = crate::chat::models::escape_sql(&params.text);

        let sql = format!(
            "INSERT INTO messages (id, channel_id, user_id, thread_id, content, deleted_at, edited_at, created_at) \
             VALUES ({id}, {channel}, {bot}, {thread}, '{text}', '', '', '{now}')",
            channel = params.channel,
            bot = bot_id,
            thread = params.thread_ts,
            text = text_escaped,
        );

        self.api
            .query_router()
            .query_sync(&sql)
            .map_err(|e| McpError::internal_error(format!("reply failed: {e}"), None))?;

        // Index in tantivy
        let channel_name =
            crate::chat::models::channel_display_name(self.api.query_router(), params.channel);
        let doc = vec![(
            id.to_string(),
            "chat".to_string(),
            channel_name,
            params.text.clone(),
        )];
        if let Err(e) = self.api.search_engine().index_documents(&doc) {
            tracing::warn!("MCP chat reply indexing failed: {e}");
        }

        let result = serde_json::json!({
            "ok": true,
            "ts": id.to_string(),
            "channel": params.channel.to_string(),
            "thread_ts": params.thread_ts.to_string(),
            "text": params.text,
        });
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap(),
        )]))
    }

    #[tool(description = "Add an emoji reaction to a message (as bot user)")]
    async fn chat_react(
        &self,
        Parameters(params): Parameters<ChatReactParams>,
    ) -> Result<CallToolResult, McpError> {
        let (bot_id, _) = self.resolve_bot_user()?;

        // Get channel from message
        let check_sql = format!(
            "SELECT channel_id FROM messages WHERE id = {}",
            params.timestamp
        );
        let result = self
            .api
            .query_router()
            .query_sync(&check_sql)
            .map_err(|e| McpError::internal_error(format!("message lookup failed: {e}"), None))?;

        if result.rows.is_empty() {
            return Err(McpError::invalid_params(
                "message_not_found".to_string(),
                None,
            ));
        }

        let channel_id = match &result.rows[0][0] {
            Value::Int(v) => *v,
            _ => {
                return Err(McpError::internal_error(
                    "invalid channel_id".to_string(),
                    None,
                ))
            }
        };

        if !self.is_member(channel_id, bot_id) {
            return Err(McpError::invalid_params(
                format!("bot is not a member of channel {channel_id}"),
                None,
            ));
        }

        // Check if already reacted
        let dup_sql = format!(
            "SELECT message_id FROM reactions WHERE message_id = {} AND user_id = {} AND emoji = '{}'",
            params.timestamp, bot_id, crate::chat::models::escape_sql(&params.name)
        );
        if let Ok(r) = self.api.query_router().query_sync(&dup_sql) {
            if !r.rows.is_empty() {
                return Err(McpError::invalid_params(
                    "already_reacted".to_string(),
                    None,
                ));
            }
        }

        let now = crate::chat::models::now_timestamp();
        let insert_sql = format!(
            "INSERT INTO reactions (message_id, user_id, emoji, created_at) \
             VALUES ({}, {}, '{}', '{now}')",
            params.timestamp,
            bot_id,
            crate::chat::models::escape_sql(&params.name)
        );

        self.api
            .query_router()
            .query_sync(&insert_sql)
            .map_err(|e| McpError::internal_error(format!("reaction insert failed: {e}"), None))?;

        let result = serde_json::json!({"ok": true});
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap(),
        )]))
    }

    #[tool(description = "List chat channels accessible to the bot")]
    async fn chat_list_channels(
        &self,
        Parameters(_params): Parameters<ChatListChannelsParams>,
    ) -> Result<CallToolResult, McpError> {
        let (bot_id, _) = self.resolve_bot_user()?;

        // List channels the bot is a member of
        let sql = format!(
            "SELECT c.id, c.name, c.kind, c.topic \
             FROM channels c \
             JOIN channel_members cm ON c.id = cm.channel_id \
             WHERE cm.user_id = {}",
            bot_id
        );

        let result =
            self.api.query_router().query_sync(&sql).map_err(|e| {
                McpError::internal_error(format!("list channels failed: {e}"), None)
            })?;

        let channels: Vec<serde_json::Value> = result
            .rows
            .iter()
            .map(|row| {
                serde_json::json!({
                    "id": row[0].to_json(),
                    "name": row[1].to_json(),
                    "kind": row[2].to_json(),
                    "topic": row[3].to_json(),
                })
            })
            .collect();

        let output = serde_json::json!({
            "ok": true,
            "channels": channels,
        });
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&output).unwrap(),
        )]))
    }

    #[tool(description = "Search chat messages by keyword")]
    async fn chat_search(
        &self,
        Parameters(params): Parameters<ChatSearchParams>,
    ) -> Result<CallToolResult, McpError> {
        let (bot_id, _) = self.resolve_bot_user()?;

        // Get channel IDs the bot is a member of for filtering results
        let member_channel_ids: std::collections::HashSet<i64> = {
            let sql = format!(
                "SELECT channel_id FROM channel_members WHERE user_id = {}",
                bot_id
            );
            match self.api.query_router().query_sync(&sql) {
                Ok(r) => r
                    .rows
                    .iter()
                    .filter_map(|row| match &row[0] {
                        Value::Int(v) => Some(*v),
                        _ => None,
                    })
                    .collect(),
                Err(e) => {
                    return Err(McpError::internal_error(
                        format!("membership lookup failed: {e}"),
                        None,
                    ));
                }
            }
        };

        let limit = params.limit.min(100);
        // Over-fetch to compensate for post-query auth filtering
        let query = SearchQuery {
            text: params.query,
            sources: Some(vec!["chat".to_string()]),
            limit: limit * crate::chat::models::SEARCH_OVERFETCH_FACTOR,
            date_from: None,
            date_to: None,
        };

        let results = self
            .api
            .search(&query)
            .map_err(|e| McpError::internal_error(format!("chat search failed: {e}"), None))?;

        // Look up channel_id for each message to filter by ID (not name)
        // Parse IDs to i64 to prevent SQL injection from arbitrary tantivy document IDs
        let msg_ids: Vec<i64> = results
            .iter()
            .filter_map(|r| r.id.parse::<i64>().ok())
            .collect();
        let msg_channel_map: std::collections::HashMap<String, i64> = if msg_ids.is_empty() {
            std::collections::HashMap::new()
        } else {
            let id_list = msg_ids
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "SELECT id, channel_id FROM messages WHERE id IN ({id_list}) AND deleted_at = ''"
            );
            match self.api.query_router().query_sync(&sql) {
                Ok(r) => r
                    .rows
                    .iter()
                    .filter_map(|row| {
                        let id = match &row[0] {
                            Value::Int(v) => v.to_string(),
                            _ => return None,
                        };
                        let ch_id = match &row[1] {
                            Value::Int(v) => *v,
                            _ => return None,
                        };
                        Some((id, ch_id))
                    })
                    .collect(),
                Err(e) => {
                    return Err(McpError::internal_error(
                        format!("message channel lookup failed: {e}"),
                        None,
                    ));
                }
            }
        };

        // Filter results to only channels the bot is a member of (by channel ID)
        let matches: Vec<serde_json::Value> = results
            .iter()
            .filter(|r| {
                msg_channel_map
                    .get(&r.id)
                    .is_some_and(|ch_id| member_channel_ids.contains(ch_id))
            })
            .take(limit)
            .map(|r| {
                serde_json::json!({
                    "ts": r.id,
                    "channel": r.title,
                    "text": r.snippet,
                    "score": r.score,
                })
            })
            .collect();

        let output = serde_json::json!({
            "ok": true,
            "messages": {
                "matches": matches,
                "total": matches.len(),
            }
        });
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&output).unwrap(),
        )]))
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
