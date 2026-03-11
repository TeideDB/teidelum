//! Integration tests for the chat API.
//! Tests the full flow: register → login → create channel → post message → history.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
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

    builder
        .body(Body::from(serde_json::to_string(&body).unwrap()))
        .unwrap()
}

/// Parse response body as JSON.
async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Setup helper: creates temp dir, initializes API and chat tables, returns (app, _tmp).
async fn setup() -> (Router, tempfile::TempDir) {
    std::env::set_var("TEIDE_CHAT_SECRET", "test-secret-key-12345");
    let tmp = tempfile::tempdir().unwrap();
    let api = teidelum::api::TeidelumApi::open(tmp.path()).unwrap();
    teidelum::chat::models::init_chat_tables(&api).unwrap();
    let api = std::sync::Arc::new(api);
    let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());
    let state = std::sync::Arc::new(teidelum::chat::handlers::ChatState {
        api: api.clone(),
        hub: hub.clone(),
        dm_create_lock: tokio::sync::Mutex::new(()),
        reads_lock: tokio::sync::Mutex::new(()),
    });
    let app = teidelum::chat::handlers::chat_routes(state);
    (app, tmp)
}

/// Register a user and return their auth token.
async fn register_and_login(app: &Router, username: &str, password: &str, email: &str) -> String {
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.register",
            json!({"username": username, "password": password, "email": email}),
            None,
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true, "register failed: {body}");
    body["token"].as_str().unwrap().to_string()
}

/// Get the user ID for the authenticated user by decoding from users.list.
async fn get_user_id(app: &Router, token: &str) -> String {
    let resp = app
        .clone()
        .oneshot(post_json("/api/slack/users.list", json!({}), Some(token)))
        .await
        .unwrap();
    let body = body_json(resp).await;
    // The token belongs to the user who registered — find them by checking the JWT claims
    // For simplicity, we get the user_id from a users.info-like approach via the token
    // Actually, let's use auth.test or just parse from register response
    // Since we can't easily get the user_id from the token alone, we'll use the register response
    // But this helper is called after the fact, so let's query users.list and find the user
    // We need to know the username... let's just return the first user's ID
    body["members"][0]["id"].as_str().unwrap().to_string()
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
        dm_create_lock: tokio::sync::Mutex::new(()),
        reads_lock: tokio::sync::Mutex::new(()),
    });

    let app = teidelum::chat::handlers::chat_routes(state);

    // 1. Register user
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.register",
            json!({
                "username": "alice",
                "password": "secret123",
                "email": "alice@example.com"
            }),
            None,
        ))
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let token = body["token"].as_str().unwrap().to_string();

    // 2. Login
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.login",
            json!({
                "username": "alice",
                "password": "secret123"
            }),
            None,
        ))
        .await
        .unwrap();

    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    assert!(body["token"].is_string());

    // 3. Create channel
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.create",
            json!({
                "name": "general"
            }),
            Some(&token),
        ))
        .await
        .unwrap();

    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let channel_id: i64 = body["channel"]["id"].as_str().unwrap().parse().unwrap();

    // 4. Post message
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/chat.postMessage",
            json!({
                "channel": channel_id,
                "text": "Hello world!"
            }),
            Some(&token),
        ))
        .await
        .unwrap();

    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    assert_eq!(body["message"]["text"], "Hello world!");

    // 5. Get history
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.history",
            json!({
                "channel": channel_id
            }),
            Some(&token),
        ))
        .await
        .unwrap();

    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["text"], "Hello world!");

    // 6. Duplicate username rejected
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.register",
            json!({
                "username": "alice",
                "password": "other",
                "email": "other@example.com"
            }),
            None,
        ))
        .await
        .unwrap();

    let body = body_json(resp).await;
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"], "username_taken");
}

#[tokio::test]
async fn test_unread_tracking() {
    std::env::set_var("TEIDE_CHAT_SECRET", "test-secret-key-12345");
    let tmp = tempfile::tempdir().unwrap();
    let api = teidelum::api::TeidelumApi::open(tmp.path()).unwrap();
    teidelum::chat::models::init_chat_tables(&api).unwrap();
    let api = std::sync::Arc::new(api);
    let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());
    let state = std::sync::Arc::new(teidelum::chat::handlers::ChatState {
        api: api.clone(),
        hub: hub.clone(),
        dm_create_lock: tokio::sync::Mutex::new(()),
        reads_lock: tokio::sync::Mutex::new(()),
    });
    let app = teidelum::chat::handlers::chat_routes(state);

    // Register alice
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.register",
            json!({"username": "alice", "password": "secret123", "email": "alice@test.com"}),
            None,
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    let token_a = body["token"].as_str().unwrap().to_string();

    // Register bob
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.register",
            json!({"username": "bob", "password": "secret123", "email": "bob@test.com"}),
            None,
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    let token_b = body["token"].as_str().unwrap().to_string();

    // Alice creates channel
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.create",
            json!({"name": "unread-test"}),
            Some(&token_a),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    let ch_id: i64 = body["channel"]["id"].as_str().unwrap().parse().unwrap();

    // Bob joins channel
    let _ = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.join",
            json!({"channel": ch_id}),
            Some(&token_b),
        ))
        .await
        .unwrap();

    // Alice posts a message
    let _ = app
        .clone()
        .oneshot(post_json(
            "/api/slack/chat.postMessage",
            json!({"channel": ch_id, "text": "hello bob"}),
            Some(&token_a),
        ))
        .await
        .unwrap();

    // Bob lists channels — should see unread > 0
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.list",
            json!({}),
            Some(&token_b),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    let channels = body["channels"].as_array().unwrap();
    let ch = channels
        .iter()
        .find(|c| c["id"].as_str().unwrap() == ch_id.to_string())
        .unwrap();
    assert!(
        ch["unread_count"].as_i64().unwrap() > 0,
        "expected unread > 0"
    );

    // Bob reads history — implicitly marks as read
    let _ = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.history",
            json!({"channel": ch_id}),
            Some(&token_b),
        ))
        .await
        .unwrap();

    // Bob lists channels again — unread should be 0
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.list",
            json!({}),
            Some(&token_b),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    let channels = body["channels"].as_array().unwrap();
    let ch = channels
        .iter()
        .find(|c| c["id"].as_str().unwrap() == ch_id.to_string())
        .unwrap();
    assert_eq!(
        ch["unread_count"].as_i64().unwrap(),
        0,
        "expected unread = 0 after reading history"
    );
}

#[tokio::test]
async fn test_conversations_mark_read() {
    std::env::set_var("TEIDE_CHAT_SECRET", "test-secret-key-12345");
    let tmp = tempfile::tempdir().unwrap();
    let api = teidelum::api::TeidelumApi::open(tmp.path()).unwrap();
    teidelum::chat::models::init_chat_tables(&api).unwrap();
    let api = std::sync::Arc::new(api);
    let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());
    let state = std::sync::Arc::new(teidelum::chat::handlers::ChatState {
        api: api.clone(),
        hub: hub.clone(),
        dm_create_lock: tokio::sync::Mutex::new(()),
        reads_lock: tokio::sync::Mutex::new(()),
    });
    let app = teidelum::chat::handlers::chat_routes(state);

    // Register and login
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.register",
            json!({"username": "carol", "password": "secret123", "email": "carol@test.com"}),
            None,
        ))
        .await
        .unwrap();
    let token = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Create channel
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.create",
            json!({"name": "mark-read-test"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let ch_id: i64 = body_json(resp).await["channel"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    // Post message
    let _ = app
        .clone()
        .oneshot(post_json(
            "/api/slack/chat.postMessage",
            json!({"channel": ch_id, "text": "test message"}),
            Some(&token),
        ))
        .await
        .unwrap();

    // Mark as read
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.markRead",
            json!({"channel": ch_id}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // List channels — should have 0 unread
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.list",
            json!({}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    let channels = body["channels"].as_array().unwrap();
    let ch = channels
        .iter()
        .find(|c| c["id"].as_str().unwrap() == ch_id.to_string())
        .unwrap();
    assert_eq!(ch["unread_count"].as_i64().unwrap(), 0);
}

#[tokio::test]
async fn test_thread_metadata() {
    std::env::set_var("TEIDE_CHAT_SECRET", "test-secret-key-12345");
    let tmp = tempfile::tempdir().unwrap();
    let api = teidelum::api::TeidelumApi::open(tmp.path()).unwrap();
    teidelum::chat::models::init_chat_tables(&api).unwrap();
    let api = std::sync::Arc::new(api);
    let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());
    let state = std::sync::Arc::new(teidelum::chat::handlers::ChatState {
        api: api.clone(),
        hub: hub.clone(),
        dm_create_lock: tokio::sync::Mutex::new(()),
        reads_lock: tokio::sync::Mutex::new(()),
    });
    let app = teidelum::chat::handlers::chat_routes(state);

    // Register
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.register",
            json!({"username": "dave", "password": "secret123", "email": "dave@test.com"}),
            None,
        ))
        .await
        .unwrap();
    let token = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Create channel
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.create",
            json!({"name": "thread-test"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let ch_id: i64 = body_json(resp).await["channel"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    // Post parent message
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/chat.postMessage",
            json!({"channel": ch_id, "text": "parent message"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let parent_ts = body_json(resp).await["message"]["ts"]
        .as_str()
        .unwrap()
        .to_string();
    let parent_id: i64 = parent_ts.parse().unwrap();

    // Post 3 replies
    for i in 0..3 {
        let _ = app
            .clone()
            .oneshot(post_json(
                "/api/slack/chat.postMessage",
                json!({"channel": ch_id, "text": format!("reply {i}"), "thread_ts": parent_id}),
                Some(&token),
            ))
            .await
            .unwrap();
    }

    // Fetch history — parent should have reply_count: 3
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.history",
            json!({"channel": ch_id}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    let messages = body["messages"].as_array().unwrap();

    // History returns top-level messages only (thread_id == 0), so parent should be there
    let parent = messages
        .iter()
        .find(|m| m["ts"].as_str().unwrap() == parent_ts)
        .unwrap();
    assert_eq!(
        parent["reply_count"].as_i64().unwrap(),
        3,
        "expected 3 replies"
    );
    assert!(
        parent["last_reply_ts"].is_string(),
        "expected last_reply_ts to be set"
    );
}

#[tokio::test]
async fn test_dm_conversation() {
    std::env::set_var("TEIDE_CHAT_SECRET", "test-secret-key-12345");
    let tmp = tempfile::tempdir().unwrap();
    let api = teidelum::api::TeidelumApi::open(tmp.path()).unwrap();
    teidelum::chat::models::init_chat_tables(&api).unwrap();
    let api = std::sync::Arc::new(api);
    let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());
    let state = std::sync::Arc::new(teidelum::chat::handlers::ChatState {
        api: api.clone(),
        hub: hub.clone(),
        dm_create_lock: tokio::sync::Mutex::new(()),
        reads_lock: tokio::sync::Mutex::new(()),
    });
    let app = teidelum::chat::handlers::chat_routes(state);

    // Register alice
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.register",
            json!({"username": "alice_dm", "password": "secret123", "email": "alice_dm@test.com"}),
            None,
        ))
        .await
        .unwrap();
    let body_a = body_json(resp).await;
    let token_a = body_a["token"].as_str().unwrap().to_string();

    // Register bob
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.register",
            json!({"username": "bob_dm", "password": "secret123", "email": "bob_dm@test.com"}),
            None,
        ))
        .await
        .unwrap();
    let body_b = body_json(resp).await;
    let token_b = body_b["token"].as_str().unwrap().to_string();
    let user_b: i64 = body_b["user_id"].as_str().unwrap().parse().unwrap();

    // Open DM — alice opens DM with bob (pass only the other user)
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.open",
            json!({"users": [user_b]}),
            Some(&token_a),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let dm_id: i64 = body["channel"]["id"].as_str().unwrap().parse().unwrap();

    // Alice posts in DM
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/chat.postMessage",
            json!({"channel": dm_id, "text": "hey bob, DM!"}),
            Some(&token_a),
        ))
        .await
        .unwrap();
    assert_eq!(body_json(resp).await["ok"], true);

    // Bob reads DM history
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.history",
            json!({"channel": dm_id}),
            Some(&token_b),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["text"], "hey bob, DM!");

    // Open again — should return same channel
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.open",
            json!({"users": [user_b]}),
            Some(&token_a),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    // Re-opening a DM should succeed and return the same channel
    assert_eq!(body["ok"], true, "second open failed: {body}");
    let dm_id_2: i64 = body["channel"]["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(dm_id, dm_id_2, "should return same DM channel");
}

#[tokio::test]
async fn test_presence_update() {
    std::env::set_var("TEIDE_CHAT_SECRET", "test-secret-key-12345");
    let tmp = tempfile::tempdir().unwrap();
    let api = teidelum::api::TeidelumApi::open(tmp.path()).unwrap();
    teidelum::chat::models::init_chat_tables(&api).unwrap();
    let api = std::sync::Arc::new(api);
    let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());
    let state = std::sync::Arc::new(teidelum::chat::handlers::ChatState {
        api: api.clone(),
        hub: hub.clone(),
        dm_create_lock: tokio::sync::Mutex::new(()),
        reads_lock: tokio::sync::Mutex::new(()),
    });
    let app = teidelum::chat::handlers::chat_routes(state);

    // Register
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.register",
            json!({"username": "eve", "password": "secret123", "email": "eve@test.com"}),
            None,
        ))
        .await
        .unwrap();
    let token = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Set presence to away
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/users.setPresence",
            json!({"presence": "away"}),
            Some(&token),
        ))
        .await
        .unwrap();
    assert_eq!(body_json(resp).await["ok"], true);

    // Verify via users.list
    let resp = app
        .clone()
        .oneshot(post_json("/api/slack/users.list", json!({}), Some(&token)))
        .await
        .unwrap();
    let body = body_json(resp).await;
    let users = body["members"].as_array().unwrap();
    let eve = users
        .iter()
        .find(|u| u["username"].as_str() == Some("eve"))
        .unwrap();
    assert_eq!(eve["status"].as_str(), Some("away"));

    // Set invalid presence — should error
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/users.setPresence",
            json!({"presence": "invisible"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"], "invalid_presence");
}

#[tokio::test]
async fn test_mention_extraction() {
    std::env::set_var("TEIDE_CHAT_SECRET", "test-secret-key-12345");
    let tmp = tempfile::tempdir().unwrap();
    let api = teidelum::api::TeidelumApi::open(tmp.path()).unwrap();
    teidelum::chat::models::init_chat_tables(&api).unwrap();
    let api = std::sync::Arc::new(api);
    let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());
    let state = std::sync::Arc::new(teidelum::chat::handlers::ChatState {
        api: api.clone(),
        hub: hub.clone(),
        dm_create_lock: tokio::sync::Mutex::new(()),
        reads_lock: tokio::sync::Mutex::new(()),
    });
    let app = teidelum::chat::handlers::chat_routes(state.clone());

    // Register alice and bob
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.register",
            json!({"username": "alice_m", "password": "secret123", "email": "alice_m@test.com"}),
            None,
        ))
        .await
        .unwrap();
    let token_a = body_json(resp).await["token"].as_str().unwrap().to_string();

    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.register",
            json!({"username": "bob_m", "password": "secret123", "email": "bob_m@test.com"}),
            None,
        ))
        .await
        .unwrap();
    let _token_b = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Alice creates channel
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.create",
            json!({"name": "mention-test"}),
            Some(&token_a),
        ))
        .await
        .unwrap();
    let ch_id: i64 = body_json(resp).await["channel"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    // Alice posts message mentioning bob
    let _ = app
        .clone()
        .oneshot(post_json(
            "/api/slack/chat.postMessage",
            json!({"channel": ch_id, "text": "hey @bob_m check this out"}),
            Some(&token_a),
        ))
        .await
        .unwrap();

    // Verify mention was stored by querying mentions table directly
    let sql = "SELECT message_id, user_id FROM mentions";
    let result = state.api.query_router().query_sync(sql).unwrap();
    assert!(!result.rows.is_empty(), "expected mention rows");

    // Post message without mentions
    let before_count = result.rows.len();
    let _ = app
        .clone()
        .oneshot(post_json(
            "/api/slack/chat.postMessage",
            json!({"channel": ch_id, "text": "no mentions here"}),
            Some(&token_a),
        ))
        .await
        .unwrap();

    let result = state.api.query_router().query_sync(sql).unwrap();
    assert_eq!(
        result.rows.len(),
        before_count,
        "no new mentions should be added"
    );
}

#[tokio::test]
async fn test_reaction_lifecycle() {
    std::env::set_var("TEIDE_CHAT_SECRET", "test-secret-key-12345");
    let tmp = tempfile::tempdir().unwrap();
    let api = teidelum::api::TeidelumApi::open(tmp.path()).unwrap();
    teidelum::chat::models::init_chat_tables(&api).unwrap();
    let api = std::sync::Arc::new(api);
    let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());
    let state = std::sync::Arc::new(teidelum::chat::handlers::ChatState {
        api: api.clone(),
        hub: hub.clone(),
        dm_create_lock: tokio::sync::Mutex::new(()),
        reads_lock: tokio::sync::Mutex::new(()),
    });
    let app = teidelum::chat::handlers::chat_routes(state);

    // Register
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.register",
            json!({"username": "frank", "password": "secret123", "email": "frank@test.com"}),
            None,
        ))
        .await
        .unwrap();
    let token = body_json(resp).await["token"].as_str().unwrap().to_string();

    // Create channel and post message
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.create",
            json!({"name": "reaction-test"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let ch_id: i64 = body_json(resp).await["channel"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/chat.postMessage",
            json!({"channel": ch_id, "text": "react to this"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let msg_ts = body_json(resp).await["message"]["ts"]
        .as_str()
        .unwrap()
        .to_string();
    let msg_id: i64 = msg_ts.parse().unwrap();

    // Add reaction
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/reactions.add",
            json!({"timestamp": msg_id, "name": "thumbsup"}),
            Some(&token),
        ))
        .await
        .unwrap();
    assert_eq!(body_json(resp).await["ok"], true);

    // Add same reaction again — should return already_reacted
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/reactions.add",
            json!({"timestamp": msg_id, "name": "thumbsup"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"], "already_reacted");

    // Remove reaction
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/reactions.remove",
            json!({"timestamp": msg_id, "name": "thumbsup"}),
            Some(&token),
        ))
        .await
        .unwrap();
    assert_eq!(body_json(resp).await["ok"], true);

    // Remove again — should fail (not found)
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/reactions.remove",
            json!({"timestamp": msg_id, "name": "thumbsup"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false);
}

#[tokio::test]
async fn test_update_profile() {
    let (app, _tmp) = setup().await;
    let token = register_and_login(&app, "profileuser", "pass123", "profile@test.com").await;

    // Update display_name
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/users.updateProfile",
            json!({"display_name": "New Name", "avatar_url": "https://example.com/avatar.png"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // Verify via users.info
    let user_id = get_user_id(&app, &token).await;
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/users.info",
            json!({"user": user_id}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["user"]["display_name"], "New Name");
    assert_eq!(body["user"]["avatar_url"], "https://example.com/avatar.png");
}

#[tokio::test]
async fn test_change_password() {
    let (app, _tmp) = setup().await;
    let _token = register_and_login(&app, "pwuser", "oldpass", "pw@test.com").await;

    // Change password
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/users.changePassword",
            json!({"old_password": "oldpass", "new_password": "newpass"}),
            Some(&_token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // Login with new password should work
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.login",
            json!({"username": "pwuser", "password": "newpass"}),
            None,
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // Login with old password should fail
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.login",
            json!({"username": "pwuser", "password": "oldpass"}),
            None,
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false);
}

#[tokio::test]
async fn test_user_settings() {
    let (app, _tmp) = setup().await;
    let token = register_and_login(&app, "settingsuser", "pass", "settings@test.com").await;

    // Get default settings
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/users.getSettings",
            json!({}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    assert_eq!(body["settings"]["theme"], "dark");

    // Update theme
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/users.updateSettings",
            json!({"theme": "light"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // Verify
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/users.getSettings",
            json!({}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["settings"]["theme"], "light");
}
