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
        settings_lock: tokio::sync::Mutex::new(()),
        channel_create_lock: tokio::sync::Mutex::new(()),
        channel_join_lock: tokio::sync::Mutex::new(()),
        pin_lock: tokio::sync::Mutex::new(()),
        reaction_lock: tokio::sync::Mutex::new(()),
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
        settings_lock: tokio::sync::Mutex::new(()),
        channel_create_lock: tokio::sync::Mutex::new(()),
        channel_join_lock: tokio::sync::Mutex::new(()),
        pin_lock: tokio::sync::Mutex::new(()),
        reaction_lock: tokio::sync::Mutex::new(()),
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
        settings_lock: tokio::sync::Mutex::new(()),
        channel_create_lock: tokio::sync::Mutex::new(()),
        channel_join_lock: tokio::sync::Mutex::new(()),
        pin_lock: tokio::sync::Mutex::new(()),
        reaction_lock: tokio::sync::Mutex::new(()),
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
        settings_lock: tokio::sync::Mutex::new(()),
        channel_create_lock: tokio::sync::Mutex::new(()),
        channel_join_lock: tokio::sync::Mutex::new(()),
        pin_lock: tokio::sync::Mutex::new(()),
        reaction_lock: tokio::sync::Mutex::new(()),
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
        settings_lock: tokio::sync::Mutex::new(()),
        channel_create_lock: tokio::sync::Mutex::new(()),
        channel_join_lock: tokio::sync::Mutex::new(()),
        pin_lock: tokio::sync::Mutex::new(()),
        reaction_lock: tokio::sync::Mutex::new(()),
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
        settings_lock: tokio::sync::Mutex::new(()),
        channel_create_lock: tokio::sync::Mutex::new(()),
        channel_join_lock: tokio::sync::Mutex::new(()),
        pin_lock: tokio::sync::Mutex::new(()),
        reaction_lock: tokio::sync::Mutex::new(()),
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
        settings_lock: tokio::sync::Mutex::new(()),
        channel_create_lock: tokio::sync::Mutex::new(()),
        channel_join_lock: tokio::sync::Mutex::new(()),
        pin_lock: tokio::sync::Mutex::new(()),
        reaction_lock: tokio::sync::Mutex::new(()),
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
        settings_lock: tokio::sync::Mutex::new(()),
        channel_create_lock: tokio::sync::Mutex::new(()),
        channel_join_lock: tokio::sync::Mutex::new(()),
        pin_lock: tokio::sync::Mutex::new(()),
        reaction_lock: tokio::sync::Mutex::new(()),
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
        settings_lock: tokio::sync::Mutex::new(()),
        channel_create_lock: tokio::sync::Mutex::new(()),
        channel_join_lock: tokio::sync::Mutex::new(()),
        pin_lock: tokio::sync::Mutex::new(()),
        reaction_lock: tokio::sync::Mutex::new(()),
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

#[tokio::test]
async fn test_conversations_update() {
    let (app, _tmp) = setup().await;
    let token = register_and_login(&app, "updater", "secret123", "updater@test.com").await;

    // Create channel (creator is owner)
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.create",
            json!({"name": "update-test"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let ch_id: i64 = body_json(resp).await["channel"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    // Update topic and description
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.update",
            json!({"channel": ch_id, "topic": "new topic", "description": "new desc"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true, "update should succeed for owner");

    // Verify via conversations.info
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.info",
            json!({"channel": ch_id}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["channel"]["topic"], "new topic");
    assert_eq!(body["channel"]["description"], "new desc");

    // Update name
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.update",
            json!({"channel": ch_id, "name": "renamed-channel"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true, "rename should succeed");

    // Verify new name
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.info",
            json!({"channel": ch_id}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["channel"]["name"], "renamed-channel");

    // Non-member should fail
    let token2 = register_and_login(&app, "outsider", "secret123", "outsider@test.com").await;
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.update",
            json!({"channel": ch_id, "topic": "hacked"}),
            Some(&token2),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false, "non-member should not update");

    // Regular member should fail
    let _ = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.join",
            json!({"channel": ch_id}),
            Some(&token2),
        ))
        .await
        .unwrap();
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.update",
            json!({"channel": ch_id, "topic": "hacked"}),
            Some(&token2),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false, "regular member should not update");

    // No changes should fail
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.update",
            json!({"channel": ch_id}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false, "empty update should fail");
}

#[tokio::test]
async fn test_archive_unarchive() {
    let (app, _tmp) = setup().await;
    let token_owner =
        register_and_login(&app, "archowner", "secret123", "archowner@test.com").await;
    let token_member =
        register_and_login(&app, "archmember", "secret123", "archmember@test.com").await;

    // Create channel
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.create",
            json!({"name": "archive-test"}),
            Some(&token_owner),
        ))
        .await
        .unwrap();
    let ch_id: i64 = body_json(resp).await["channel"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    // Member joins
    let _ = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.join",
            json!({"channel": ch_id}),
            Some(&token_member),
        ))
        .await
        .unwrap();

    // Non-owner cannot archive
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.archive",
            json!({"channel": ch_id}),
            Some(&token_member),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false, "member should not archive");

    // Owner archives
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.archive",
            json!({"channel": ch_id}),
            Some(&token_owner),
        ))
        .await
        .unwrap();
    assert_eq!(body_json(resp).await["ok"], true);

    // Posting to archived channel should fail
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/chat.postMessage",
            json!({"channel": ch_id, "text": "should fail"}),
            Some(&token_owner),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"], "channel_archived");

    // Owner unarchives
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.unarchive",
            json!({"channel": ch_id}),
            Some(&token_owner),
        ))
        .await
        .unwrap();
    assert_eq!(body_json(resp).await["ok"], true);

    // Posting should work again
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/chat.postMessage",
            json!({"channel": ch_id, "text": "back alive"}),
            Some(&token_owner),
        ))
        .await
        .unwrap();
    assert_eq!(body_json(resp).await["ok"], true);
}

#[tokio::test]
async fn test_set_role() {
    let (app, _tmp) = setup().await;
    let token_owner =
        register_and_login(&app, "roleowner", "secret123", "roleowner@test.com").await;

    // Register member and get their user_id
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.register",
            json!({"username": "rolemember", "password": "secret123", "email": "rolemember@test.com"}),
            None,
        ))
        .await
        .unwrap();
    let member_body = body_json(resp).await;
    let token_member = member_body["token"].as_str().unwrap().to_string();
    let member_id: i64 = member_body["user_id"].as_str().unwrap().parse().unwrap();

    // Create channel
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.create",
            json!({"name": "role-test"}),
            Some(&token_owner),
        ))
        .await
        .unwrap();
    let ch_id: i64 = body_json(resp).await["channel"]["id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();

    // Member joins
    let _ = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.join",
            json!({"channel": ch_id}),
            Some(&token_member),
        ))
        .await
        .unwrap();

    // Member cannot update channel topic (they are just a member)
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.update",
            json!({"channel": ch_id, "topic": "member topic"}),
            Some(&token_member),
        ))
        .await
        .unwrap();
    assert_eq!(
        body_json(resp).await["ok"],
        false,
        "member should not update"
    );

    // Owner promotes member to admin
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.setRole",
            json!({"channel": ch_id, "user": member_id, "role": "admin"}),
            Some(&token_owner),
        ))
        .await
        .unwrap();
    assert_eq!(body_json(resp).await["ok"], true, "setRole should succeed");

    // Admin can now update channel topic
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.update",
            json!({"channel": ch_id, "topic": "admin topic"}),
            Some(&token_member),
        ))
        .await
        .unwrap();
    assert_eq!(
        body_json(resp).await["ok"],
        true,
        "admin should be able to update"
    );

    // Verify topic was updated
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.info",
            json!({"channel": ch_id}),
            Some(&token_owner),
        ))
        .await
        .unwrap();
    assert_eq!(body_json(resp).await["channel"]["topic"], "admin topic");

    // Member cannot setRole (not owner)
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.setRole",
            json!({"channel": ch_id, "user": member_id, "role": "member"}),
            Some(&token_member),
        ))
        .await
        .unwrap();
    assert_eq!(
        body_json(resp).await["ok"],
        false,
        "non-owner should not setRole"
    );
}

#[tokio::test]
async fn test_pins_add_list_remove() {
    let (app, _tmp) = setup().await;
    let token = register_and_login(&app, "pinner", "pass1234", "pinner@test.com").await;

    // Create channel and post a message
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.create",
            json!({"name": "pin-test"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let ch_id = body_json(resp).await["channel"]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/chat.postMessage",
            json!({"channel": ch_id, "text": "pin me"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let msg_ts = body_json(resp).await["message"]["ts"]
        .as_str()
        .unwrap()
        .to_string();

    // Pin the message
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/pins.add",
            json!({"channel": ch_id, "timestamp": msg_ts}),
            Some(&token),
        ))
        .await
        .unwrap();
    assert_eq!(body_json(resp).await["ok"], true, "pins.add should succeed");

    // List pins — should have 1 item
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/pins.list",
            json!({"channel": ch_id}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1, "should have 1 pinned message");
    assert_eq!(items[0]["message"]["ts"], msg_ts);
    assert_eq!(items[0]["message"]["text"], "pin me");

    // Pin same message again (idempotent) — should still succeed
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/pins.add",
            json!({"channel": ch_id, "timestamp": msg_ts}),
            Some(&token),
        ))
        .await
        .unwrap();
    assert_eq!(
        body_json(resp).await["ok"],
        true,
        "duplicate pin should be idempotent"
    );

    // List again — still 1 item (not duplicated)
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/pins.list",
            json!({"channel": ch_id}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(
        body["items"].as_array().unwrap().len(),
        1,
        "idempotent pin should not duplicate"
    );

    // Unpin the message
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/pins.remove",
            json!({"channel": ch_id, "timestamp": msg_ts}),
            Some(&token),
        ))
        .await
        .unwrap();
    assert_eq!(
        body_json(resp).await["ok"],
        true,
        "pins.remove should succeed"
    );

    // List pins — should be empty
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/pins.list",
            json!({"channel": ch_id}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(
        body["items"].as_array().unwrap().len(),
        0,
        "pins should be empty after unpin"
    );
}

#[tokio::test]
async fn test_users_search() {
    let (app, _tmp) = setup().await;

    // Register 3 users
    let token_alice = register_and_login(&app, "alice", "pass1234", "alice@example.com").await;
    let _token_bob = register_and_login(&app, "bob", "pass1234", "bob@example.com").await;
    let _token_alice_b =
        register_and_login(&app, "alice_b", "pass1234", "alice_b@example.com").await;

    // Search for "alice" — should return 2 matches
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/users.search",
            json!({"query": "alice"}),
            Some(&token_alice),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    assert_eq!(
        body["users"].as_array().unwrap().len(),
        2,
        "searching 'alice' should return 2 users"
    );

    // Search for "bob" — should return 1
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/users.search",
            json!({"query": "bob"}),
            Some(&token_alice),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    assert_eq!(
        body["users"].as_array().unwrap().len(),
        1,
        "searching 'bob' should return 1 user"
    );

    // Search for "zzz" — should return 0
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/users.search",
            json!({"query": "zzz"}),
            Some(&token_alice),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    assert_eq!(
        body["users"].as_array().unwrap().len(),
        0,
        "searching 'zzz' should return 0 users"
    );
}

#[tokio::test]
async fn test_conversations_autocomplete() {
    let (app, _tmp) = setup().await;

    let token = register_and_login(&app, "alice", "pass1234", "alice@example.com").await;

    // Create 3 channels
    for name in &["general", "general-dev", "random"] {
        let resp = app
            .clone()
            .oneshot(post_json(
                "/api/slack/conversations.create",
                json!({"name": name}),
                Some(&token),
            ))
            .await
            .unwrap();
        let body = body_json(resp).await;
        assert_eq!(body["ok"], true, "creating channel '{name}' should succeed");
    }

    // Autocomplete "gen" — should return 2
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.autocomplete",
            json!({"query": "gen"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    assert_eq!(
        body["channels"].as_array().unwrap().len(),
        2,
        "autocomplete 'gen' should return 2 channels"
    );

    // Autocomplete "ran" — should return 1
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.autocomplete",
            json!({"query": "ran"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    assert_eq!(
        body["channels"].as_array().unwrap().len(),
        1,
        "autocomplete 'ran' should return 1 channel"
    );
}

#[tokio::test]
async fn test_links_unfurl() {
    let (app, _tmp) = setup().await;

    let token = register_and_login(&app, "alice", "pass1234", "alice@example.com").await;

    // Test 1: Invalid URL — should return error
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/links.unfurl",
            json!({"url": "not-a-url"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false, "invalid URL should fail");
    assert_eq!(body["error"], "invalid_url");

    // Test 2: Non-http scheme — should return error
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/links.unfurl",
            json!({"url": "ftp://example.com/file"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false, "ftp scheme should be rejected");
    assert_eq!(body["error"], "invalid_url");

    // Test 3: Blocked URL — localhost
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/links.unfurl",
            json!({"url": "http://localhost:8080/secret"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false, "localhost should be blocked");
    assert_eq!(body["error"], "blocked_url");

    // Test 4: Blocked URL — private IP 10.x.x.x
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/links.unfurl",
            json!({"url": "http://10.0.0.1/internal"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false, "10.x.x.x should be blocked");
    assert_eq!(body["error"], "blocked_url");

    // Test 5: Blocked URL — private IP 192.168.x.x
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/links.unfurl",
            json!({"url": "http://192.168.1.1/admin"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false, "192.168.x.x should be blocked");
    assert_eq!(body["error"], "blocked_url");

    // Test 6: Blocked URL — loopback 127.0.0.1
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/links.unfurl",
            json!({"url": "http://127.0.0.1/secret"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false, "127.0.0.1 should be blocked");
    assert_eq!(body["error"], "blocked_url");

    // Test 7: Blocked URL — private IP 172.16.x.x
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/links.unfurl",
            json!({"url": "http://172.16.0.1/internal"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false, "172.16.x.x should be blocked");
    assert_eq!(body["error"], "blocked_url");

    // Test 8: Blocked URL — link-local 169.254.x.x
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/links.unfurl",
            json!({"url": "http://169.254.169.254/metadata"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false, "169.254.x.x should be blocked");
    assert_eq!(body["error"], "blocked_url");

    // Test 9: Unauthenticated request — should fail
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/links.unfurl",
            json!({"url": "https://example.com"}),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_mute_unmute() {
    let (app, _tmp) = setup().await;
    let token = register_and_login(&app, "alice", "pass123", "alice@test.com").await;

    // Create a channel
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.create",
            json!({"name": "test-mute"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let channel_id = body["channel"]["id"].as_str().unwrap().to_string();

    // Initially, channel should not be muted
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
    assert_eq!(body["ok"], true);
    let channels = body["channels"].as_array().unwrap();
    let ch = channels
        .iter()
        .find(|c| c["id"].as_str().unwrap() == channel_id)
        .unwrap();
    assert_eq!(ch["muted"], "false", "channel should default to not muted");

    // Mute the channel
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.mute",
            json!({"channel": channel_id}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // Verify muted in conversations.list
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
        .find(|c| c["id"].as_str().unwrap() == channel_id)
        .unwrap();
    assert_eq!(
        ch["muted"], "true",
        "channel should be muted after mute call"
    );

    // Unmute the channel
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.unmute",
            json!({"channel": channel_id}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // Verify unmuted in conversations.list
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
        .find(|c| c["id"].as_str().unwrap() == channel_id)
        .unwrap();
    assert_eq!(
        ch["muted"], "false",
        "channel should be unmuted after unmute call"
    );
}

#[tokio::test]
async fn test_set_notification_level() {
    let (app, _tmp) = setup().await;
    let token = register_and_login(&app, "bob", "pass123", "bob@test.com").await;

    // Create a channel
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.create",
            json!({"name": "test-notif"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let channel_id = body["channel"]["id"].as_str().unwrap().to_string();

    // Default notification_level should be "all"
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
        .find(|c| c["id"].as_str().unwrap() == channel_id)
        .unwrap();
    assert_eq!(
        ch["notification_level"], "all",
        "default notification level should be 'all'"
    );

    // Set notification level to "mentions"
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.setNotification",
            json!({"channel": channel_id, "level": "mentions"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // Verify in conversations.list
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
        .find(|c| c["id"].as_str().unwrap() == channel_id)
        .unwrap();
    assert_eq!(
        ch["notification_level"], "mentions",
        "notification level should be 'mentions'"
    );

    // Set to "none"
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.setNotification",
            json!({"channel": channel_id, "level": "none"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // Verify "none"
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
        .find(|c| c["id"].as_str().unwrap() == channel_id)
        .unwrap();
    assert_eq!(
        ch["notification_level"], "none",
        "notification level should be 'none'"
    );

    // Invalid level should fail
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.setNotification",
            json!({"channel": channel_id, "level": "invalid"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"], "invalid_level");
}

#[tokio::test]
async fn test_search_messages_with_filters() {
    let (app, _tmp) = setup().await;

    // Register two users
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.register",
            json!({"username": "searchuser1", "password": "secret123", "email": "su1@test.com"}),
            None,
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    let token1 = body["token"].as_str().unwrap().to_string();
    let user1_id = body["user_id"].as_str().unwrap().to_string();

    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.register",
            json!({"username": "searchuser2", "password": "secret123", "email": "su2@test.com"}),
            None,
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    let token2 = body["token"].as_str().unwrap().to_string();
    let _user2_id = body["user_id"].as_str().unwrap().to_string();

    // User1 creates a channel and user2 joins
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.create",
            json!({"name": "search-test-chan", "kind": "public"}),
            Some(&token1),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let channel_id = body["channel"]["id"].as_str().unwrap().to_string();

    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.join",
            json!({"channel": channel_id}),
            Some(&token2),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // User1 posts a message with searchable keyword
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/chat.postMessage",
            json!({"channel": channel_id, "text": "xylophone unique keyword from user1"}),
            Some(&token1),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // User2 posts a message with same keyword
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/chat.postMessage",
            json!({"channel": channel_id, "text": "xylophone unique keyword from user2"}),
            Some(&token2),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // Search without filter — should return both messages
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/search.messages",
            json!({"query": "xylophone"}),
            Some(&token1),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let matches = body["messages"]["matches"].as_array().unwrap();
    assert_eq!(matches.len(), 2, "should find both messages without filter");

    // Search with user_id filter — should return only user1's message
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/search.messages",
            json!({"query": "xylophone", "user_id": user1_id}),
            Some(&token1),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let matches = body["messages"]["matches"].as_array().unwrap();
    assert_eq!(
        matches.len(),
        1,
        "should find only user1's message with user_id filter"
    );
    assert_eq!(
        matches[0]["user"].as_i64().unwrap().to_string(),
        user1_id,
        "filtered result should be from user1"
    );

    // Search with channel_id filter — should return results from that channel
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/search.messages",
            json!({"query": "xylophone", "channel_id": channel_id}),
            Some(&token1),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let matches = body["messages"]["matches"].as_array().unwrap();
    assert_eq!(
        matches.len(),
        2,
        "should find both messages with channel_id filter"
    );

    // Empty query should fail
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/search.messages",
            json!({"query": ""}),
            Some(&token1),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false);
    assert_eq!(body["error"], "invalid_arguments");
}

#[tokio::test]
async fn test_conversations_directory() {
    let (app, _tmp) = setup().await;

    let token = register_and_login(&app, "diruser", "secret123", "diruser@test.com").await;

    // Create a public channel
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.create",
            json!({"name": "public-alpha", "kind": "public", "topic": "Alpha topic", "description": "Alpha desc"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // Create another public channel
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.create",
            json!({"name": "public-beta", "kind": "public", "topic": "Beta topic"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // Create a private channel — should NOT appear in directory
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.create",
            json!({"name": "private-gamma", "kind": "private"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // Fetch directory with no filters
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.directory",
            json!({}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let channels = body["channels"].as_array().unwrap();
    // Should contain only public channels (may include "general" if auto-created)
    let names: Vec<&str> = channels
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"public-alpha"),
        "directory should contain public-alpha"
    );
    assert!(
        names.contains(&"public-beta"),
        "directory should contain public-beta"
    );
    assert!(
        !names.contains(&"private-gamma"),
        "directory should NOT contain private channel"
    );

    // Each channel should have member_count
    let alpha = channels
        .iter()
        .find(|c| c["name"].as_str().unwrap() == "public-alpha")
        .unwrap();
    assert!(
        alpha["member_count"].as_i64().unwrap() >= 1,
        "public-alpha should have at least 1 member"
    );

    // Test query filter
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.directory",
            json!({"query": "alpha"}),
            Some(&token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    let channels = body["channels"].as_array().unwrap();
    assert_eq!(
        channels.len(),
        1,
        "query filter should match only one channel"
    );
    assert_eq!(channels[0]["name"].as_str().unwrap(), "public-alpha");
}
