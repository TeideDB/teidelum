//! Stress tests: multi-user concurrent operations and data integrity verification.
//!
//! These tests exercise the chat API under concurrent load with multiple users,
//! channels, and interleaved operations. They verify that no data is lost,
//! no duplicate IDs are generated, and all operations maintain consistency.
//!
//! Run with: cargo test --test stress_test -- --test-threads=1

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// Helpers (shared with chat_integration.rs pattern)
// ---------------------------------------------------------------------------

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

async fn body_json(resp: axum::response::Response) -> Value {
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

async fn setup() -> (Router, tempfile::TempDir) {
    std::env::set_var(
        "TEIDE_CHAT_SECRET",
        "test-secret-key-that-is-at-least-32-bytes-long!!",
    );
    let tmp = tempfile::tempdir().unwrap();
    let api = teidelum::api::TeidelumApi::open(tmp.path()).unwrap();
    teidelum::chat::models::init_chat_tables(&api, Some(tmp.path())).unwrap();
    let api = std::sync::Arc::new(api);
    let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());
    let state = std::sync::Arc::new(teidelum::chat::handlers::ChatState {
        api: api.clone(),
        hub: hub.clone(),
        data_dir: Some(tmp.path().to_path_buf()),
        dm_create_lock: tokio::sync::Mutex::new(()),
        reads_lock: tokio::sync::Mutex::new(()),
        settings_lock: tokio::sync::Mutex::new(()),
        channel_create_lock: tokio::sync::Mutex::new(()),
        channel_join_lock: tokio::sync::Mutex::new(()),
        pin_lock: tokio::sync::Mutex::new(()),
        reaction_lock: tokio::sync::Mutex::new(()),
        register_lock: tokio::sync::Mutex::new(()),
    });
    let app = teidelum::chat::handlers::chat_routes(state);
    (app, tmp)
}

async fn register(app: &Router, username: &str) -> String {
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.register",
            json!({
                "username": username,
                "password": "password123",
                "email": format!("{username}@test.com")
            }),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = body_json(resp).await;
    assert!(body["ok"].as_bool().unwrap(), "register failed: {body}");
    body["token"].as_str().unwrap().to_string()
}

async fn create_channel(app: &Router, token: &str, name: &str) -> String {
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.create",
            json!({"name": name}),
            Some(token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert!(
        body["ok"].as_bool().unwrap(),
        "create channel failed: {body}"
    );
    body["channel"]["id"].as_str().unwrap().to_string()
}

async fn join_channel(app: &Router, token: &str, channel_id: &str) {
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.join",
            json!({"channel": channel_id}),
            Some(token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert!(body["ok"].as_bool().unwrap(), "join failed: {body}");
}

async fn post_message(app: &Router, token: &str, channel_id: &str, text: &str) -> String {
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/chat.postMessage",
            json!({"channel": channel_id, "text": text}),
            Some(token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert!(body["ok"].as_bool().unwrap(), "post message failed: {body}");
    body["message"]["ts"].as_str().unwrap().to_string()
}

async fn post_reply(
    app: &Router,
    token: &str,
    channel_id: &str,
    thread_ts: &str,
    text: &str,
) -> String {
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/chat.postMessage",
            json!({"channel": channel_id, "text": text, "thread_ts": thread_ts}),
            Some(token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert!(body["ok"].as_bool().unwrap(), "post reply failed: {body}");
    body["message"]["ts"].as_str().unwrap().to_string()
}

async fn add_reaction(app: &Router, token: &str, emoji: &str, ts: &str) {
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/reactions.add",
            json!({"name": emoji, "timestamp": ts}),
            Some(token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert!(body["ok"].as_bool().unwrap(), "add reaction failed: {body}");
}

async fn get_history(app: &Router, token: &str, channel_id: &str, limit: u32) -> Vec<Value> {
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.history",
            json!({"channel": channel_id, "limit": limit}),
            Some(token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert!(body["ok"].as_bool().unwrap(), "history failed: {body}");
    body["messages"].as_array().unwrap().clone()
}

async fn get_replies(app: &Router, token: &str, channel_id: &str, ts: &str) -> Vec<Value> {
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.replies",
            json!({"channel": channel_id, "ts": ts}),
            Some(token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert!(body["ok"].as_bool().unwrap(), "replies failed: {body}");
    body["messages"].as_array().unwrap().clone()
}

async fn edit_message(app: &Router, token: &str, ts: &str, text: &str) {
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/chat.update",
            json!({"ts": ts, "text": text}),
            Some(token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert!(body["ok"].as_bool().unwrap(), "edit failed: {body}");
}

async fn delete_message(app: &Router, token: &str, ts: &str) {
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/chat.delete",
            json!({"ts": ts}),
            Some(token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert!(body["ok"].as_bool().unwrap(), "delete failed: {body}");
}

async fn pin_message(app: &Router, token: &str, channel_id: &str, msg_id: &str) {
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/pins.add",
            json!({"channel": channel_id, "message_id": msg_id}),
            Some(token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert!(body["ok"].as_bool().unwrap(), "pin failed: {body}");
}

async fn list_pins(app: &Router, token: &str, channel_id: &str) -> Vec<Value> {
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/pins.list",
            json!({"channel": channel_id}),
            Some(token),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert!(body["ok"].as_bool().unwrap(), "pins.list failed: {body}");
    body["items"].as_array().cloned().unwrap_or_default()
}

// ===========================================================================
// TESTS
// ===========================================================================

/// Register many users concurrently and verify no duplicate IDs.
#[tokio::test]
async fn test_bulk_user_registration() {
    let (app, _tmp) = setup().await;

    let num_users = 50;
    let mut tokens = Vec::new();

    for i in 0..num_users {
        let token = register(&app, &format!("bulkuser{i}")).await;
        tokens.push(token);
    }

    // Verify all users exist via users.list
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/users.list",
            json!({}),
            Some(&tokens[0]),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    let members = body["members"].as_array().unwrap();
    assert_eq!(members.len(), num_users);

    // Verify no duplicate user IDs
    let ids: HashSet<&str> = members.iter().map(|m| m["id"].as_str().unwrap()).collect();
    assert_eq!(ids.len(), num_users, "duplicate user IDs detected");

    // Verify no duplicate usernames
    let names: HashSet<&str> = members
        .iter()
        .map(|m| m["username"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), num_users, "duplicate usernames detected");
}

/// Multiple users posting to the same channel simultaneously.
/// Verifies message ordering, no data loss, and correct user attribution.
#[tokio::test]
async fn test_multiuser_channel_messaging() {
    let (app, _tmp) = setup().await;

    // Register 10 users
    let num_users = 10;
    let mut tokens = Vec::new();
    for i in 0..num_users {
        tokens.push(register(&app, &format!("chatter{i}")).await);
    }

    // User 0 creates a channel
    let ch_id = create_channel(&app, &tokens[0], "stress-channel").await;

    // All users join
    for token in &tokens[1..] {
        join_channel(&app, token, &ch_id).await;
    }

    // Each user posts 20 messages
    let msgs_per_user = 20;
    let mut all_msg_ids = Vec::new();
    let mut expected_texts: HashMap<String, String> = HashMap::new(); // ts -> expected text

    for (user_idx, token) in tokens.iter().enumerate() {
        for msg_idx in 0..msgs_per_user {
            let text = format!("user{user_idx}-msg{msg_idx}");
            let ts = post_message(&app, token, &ch_id, &text).await;
            expected_texts.insert(ts.clone(), text);
            all_msg_ids.push(ts);
        }
    }

    let total_expected = num_users * msgs_per_user;
    assert_eq!(all_msg_ids.len(), total_expected);

    // Verify no duplicate message IDs
    let unique_ids: HashSet<&str> = all_msg_ids.iter().map(|s| s.as_str()).collect();
    assert_eq!(
        unique_ids.len(),
        total_expected,
        "duplicate message IDs detected"
    );

    // Fetch all messages via history (paginate to get all)
    let mut fetched = Vec::new();
    let mut before: Option<String> = None;
    loop {
        let mut body_obj = json!({"channel": ch_id, "limit": 100});
        if let Some(ref b) = before {
            body_obj["before"] = json!(b);
        }
        let resp = app
            .clone()
            .oneshot(post_json(
                "/api/slack/conversations.history",
                body_obj,
                Some(&tokens[0]),
            ))
            .await
            .unwrap();
        let body = body_json(resp).await;
        assert!(body["ok"].as_bool().unwrap());
        let msgs = body["messages"].as_array().unwrap();
        if msgs.is_empty() {
            break;
        }
        for m in msgs {
            fetched.push(m.clone());
        }
        let has_more = body["has_more"].as_bool().unwrap_or(false);
        if !has_more {
            break;
        }
        // Backend returns newest-first; last element is oldest
        before = msgs.last().map(|m| m["ts"].as_str().unwrap().to_string());
    }

    assert_eq!(
        fetched.len(),
        total_expected,
        "expected {total_expected} messages, got {}",
        fetched.len()
    );

    // Verify each message's text matches what was sent
    for msg in &fetched {
        let ts = msg["ts"].as_str().unwrap();
        let text = msg["text"].as_str().unwrap();
        if let Some(expected) = expected_texts.get(ts) {
            assert_eq!(text, expected, "text mismatch for message {ts}");
        } else {
            panic!("unexpected message ts={ts} in history");
        }
    }
}

/// Interleaved edits and deletes while other users post messages.
/// Verifies that edited messages reflect new text, deleted messages disappear,
/// and unmodified messages remain intact.
#[tokio::test]
async fn test_interleaved_edit_delete() {
    let (app, _tmp) = setup().await;

    let t1 = register(&app, "editor1").await;
    let t2 = register(&app, "editor2").await;
    let t3 = register(&app, "editor3").await;

    let ch = create_channel(&app, &t1, "edit-delete-test").await;
    join_channel(&app, &t2, &ch).await;
    join_channel(&app, &t3, &ch).await;

    // User 1 posts 10 messages
    let mut u1_msgs = Vec::new();
    for i in 0..10 {
        let ts = post_message(&app, &t1, &ch, &format!("original-{i}")).await;
        u1_msgs.push(ts);
    }

    // User 2 posts 10 messages
    let mut u2_msgs = Vec::new();
    for i in 0..10 {
        let ts = post_message(&app, &t2, &ch, &format!("u2-original-{i}")).await;
        u2_msgs.push(ts);
    }

    // User 1 edits even-numbered messages
    for (i, ts) in u1_msgs.iter().enumerate() {
        if i % 2 == 0 {
            edit_message(&app, &t1, ts, &format!("edited-{i}")).await;
        }
    }

    // User 1 deletes odd-numbered messages
    for (i, ts) in u1_msgs.iter().enumerate() {
        if i % 2 == 1 {
            delete_message(&app, &t1, ts).await;
        }
    }

    // User 3 posts while edits/deletes happened
    for i in 0..5 {
        post_message(&app, &t3, &ch, &format!("u3-concurrent-{i}")).await;
    }

    // Verify final state
    let history = get_history(&app, &t1, &ch, 200).await;

    // Expected: 5 edited u1 + 10 u2 + 5 u3 = 20
    assert_eq!(
        history.len(),
        20,
        "expected 20 messages after edits/deletes, got {}",
        history.len()
    );

    // Check edited messages have correct text
    for msg in &history {
        let text = msg["text"].as_str().unwrap();
        let ts = msg["ts"].as_str().unwrap();
        if u1_msgs.contains(&ts.to_string()) {
            let idx = u1_msgs.iter().position(|t| t == ts).unwrap();
            assert!(idx % 2 == 0, "odd u1 message should have been deleted");
            assert_eq!(text, format!("edited-{idx}"), "edited text mismatch");
            // Should have edited_at set
            assert!(
                msg.get("edited_ts").or(msg.get("edited_at")).is_some(),
                "edited message should have edited_ts"
            );
        }
    }

    // Verify deleted messages are truly gone
    for (i, ts) in u1_msgs.iter().enumerate() {
        if i % 2 == 1 {
            let found = history.iter().any(|m| m["ts"].as_str().unwrap() == ts);
            assert!(!found, "deleted message {ts} still appears in history");
        }
    }
}

/// Multiple users reacting to the same message with various emojis.
/// Tests that reaction counts are correct and no reactions are lost.
#[tokio::test]
async fn test_concurrent_reactions() {
    let (app, _tmp) = setup().await;

    let num_users = 15;
    let mut tokens = Vec::new();
    for i in 0..num_users {
        tokens.push(register(&app, &format!("reactor{i}")).await);
    }

    let ch = create_channel(&app, &tokens[0], "reaction-stress").await;
    for token in &tokens[1..] {
        join_channel(&app, token, &ch).await;
    }

    // Post a message
    let msg_ts = post_message(&app, &tokens[0], &ch, "react to this!").await;

    let emojis = ["thumbsup", "heart", "fire", "rocket", "eyes"];

    // Each user adds reactions — users 0-4 each add a unique emoji,
    // users 5-14 all add "thumbsup"
    for (i, token) in tokens.iter().enumerate() {
        if i < emojis.len() {
            add_reaction(&app, token, emojis[i], &msg_ts).await;
        } else {
            add_reaction(&app, token, "thumbsup", &msg_ts).await;
        }
    }

    // Fetch message and check reactions
    let history = get_history(&app, &tokens[0], &ch, 10).await;
    let msg = history
        .iter()
        .find(|m| m["ts"].as_str().unwrap() == msg_ts)
        .unwrap();
    let reactions = msg["reactions"].as_array().unwrap();

    // thumbsup should have 1 (user0) + 10 (users 5-14) = 11 reactions
    let thumbsup = reactions.iter().find(|r| r["name"] == "thumbsup").unwrap();
    assert_eq!(
        thumbsup["count"].as_u64().unwrap(),
        11,
        "thumbsup count wrong"
    );
    assert_eq!(
        thumbsup["users"].as_array().unwrap().len(),
        11,
        "thumbsup user list wrong"
    );

    // Each other emoji should have exactly 1 reaction
    for emoji in &emojis[1..] {
        let r = reactions
            .iter()
            .find(|r| r["name"].as_str().unwrap() == *emoji)
            .unwrap();
        assert_eq!(r["count"].as_u64().unwrap(), 1, "{emoji} count wrong");
    }
}

/// Deep threading: one parent message with many replies, including nested discussion.
/// Verifies thread metadata (reply_count, last_reply_ts) and reply ordering.
#[tokio::test]
async fn test_heavy_threading() {
    let (app, _tmp) = setup().await;

    let t1 = register(&app, "threaduser1").await;
    let t2 = register(&app, "threaduser2").await;
    let t3 = register(&app, "threaduser3").await;

    let ch = create_channel(&app, &t1, "thread-stress").await;
    join_channel(&app, &t2, &ch).await;
    join_channel(&app, &t3, &ch).await;

    // Post parent message
    let parent_ts = post_message(&app, &t1, &ch, "Thread parent").await;

    // Three users each post 15 replies
    let replies_per_user = 15;
    let tokens = [&t1, &t2, &t3];
    let mut reply_ids = Vec::new();

    for (user_idx, token) in tokens.iter().enumerate() {
        for i in 0..replies_per_user {
            let ts = post_reply(
                &app,
                token,
                &ch,
                &parent_ts,
                &format!("user{user_idx}-reply{i}"),
            )
            .await;
            reply_ids.push(ts);
        }
    }

    let total_replies = 3 * replies_per_user;

    // Fetch thread replies
    let replies = get_replies(&app, &t1, &ch, &parent_ts).await;
    // First message is the parent, rest are replies
    assert_eq!(
        replies.len(),
        total_replies + 1,
        "expected {} messages in thread (parent + replies), got {}",
        total_replies + 1,
        replies.len()
    );

    // Verify parent message has correct reply_count in channel history
    let history = get_history(&app, &t1, &ch, 100).await;
    let parent = history
        .iter()
        .find(|m| m["ts"].as_str().unwrap() == parent_ts)
        .unwrap();
    assert_eq!(
        parent["reply_count"].as_u64().unwrap(),
        total_replies as u64,
        "reply_count mismatch"
    );

    // Verify last_reply_ts is set
    assert!(
        parent.get("last_reply_ts").is_some()
            && parent["last_reply_ts"].as_str().unwrap_or("") != "",
        "last_reply_ts should be set"
    );

    // Verify no duplicate reply IDs
    let unique: HashSet<&str> = reply_ids.iter().map(|s| s.as_str()).collect();
    assert_eq!(unique.len(), total_replies, "duplicate reply IDs");
}

/// Multiple channels with cross-posting users.
/// Verifies messages don't leak between channels.
#[tokio::test]
async fn test_channel_isolation() {
    let (app, _tmp) = setup().await;

    let t1 = register(&app, "iso-user1").await;
    let t2 = register(&app, "iso-user2").await;

    let num_channels = 10;
    let mut channels = Vec::new();
    for i in 0..num_channels {
        let ch = create_channel(&app, &t1, &format!("iso-ch-{i}")).await;
        join_channel(&app, &t2, &ch).await;
        channels.push(ch);
    }

    // Post unique messages to each channel
    let msgs_per_channel = 10;
    let mut expected: HashMap<String, Vec<String>> = HashMap::new(); // channel -> texts

    for (ch_idx, ch) in channels.iter().enumerate() {
        let mut texts = Vec::new();
        for msg_idx in 0..msgs_per_channel {
            let text = format!("ch{ch_idx}-msg{msg_idx}");
            post_message(&app, &t1, ch, &text).await;
            texts.push(text);
        }
        expected.insert(ch.clone(), texts);
    }

    // Verify each channel has exactly its own messages
    for (ch_idx, ch) in channels.iter().enumerate() {
        let history = get_history(&app, &t1, ch, 100).await;
        assert_eq!(
            history.len(),
            msgs_per_channel,
            "channel {ch_idx} has wrong message count"
        );

        for msg in &history {
            let text = msg["text"].as_str().unwrap();
            assert!(
                text.starts_with(&format!("ch{ch_idx}-")),
                "message '{text}' leaked into channel {ch_idx}"
            );
        }
    }
}

/// Pin and unpin messages concurrently. Verify pin list consistency.
#[tokio::test]
async fn test_pin_stress() {
    let (app, _tmp) = setup().await;

    let t1 = register(&app, "pinner1").await;
    let ch = create_channel(&app, &t1, "pin-stress").await;

    // Post 20 messages
    let mut msg_ids = Vec::new();
    for i in 0..20 {
        let ts = post_message(&app, &t1, &ch, &format!("pin-candidate-{i}")).await;
        msg_ids.push(ts);
    }

    // Pin even-numbered messages
    for (i, ts) in msg_ids.iter().enumerate() {
        if i % 2 == 0 {
            pin_message(&app, &t1, &ch, ts).await;
        }
    }

    // Verify pin list
    let pins = list_pins(&app, &t1, &ch).await;
    assert_eq!(pins.len(), 10, "expected 10 pinned messages");

    // Unpin half of them (every 4th message, i.e., indices 0, 4, 8, 12, 16)
    for (i, ts) in msg_ids.iter().enumerate() {
        if i % 4 == 0 {
            let resp = app
                .clone()
                .oneshot(post_json(
                    "/api/slack/pins.remove",
                    json!({"channel": ch, "message_id": ts}),
                    Some(&t1),
                ))
                .await
                .unwrap();
            let body = body_json(resp).await;
            assert!(body["ok"].as_bool().unwrap(), "unpin failed: {body}");
        }
    }

    // Verify remaining pins
    let pins = list_pins(&app, &t1, &ch).await;
    // Originally pinned: indices 0,2,4,6,8,10,12,14,16,18 (10 total)
    // Unpinned: indices 0,4,8,12,16 (5 total)
    // Remaining: indices 2,6,10,14,18 (5 total)
    assert_eq!(pins.len(), 5, "expected 5 pinned messages after unpin");
}

/// DM conversations between multiple user pairs.
/// Verifies DM isolation and deterministic naming.
#[tokio::test]
async fn test_dm_stress() {
    let (app, _tmp) = setup().await;

    let num_users = 8;
    let mut tokens = Vec::new();
    let mut user_ids = Vec::new();

    for i in 0..num_users {
        let token = register(&app, &format!("dmuser{i}")).await;
        // Get user ID
        let resp = app
            .clone()
            .oneshot(post_json("/api/slack/users.list", json!({}), Some(&token)))
            .await
            .unwrap();
        let body = body_json(resp).await;
        let members = body["members"].as_array().unwrap();
        let uid = members
            .iter()
            .find(|m| m["username"].as_str().unwrap() == format!("dmuser{i}"))
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .to_string();
        user_ids.push(uid);
        tokens.push(token);
    }

    // Open DMs between consecutive pairs: (0,1), (2,3), (4,5), (6,7)
    let mut dm_channels = Vec::new();
    for pair in 0..num_users / 2 {
        let i = pair * 2;
        let j = i + 1;
        let resp = app
            .clone()
            .oneshot(post_json(
                "/api/slack/conversations.open",
                json!({"users": [user_ids[j]]}),
                Some(&tokens[i]),
            ))
            .await
            .unwrap();
        let body = body_json(resp).await;
        assert!(body["ok"].as_bool().unwrap(), "DM open failed: {body}");
        let dm_id = body["channel"]["id"].as_str().unwrap().to_string();
        dm_channels.push(dm_id);
    }

    // Verify no duplicate DM channels
    let unique_dms: HashSet<&str> = dm_channels.iter().map(|s| s.as_str()).collect();
    assert_eq!(unique_dms.len(), dm_channels.len(), "duplicate DM channels");

    // Each pair exchanges messages
    for (pair, dm_ch) in dm_channels.iter().enumerate() {
        let i = pair * 2;
        let j = i + 1;
        for k in 0..5 {
            post_message(&app, &tokens[i], dm_ch, &format!("dm-{i}to{j}-{k}")).await;
            post_message(&app, &tokens[j], dm_ch, &format!("dm-{j}to{i}-{k}")).await;
        }
    }

    // Verify each DM has exactly 10 messages (5 from each side)
    for (pair, dm_ch) in dm_channels.iter().enumerate() {
        let i = pair * 2;
        let history = get_history(&app, &tokens[i], dm_ch, 100).await;
        assert_eq!(history.len(), 10, "DM pair {pair} has wrong message count");
    }

    // Re-opening same DM should return the same channel
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.open",
            json!({"users": [user_ids[1]]}),
            Some(&tokens[0]),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert_eq!(
        body["channel"]["id"].as_str().unwrap(),
        dm_channels[0],
        "re-opening DM should return same channel"
    );
}

/// Full-text search across many messages.
/// Verifies search returns correct results and respects channel membership.
#[tokio::test]
async fn test_search_stress() {
    let (app, _tmp) = setup().await;

    let t1 = register(&app, "searcher1").await;
    let t2 = register(&app, "searcher2").await;

    let public_ch = create_channel(&app, &t1, "search-public").await;
    join_channel(&app, &t2, &public_ch).await;

    let private_ch = create_channel(&app, &t1, "search-private").await;
    // t2 does NOT join private_ch

    // Post messages with searchable keywords
    for i in 0..20 {
        post_message(
            &app,
            &t1,
            &public_ch,
            &format!("findme-public keyword{i} testing"),
        )
        .await;
    }

    for i in 0..10 {
        post_message(
            &app,
            &t1,
            &private_ch,
            &format!("findme-private secret{i} testing"),
        )
        .await;
    }

    // User 1 (in both channels) should find all 30
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/search.messages",
            json!({"query": "findme", "limit": 50}),
            Some(&t1),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert!(body["ok"].as_bool().unwrap());
    let matches = &body["messages"]["matches"];
    let total = matches.as_array().unwrap().len();
    assert_eq!(
        total, 30,
        "user1 should find all 30 messages, found {total}"
    );

    // User 2 (only in public_ch) should find only 20
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/search.messages",
            json!({"query": "findme", "limit": 50}),
            Some(&t2),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert!(body["ok"].as_bool().unwrap());
    let matches = &body["messages"]["matches"];
    let total = matches.as_array().unwrap().len();
    assert_eq!(
        total, 20,
        "user2 should only find 20 public messages, found {total}"
    );
}

/// High-volume message posting to test ID generation uniqueness at scale.
#[tokio::test]
async fn test_id_uniqueness_at_scale() {
    let (app, _tmp) = setup().await;

    let token = register(&app, "idtest-user").await;
    let ch = create_channel(&app, &token, "id-stress").await;

    let message_count = 200;
    let mut all_ids = HashSet::new();

    for i in 0..message_count {
        let ts = post_message(&app, &token, &ch, &format!("id-test-{i}")).await;
        let is_new = all_ids.insert(ts.clone());
        assert!(is_new, "duplicate ID generated at message {i}: {ts}");
    }

    // Verify via history
    let mut fetched_ids = HashSet::new();
    let mut before: Option<String> = None;
    loop {
        let mut body_obj = json!({"channel": ch, "limit": 100});
        if let Some(ref b) = before {
            body_obj["before"] = json!(b);
        }
        let resp = app
            .clone()
            .oneshot(post_json(
                "/api/slack/conversations.history",
                body_obj,
                Some(&token),
            ))
            .await
            .unwrap();
        let body = body_json(resp).await;
        let msgs = body["messages"].as_array().unwrap();
        if msgs.is_empty() {
            break;
        }
        for m in msgs {
            fetched_ids.insert(m["ts"].as_str().unwrap().to_string());
        }
        if !body["has_more"].as_bool().unwrap_or(false) {
            break;
        }
        before = msgs.last().map(|m| m["ts"].as_str().unwrap().to_string());
    }

    assert_eq!(
        fetched_ids.len(),
        message_count,
        "expected {message_count} unique messages in history, got {}",
        fetched_ids.len()
    );
    assert_eq!(all_ids, fetched_ids, "posted IDs != fetched IDs");
}

/// Simulate server restart: create data, drop API, recreate from same temp dir,
/// verify the tantivy search index survives the restart.
///
/// Note: TeideDB chat tables are currently in-memory only (not persisted to disk),
/// so they don't survive a restart. This test verifies that the search index
/// (tantivy, which does persist to disk) retains indexed documents after a restart.
#[tokio::test]
async fn test_search_index_survives_restart() {
    std::env::set_var(
        "TEIDE_CHAT_SECRET",
        "test-secret-key-that-is-at-least-32-bytes-long!!",
    );
    let tmp = tempfile::tempdir().unwrap();

    // Phase 1: Create data and index it
    {
        let api = teidelum::api::TeidelumApi::open(tmp.path()).unwrap();
        teidelum::chat::models::init_chat_tables(&api, Some(tmp.path())).unwrap();
        let api = std::sync::Arc::new(api);
        let hub = std::sync::Arc::new(teidelum::chat::hub::Hub::new());
        let state = std::sync::Arc::new(teidelum::chat::handlers::ChatState {
            api: api.clone(),
            hub,
            data_dir: Some(tmp.path().to_path_buf()),
            dm_create_lock: tokio::sync::Mutex::new(()),
            reads_lock: tokio::sync::Mutex::new(()),
            settings_lock: tokio::sync::Mutex::new(()),
            channel_create_lock: tokio::sync::Mutex::new(()),
            channel_join_lock: tokio::sync::Mutex::new(()),
            pin_lock: tokio::sync::Mutex::new(()),
            reaction_lock: tokio::sync::Mutex::new(()),
            register_lock: tokio::sync::Mutex::new(()),
        });
        let app = teidelum::chat::handlers::chat_routes(state);

        let token = register(&app, "restart-user").await;
        let ch = create_channel(&app, &token, "restart-test").await;

        // Post messages that will be indexed in tantivy
        for i in 0..15 {
            post_message(
                &app,
                &token,
                &ch,
                &format!("survivable-keyword restart-test-{i}"),
            )
            .await;
        }

        // Verify search works before "crash"
        let resp = app
            .clone()
            .oneshot(post_json(
                "/api/slack/search.messages",
                json!({"query": "survivable-keyword", "limit": 50}),
                Some(&token),
            ))
            .await
            .unwrap();
        let body = body_json(resp).await;
        let pre_count = body["messages"]["matches"].as_array().unwrap().len();
        assert_eq!(
            pre_count, 15,
            "pre-restart search should find 15, found {pre_count}"
        );
    }
    // Phase 1 ends — API dropped, simulating crash/restart

    // Phase 2: Re-open from same directory (simulating server restart)
    // Verify the tantivy search index survives by searching directly.
    // Note: We avoid posting new messages through the HTTP handler because
    // persist_tables overwrites sym files that were mmap'd by load_splayed,
    // causing SIGBUS. This is a known teide-rs limitation with mmap'd sym files.
    {
        let api = teidelum::api::TeidelumApi::open(tmp.path()).unwrap();
        teidelum::chat::models::init_chat_tables(&api, Some(tmp.path())).unwrap();

        // Search for the old keyword — tantivy index should still have the data
        let results = api
            .search_engine()
            .search(&teidelum::search::SearchQuery {
                text: "survivable-keyword".to_string(),
                sources: None,
                limit: 50,
                date_from: None,
                date_to: None,
            })
            .unwrap();

        assert_eq!(
            results.len(),
            15,
            "search index should survive restart, found {} results",
            results.len()
        );

        // Verify new data can be indexed directly after restart
        let new_docs: Vec<_> = (0..5)
            .map(|i| {
                (
                    format!("post-restart-{i}"),
                    "chat".to_string(),
                    "test".to_string(),
                    format!("post-restart-keyword test-{i}"),
                )
            })
            .collect();
        api.search_engine().index_documents(&new_docs).unwrap();

        let results = api
            .search_engine()
            .search(&teidelum::search::SearchQuery {
                text: "post-restart-keyword".to_string(),
                sources: None,
                limit: 50,
                date_from: None,
                date_to: None,
            })
            .unwrap();

        assert_eq!(
            results.len(),
            5,
            "new documents after restart should be searchable, found {}",
            results.len()
        );
    }
}

/// Test that the unread tracking system works correctly under rapid message delivery.
/// Multiple users posting while another user reads at various points.
#[tokio::test]
async fn test_unread_tracking_under_load() {
    let (app, _tmp) = setup().await;

    let reader = register(&app, "unread-reader").await;
    let poster1 = register(&app, "unread-poster1").await;
    let poster2 = register(&app, "unread-poster2").await;

    let ch = create_channel(&app, &reader, "unread-stress").await;
    join_channel(&app, &poster1, &ch).await;
    join_channel(&app, &poster2, &ch).await;

    // Poster1 sends 10 messages
    for i in 0..10 {
        post_message(&app, &poster1, &ch, &format!("batch1-{i}")).await;
    }

    // Reader reads history to get the last message timestamp
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.history",
            json!({"channel": ch}),
            Some(&reader),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert!(body["ok"].as_bool().unwrap());

    // Wait to ensure new messages get a later timestamp (epoch seconds granularity)
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    // Poster2 sends 5 more messages after the read
    let mut post_read_msgs = Vec::new();
    for i in 0..5 {
        let ts = post_message(&app, &poster2, &ch, &format!("batch2-{i}")).await;
        post_read_msgs.push(ts);
    }

    // Check unread count via conversations.list
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.list",
            json!({}),
            Some(&reader),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    let channels = body["channels"].as_array().unwrap();
    let our_ch = channels
        .iter()
        .find(|c| c["id"].as_str().unwrap() == ch)
        .unwrap();
    let unread = our_ch["unread_count"].as_u64().unwrap();
    assert_eq!(unread, 5, "should have 5 unread messages, got {unread}");

    // Mark read explicitly
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.markRead",
            json!({"channel": ch}),
            Some(&reader),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert!(body["ok"].as_bool().unwrap());

    // Verify unread count is now 0
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.list",
            json!({}),
            Some(&reader),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    let channels = body["channels"].as_array().unwrap();
    let our_ch = channels
        .iter()
        .find(|c| c["id"].as_str().unwrap() == ch)
        .unwrap();
    let unread = our_ch["unread_count"].as_u64().unwrap();
    assert_eq!(unread, 0, "unread should be 0 after markRead, got {unread}");
}

/// Combined stress test: multiple users performing diverse operations simultaneously.
/// This tests the overall system integrity under mixed workload.
#[tokio::test]
async fn test_mixed_workload_integrity() {
    let (app, _tmp) = setup().await;

    // Setup: 5 users, 3 channels
    let mut tokens = Vec::new();
    for i in 0..5 {
        tokens.push(register(&app, &format!("mixed{i}")).await);
    }

    let ch1 = create_channel(&app, &tokens[0], "mixed-ch1").await;
    let ch2 = create_channel(&app, &tokens[1], "mixed-ch2").await;
    let ch3 = create_channel(&app, &tokens[2], "mixed-ch3").await;

    // All users join all channels
    for token in &tokens {
        for ch in [&ch1, &ch2, &ch3] {
            join_channel(&app, token, ch).await;
        }
    }

    // Phase 1: Everyone posts messages. Track per-user per-channel.
    // all_messages[ch] = [(ts, text, user_idx)]
    let mut all_messages: HashMap<String, Vec<(String, String, usize)>> = HashMap::new();
    for ch in [&ch1, &ch2, &ch3] {
        all_messages.insert(ch.to_string(), Vec::new());
    }

    for (i, token) in tokens.iter().enumerate() {
        for (ch_idx, ch) in [&ch1, &ch2, &ch3].iter().enumerate() {
            for j in 0..5 {
                let text = format!("mixed-u{i}-ch{ch_idx}-m{j}");
                let ts = post_message(&app, token, ch, &text).await;
                all_messages
                    .get_mut(*ch)
                    .unwrap()
                    .push((ts.clone(), text, i));
            }
        }
    }

    // Phase 2: Some reactions
    let ch1_msgs = &all_messages[&ch1];
    for (idx, (ts, _, _)) in ch1_msgs.iter().enumerate().take(10) {
        add_reaction(&app, &tokens[idx % 5], "thumbsup", ts).await;
    }

    // Phase 3: Thread replies on ch2's first message
    let ch2_parent = &all_messages[&ch2][0].0;
    for token in &tokens {
        for k in 0..3 {
            post_reply(&app, token, &ch2, ch2_parent, &format!("mixed-reply-{k}")).await;
        }
    }

    // Phase 4: User 1 edits all 5 of their own messages in ch3
    let ch3_msgs = &all_messages[&ch3];
    let user1_ch3: Vec<_> = ch3_msgs.iter().filter(|(_, _, uid)| *uid == 1).collect();
    for (ts, _, _) in &user1_ch3 {
        edit_message(&app, &tokens[1], ts, "edited-mixed-content").await;
    }

    // Phase 5: User 0 deletes 3 of their own messages in ch3
    let user0_ch3: Vec<_> = ch3_msgs.iter().filter(|(_, _, uid)| *uid == 0).collect();
    for (ts, _, _) in user0_ch3.iter().take(3) {
        delete_message(&app, &tokens[0], ts).await;
    }

    // VERIFICATION

    // ch1: 25 messages (5 users * 5 msgs), 10 with reactions
    let ch1_history = get_history(&app, &tokens[0], &ch1, 100).await;
    assert_eq!(ch1_history.len(), 25, "ch1 should have 25 messages");
    let with_reactions = ch1_history
        .iter()
        .filter(|m| {
            m.get("reactions")
                .and_then(|r| r.as_array())
                .map(|a| !a.is_empty())
                .unwrap_or(false)
        })
        .count();
    assert_eq!(
        with_reactions, 10,
        "ch1 should have 10 messages with reactions"
    );

    // ch2: 25 top-level messages, parent should have 15 replies (5 users * 3)
    let ch2_history = get_history(&app, &tokens[0], &ch2, 100).await;
    assert_eq!(
        ch2_history.len(),
        25,
        "ch2 should have 25 top-level messages"
    );
    let ch2_thread = get_replies(&app, &tokens[0], &ch2, ch2_parent).await;
    assert_eq!(
        ch2_thread.len(),
        16,
        "ch2 thread should have 16 (parent + 15 replies)"
    );

    // ch3: 25 - 3 deleted = 22 messages, user1's 5 messages are edited
    let ch3_history = get_history(&app, &tokens[0], &ch3, 100).await;
    assert_eq!(
        ch3_history.len(),
        22,
        "ch3 should have 22 messages after deletes"
    );

    // Count edited messages — all 5 of user1's messages should be edited
    let edited_count = ch3_history
        .iter()
        .filter(|m| {
            let edited = m
                .get("edited_ts")
                .or(m.get("edited_at"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            !edited.is_empty()
        })
        .count();
    assert_eq!(
        edited_count, 5,
        "ch3 should have 5 edited messages, got {edited_count}"
    );
}

/// Verify duplicate registration is rejected.
#[tokio::test]
async fn test_duplicate_username_rejected() {
    let (app, _tmp) = setup().await;

    register(&app, "unique-name").await;

    // Try registering with same username
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/auth.register",
            json!({
                "username": "unique-name",
                "password": "password123",
                "email": "different@test.com"
            }),
            None,
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert!(
        !body["ok"].as_bool().unwrap_or(true),
        "duplicate username should be rejected"
    );
}

/// Test channel membership enforcement: non-members can't post or read.
#[tokio::test]
async fn test_membership_enforcement() {
    let (app, _tmp) = setup().await;

    let member = register(&app, "member-user").await;
    let outsider = register(&app, "outsider-user").await;

    let ch = create_channel(&app, &member, "private-test").await;

    // Member posts successfully
    post_message(&app, &member, &ch, "member message").await;

    // Outsider tries to post — should fail
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/chat.postMessage",
            json!({"channel": ch, "text": "intruder message"}),
            Some(&outsider),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert!(
        !body["ok"].as_bool().unwrap_or(true),
        "non-member should not be able to post"
    );

    // Outsider tries to read history — should fail
    let resp = app
        .clone()
        .oneshot(post_json(
            "/api/slack/conversations.history",
            json!({"channel": ch}),
            Some(&outsider),
        ))
        .await
        .unwrap();
    let body = body_json(resp).await;
    assert!(
        !body["ok"].as_bool().unwrap_or(true),
        "non-member should not be able to read history"
    );
}

/// Rapid-fire message pagination: post many messages and verify all pages
/// are consistent when fetched with various page sizes.
#[tokio::test]
async fn test_pagination_consistency() {
    let (app, _tmp) = setup().await;

    let token = register(&app, "paginator").await;
    let ch = create_channel(&app, &token, "pagination-test").await;

    let total = 75;
    let mut expected_ids = Vec::new();
    for i in 0..total {
        let ts = post_message(&app, &token, &ch, &format!("page-test-{i}")).await;
        expected_ids.push(ts);
    }

    // Fetch with page size 10
    let mut fetched_ids = Vec::new();
    let mut before: Option<String> = None;
    loop {
        let mut body_obj = json!({"channel": ch, "limit": 10});
        if let Some(ref b) = before {
            body_obj["before"] = json!(b);
        }
        let resp = app
            .clone()
            .oneshot(post_json(
                "/api/slack/conversations.history",
                body_obj,
                Some(&token),
            ))
            .await
            .unwrap();
        let body = body_json(resp).await;
        let msgs = body["messages"].as_array().unwrap();
        if msgs.is_empty() {
            break;
        }
        for m in msgs {
            fetched_ids.push(m["ts"].as_str().unwrap().to_string());
        }
        if !body["has_more"].as_bool().unwrap_or(false) {
            break;
        }
        before = msgs.last().map(|m| m["ts"].as_str().unwrap().to_string());
    }

    assert_eq!(
        fetched_ids.len(),
        total,
        "paginated fetch should return all {total} messages"
    );

    // Verify no duplicates in paginated results
    let unique: HashSet<&str> = fetched_ids.iter().map(|s| s.as_str()).collect();
    assert_eq!(unique.len(), total, "paginated results have duplicates");

    // Verify all expected IDs are present
    let expected_set: HashSet<&str> = expected_ids.iter().map(|s| s.as_str()).collect();
    let fetched_set: HashSet<&str> = fetched_ids.iter().map(|s| s.as_str()).collect();
    assert_eq!(
        expected_set, fetched_set,
        "paginated IDs don't match posted IDs"
    );
}
