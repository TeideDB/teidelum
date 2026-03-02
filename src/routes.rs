use std::sync::Arc;

use axum::http::StatusCode;
use axum::{
    extract::{Path, State},
    routing::{delete, get, post},
    Json, Router,
};
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
    let results = api
        .search(&query)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::to_value(results).unwrap()))
}

async fn sql_handler(
    State(api): State<AppState>,
    Json(req): Json<SqlRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let result = api
        .query(&req.query)
        .map_err(|e| err(StatusCode::BAD_REQUEST, e))?;
    Ok(Json(serde_json::to_value(result).unwrap()))
}

async fn describe_handler(
    State(api): State<AppState>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let desc = api
        .describe(None)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
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
        .map(|row| {
            row.iter()
                .zip(columns.iter())
                .map(|(v, c)| json_to_value(v, &c.dtype))
                .collect()
        })
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
    let desc = api
        .describe(None)
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let tables = desc["tables"].as_array().ok_or_else(|| {
        err(
            StatusCode::INTERNAL_SERVER_ERROR,
            anyhow::anyhow!("unexpected catalog format"),
        )
    })?;
    let table_entry = tables
        .iter()
        .find(|t| t["name"].as_str() == Some(&name))
        .ok_or_else(|| {
            err(
                StatusCode::NOT_FOUND,
                anyhow::anyhow!("table '{name}' not found"),
            )
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

    let rows: Vec<Vec<Value>> = req
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .zip(columns.iter())
                .map(|(v, c)| json_to_value(v, &c.dtype))
                .collect()
        })
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
    api.delete_documents(std::slice::from_ref(&id))
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

fn err(status: StatusCode, e: anyhow::Error) -> (StatusCode, Json<serde_json::Value>) {
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
