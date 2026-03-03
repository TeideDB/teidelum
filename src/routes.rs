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
            } else if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Null
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use crate::api::TeidelumApi;
    use crate::catalog::Relationship;
    use crate::connector::{ColumnSchema, Value};
    use crate::sync::SearchDocument;

    /// Build a test router with a fresh TeidelumApi.
    fn test_router(tmp: &std::path::Path) -> axum::Router {
        let api = Arc::new(TeidelumApi::new(tmp).unwrap());
        super::api_routes().with_state(api)
    }

    /// Build a test router pre-loaded with demo data and relationships.
    fn test_router_with_data(tmp: &std::path::Path) -> axum::Router {
        crate::demo::generate(tmp).unwrap();
        let api = Arc::new(TeidelumApi::open(tmp).unwrap());
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
        super::api_routes().with_state(api)
    }

    /// Helper: extract JSON body from response.
    async fn json_body(resp: axum::response::Response) -> serde_json::Value {
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    // --- Search ---

    #[tokio::test]
    async fn test_search_endpoint() {
        let tmp = tempfile::tempdir().unwrap();
        let app = test_router_with_data(tmp.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/search")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"query": "authentication"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        let results = body.as_array().unwrap();
        assert!(!results.is_empty());
    }

    #[tokio::test]
    async fn test_search_endpoint_with_source_filter() {
        let tmp = tempfile::tempdir().unwrap();
        let app = test_router_with_data(tmp.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/search")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"query": "authentication", "sources": ["notion"]})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        let results = body.as_array().unwrap();
        for r in results {
            assert_eq!(r["source"], "notion");
        }
    }

    #[tokio::test]
    async fn test_search_endpoint_empty_results() {
        let tmp = tempfile::tempdir().unwrap();
        let app = test_router_with_data(tmp.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/search")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"query": "xyznonexistentterm"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        assert!(body.as_array().unwrap().is_empty());
    }

    // --- SQL ---

    #[tokio::test]
    async fn test_sql_endpoint() {
        let tmp = tempfile::tempdir().unwrap();
        let app = test_router_with_data(tmp.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sql")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"query": "SELECT count(*) FROM team_members"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        assert!(body["columns"].is_array());
        assert!(body["rows"].is_array());
        assert!(!body["rows"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_sql_endpoint_invalid_query() {
        let tmp = tempfile::tempdir().unwrap();
        let app = test_router_with_data(tmp.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sql")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"query": "SELECTZ INVALID"}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = json_body(resp).await;
        assert!(body["error"].is_string());
    }

    // --- Describe ---

    #[tokio::test]
    async fn test_describe_endpoint() {
        let tmp = tempfile::tempdir().unwrap();
        let app = test_router_with_data(tmp.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/describe")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        assert!(body["tables"].as_array().unwrap().len() >= 3);
        assert!(body["relationships"].is_array());
    }

    #[tokio::test]
    async fn test_describe_source_filter() {
        let tmp = tempfile::tempdir().unwrap();
        let app = test_router_with_data(tmp.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/describe/demo")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        let tables = body["tables"].as_array().unwrap();
        assert!(!tables.is_empty());
        for t in tables {
            assert_eq!(t["source"], "demo");
        }
    }

    #[tokio::test]
    async fn test_describe_source_filter_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let app = test_router_with_data(tmp.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/describe/ghostsource")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        assert!(body["tables"].as_array().unwrap().is_empty());
    }

    // --- Graph ---

    #[tokio::test]
    async fn test_neighbors_endpoint() {
        let tmp = tempfile::tempdir().unwrap();
        let app = test_router_with_data(tmp.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/graph/neighbors")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "table": "team_members",
                            "key": "Alice Chen",
                            "depth": 1
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        assert!(body["nodes"].as_array().unwrap().len() >= 2);
        assert!(!body["edges"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_path_endpoint() {
        let tmp = tempfile::tempdir().unwrap();
        crate::demo::generate(tmp.path()).unwrap();
        let api = Arc::new(TeidelumApi::open(tmp.path()).unwrap());
        api.register_relationships(vec![Relationship {
            from_table: "project_tasks".to_string(),
            from_col: "assignee".to_string(),
            to_table: "team_members".to_string(),
            to_col: "name".to_string(),
            relation: "assigned_to".to_string(),
        }])
        .unwrap();

        // Get a real task/assignee pair
        let result = api
            .query("SELECT title, assignee FROM project_tasks LIMIT 1")
            .unwrap();
        let title = match &result.rows[0][0] {
            Value::String(s) => s.clone(),
            other => panic!("expected String, got {other:?}"),
        };
        let assignee = match &result.rows[0][1] {
            Value::String(s) => s.clone(),
            other => panic!("expected String, got {other:?}"),
        };

        let app = super::api_routes().with_state(api);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/graph/path")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "table": "project_tasks",
                            "key": title,
                            "key_col": "title",
                            "to_table": "team_members",
                            "to_key": assignee,
                            "to_key_col": "name",
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        assert_eq!(body["found"], true);
        assert!(body["path"].as_array().unwrap().len() >= 2);
    }

    #[tokio::test]
    async fn test_path_endpoint_not_found() {
        let tmp = tempfile::tempdir().unwrap();
        let app = test_router_with_data(tmp.path());

        // When the target key does not exist in the database at all, the graph
        // engine returns an error (target node not found), which the handler
        // maps to BAD_REQUEST.
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/graph/path")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "table": "team_members",
                            "key": "Alice Chen",
                            "key_col": "name",
                            "to_table": "incidents",
                            "to_key": "NONEXISTENT",
                            "to_key_col": "title",
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = json_body(resp).await;
        assert!(body["error"].as_str().unwrap().contains("not found"));
    }

    // --- Tables CRUD ---

    #[tokio::test]
    async fn test_create_table_endpoint() {
        let tmp = tempfile::tempdir().unwrap();
        let app = test_router(tmp.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/tables")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "name": "users",
                            "source": "test",
                            "columns": [
                                {"name": "id", "type": "integer"},
                                {"name": "name", "type": "string"}
                            ],
                            "rows": [[1, "Alice"], [2, "Bob"]]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED);
        let body = json_body(resp).await;
        assert_eq!(body["table"], "users");
        assert_eq!(body["rows_inserted"], 2);
    }

    #[tokio::test]
    async fn test_create_table_invalid_name() {
        let tmp = tempfile::tempdir().unwrap();
        let app = test_router(tmp.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/tables")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "name": "'; DROP TABLE x;--",
                            "source": "test",
                            "columns": [{"name": "id", "type": "integer"}],
                            "rows": []
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let body = json_body(resp).await;
        assert!(body["error"]
            .as_str()
            .unwrap()
            .contains("invalid identifier"));
    }

    #[tokio::test]
    async fn test_insert_rows_endpoint() {
        let tmp = tempfile::tempdir().unwrap();
        let api = Arc::new(TeidelumApi::new(tmp.path()).unwrap());
        api.create_table(
            "items",
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
            ],
            &[vec![Value::Int(1), Value::String("first".to_string())]],
        )
        .unwrap();

        let app = super::api_routes().with_state(api);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/tables/items/rows")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "rows": [[2, "second"], [3, "third"]]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        assert_eq!(body["rows_inserted"], 2);
    }

    #[tokio::test]
    async fn test_insert_rows_nonexistent_table() {
        let tmp = tempfile::tempdir().unwrap();
        let app = test_router(tmp.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/tables/ghost/rows")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"rows": [[1, "a"]]}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_delete_table_endpoint() {
        let tmp = tempfile::tempdir().unwrap();
        let api = Arc::new(TeidelumApi::new(tmp.path()).unwrap());
        api.create_table(
            "ephemeral",
            "test",
            &[ColumnSchema {
                name: "id".to_string(),
                dtype: "i64".to_string(),
            }],
            &[vec![Value::Int(1)]],
        )
        .unwrap();

        let app = super::api_routes().with_state(api);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/v1/tables/ephemeral")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        assert_eq!(body["deleted"], "ephemeral");
    }

    #[tokio::test]
    async fn test_delete_table_nonexistent() {
        let tmp = tempfile::tempdir().unwrap();
        let app = test_router(tmp.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/v1/tables/ghost")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    // --- Documents ---

    #[tokio::test]
    async fn test_add_documents_endpoint() {
        let tmp = tempfile::tempdir().unwrap();
        let app = test_router(tmp.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/documents")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "documents": [
                                {"id": "d1", "source": "test", "title": "Doc One", "body": "content one"},
                                {"id": "d2", "source": "test", "title": "Doc Two", "body": "content two"}
                            ]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED);
        let body = json_body(resp).await;
        assert_eq!(body["documents_indexed"], 2);
    }

    #[tokio::test]
    async fn test_delete_document_endpoint() {
        let tmp = tempfile::tempdir().unwrap();
        let api = Arc::new(TeidelumApi::new(tmp.path()).unwrap());
        api.add_documents(&[SearchDocument {
            id: "d1".to_string(),
            source: "test".to_string(),
            title: "Title".to_string(),
            body: "body".to_string(),
            metadata: serde_json::Map::new(),
        }])
        .unwrap();

        let app = super::api_routes().with_state(api);
        let resp = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/v1/documents/d1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        assert_eq!(body["deleted"], "d1");
    }

    // --- Relationships ---

    #[tokio::test]
    async fn test_add_relationship_endpoint() {
        let tmp = tempfile::tempdir().unwrap();
        let app = test_router(tmp.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/relationships")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "from_table": "tasks",
                            "from_col": "owner",
                            "to_table": "people",
                            "to_col": "name",
                            "relation": "owned_by"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::CREATED);
        let body = json_body(resp).await;
        assert!(body["relationship"]
            .as_str()
            .unwrap()
            .contains("tasks.owner"));
    }

    #[tokio::test]
    async fn test_add_relationship_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        let app = test_router(tmp.path());

        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/relationships")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "from_table": "bad table!",
                            "from_col": "col",
                            "to_table": "t2",
                            "to_col": "col",
                            "relation": "rel"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // --- End-to-end roundtrip ---

    #[tokio::test]
    async fn test_create_then_query_roundtrip() {
        let tmp = tempfile::tempdir().unwrap();
        let api = Arc::new(TeidelumApi::new(tmp.path()).unwrap());
        let app = super::api_routes().with_state(api);

        // Create table
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/tables")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({
                            "name": "products",
                            "source": "test",
                            "columns": [
                                {"name": "id", "type": "integer"},
                                {"name": "name", "type": "string"},
                                {"name": "price", "type": "float"}
                            ],
                            "rows": [[1, "Widget", 9.99], [2, "Gadget", 19.99]]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);

        // Query it back
        let resp = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sql")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        serde_json::json!({"query": "SELECT name, price FROM products WHERE price > 10"})
                            .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        let rows = body["rows"].as_array().unwrap();
        assert_eq!(rows.len(), 1);

        // Verify describe shows it
        let resp = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/api/v1/describe")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = json_body(resp).await;
        let tables = body["tables"].as_array().unwrap();
        assert!(tables.iter().any(|t| t["name"] == "products"));
    }
}
