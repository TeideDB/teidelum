use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use teidelum::api::TeidelumApi;
use teidelum::server::build_router;

/// Create a test app backed by a fresh temporary directory.
///
/// Returns the router and the `TempDir` guard — the caller must keep
/// the `TempDir` alive for the duration of the test so that the
/// tantivy mmap index directory is not deleted prematurely.
fn test_app() -> (axum::Router, tempfile::TempDir) {
    let tmp = tempfile::tempdir().unwrap();
    let api = TeidelumApi::new(tmp.path()).unwrap();
    let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());
    let ct = tokio_util::sync::CancellationToken::new();
    (build_router(Arc::new(api), hub, None, ct), tmp)
}

fn json_request(method: &str, uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap()
}

#[tokio::test]
async fn test_create_table_and_query() {
    let (app, _tmp) = test_app();

    // Create table
    let req = json_request(
        "POST",
        "/api/v1/tables",
        serde_json::json!({
            "name": "users",
            "source": "test",
            "columns": [
                {"name": "id", "type": "int"},
                {"name": "name", "type": "varchar"}
            ],
            "rows": [[1, "Alice"], [2, "Bob"]]
        }),
    );
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["table"], "users");
    assert_eq!(json["rows_inserted"], 2);

    // Query via SQL
    let req = json_request(
        "POST",
        "/api/v1/sql",
        serde_json::json!({"query": "SELECT name FROM users WHERE id = 1"}),
    );
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Describe
    let req = Request::builder()
        .uri("/api/v1/describe")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["tables"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn test_insert_rows() {
    let (app, _tmp) = test_app();

    // Create table first
    let req = json_request(
        "POST",
        "/api/v1/tables",
        serde_json::json!({
            "name": "items",
            "source": "test",
            "columns": [{"name": "id", "type": "int"}, {"name": "label", "type": "varchar"}],
            "rows": [[1, "first"]]
        }),
    );
    app.clone().oneshot(req).await.unwrap();

    // Insert more rows
    let req = json_request(
        "POST",
        "/api/v1/tables/items/rows",
        serde_json::json!({"rows": [[2, "second"], [3, "third"]]}),
    );
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["rows_inserted"], 2);
}

#[tokio::test]
async fn test_delete_table() {
    let (app, _tmp) = test_app();

    // Create then delete
    let req = json_request(
        "POST",
        "/api/v1/tables",
        serde_json::json!({
            "name": "ephemeral",
            "source": "test",
            "columns": [{"name": "id", "type": "int"}],
            "rows": []
        }),
    );
    app.clone().oneshot(req).await.unwrap();

    let req = Request::builder()
        .method("DELETE")
        .uri("/api/v1/tables/ephemeral")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_add_and_search_documents() {
    let (app, _tmp) = test_app();

    // Add documents
    let req = json_request(
        "POST",
        "/api/v1/documents",
        serde_json::json!({
            "documents": [
                {"id": "doc1", "source": "test", "title": "Auth Guide", "body": "JWT authentication tokens"}
            ]
        }),
    );
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Search
    let req = json_request(
        "POST",
        "/api/v1/search",
        serde_json::json!({"query": "JWT authentication", "limit": 5}),
    );
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_add_relationship() {
    let (app, _tmp) = test_app();

    let req = json_request(
        "POST",
        "/api/v1/relationships",
        serde_json::json!({
            "from_table": "orders",
            "from_col": "customer_id",
            "to_table": "customers",
            "to_col": "id",
            "relation": "belongs_to"
        }),
    );
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
}
