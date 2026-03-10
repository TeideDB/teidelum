use crate::api::TeidelumApi;
use crate::catalog::Relationship;
use anyhow::Result;

/// SQL statements to create all chat tables.
const CREATE_TABLES: &[&str] = &[
    "CREATE TABLE users (
        id BIGINT, username VARCHAR, display_name VARCHAR, email VARCHAR,
        password_hash VARCHAR, avatar_url VARCHAR, status VARCHAR,
        is_bot BOOLEAN, created_at VARCHAR
    )",
    "CREATE TABLE channels (
        id BIGINT, name VARCHAR, kind VARCHAR, topic VARCHAR,
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
];

/// All FK relationships for the chat data model.
fn chat_relationships() -> Vec<Relationship> {
    vec![
        Relationship {
            from_table: "messages".into(), from_col: "user_id".into(),
            to_table: "users".into(), to_col: "id".into(),
            relation: "sent_by".into(),
        },
        Relationship {
            from_table: "messages".into(), from_col: "channel_id".into(),
            to_table: "channels".into(), to_col: "id".into(),
            relation: "posted_in".into(),
        },
        Relationship {
            from_table: "messages".into(), from_col: "thread_id".into(),
            to_table: "messages".into(), to_col: "id".into(),
            relation: "reply_to".into(),
        },
        Relationship {
            from_table: "channel_members".into(), from_col: "user_id".into(),
            to_table: "users".into(), to_col: "id".into(),
            relation: "member".into(),
        },
        Relationship {
            from_table: "channel_members".into(), from_col: "channel_id".into(),
            to_table: "channels".into(), to_col: "id".into(),
            relation: "belongs_to".into(),
        },
        Relationship {
            from_table: "reactions".into(), from_col: "message_id".into(),
            to_table: "messages".into(), to_col: "id".into(),
            relation: "reacted_to".into(),
        },
        Relationship {
            from_table: "mentions".into(), from_col: "message_id".into(),
            to_table: "messages".into(), to_col: "id".into(),
            relation: "mentioned_in".into(),
        },
        Relationship {
            from_table: "mentions".into(), from_col: "user_id".into(),
            to_table: "users".into(), to_col: "id".into(),
            relation: "mentions".into(),
        },
        Relationship {
            from_table: "channel_reads".into(), from_col: "channel_id".into(),
            to_table: "channels".into(), to_col: "id".into(),
            relation: "read_status_for".into(),
        },
        Relationship {
            from_table: "channel_reads".into(), from_col: "user_id".into(),
            to_table: "users".into(), to_col: "id".into(),
            relation: "read_by".into(),
        },
        Relationship {
            from_table: "files".into(), from_col: "message_id".into(),
            to_table: "messages".into(), to_col: "id".into(),
            relation: "attached_to".into(),
        },
        Relationship {
            from_table: "files".into(), from_col: "user_id".into(),
            to_table: "users".into(), to_col: "id".into(),
            relation: "uploaded_by".into(),
        },
        Relationship {
            from_table: "files".into(), from_col: "channel_id".into(),
            to_table: "channels".into(), to_col: "id".into(),
            relation: "uploaded_in".into(),
        },
    ]
}

/// Initialize all chat tables and register relationships.
/// Safe to call if tables already exist — will skip existing ones.
pub fn init_chat_tables(api: &TeidelumApi) -> Result<()> {
    let router = api.query_router();

    for sql in CREATE_TABLES {
        // Ignore "table already exists" errors
        match router.query_sync(sql) {
            Ok(_) => {}
            Err(e) if e.to_string().contains("already exists") => {}
            Err(e) => return Err(e),
        }
    }

    api.register_relationships(chat_relationships())?;

    Ok(())
}

/// Escape a string value for SQL (double single quotes).
pub fn escape_sql(s: &str) -> String {
    s.replace('\'', "''")
}

/// Format an optional string as SQL NULL or quoted value.
pub fn sql_str_or_null(v: &Option<String>) -> String {
    match v {
        Some(s) => format!("'{}'", escape_sql(s)),
        None => "NULL".to_string(),
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
        assert_eq!(rels.len(), 13);
        // All identifiers should be valid
        for rel in &rels {
            assert!(crate::catalog::is_valid_identifier(&rel.from_table), "invalid: {}", rel.from_table);
            assert!(crate::catalog::is_valid_identifier(&rel.from_col), "invalid: {}", rel.from_col);
            assert!(crate::catalog::is_valid_identifier(&rel.to_table), "invalid: {}", rel.to_table);
            assert!(crate::catalog::is_valid_identifier(&rel.to_col), "invalid: {}", rel.to_col);
        }
    }
}
