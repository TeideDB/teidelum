//! Integration tests for Chat Plan 2: Search, Files, MCP Tools.
//!
//! Tests the full flow: post messages → search indexes them,
//! file upload → file metadata stored, search via MCP tools.

use std::sync::Arc;
use teidelum::api::TeidelumApi;
use teidelum::chat::models::init_chat_tables;
use teidelum::search::SearchQuery;

/// Create a test API with chat tables initialized.
fn setup() -> (tempfile::TempDir, Arc<TeidelumApi>) {
    let tmp = tempfile::tempdir().unwrap();
    let api = TeidelumApi::new(tmp.path()).unwrap();
    init_chat_tables(&api).unwrap();
    (tmp, Arc::new(api))
}

/// Helper: insert a user directly via SQL. Returns user_id.
fn create_user(api: &TeidelumApi, username: &str, is_bot: bool) -> i64 {
    let id = teidelum::chat::id::next_id();
    let now = teidelum::chat::models::now_timestamp();
    let sql = format!(
        "INSERT INTO users (id, username, display_name, email, password_hash, avatar_url, status, status_text, status_emoji, is_bot, created_at) \
         VALUES ({id}, '{username}', '{username}', '{username}@test.com', 'hash', '', 'offline', '', '', {is_bot}, '{now}')"
    );
    api.query_router().query_sync(&sql).unwrap();
    id
}

/// Helper: create a channel and add the user as owner. Returns channel_id.
fn create_channel(api: &TeidelumApi, name: &str, user_id: i64) -> i64 {
    let id = teidelum::chat::id::next_id();
    let now = teidelum::chat::models::now_timestamp();
    let sql = format!(
        "INSERT INTO channels (id, name, kind, topic, description, archived_at, created_by, created_at) \
         VALUES ({id}, '{name}', 'public', '', '', '', {user_id}, '{now}')"
    );
    api.query_router().query_sync(&sql).unwrap();

    let member_sql = format!(
        "INSERT INTO channel_members (channel_id, user_id, role, joined_at) \
         VALUES ({id}, {user_id}, 'owner', '{now}')"
    );
    api.query_router().query_sync(&member_sql).unwrap();
    id
}

/// Helper: post a message and index it in tantivy. Returns message_id.
fn post_and_index(api: &TeidelumApi, channel_id: i64, user_id: i64, text: &str) -> i64 {
    let id = teidelum::chat::id::next_id();
    let now = teidelum::chat::models::now_timestamp();
    let sql = format!(
        "INSERT INTO messages (id, channel_id, user_id, thread_id, content, deleted_at, edited_at, created_at) \
         VALUES ({id}, {channel_id}, {user_id}, 0, '{}', NULL, NULL, '{now}')",
        teidelum::chat::models::escape_sql(text),
    );
    api.query_router().query_sync(&sql).unwrap();

    // Index in tantivy (same as what chat_post_message does)
    let name_sql = format!("SELECT name FROM channels WHERE id = {channel_id}");
    let channel_name = match api.query_router().query_sync(&name_sql) {
        Ok(r) if !r.rows.is_empty() => match &r.rows[0][0] {
            teidelum::connector::Value::String(s) => format!("#{s}"),
            _ => format!("#{channel_id}"),
        },
        _ => format!("#{channel_id}"),
    };
    let doc = vec![(
        id.to_string(),
        "chat".to_string(),
        channel_name,
        text.to_string(),
    )];
    api.search_engine().index_documents(&doc).unwrap();

    id
}

#[test]
fn test_message_search_indexing() {
    let (_tmp, api) = setup();
    let user_id = create_user(&api, "alice", false);
    let channel_id = create_channel(&api, "general", user_id);

    post_and_index(
        &api,
        channel_id,
        user_id,
        "The deployment pipeline is broken",
    );
    post_and_index(
        &api,
        channel_id,
        user_id,
        "Can someone review my pull request?",
    );
    post_and_index(
        &api,
        channel_id,
        user_id,
        "Meeting at 3pm to discuss the roadmap",
    );

    // Search for "deployment" — should find the first message
    let results = api
        .search_engine()
        .search(&SearchQuery {
            text: "deployment pipeline".to_string(),
            sources: Some(vec!["chat".to_string()]),
            limit: 10,
            date_from: None,
            date_to: None,
        })
        .unwrap();

    assert!(!results.is_empty(), "should find deployment message");
    assert_eq!(results[0].source, "chat");
    assert_eq!(results[0].title, "#general");

    // Search for "roadmap" — should find the third message
    let results = api
        .search_engine()
        .search(&SearchQuery {
            text: "roadmap".to_string(),
            sources: Some(vec!["chat".to_string()]),
            limit: 10,
            date_from: None,
            date_to: None,
        })
        .unwrap();

    assert!(!results.is_empty(), "should find roadmap message");

    // Search with no chat filter should also work
    let results = api
        .search_engine()
        .search(&SearchQuery {
            text: "pull request".to_string(),
            sources: None,
            limit: 10,
            date_from: None,
            date_to: None,
        })
        .unwrap();

    assert!(
        !results.is_empty(),
        "should find pull request message without source filter"
    );
}

#[test]
fn test_search_does_not_find_other_sources() {
    let (_tmp, api) = setup();
    let user_id = create_user(&api, "bob", false);
    let channel_id = create_channel(&api, "random", user_id);

    post_and_index(&api, channel_id, user_id, "unique test message for chat");

    // Index a non-chat document
    let non_chat_doc = vec![(
        "notion-123".to_string(),
        "notion".to_string(),
        "Notion Page".to_string(),
        "unique test message for notion".to_string(),
    )];
    api.search_engine().index_documents(&non_chat_doc).unwrap();

    // Search with chat filter should only find the chat message
    let results = api
        .search_engine()
        .search(&SearchQuery {
            text: "unique test message".to_string(),
            sources: Some(vec!["chat".to_string()]),
            limit: 10,
            date_from: None,
            date_to: None,
        })
        .unwrap();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].source, "chat");
}

#[test]
fn test_file_metadata_storage() {
    let (_tmp, api) = setup();
    let user_id = create_user(&api, "carol", false);
    let channel_id = create_channel(&api, "files-test", user_id);

    let msg_id = teidelum::chat::id::next_id();
    let file_id = teidelum::chat::id::next_id();
    let now = teidelum::chat::models::now_timestamp();

    // Insert message
    let msg_sql = format!(
        "INSERT INTO messages (id, channel_id, user_id, thread_id, content, deleted_at, edited_at, created_at) \
         VALUES ({msg_id}, {channel_id}, {user_id}, 0, '[file: report.pdf]', NULL, NULL, '{now}')"
    );
    api.query_router().query_sync(&msg_sql).unwrap();

    // Insert file metadata
    let file_sql = format!(
        "INSERT INTO files (id, message_id, user_id, channel_id, filename, mime_type, size_bytes, storage_path, created_at) \
         VALUES ({file_id}, {msg_id}, {user_id}, {channel_id}, 'report.pdf', 'application/pdf', 1024, 'data/files/uuid123/report.pdf', '{now}')"
    );
    api.query_router().query_sync(&file_sql).unwrap();

    // Query back the file
    let result = api
        .query_router()
        .query_sync(&format!(
            "SELECT filename, mime_type, size_bytes FROM files WHERE id = {file_id}"
        ))
        .unwrap();

    assert_eq!(result.rows.len(), 1);
    match &result.rows[0][0] {
        teidelum::connector::Value::String(s) => assert_eq!(s, "report.pdf"),
        other => panic!("expected string filename, got {other:?}"),
    }
    match &result.rows[0][1] {
        teidelum::connector::Value::String(s) => assert_eq!(s, "application/pdf"),
        other => panic!("expected string mime_type, got {other:?}"),
    }
    match &result.rows[0][2] {
        teidelum::connector::Value::Int(v) => assert_eq!(*v, 1024),
        other => panic!("expected int size, got {other:?}"),
    }
}

#[test]
fn test_bot_user_query() {
    let (_tmp, api) = setup();
    let _human = create_user(&api, "dave", false);
    let bot_id = create_user(&api, "teidebot", true);

    // Query for bot users
    let result = api
        .query_router()
        .query_sync("SELECT id, username FROM users WHERE is_bot = true LIMIT 1")
        .unwrap();

    assert_eq!(result.rows.len(), 1);
    match &result.rows[0][0] {
        teidelum::connector::Value::Int(v) => assert_eq!(*v, bot_id),
        other => panic!("expected int bot id, got {other:?}"),
    }
}
