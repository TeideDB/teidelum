use crate::chat::auth::Claims;
use crate::chat::handlers::AppState;
use crate::chat::id::next_id;
use crate::chat::models::{escape_sql, now_timestamp};
use crate::chat::slack;
use axum::extract::State;
use axum::response::Response;
use axum::Extension;
use axum::extract::Multipart;
use serde_json::json;

/// Maximum file size: 10 MB.
const MAX_FILE_SIZE: usize = 10 * 1024 * 1024;

/// Allowed MIME types for upload.
const ALLOWED_MIME_TYPES: &[&str] = &[
    "text/plain",
    "text/csv",
    "text/markdown",
    "text/html",
    "application/json",
    "application/pdf",
    "application/zip",
    "application/gzip",
    "image/png",
    "image/jpeg",
    "image/gif",
    "image/webp",
    "image/svg+xml",
    "audio/mpeg",
    "audio/ogg",
    "video/mp4",
    "video/webm",
    "application/octet-stream",
];

/// Guess MIME type from file extension.
fn guess_mime(filename: &str) -> &'static str {
    let ext = filename.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "txt" => "text/plain",
        "csv" => "text/csv",
        "md" => "text/markdown",
        "html" | "htm" => "text/html",
        "json" => "application/json",
        "pdf" => "application/pdf",
        "zip" => "application/zip",
        "gz" | "gzip" => "application/gzip",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "mp3" => "audio/mpeg",
        "ogg" => "audio/ogg",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        _ => "application/octet-stream",
    }
}

/// Handle `files.upload` — multipart file upload.
///
/// Multipart fields:
/// - `channel` (text): channel ID (required)
/// - `file` (file): the file to upload (required)
/// - `message` (text): optional message text to accompany the file
/// - `thread_ts` (text): optional thread parent ID
pub async fn files_upload(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    mut multipart: Multipart,
) -> Response {
    let mut channel_id: Option<i64> = None;
    let mut file_data: Option<Vec<u8>> = None;
    let mut file_name: Option<String> = None;
    let mut file_mime: Option<String> = None;
    let mut message_text: Option<String> = None;
    let mut thread_ts: Option<i64> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "channel" => {
                if let Ok(text) = field.text().await {
                    channel_id = text.parse().ok();
                }
            }
            "file" => {
                file_name = field.file_name().map(|s| s.to_string());
                file_mime = field.content_type().map(|s| s.to_string());
                match field.bytes().await {
                    Ok(bytes) => {
                        if bytes.len() > MAX_FILE_SIZE {
                            return slack::err("file_too_large");
                        }
                        file_data = Some(bytes.to_vec());
                    }
                    Err(e) => {
                        tracing::error!("file read failed: {e}");
                        return slack::err("internal_error");
                    }
                }
            }
            "message" => {
                if let Ok(text) = field.text().await {
                    message_text = Some(text);
                }
            }
            "thread_ts" => {
                if let Ok(text) = field.text().await {
                    thread_ts = text.parse().ok();
                }
            }
            _ => {}
        }
    }

    let channel_id = match channel_id {
        Some(c) => c,
        None => return slack::err("invalid_arguments"),
    };

    let file_data = match file_data {
        Some(d) => d,
        None => return slack::err("no_file_uploaded"),
    };

    let file_name = file_name.unwrap_or_else(|| "unnamed".to_string());

    // Determine MIME type
    let mime_type = file_mime.unwrap_or_else(|| guess_mime(&file_name).to_string());
    if !ALLOWED_MIME_TYPES.contains(&mime_type.as_str()) {
        return slack::err("invalid_file_type");
    }

    // Check channel membership
    let check_sql = format!(
        "SELECT channel_id FROM channel_members WHERE channel_id = {} AND user_id = {}",
        channel_id, claims.user_id
    );
    match state.api.query_router().query_sync(&check_sql) {
        Ok(r) if r.rows.is_empty() => return slack::err("channel_not_found"),
        Err(e) => {
            tracing::error!("file upload membership check failed: {e}");
            return slack::err("internal_error");
        }
        _ => {}
    }

    // Store file on disk: data/files/<uuid>/<filename>
    let file_uuid = uuid::Uuid::new_v4().to_string();
    let storage_dir = format!("data/files/{file_uuid}");
    let storage_path = format!("{storage_dir}/{file_name}");

    if let Err(e) = std::fs::create_dir_all(&storage_dir) {
        tracing::error!("file dir creation failed: {e}");
        return slack::err("internal_error");
    }

    if let Err(e) = std::fs::write(&storage_path, &file_data) {
        tracing::error!("file write failed: {e}");
        return slack::err("internal_error");
    }

    let file_size = file_data.len() as i64;

    // Post a message with file reference
    let msg_id = next_id();
    let now = now_timestamp();
    let thread_id = thread_ts.unwrap_or(0);
    let msg_text = message_text.unwrap_or_else(|| format!("[file: {}]", file_name));

    let msg_insert = format!(
        "INSERT INTO messages (id, channel_id, user_id, thread_id, content, deleted_at, edited_at, created_at) \
         VALUES ({msg_id}, {channel_id}, {user_id}, {thread_id}, '{text}', NULL, NULL, '{now}')",
        user_id = claims.user_id,
        text = escape_sql(&msg_text),
    );

    if let Err(e) = state.api.query_router().query_sync(&msg_insert) {
        tracing::error!("file message insert failed: {e}");
        return slack::err("internal_error");
    }

    // Insert file metadata row
    let file_id = next_id();
    let file_insert = format!(
        "INSERT INTO files (id, message_id, user_id, channel_id, filename, mime_type, size_bytes, storage_path, created_at) \
         VALUES ({file_id}, {msg_id}, {user_id}, {channel_id}, '{filename}', '{mime}', {size}, '{path}', '{now}')",
        user_id = claims.user_id,
        filename = escape_sql(&file_name),
        mime = escape_sql(&mime_type),
        size = file_size,
        path = escape_sql(&storage_path),
    );

    if let Err(e) = state.api.query_router().query_sync(&file_insert) {
        tracing::error!("file metadata insert failed: {e}");
        return slack::err("internal_error");
    }

    // Index the message in tantivy
    let channel_name = {
        let name_sql = format!("SELECT name FROM channels WHERE id = {}", channel_id);
        match state.api.query_router().query_sync(&name_sql) {
            Ok(r) if !r.rows.is_empty() => {
                match &r.rows[0][0] {
                    crate::connector::Value::String(s) => format!("#{s}"),
                    _ => format!("#{channel_id}"),
                }
            }
            _ => format!("#{channel_id}"),
        }
    };
    let doc = vec![(
        msg_id.to_string(),
        "chat".to_string(),
        channel_name,
        msg_text.clone(),
    )];
    if let Err(e) = state.api.search_engine().index_documents(&doc) {
        tracing::warn!("file message search indexing failed: {e}");
    }

    // Broadcast message event
    let event = crate::chat::events::ServerEvent::Message {
        channel: channel_id.to_string(),
        user: claims.user_id.to_string(),
        text: msg_text,
        ts: msg_id.to_string(),
        thread_ts: if thread_id != 0 {
            Some(thread_id.to_string())
        } else {
            None
        },
    };
    state.hub.broadcast_to_channel(channel_id, &event).await;

    slack::ok(json!({
        "file": {
            "id": file_id.to_string(),
            "name": file_name,
            "mime_type": mime_type,
            "size": file_size,
        },
        "message": {
            "ts": msg_id.to_string(),
            "channel": channel_id.to_string(),
        }
    }))
}
