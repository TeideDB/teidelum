use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};

/// Slack-compatible success response: {"ok": true, ...extra_fields}
pub fn ok(data: Value) -> Response {
    let mut obj = match data {
        Value::Object(map) => map,
        _ => {
            let mut map = serde_json::Map::new();
            map.insert("data".into(), data);
            map
        }
    };
    obj.insert("ok".into(), json!(true));
    (StatusCode::OK, Json(Value::Object(obj))).into_response()
}

/// Slack-compatible created response: {"ok": true, ...extra_fields}
pub fn created(data: Value) -> Response {
    let mut obj = match data {
        Value::Object(map) => map,
        _ => {
            let mut map = serde_json::Map::new();
            map.insert("data".into(), data);
            map
        }
    };
    obj.insert("ok".into(), json!(true));
    (StatusCode::CREATED, Json(Value::Object(obj))).into_response()
}

/// Slack-compatible error response: {"ok": false, "error": "error_code"}
pub fn err(error: &str) -> Response {
    (
        StatusCode::OK, // Slack returns 200 even for app-level errors
        Json(json!({"ok": false, "error": error})),
    )
        .into_response()
}

/// Slack-compatible error with HTTP status code (for transport-level errors).
pub fn http_err(status: StatusCode, error: &str) -> Response {
    (status, Json(json!({"ok": false, "error": error}))).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ok_response_merges_fields() {
        let _resp = ok(json!({"channel": "general", "ts": "123"}));
    }

    #[test]
    fn test_err_response() {
        let _resp = err("channel_not_found");
    }
}
