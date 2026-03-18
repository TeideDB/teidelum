use std::path::Path;

use crate::api::TeidelumApi;
use crate::catalog::Relationship;
use anyhow::Result;

/// SQL statements to create all chat tables.
const CREATE_TABLES: &[&str] = &[
    "CREATE TABLE users (
        id BIGINT, username VARCHAR, display_name VARCHAR, email VARCHAR,
        password_hash VARCHAR, avatar_url VARCHAR, status VARCHAR,
        status_text VARCHAR, status_emoji VARCHAR,
        is_bot BOOLEAN, created_at VARCHAR
    )",
    "CREATE TABLE channels (
        id BIGINT, name VARCHAR, kind VARCHAR, topic VARCHAR,
        description VARCHAR, archived_at VARCHAR,
        created_by BIGINT, created_at VARCHAR
    )",
    "CREATE TABLE channel_members (
        channel_id BIGINT, user_id BIGINT, role VARCHAR, joined_at VARCHAR
    )",
    "CREATE TABLE messages (
        id BIGINT, channel_id BIGINT, user_id BIGINT, thread_id BIGINT,
        content VARCHAR, deleted_at VARCHAR, edited_at VARCHAR, created_at VARCHAR
    )",
    "CREATE TABLE reactions (
        message_id BIGINT, user_id BIGINT, emoji VARCHAR, created_at VARCHAR
    )",
    "CREATE TABLE mentions (
        message_id BIGINT, user_id BIGINT
    )",
    "CREATE TABLE channel_reads (
        channel_id BIGINT, user_id BIGINT, last_read_ts VARCHAR
    )",
    "CREATE TABLE files (
        id BIGINT, message_id BIGINT, user_id BIGINT, channel_id BIGINT,
        filename VARCHAR, mime_type VARCHAR, size_bytes BIGINT,
        storage_path VARCHAR, created_at VARCHAR
    )",
    "CREATE TABLE user_settings (
        user_id BIGINT, theme VARCHAR, notification_default VARCHAR,
        timezone VARCHAR, created_at VARCHAR
    )",
    "CREATE TABLE pinned_messages (
        channel_id BIGINT, message_id BIGINT, user_id BIGINT, created_at VARCHAR
    )",
    "CREATE TABLE channel_settings (
        channel_id BIGINT, user_id BIGINT, muted VARCHAR, notification_level VARCHAR, created_at VARCHAR
    )",
];

/// All FK relationships for the chat data model.
fn chat_relationships() -> Vec<Relationship> {
    vec![
        Relationship {
            from_table: "messages".into(),
            from_col: "user_id".into(),
            to_table: "users".into(),
            to_col: "id".into(),
            relation: "sent_by".into(),
        },
        Relationship {
            from_table: "messages".into(),
            from_col: "channel_id".into(),
            to_table: "channels".into(),
            to_col: "id".into(),
            relation: "posted_in".into(),
        },
        Relationship {
            from_table: "messages".into(),
            from_col: "thread_id".into(),
            to_table: "messages".into(),
            to_col: "id".into(),
            relation: "reply_to".into(),
        },
        Relationship {
            from_table: "channel_members".into(),
            from_col: "user_id".into(),
            to_table: "users".into(),
            to_col: "id".into(),
            relation: "member".into(),
        },
        Relationship {
            from_table: "channel_members".into(),
            from_col: "channel_id".into(),
            to_table: "channels".into(),
            to_col: "id".into(),
            relation: "belongs_to".into(),
        },
        Relationship {
            from_table: "reactions".into(),
            from_col: "message_id".into(),
            to_table: "messages".into(),
            to_col: "id".into(),
            relation: "reacted_to".into(),
        },
        Relationship {
            from_table: "mentions".into(),
            from_col: "message_id".into(),
            to_table: "messages".into(),
            to_col: "id".into(),
            relation: "mentioned_in".into(),
        },
        Relationship {
            from_table: "mentions".into(),
            from_col: "user_id".into(),
            to_table: "users".into(),
            to_col: "id".into(),
            relation: "mentions".into(),
        },
        Relationship {
            from_table: "channel_reads".into(),
            from_col: "channel_id".into(),
            to_table: "channels".into(),
            to_col: "id".into(),
            relation: "read_status_for".into(),
        },
        Relationship {
            from_table: "channel_reads".into(),
            from_col: "user_id".into(),
            to_table: "users".into(),
            to_col: "id".into(),
            relation: "read_by".into(),
        },
        Relationship {
            from_table: "files".into(),
            from_col: "message_id".into(),
            to_table: "messages".into(),
            to_col: "id".into(),
            relation: "attached_to".into(),
        },
        Relationship {
            from_table: "files".into(),
            from_col: "user_id".into(),
            to_table: "users".into(),
            to_col: "id".into(),
            relation: "uploaded_by".into(),
        },
        Relationship {
            from_table: "files".into(),
            from_col: "channel_id".into(),
            to_table: "channels".into(),
            to_col: "id".into(),
            relation: "uploaded_in".into(),
        },
        Relationship {
            from_table: "user_settings".into(),
            from_col: "user_id".into(),
            to_table: "users".into(),
            to_col: "id".into(),
            relation: "settings_for".into(),
        },
        Relationship {
            from_table: "pinned_messages".into(),
            from_col: "message_id".into(),
            to_table: "messages".into(),
            to_col: "id".into(),
            relation: "pinned".into(),
        },
        Relationship {
            from_table: "pinned_messages".into(),
            from_col: "channel_id".into(),
            to_table: "channels".into(),
            to_col: "id".into(),
            relation: "pinned_in".into(),
        },
        Relationship {
            from_table: "pinned_messages".into(),
            from_col: "user_id".into(),
            to_table: "users".into(),
            to_col: "id".into(),
            relation: "pinned_by".into(),
        },
        Relationship {
            from_table: "channel_settings".into(),
            from_col: "channel_id".into(),
            to_table: "channels".into(),
            to_col: "id".into(),
            relation: "channel_setting_for".into(),
        },
        Relationship {
            from_table: "channel_settings".into(),
            from_col: "user_id".into(),
            to_table: "users".into(),
            to_col: "id".into(),
            relation: "channel_setting_by".into(),
        },
    ]
}

/// Ordered chat table names matching CREATE_TABLES.
pub const CHAT_TABLE_NAMES: &[&str] = &[
    "users",
    "channels",
    "channel_members",
    "messages",
    "reactions",
    "mentions",
    "channel_reads",
    "files",
    "user_settings",
    "pinned_messages",
    "channel_settings",
];

/// Initialize all chat tables and register relationships.
/// If `data_dir` is provided, attempts to load persisted tables from `data_dir/chat/`.
/// Falls back to creating fresh tables for any that can't be loaded.
pub fn init_chat_tables(api: &TeidelumApi, data_dir: Option<&Path>) -> Result<()> {
    let router = api.query_router();

    let chat_dir = data_dir.map(|d| d.join("chat"));
    let sym_path = data_dir.map(|d| d.join("tables").join("sym"));
    let sym = sym_path.as_deref().filter(|p| p.exists());

    for (i, sql) in CREATE_TABLES.iter().enumerate() {
        let name = CHAT_TABLE_NAMES[i];

        // Try loading persisted table from disk
        if let Some(ref cd) = chat_dir {
            let table_dir = cd.join(name);
            if table_dir.join(".d").exists() {
                match router.load_splayed(name, &table_dir, sym) {
                    Ok(_) => {
                        tracing::info!("loaded persisted chat table: {name}");
                        continue;
                    }
                    Err(e) => {
                        tracing::warn!("failed to load persisted {name}, creating fresh: {e}");
                    }
                }
            }
        }

        // Create fresh table
        match router.query_sync(sql) {
            Ok(_) => {}
            Err(e) if e.to_string().contains("already exists") => {}
            Err(e) => return Err(e),
        }
    }

    api.register_relationships(chat_relationships())?;

    // Ensure #general channel exists
    let created_general = ensure_general_channel(router);

    // Ensure all users are members of all public channels (backfill for users
    // created before auto-join was added, or after a data loss on restart).
    let backfilled = ensure_all_users_in_public_channels(router);

    if created_general || backfilled {
        if let Some(data_dir) = data_dir {
            let chat_dir = data_dir.join("chat");
            if created_general {
                let _ = router.save_table("channels", &chat_dir.join("channels"));
            }
            if backfilled {
                let _ = router.save_table("channel_members", &chat_dir.join("channel_members"));
            }
            let sym_dir = data_dir.join("tables");
            let _ = std::fs::create_dir_all(&sym_dir);
            let _ = router.save_sym(&sym_dir.join("sym"));
        }
    }

    Ok(())
}

/// Well-known channel ID for #general.
pub const GENERAL_CHANNEL_ID: i64 = 1;

/// Create the #general channel if it doesn't already exist. Returns true if created.
fn ensure_general_channel(router: &crate::router::QueryRouter) -> bool {
    let check = format!("SELECT id FROM channels WHERE id = {}", GENERAL_CHANNEL_ID);
    match router.query_sync(&check) {
        Ok(r) if !r.rows.is_empty() => return false, // already exists
        _ => {}
    }
    let now = now_timestamp();
    let sql = format!(
        "INSERT INTO channels (id, name, kind, topic, description, archived_at, created_by, created_at) \
         VALUES ({}, 'general', 'public', 'General discussion', '', '', 0, '{now}')",
        GENERAL_CHANNEL_ID
    );
    match router.query_sync(&sql) {
        Ok(_) => {
            tracing::info!("created #general channel");
            true
        }
        Err(e) => {
            tracing::warn!("failed to create #general: {e}");
            false
        }
    }
}

/// Ensure every user is a member of #general. Returns true if any rows were inserted.
fn ensure_all_users_in_public_channels(router: &crate::router::QueryRouter) -> bool {
    // Get all user IDs
    let all_users: Vec<i64> = match router.query_sync("SELECT id FROM users") {
        Ok(r) => r
            .rows
            .iter()
            .filter_map(|row| match &row[0] {
                crate::connector::Value::Int(v) => Some(*v),
                _ => None,
            })
            .collect(),
        Err(_) => return false,
    };
    if all_users.is_empty() {
        return false;
    }

    // Get all public channel IDs
    let public_channels: Vec<i64> =
        match router.query_sync("SELECT id FROM channels WHERE kind = 'public'") {
            Ok(r) => r
                .rows
                .iter()
                .filter_map(|row| match &row[0] {
                    crate::connector::Value::Int(v) => Some(*v),
                    _ => None,
                })
                .collect(),
            Err(_) => return false,
        };

    let now = now_timestamp();
    let mut added = false;
    for ch_id in &public_channels {
        // Get existing members of this channel
        let existing: std::collections::HashSet<i64> = match router.query_sync(&format!(
            "SELECT user_id FROM channel_members WHERE channel_id = {}",
            ch_id
        )) {
            Ok(r) => r
                .rows
                .iter()
                .filter_map(|row| match &row[0] {
                    crate::connector::Value::Int(v) => Some(*v),
                    _ => None,
                })
                .collect(),
            Err(_) => continue,
        };

        for uid in &all_users {
            if !existing.contains(uid) {
                let sql = format!(
                    "INSERT INTO channel_members (channel_id, user_id, role, joined_at) \
                     VALUES ({ch_id}, {uid}, 'member', '{now}')"
                );
                if router.query_sync(&sql).is_ok() {
                    tracing::info!("backfilled user {uid} into public channel {ch_id}");
                    added = true;
                }
            }
        }
    }
    added
}

/// Escape a string value for SQL: strip null bytes and double single quotes.
/// Note: TeideDB uses DuckDB dialect where backslash is NOT an escape character,
/// so we must NOT double backslashes — that would corrupt stored data.
pub fn escape_sql(s: &str) -> String {
    s.replace('\0', "").replace('\'', "''")
}

/// Escape a string for use inside a SQL LIKE pattern.
/// In addition to standard SQL escaping, escapes `%` and `_` wildcards
/// so user input is treated as literal text.
pub fn escape_sql_like(s: &str) -> String {
    escape_sql(s)
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Format an optional string as SQL NULL or quoted value.
pub fn sql_str_or_null(v: &Option<String>) -> String {
    match v {
        Some(s) => format!("'{}'", escape_sql(s)),
        None => "NULL".to_string(),
    }
}

/// Over-fetch multiplier for search results before post-query auth filtering.
/// Since tantivy has no per-user access control, we fetch extra results and
/// filter in-app, then take the requested limit.
pub const SEARCH_OVERFETCH_FACTOR: usize = 3;

/// Look up channel display name (e.g. "#general") for tantivy indexing.
/// Falls back to "#<channel_id>" if the channel name is unavailable.
pub fn channel_display_name(router: &crate::router::QueryRouter, channel_id: i64) -> String {
    let sql = format!("SELECT name FROM channels WHERE id = {}", channel_id);
    match router.query_sync(&sql) {
        Ok(r) if !r.rows.is_empty() => match &r.rows[0][0] {
            crate::connector::Value::String(s) => format!("#{s}"),
            _ => format!("#{channel_id}"),
        },
        _ => format!("#{channel_id}"),
    }
}

/// Get current timestamp as ISO 8601 string.
pub fn now_timestamp() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    // Simple ISO-ish format: seconds since epoch as string
    // TeideDB stores timestamps as VARCHAR
    format!("{}", now.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_sql() {
        assert_eq!(escape_sql("hello"), "hello");
        assert_eq!(escape_sql("it's"), "it''s");
        assert_eq!(escape_sql("a''b"), "a''''b");
        // Backslash preserved as-is (TeideDB/DuckDB dialect, not an escape char)
        assert_eq!(escape_sql("back\\slash"), "back\\slash");
        // Null byte stripped
        assert_eq!(escape_sql("null\0byte"), "nullbyte");
        assert_eq!(escape_sql("combo\\' test"), "combo\\'' test");
    }

    #[test]
    fn test_escape_sql_like() {
        assert_eq!(escape_sql_like("hello"), "hello");
        assert_eq!(escape_sql_like("100%"), "100\\%");
        assert_eq!(escape_sql_like("under_score"), "under\\_score");
        // Backslash must be escaped so LIKE ESCAPE '\' treats it as literal
        assert_eq!(escape_sql_like("back\\slash"), "back\\\\slash");
        assert_eq!(escape_sql_like("a\\%b"), "a\\\\\\%b");
    }

    #[test]
    fn test_sql_str_or_null() {
        assert_eq!(sql_str_or_null(&None), "NULL");
        assert_eq!(sql_str_or_null(&Some("test".into())), "'test'");
        assert_eq!(sql_str_or_null(&Some("it's".into())), "'it''s'");
    }

    #[test]
    fn test_chat_relationships_valid() {
        let rels = chat_relationships();
        assert_eq!(rels.len(), 19);
        // All identifiers should be valid
        for rel in &rels {
            assert!(
                crate::catalog::is_valid_identifier(&rel.from_table),
                "invalid: {}",
                rel.from_table
            );
            assert!(
                crate::catalog::is_valid_identifier(&rel.from_col),
                "invalid: {}",
                rel.from_col
            );
            assert!(
                crate::catalog::is_valid_identifier(&rel.to_table),
                "invalid: {}",
                rel.to_table
            );
            assert!(
                crate::catalog::is_valid_identifier(&rel.to_col),
                "invalid: {}",
                rel.to_col
            );
        }
    }
}
