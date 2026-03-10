//! Integration tests for the chat API.
//! Tests the full flow: register → login → create channel → post message → history.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

/// Helper to build a POST request with JSON body.
fn post_json(uri: &str, body: Value, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .uri(uri)
        .method("POST")
        .header("content-type", "application/json");

    if let Some(t) = token {
        builder = builder.header("authorization", format!("Bearer {t}"));
    }

    builder.body(Body::from(serde_json::to_string(&body).unwrap())).unwrap()
}

/// Parse response body as JSON.
async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn test_chat_flow() {
    // Set JWT secret for tests
    std::env::set_var("TEIDE_CHAT_SECRET", "test-secret-key-12345");

    // Create temp data dir
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path();

    // Initialize API and chat tables
    let api = teidelum::api::TeidelumApi::open(data_dir).unwrap();
    teidelum::chat::models::init_chat_tables(&api).unwrap();

    let api = std::sync::Arc::new(api);
    let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());

    let state = std::sync::Arc::new(teidelum::chat::handlers::ChatState {
        api: api.clone(),
        hub: hub.clone(),
    });

    let app = teidelum::chat::handlers::chat_routes(state);

    // 1. Register user
    let resp = app.clone().oneshot(
        post_json("/api/slack/auth.register", json!({
            "username": "alice",
            "password": "secret123",
            "email": "alice@example.com"
        }), None)
    ).await.unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let token = body["token"].as_str().unwrap().to_string();

    // 2. Login
    let resp = app.clone().oneshot(
        post_json("/api/slack/auth.login", json!({
            "username": "alice",
            "password": "secret123"
        }), None)
    ).await.unwrap();

    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    assert!(body["token"].is_string());

    // 3. Create channel
    let resp = app.clone().oneshot(
        post_json("/api/slack/conversations.create", json!({
            "name": "general"
        }), Some(&token))
    ).await.unwrap();

    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let channel_id: i64 = body["channel"]["id"].as_str().unwrap().parse().unwrap();

    // 4. Post message
    let resp = app.clone().oneshot(
        post_json("/api/slack/chat.postMessage", json!({
            "channel": channel_id,
            "text": "Hello world!"
        }), Some(&token))
    ).await.unwrap();

    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    assert_eq!(body["message"]["text"], "Hello world!");

    // 5. Get history
    let resp = app.clone().oneshot(
        post_json("/api/slack/conversations.history", json!({
            "channel": channel_id
        }), Some(&token))
    ).await.unwrap();

    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["text"], "Hello world!");

    // 6. Duplicate username rejected
    let resp = app.clone().oneshot(
        post_json("/api/slack/auth.register", json!({
            "username": "alice",
            "password": "other",
            "email": "other@example.com"
        }), None)
    ).await.unwrap();

    let body = body_json(resp).await;
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"], "username_taken");
}
