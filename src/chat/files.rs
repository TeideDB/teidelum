use crate::chat::auth::Claims;
use crate::chat::handlers::AppState;
use crate::chat::id::next_id;
use crate::chat::models::{escape_sql, now_timestamp};
use crate::chat::slack;
use axum::body::Body;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Extension;
use serde::Deserialize;
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

    let file_name = file_name
        .and_then(|n| {
            // Strip directory components to prevent path traversal
            let sanitized = n.rsplit(['/', '\\']).next().unwrap_or("").to_string();
            if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
                None
            } else {
                Some(sanitized)
            }
        })
        .unwrap_or_else(|| "unnamed".to_string());

    // Determine MIME type from file extension (ignore client-supplied content type
    // to prevent MIME spoofing that could bypass the allowlist)
    let mime_type = guess_mime(&file_name).to_string();
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
        let _ = std::fs::remove_file(&storage_path);
        let _ = std::fs::remove_dir(&storage_dir);
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
        // Clean up orphaned message row
        let del_msg = format!("DELETE FROM messages WHERE id = {msg_id}");
        let _ = state.api.query_router().query_sync(&del_msg);
        let _ = std::fs::remove_file(&storage_path);
        let _ = std::fs::remove_dir(&storage_dir);
        return slack::err("internal_error");
    }

    // Index the message in tantivy
    let channel_name =
        crate::chat::models::channel_display_name(state.api.query_router(), channel_id);
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

#[derive(Deserialize)]
pub struct FileDownloadQuery {
    pub token: String,
}

/// Handle `GET /files/:id/:filename` — download a file with auth check.
pub async fn files_download(
    State(state): State<AppState>,
    Path((file_id, _filename)): Path<(String, String)>,
    Query(query): Query<FileDownloadQuery>,
) -> Response {
    // Validate JWT from query param
    let secret = match std::env::var("TEIDE_CHAT_SECRET") {
        Ok(s) if !s.is_empty() => s,
        _ => return slack::http_err(StatusCode::INTERNAL_SERVER_ERROR, "server_misconfigured"),
    };

    let claims = match crate::chat::auth::validate_token(&secret, &query.token) {
        Ok(c) => c,
        Err(_) => return slack::http_err(StatusCode::UNAUTHORIZED, "invalid_auth"),
    };

    // Validate file_id is numeric to prevent SQL injection
    let file_id_num: i64 = match file_id.parse() {
        Ok(id) => id,
        Err(_) => return slack::http_err(StatusCode::BAD_REQUEST, "invalid_file_id"),
    };

    // Look up file metadata
    let sql = format!(
        "SELECT channel_id, filename, mime_type, storage_path FROM files WHERE id = {}",
        file_id_num
    );
    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("file download query failed: {e}");
            return slack::http_err(StatusCode::INTERNAL_SERVER_ERROR, "internal_error");
        }
    };

    if result.rows.is_empty() {
        return slack::http_err(StatusCode::NOT_FOUND, "file_not_found");
    }

    let row = &result.rows[0];
    let channel_id = match &row[0] {
        crate::connector::Value::Int(v) => *v,
        _ => return slack::http_err(StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
    };
    let filename = match &row[1] {
        crate::connector::Value::String(s) => s.clone(),
        _ => return slack::http_err(StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
    };
    // Re-derive MIME from filename rather than trusting DB value (defense-in-depth)
    let mime_type = guess_mime(&filename).to_string();
    let storage_path = match &row[3] {
        crate::connector::Value::String(s) => s.clone(),
        _ => return slack::http_err(StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
    };

    // Check channel membership
    let check_sql = format!(
        "SELECT channel_id FROM channel_members WHERE channel_id = {} AND user_id = {}",
        channel_id, claims.user_id
    );
    match state.api.query_router().query_sync(&check_sql) {
        Ok(r) if r.rows.is_empty() => {
            return slack::http_err(StatusCode::FORBIDDEN, "not_in_channel");
        }
        Err(e) => {
            tracing::error!("file download membership check failed: {e}");
            return slack::http_err(StatusCode::INTERNAL_SERVER_ERROR, "internal_error");
        }
        _ => {}
    }

    // Read file from disk and stream it
    let file_bytes = match std::fs::read(&storage_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::error!("file read from disk failed: {e}");
            return slack::http_err(StatusCode::NOT_FOUND, "file_not_found");
        }
    };

    // Sanitize filename: strip control chars to avoid header injection / panic
    let safe_filename: String = filename.chars().filter(|c| !c.is_control()).collect();
    let content_disposition = format!(
        "attachment; filename=\"{}\"",
        safe_filename.replace('\\', "\\\\").replace('"', "\\\"")
    );

    axum::http::Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime_type)
        .header(header::CONTENT_DISPOSITION, content_disposition)
        .header("X-Content-Type-Options", "nosniff")
        .body(Body::from(file_bytes))
        .unwrap_or_else(|_| {
            axum::http::Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        })
        .into_response()
}
