use std::path::PathBuf;
use std::sync::Arc;

use crate::api::TeidelumApi;
use crate::chat::auth::{self, Claims};
use crate::chat::id::next_id;
use crate::chat::models::{escape_sql, escape_sql_like, now_timestamp};
use crate::chat::slack;
use axum::{extract::State, response::Response, Extension, Json};
use serde::{Deserialize, Deserializer};
use serde_json::json;

/// Deserialize an i64 from either a JSON number or a JSON string (Slack convention).
fn deserialize_id<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
    use serde::de;
    struct IdVisitor;
    impl<'de> de::Visitor<'de> for IdVisitor {
        type Value = i64;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("an integer or string-encoded integer")
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<i64, E> {
            Ok(v)
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<i64, E> {
            i64::try_from(v).map_err(|_| de::Error::custom("id overflow"))
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<i64, E> {
            v.parse().map_err(de::Error::custom)
        }
    }
    d.deserialize_any(IdVisitor)
}

/// Deserialize a Vec<i64> where each element may be a number or string.
fn deserialize_id_vec<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<i64>, D::Error> {
    use serde::de;
    let values: Vec<serde_json::Value> = Vec::deserialize(d)?;
    values
        .into_iter()
        .map(|v| match v {
            serde_json::Value::Number(n) => {
                n.as_i64().ok_or_else(|| de::Error::custom("invalid id"))
            }
            serde_json::Value::String(s) => s.parse().map_err(de::Error::custom),
            _ => Err(de::Error::custom("expected number or string")),
        })
        .collect()
}

/// Deserialize an optional i64 that may be a number, string, or absent.
fn deserialize_opt_id<'de, D: Deserializer<'de>>(d: D) -> Result<Option<i64>, D::Error> {
    use serde::de;
    struct OptIdVisitor;
    impl<'de> de::Visitor<'de> for OptIdVisitor {
        type Value = Option<i64>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("an integer, string-encoded integer, or null")
        }
        fn visit_none<E: de::Error>(self) -> Result<Option<i64>, E> {
            Ok(None)
        }
        fn visit_unit<E: de::Error>(self) -> Result<Option<i64>, E> {
            Ok(None)
        }
        fn visit_i64<E: de::Error>(self, v: i64) -> Result<Option<i64>, E> {
            Ok(Some(v))
        }
        fn visit_u64<E: de::Error>(self, v: u64) -> Result<Option<i64>, E> {
            Ok(Some(
                i64::try_from(v).map_err(|_| de::Error::custom("id overflow"))?,
            ))
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<Option<i64>, E> {
            v.parse().map(Some).map_err(de::Error::custom)
        }
    }
    d.deserialize_any(OptIdVisitor)
}
use tokio::sync::Mutex;

pub type AppState = Arc<ChatState>;

pub struct ChatState {
    pub api: Arc<TeidelumApi>,
    pub hub: Arc<crate::chat::hub::Hub>,
    /// Data directory for persisting chat tables to disk.
    pub data_dir: Option<PathBuf>,
    /// Serializes DM channel creation to prevent TOCTOU races (check-then-insert
    /// without DB-level unique constraints).
    pub dm_create_lock: Mutex<()>,
    /// Serializes channel_reads upserts to prevent TOCTOU races.
    pub reads_lock: Mutex<()>,
    /// Serializes channel_settings upserts to prevent TOCTOU races.
    pub settings_lock: Mutex<()>,
    /// Serializes channel creation to prevent duplicate names from TOCTOU races.
    pub channel_create_lock: Mutex<()>,
    /// Serializes channel join to prevent duplicate membership from TOCTOU races.
    pub channel_join_lock: Mutex<()>,
    /// Serializes pin add to prevent duplicate pins from TOCTOU races.
    pub pin_lock: Mutex<()>,
    /// Serializes reaction add to prevent duplicate reactions from TOCTOU races.
    pub reaction_lock: Mutex<()>,
    /// Serializes user registration to prevent duplicate usernames/emails from TOCTOU races.
    pub register_lock: Mutex<()>,
}

impl ChatState {
    /// Persist the given chat tables to disk. No-op if data_dir is not set.
    pub fn persist_tables(&self, tables: &[&str]) {
        let Some(ref data_dir) = self.data_dir else {
            return;
        };
        let chat_dir = data_dir.join("chat");
        let router = self.api.query_router();
        for &table in tables {
            let table_dir = chat_dir.join(table);
            if let Err(e) = router.save_table(table, &table_dir) {
                tracing::error!("failed to persist {table}: {e}");
            }
        }
        let sym_dir = data_dir.join("tables");
        let _ = std::fs::create_dir_all(&sym_dir);
        let sym_path = sym_dir.join("sym");
        if let Err(e) = router.save_sym(&sym_path) {
            tracing::error!("failed to persist sym: {e}");
        }
    }
}

// ── Auth ──

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
    pub email: String,
    #[serde(default)]
    pub display_name: Option<String>,
}

pub async fn auth_register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Response {
    if req.username.is_empty() || req.password.is_empty() || req.email.is_empty() {
        return slack::err("invalid_arguments");
    }
    if req.password.chars().count() < 8 {
        return slack::err("password_too_short");
    }

    let _register_guard = state.register_lock.lock().await;

    // Check if username already exists
    let check_sql = format!(
        "SELECT id FROM users WHERE username = '{}'",
        escape_sql(&req.username)
    );
    match state.api.query_router().query_sync(&check_sql) {
        Ok(result) if !result.rows.is_empty() => {
            return slack::err("username_taken");
        }
        Err(e) => {
            tracing::error!("register check failed: {e}");
            return slack::err("internal_error");
        }
        _ => {}
    }

    // Check if email already exists
    let check_email = format!(
        "SELECT id FROM users WHERE email = '{}'",
        escape_sql(&req.email)
    );
    match state.api.query_router().query_sync(&check_email) {
        Ok(result) if !result.rows.is_empty() => {
            return slack::err("email_taken");
        }
        Err(e) => {
            tracing::error!("register email check failed: {e}");
            return slack::err("internal_error");
        }
        _ => {}
    }

    let password_hash = match auth::hash_password(&req.password) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("password hash failed: {e}");
            return slack::err("internal_error");
        }
    };

    let id = next_id();
    let display_name = req.display_name.unwrap_or_else(|| req.username.clone());
    let now = now_timestamp();

    let insert_sql = format!(
        "INSERT INTO users (id, username, display_name, email, password_hash, avatar_url, status, status_text, status_emoji, is_bot, created_at) \
         VALUES ({id}, '{username}', '{display}', '{email}', '{hash}', '', 'offline', '', '', false, '{now}')",
        username = escape_sql(&req.username),
        display = escape_sql(&display_name),
        email = escape_sql(&req.email),
        hash = escape_sql(&password_hash),
    );

    if let Err(e) = state.api.query_router().query_sync(&insert_sql) {
        tracing::error!("user insert failed: {e}");
        return slack::err("internal_error");
    }
    state.persist_tables(&["users"]);

    // Auto-join #general channel
    let general_id = crate::chat::models::GENERAL_CHANNEL_ID;
    let join_sql = format!(
        "INSERT INTO channel_members (channel_id, user_id, role, joined_at) \
         VALUES ({general_id}, {id}, 'member', '{now}')"
    );
    if let Err(e) = state.api.query_router().query_sync(&join_sql) {
        tracing::warn!("auto-join #general failed: {e}");
    } else {
        state.persist_tables(&["channel_members"]);
        state.hub.add_channel_member(general_id, id).await;
    }

    let secret = match std::env::var("TEIDE_CHAT_SECRET") {
        Ok(s) if !s.is_empty() => s,
        _ => return slack::err("server_misconfigured"),
    };

    let token = match auth::create_token(&secret, id, &req.username, false) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("token creation failed: {e}");
            return slack::err("internal_error");
        }
    };

    slack::created(json!({
        "user_id": id.to_string(),
        "username": req.username,
        "token": token,
    }))
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

pub async fn auth_login(State(state): State<AppState>, Json(req): Json<LoginRequest>) -> Response {
    let sql = format!(
        "SELECT id, username, password_hash, is_bot FROM users WHERE username = '{}'",
        escape_sql(&req.username)
    );

    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("login query failed: {e}");
            return slack::err("internal_error");
        }
    };

    if result.rows.is_empty() {
        return slack::err("invalid_credentials");
    }

    let row = &result.rows[0];
    let user_id = match &row[0] {
        crate::connector::Value::Int(v) => *v,
        _ => return slack::err("internal_error"),
    };
    let username = match &row[1] {
        crate::connector::Value::String(v) => v.clone(),
        _ => return slack::err("internal_error"),
    };
    let password_hash = match &row[2] {
        crate::connector::Value::String(v) => v.clone(),
        _ => return slack::err("internal_error"),
    };
    let is_bot = match &row[3] {
        crate::connector::Value::Bool(v) => *v,
        _ => false,
    };

    match auth::verify_password(&req.password, &password_hash) {
        Ok(true) => {}
        Ok(false) => return slack::err("invalid_credentials"),
        Err(e) => {
            tracing::error!("password verify failed: {e}");
            return slack::err("internal_error");
        }
    }

    let secret = match std::env::var("TEIDE_CHAT_SECRET") {
        Ok(s) if !s.is_empty() => s,
        _ => return slack::err("server_misconfigured"),
    };

    let token = match auth::create_token(&secret, user_id, &username, is_bot) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("token creation failed: {e}");
            return slack::err("internal_error");
        }
    };

    slack::ok(json!({
        "user_id": user_id.to_string(),
        "username": username,
        "token": token,
    }))
}

// ── Users ──

pub async fn users_list(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
) -> Response {
    let sql = "SELECT id, username, display_name, email, avatar_url, status, is_bot, status_text, status_emoji, created_at FROM users";
    let result = match state.api.query_router().query_sync(sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("users list failed: {e}");
            return slack::err("internal_error");
        }
    };

    let members: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            json!({
                "id": row[0].to_json(),
                "username": row[1].to_json(),
                "display_name": row[2].to_json(),
                "email": row[3].to_json(),
                "avatar_url": row[4].to_json(),
                "status": row[5].to_json(),
                "is_bot": row[6].to_json(),
                "status_text": row[7].to_json(),
                "status_emoji": row[8].to_json(),
                "created_at": row[9].to_json(),
            })
        })
        .collect();

    slack::ok(json!({"members": members}))
}

pub async fn users_info(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(req): Json<UserInfoRequest>,
) -> Response {
    let sql = format!(
        "SELECT id, username, display_name, email, avatar_url, status, is_bot, status_text, status_emoji, created_at \
         FROM users WHERE id = {}",
        req.user
    );
    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("users info failed: {e}");
            return slack::err("internal_error");
        }
    };

    if result.rows.is_empty() {
        return slack::err("user_not_found");
    }

    let row = &result.rows[0];
    slack::ok(json!({
        "user": {
            "id": row[0].to_json(),
            "username": row[1].to_json(),
            "display_name": row[2].to_json(),
            "email": row[3].to_json(),
            "avatar_url": row[4].to_json(),
            "status": row[5].to_json(),
            "is_bot": row[6].to_json(),
            "status_text": row[7].to_json(),
            "status_emoji": row[8].to_json(),
            "created_at": row[9].to_json(),
        }
    }))
}

#[derive(Deserialize)]
pub struct UserInfoRequest {
    #[serde(deserialize_with = "deserialize_id")]
    pub user: i64,
}

pub async fn users_set_presence(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<SetPresenceRequest>,
) -> Response {
    let valid = ["online", "away", "dnd", "offline"];
    if !valid.contains(&req.presence.as_str()) {
        return slack::err("invalid_presence");
    }

    let sql = format!(
        "UPDATE users SET status = '{}' WHERE id = {}",
        escape_sql(&req.presence),
        claims.user_id
    );

    if let Err(e) = state.api.query_router().query_sync(&sql) {
        tracing::error!("set presence failed: {e}");
        return slack::err("internal_error");
    }
    state.persist_tables(&["users"]);

    // Fetch status fields for broadcast
    let fetch = format!(
        "SELECT status_text, status_emoji FROM users WHERE id = {}",
        claims.user_id
    );
    let (status_text, status_emoji) = match state.api.query_router().query_sync(&fetch) {
        Ok(r) if !r.rows.is_empty() => {
            let st = r.rows[0][0].to_json().as_str().unwrap_or("").to_string();
            let se = r.rows[0][1].to_json().as_str().unwrap_or("").to_string();
            (
                Some(st).filter(|s| !s.is_empty()),
                Some(se).filter(|s| !s.is_empty()),
            )
        }
        _ => (None, None),
    };

    // Broadcast presence change
    let event = crate::chat::events::ServerEvent::PresenceChange {
        user: claims.user_id.to_string(),
        presence: req.presence,
        status_text,
        status_emoji,
    };
    let online = state.hub.online_users().await;
    for uid in online {
        state.hub.send_to_user(uid, &event).await;
    }

    slack::ok(json!({}))
}

#[derive(Deserialize)]
pub struct SetPresenceRequest {
    pub presence: String,
}

#[derive(Deserialize)]
pub struct UpdateProfileRequest {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub status_text: Option<String>,
    #[serde(default)]
    pub status_emoji: Option<String>,
}

pub async fn users_update_profile(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<UpdateProfileRequest>,
) -> Response {
    let mut sets = Vec::new();
    if let Some(ref name) = req.display_name {
        sets.push(format!("display_name = '{}'", escape_sql(name)));
    }
    if let Some(ref url) = req.avatar_url {
        sets.push(format!("avatar_url = '{}'", escape_sql(url)));
    }
    if let Some(ref email) = req.email {
        let check = format!(
            "SELECT id FROM users WHERE email = '{}' AND id != {}",
            escape_sql(email),
            claims.user_id
        );
        match state.api.query_router().query_sync(&check) {
            Ok(r) if !r.rows.is_empty() => return slack::err("email_taken"),
            Err(e) => {
                tracing::error!("email check failed: {e}");
                return slack::err("internal_error");
            }
            _ => {}
        }
        sets.push(format!("email = '{}'", escape_sql(email)));
    }
    if let Some(ref text) = req.status_text {
        sets.push(format!("status_text = '{}'", escape_sql(text)));
    }
    if let Some(ref emoji) = req.status_emoji {
        sets.push(format!("status_emoji = '{}'", escape_sql(emoji)));
    }

    if sets.is_empty() {
        return slack::err("no_changes");
    }

    let sql = format!(
        "UPDATE users SET {} WHERE id = {}",
        sets.join(", "),
        claims.user_id
    );

    if let Err(e) = state.api.query_router().query_sync(&sql) {
        tracing::error!("update profile failed: {e}");
        return slack::err("internal_error");
    }
    state.persist_tables(&["users"]);

    // Fetch updated user for broadcast
    let fetch = format!(
        "SELECT display_name, avatar_url, status_text, status_emoji FROM users WHERE id = {}",
        claims.user_id
    );
    if let Ok(r) = state.api.query_router().query_sync(&fetch) {
        if let Some(row) = r.rows.first() {
            let display_name = row[0].to_json().as_str().unwrap_or("").to_string();
            let avatar_url = row[1].to_json().as_str().unwrap_or("").to_string();
            let status_text = row[2].to_json().as_str().unwrap_or("").to_string();
            let status_emoji = row[3].to_json().as_str().unwrap_or("").to_string();

            let event = crate::chat::events::ServerEvent::UserProfileUpdated {
                user: claims.user_id.to_string(),
                display_name,
                avatar_url,
                status_text,
                status_emoji,
            };
            let online = state.hub.online_users().await;
            for uid in online {
                state.hub.send_to_user(uid, &event).await;
            }
        }
    }

    slack::ok(json!({}))
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub old_password: String,
    pub new_password: String,
}

pub async fn users_change_password(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ChangePasswordRequest>,
) -> Response {
    if req.new_password.is_empty() {
        return slack::err("invalid_arguments");
    }
    if req.new_password.chars().count() < 8 {
        return slack::err("password_too_short");
    }

    // Fetch current password hash
    let sql = format!(
        "SELECT password_hash FROM users WHERE id = {}",
        claims.user_id
    );
    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("password fetch failed: {e}");
            return slack::err("internal_error");
        }
    };

    if result.rows.is_empty() {
        return slack::err("user_not_found");
    }

    let current_hash = match &result.rows[0][0] {
        crate::connector::Value::String(v) => v.clone(),
        _ => return slack::err("internal_error"),
    };

    // Verify old password
    match auth::verify_password(&req.old_password, &current_hash) {
        Ok(true) => {}
        Ok(false) => return slack::err("invalid_password"),
        Err(e) => {
            tracing::error!("password verify failed: {e}");
            return slack::err("internal_error");
        }
    }

    // Hash new password
    let new_hash = match auth::hash_password(&req.new_password) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!("password hash failed: {e}");
            return slack::err("internal_error");
        }
    };

    let update = format!(
        "UPDATE users SET password_hash = '{}' WHERE id = {}",
        escape_sql(&new_hash),
        claims.user_id
    );

    if let Err(e) = state.api.query_router().query_sync(&update) {
        tracing::error!("password update failed: {e}");
        return slack::err("internal_error");
    }
    state.persist_tables(&["users"]);

    slack::ok(json!({}))
}

// ── User Settings ──

pub async fn users_get_settings(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(_req): Json<serde_json::Value>,
) -> Response {
    // Hold lock for entire check-then-insert to prevent TOCTOU duplicate rows
    let _guard = state.settings_lock.lock().await;
    let sql = format!(
        "SELECT theme, notification_default, timezone FROM user_settings WHERE user_id = {}",
        claims.user_id
    );

    match state.api.query_router().query_sync(&sql) {
        Ok(r) if !r.rows.is_empty() => {
            let row = &r.rows[0];
            slack::ok(json!({
                "settings": {
                    "theme": row[0].to_json(),
                    "notification_default": row[1].to_json(),
                    "timezone": row[2].to_json(),
                }
            }))
        }
        Ok(_) => {
            let now = now_timestamp();
            let insert = format!(
                "INSERT INTO user_settings (user_id, theme, notification_default, timezone, created_at) \
                 VALUES ({}, 'dark', 'all', 'UTC', '{now}')",
                claims.user_id
            );
            let _ = state.api.query_router().query_sync(&insert);
            state.persist_tables(&["user_settings"]);
            slack::ok(json!({
                "settings": {
                    "theme": "dark",
                    "notification_default": "all",
                    "timezone": "UTC",
                }
            }))
        }
        Err(e) => {
            tracing::error!("get settings failed: {e}");
            slack::err("internal_error")
        }
    }
}

#[derive(Deserialize)]
pub struct UpdateSettingsRequest {
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub notification_default: Option<String>,
    #[serde(default)]
    pub timezone: Option<String>,
}

pub async fn users_update_settings(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<UpdateSettingsRequest>,
) -> Response {
    // Ensure settings row exists (upsert pattern, hold lock to prevent TOCTOU race)
    let _guard = state.settings_lock.lock().await;
    let check = format!(
        "SELECT user_id FROM user_settings WHERE user_id = {}",
        claims.user_id
    );
    let exists = state
        .api
        .query_router()
        .query_sync(&check)
        .map(|r| !r.rows.is_empty())
        .unwrap_or(false);

    if !exists {
        let now = now_timestamp();
        let insert = format!(
            "INSERT INTO user_settings (user_id, theme, notification_default, timezone, created_at) \
             VALUES ({}, 'dark', 'all', 'UTC', '{now}')",
            claims.user_id
        );
        let _ = state.api.query_router().query_sync(&insert);
        state.persist_tables(&["user_settings"]);
    }
    let mut sets = Vec::new();
    if let Some(ref theme) = req.theme {
        if !["dark", "light"].contains(&theme.as_str()) {
            return slack::err("invalid_theme");
        }
        sets.push(format!("theme = '{}'", escape_sql(theme)));
    }
    if let Some(ref notif) = req.notification_default {
        if !["all", "mentions", "none"].contains(&notif.as_str()) {
            return slack::err("invalid_notification_default");
        }
        sets.push(format!("notification_default = '{}'", escape_sql(notif)));
    }
    if let Some(ref tz) = req.timezone {
        sets.push(format!("timezone = '{}'", escape_sql(tz)));
    }

    if sets.is_empty() {
        return slack::err("no_changes");
    }

    let sql = format!(
        "UPDATE user_settings SET {} WHERE user_id = {}",
        sets.join(", "),
        claims.user_id
    );

    if let Err(e) = state.api.query_router().query_sync(&sql) {
        tracing::error!("update settings failed: {e}");
        return slack::err("internal_error");
    }
    state.persist_tables(&["user_settings"]);

    slack::ok(json!({}))
}

// ── Search / Autocomplete ──

#[derive(Deserialize)]
pub struct UsersSearchRequest {
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

pub async fn users_search(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(req): Json<UsersSearchRequest>,
) -> Response {
    let query_escaped = escape_sql_like(&req.query.to_lowercase());
    let limit = req.limit.min(100);
    let sql = format!(
        "SELECT id, username, display_name, avatar_url FROM users \
         WHERE LOWER(username) LIKE '%{query_escaped}%' ESCAPE '\\' OR LOWER(display_name) LIKE '%{query_escaped}%' ESCAPE '\\' \
         LIMIT {limit}",
    );
    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("users search failed: {e}");
            return slack::err("internal_error");
        }
    };

    let matches: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            json!({
                "id": row[0].to_json(),
                "username": row[1].to_json().as_str().unwrap_or("").to_string(),
                "display_name": row[2].to_json().as_str().unwrap_or("").to_string(),
                "avatar_url": row[3].to_json(),
            })
        })
        .collect();

    slack::ok(json!({"users": matches}))
}

#[derive(Deserialize)]
pub struct ConversationsAutocompleteRequest {
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

pub async fn conversations_autocomplete(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(req): Json<ConversationsAutocompleteRequest>,
) -> Response {
    let query_escaped = escape_sql_like(&req.query.to_lowercase());
    let limit = req.limit.min(100);
    let sql = format!(
        "SELECT id, name, topic FROM channels WHERE kind = 'public' AND LOWER(name) LIKE '{query_escaped}%' ESCAPE '\\' LIMIT {limit}",
    );
    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("conversations autocomplete failed: {e}");
            return slack::err("internal_error");
        }
    };

    let matches: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            json!({
                "id": row[0].to_json(),
                "name": row[1].to_json().as_str().unwrap_or("").to_string(),
                "topic": row[2].to_json(),
            })
        })
        .collect();

    slack::ok(json!({"channels": matches}))
}

// ── Conversations ──

#[derive(Deserialize)]
pub struct ConversationsCreateRequest {
    pub name: String,
    #[serde(default = "default_channel_kind")]
    pub kind: String,
    #[serde(default)]
    pub topic: Option<String>,
}

fn default_channel_kind() -> String {
    "public".to_string()
}

pub async fn conversations_create(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ConversationsCreateRequest>,
) -> Response {
    let valid_kinds = ["public", "private", "dm"];
    if !valid_kinds.contains(&req.kind.as_str()) {
        return slack::err("invalid_kind");
    }

    let name_trimmed = req.name.trim().to_string();
    if name_trimmed.is_empty() || name_trimmed.len() > 80 {
        return slack::err("invalid_name");
    }
    if !name_trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return slack::err("invalid_name");
    }

    // Lock to prevent TOCTOU race on channel name uniqueness
    let _create_guard = state.channel_create_lock.lock().await;

    // Check if channel name already exists
    let check = format!(
        "SELECT id FROM channels WHERE name = '{}'",
        escape_sql(&name_trimmed)
    );
    match state.api.query_router().query_sync(&check) {
        Ok(r) if !r.rows.is_empty() => return slack::err("name_taken"),
        Err(e) => {
            tracing::error!("channel check failed: {e}");
            return slack::err("internal_error");
        }
        _ => {}
    }

    let id = next_id();
    let now = now_timestamp();
    let topic = req.topic.unwrap_or_default();

    let insert = format!(
        "INSERT INTO channels (id, name, kind, topic, description, archived_at, created_by, created_at) \
         VALUES ({id}, '{name}', '{kind}', '{topic}', '', '', {created_by}, '{now}')",
        name = escape_sql(&name_trimmed),
        kind = escape_sql(&req.kind),
        topic = escape_sql(&topic),
        created_by = claims.user_id,
    );

    if let Err(e) = state.api.query_router().query_sync(&insert) {
        tracing::error!("channel create failed: {e}");
        return slack::err("internal_error");
    }

    // Add creator as owner
    let member_insert = format!(
        "INSERT INTO channel_members (channel_id, user_id, role, joined_at) \
         VALUES ({id}, {user_id}, 'owner', '{now}')",
        user_id = claims.user_id,
    );
    if let Err(e) = state.api.query_router().query_sync(&member_insert) {
        tracing::error!("channel member insert failed: {e}");
        return slack::err("internal_error");
    }
    state.persist_tables(&["channels", "channel_members"]);

    // Update hub membership cache
    let mut members = std::collections::HashSet::new();
    members.insert(claims.user_id);
    state.hub.set_channel_members(id, members).await;

    slack::created(json!({
        "channel": {
            "id": id.to_string(),
            "name": name_trimmed,
            "kind": req.kind,
            "topic": topic,
            "description": "",
            "archived_at": "",
            "created_by": claims.user_id.to_string(),
            "created_at": now,
        }
    }))
}

pub async fn conversations_list(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    // List channels the user is a member of
    let sql = format!(
        "SELECT c.id, c.name, c.kind, c.topic, c.description, c.archived_at, c.created_by, c.created_at \
         FROM channels c \
         JOIN channel_members cm ON c.id = cm.channel_id \
         WHERE cm.user_id = {}",
        claims.user_id
    );
    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("conversations list failed: {e}");
            return slack::err("internal_error");
        }
    };

    // Batch: fetch all channel_reads for this user
    let reads_sql = format!(
        "SELECT channel_id, last_read_ts FROM channel_reads WHERE user_id = {}",
        claims.user_id
    );
    let reads_map: std::collections::HashMap<String, String> = state
        .api
        .query_router()
        .query_sync(&reads_sql)
        .ok()
        .map(|r| {
            r.rows
                .iter()
                .filter_map(|row| {
                    let ch = match &row[0] {
                        crate::connector::Value::Int(n) => n.to_string(),
                        _ => return None,
                    };
                    let ts = row[1].to_json().as_str().unwrap_or("").to_string();
                    Some((ch, ts))
                })
                .collect()
        })
        .unwrap_or_default();

    // Batch: fetch all channel_settings for this user
    let settings_sql = format!(
        "SELECT channel_id, muted, notification_level FROM channel_settings WHERE user_id = {}",
        claims.user_id
    );
    let settings_map: std::collections::HashMap<String, (String, String)> = state
        .api
        .query_router()
        .query_sync(&settings_sql)
        .ok()
        .map(|r| {
            r.rows
                .iter()
                .filter_map(|row| {
                    let ch = match &row[0] {
                        crate::connector::Value::Int(n) => n.to_string(),
                        _ => return None,
                    };
                    let muted = row[1].to_json().as_str().unwrap_or("false").to_string();
                    let notif = row[2].to_json().as_str().unwrap_or("all").to_string();
                    Some((ch, (muted, notif)))
                })
                .collect()
        })
        .unwrap_or_default();

    let channels: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            let ch_id_str = match &row[0] {
                crate::connector::Value::Int(n) => n.to_string(),
                _ => row[0].to_json().as_str().unwrap_or("0").to_string(),
            };

            let last_read_ts = reads_map.get(&ch_id_str).cloned().unwrap_or_default();

            // Count unread — still per-channel but unavoidable without GROUP BY support
            let unread_sql = if last_read_ts.is_empty() {
                format!(
                    "SELECT COUNT(*) AS cnt FROM messages WHERE channel_id = {} AND thread_id = 0 AND deleted_at = ''",
                    ch_id_str
                )
            } else {
                format!(
                    "SELECT COUNT(*) AS cnt FROM messages WHERE channel_id = {} AND thread_id = 0 AND created_at > '{}' AND deleted_at = ''",
                    ch_id_str, escape_sql(&last_read_ts)
                )
            };
            let unread_count = state.api.query_router().query_sync(&unread_sql).ok()
                .and_then(|r| r.rows.first().map(|row| row[0].to_json()))
                .and_then(|v| v.as_str().and_then(|s| s.parse::<i64>().ok()))
                .unwrap_or(0);

            let (muted, notification_level) = settings_map
                .get(&ch_id_str)
                .cloned()
                .unwrap_or_else(|| ("false".to_string(), "all".to_string()));

            json!({
                "id": row[0].to_json(),
                "name": row[1].to_json(),
                "kind": row[2].to_json(),
                "topic": row[3].to_json(),
                "description": row[4].to_json(),
                "archived_at": row[5].to_json(),
                "created_by": row[6].to_json(),
                "created_at": row[7].to_json(),
                "unread_count": unread_count,
                "muted": muted,
                "notification_level": notification_level,
            })
        })
        .collect();

    slack::ok(json!({"channels": channels}))
}

pub async fn conversations_info(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ChannelIdRequest>,
) -> Response {
    // Check membership
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    let sql = format!(
        "SELECT id, name, kind, topic, description, archived_at, created_by, created_at \
         FROM channels WHERE id = {}",
        req.channel
    );
    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("conversations info failed: {e}");
            return slack::err("internal_error");
        }
    };

    if result.rows.is_empty() {
        return slack::err("channel_not_found");
    }

    let row = &result.rows[0];
    slack::ok(json!({
        "channel": {
            "id": row[0].to_json(),
            "name": row[1].to_json(),
            "kind": row[2].to_json(),
            "topic": row[3].to_json(),
            "description": row[4].to_json(),
            "archived_at": row[5].to_json(),
            "created_by": row[6].to_json(),
            "created_at": row[7].to_json(),
        }
    }))
}

#[derive(Deserialize)]
pub struct HistoryRequest {
    #[serde(deserialize_with = "deserialize_id")]
    pub channel: i64,
    #[serde(default = "default_history_limit")]
    pub limit: usize,
    #[serde(default, deserialize_with = "deserialize_opt_id")]
    pub before: Option<i64>,
}

fn default_history_limit() -> usize {
    50
}

pub async fn conversations_history(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<HistoryRequest>,
) -> Response {
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    let limit = req.limit.min(200);
    let before_clause = match req.before {
        Some(ts) => format!(" AND m.id < {}", ts),
        None => String::new(),
    };

    let sql = format!(
        "SELECT m.id, m.channel_id, m.user_id, m.thread_id, m.content, \
         m.deleted_at, m.edited_at, m.created_at, u.username \
         FROM messages m \
         JOIN users u ON m.user_id = u.id \
         WHERE m.channel_id = {} AND m.thread_id = 0 AND m.deleted_at = ''{} \
         ORDER BY m.id DESC LIMIT {}",
        req.channel, before_clause, limit
    );

    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("history failed: {e}");
            return slack::err("internal_error");
        }
    };

    let mut messages: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            json!({
                "ts": row[0].to_json(),
                "channel": row[1].to_json(),
                "user": row[2].to_json(),
                "thread_ts": row[3].to_json(),
                "text": row[4].to_json(),
                "edited_ts": row[6].to_json(),
                "created_at": row[7].to_json(),
                "username": row[8].to_json(),
            })
        })
        .collect();

    // Collect all message IDs and fetch reply counts per message.
    // Still N queries (TeideDB lacks IN/GROUP BY), but separates querying from mutation.
    let msg_ids: Vec<i64> = messages
        .iter()
        .filter_map(|msg| msg["ts"].as_str()?.parse::<i64>().ok())
        .filter(|&id| id != 0)
        .collect();

    // Fetch reply metadata for each message, building a map of thread_id -> (count, last_reply_ts)
    let mut reply_meta: std::collections::HashMap<i64, (i64, String)> =
        std::collections::HashMap::new();
    for &msg_id in &msg_ids {
        let reply_sql = format!(
            "SELECT COUNT(*) AS cnt FROM messages WHERE thread_id = {} AND deleted_at = ''",
            msg_id
        );
        if let Ok(reply_result) = state.api.query_router().query_sync(&reply_sql) {
            if let Some(row) = reply_result.rows.first() {
                let count: i64 = row[0]
                    .to_json()
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
                if count > 0 {
                    let last_sql = format!(
                        "SELECT MAX(created_at) AS last_reply FROM messages WHERE thread_id = {} AND deleted_at = ''",
                        msg_id
                    );
                    let last_ts = state
                        .api
                        .query_router()
                        .query_sync(&last_sql)
                        .ok()
                        .and_then(|r| r.rows.first().map(|row| row[0].to_json()))
                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                        .unwrap_or_default();
                    reply_meta.insert(msg_id, (count, last_ts));
                }
            }
        }
    }

    // Apply reply metadata from the map
    for msg in messages.iter_mut() {
        let msg_id: i64 = msg["ts"].as_str().unwrap_or("0").parse().unwrap_or(0);
        if let Some((count, last_ts)) = reply_meta.get(&msg_id) {
            let obj = msg.as_object_mut().unwrap();
            obj.insert("reply_count".to_string(), serde_json::json!(count));
            obj.insert("last_reply_ts".to_string(), serde_json::json!(last_ts));
        }
    }

    // Enrich messages with reactions and files
    enrich_reactions(&state, &mut messages);
    enrich_files(&state, &mut messages);

    // Update channel_reads for this user (locked to prevent TOCTOU duplicates)
    let now = now_timestamp();
    {
        let _reads_guard = state.reads_lock.lock().await;
        let read_check = format!(
            "SELECT user_id FROM channel_reads WHERE channel_id = {} AND user_id = {}",
            req.channel, claims.user_id
        );
        let has_existing = state
            .api
            .query_router()
            .query_sync(&read_check)
            .map(|r| !r.rows.is_empty())
            .unwrap_or(false);

        if has_existing {
            let update_sql =
                format!(
                "UPDATE channel_reads SET last_read_ts = '{}' WHERE channel_id = {} AND user_id = {}",
                escape_sql(&now), req.channel, claims.user_id
            );
            if let Err(e) = state.api.query_router().query_sync(&update_sql) {
                tracing::warn!("channel_reads update failed: {e}");
            }
        } else {
            let insert_sql =
                format!(
                "INSERT INTO channel_reads (channel_id, user_id, last_read_ts) VALUES ({}, {}, '{}')",
                req.channel, claims.user_id, escape_sql(&now)
            );
            if let Err(e) = state.api.query_router().query_sync(&insert_sql) {
                tracing::warn!("channel_reads insert failed: {e}");
            }
        }
        state.persist_tables(&["channel_reads"]);
    }

    slack::ok(json!({"messages": messages, "has_more": messages.len() == limit}))
}

pub async fn conversations_replies(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<RepliesRequest>,
) -> Response {
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    let sql = format!(
        "SELECT m.id, m.channel_id, m.user_id, m.thread_id, m.content, \
         m.deleted_at, m.edited_at, m.created_at, u.username \
         FROM messages m \
         JOIN users u ON m.user_id = u.id \
         WHERE m.channel_id = {} AND (m.id = {} OR m.thread_id = {}) AND m.deleted_at = '' \
         ORDER BY m.id ASC",
        req.channel, req.ts, req.ts
    );

    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("replies failed: {e}");
            return slack::err("internal_error");
        }
    };

    let mut messages: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            json!({
                "ts": row[0].to_json(),
                "channel": row[1].to_json(),
                "user": row[2].to_json(),
                "thread_ts": row[3].to_json(),
                "text": row[4].to_json(),
                "edited_ts": row[6].to_json(),
                "created_at": row[7].to_json(),
                "username": row[8].to_json(),
            })
        })
        .collect();

    enrich_reactions(&state, &mut messages);
    enrich_files(&state, &mut messages);

    slack::ok(json!({"messages": messages}))
}

#[derive(Deserialize)]
pub struct RepliesRequest {
    #[serde(deserialize_with = "deserialize_id")]
    pub channel: i64,
    #[serde(deserialize_with = "deserialize_id")]
    pub ts: i64,
}

pub async fn conversations_join(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ChannelIdRequest>,
) -> Response {
    // Check channel exists, is public, and not archived
    let sql = format!(
        "SELECT kind, archived_at FROM channels WHERE id = {}",
        req.channel
    );
    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("join check failed: {e}");
            return slack::err("internal_error");
        }
    };

    if result.rows.is_empty() {
        return slack::err("channel_not_found");
    }

    let kind = match &result.rows[0][0] {
        crate::connector::Value::String(s) => s.clone(),
        _ => return slack::err("internal_error"),
    };

    if kind != "public" {
        return slack::err("method_not_allowed_for_channel_type");
    }

    let archived_at = match &result.rows[0][1] {
        crate::connector::Value::String(s) => s.clone(),
        _ => String::new(),
    };
    if !archived_at.is_empty() {
        return slack::err("channel_archived");
    }

    // Lock to prevent TOCTOU race on duplicate membership
    let _join_guard = state.channel_join_lock.lock().await;

    // Check if already a member
    if is_channel_member(&state, req.channel, claims.user_id) {
        return slack::ok(json!({"already_in_channel": true}));
    }

    let now = now_timestamp();
    let insert = format!(
        "INSERT INTO channel_members (channel_id, user_id, role, joined_at) \
         VALUES ({}, {}, 'member', '{now}')",
        req.channel, claims.user_id
    );

    if let Err(e) = state.api.query_router().query_sync(&insert) {
        tracing::error!("join insert failed: {e}");
        return slack::err("internal_error");
    }
    state.persist_tables(&["channel_members"]);

    state
        .hub
        .add_channel_member(req.channel, claims.user_id)
        .await;

    // Broadcast member joined
    let event = crate::chat::events::ServerEvent::MemberJoinedChannel {
        channel: req.channel.to_string(),
        user: claims.user_id.to_string(),
    };
    state.hub.broadcast_to_channel(req.channel, &event).await;

    slack::ok(json!({"channel": {"id": req.channel.to_string()}}))
}

pub async fn conversations_leave(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ChannelIdRequest>,
) -> Response {
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("not_in_channel");
    }

    // Prevent the channel owner from leaving (would make channel unmanageable)
    let role_sql = format!(
        "SELECT role FROM channel_members WHERE channel_id = {} AND user_id = {}",
        req.channel, claims.user_id
    );
    if let Ok(r) = state.api.query_router().query_sync(&role_sql) {
        if !r.rows.is_empty() {
            let role = r.rows[0][0].to_json();
            if role.as_str() == Some("owner") {
                return slack::err("cant_leave_as_owner");
            }
        }
    }

    let sql = format!(
        "DELETE FROM channel_members WHERE channel_id = {} AND user_id = {}",
        req.channel, claims.user_id
    );

    if let Err(e) = state.api.query_router().query_sync(&sql) {
        tracing::error!("leave failed: {e}");
        return slack::err("internal_error");
    }
    state.persist_tables(&["channel_members"]);

    state
        .hub
        .remove_channel_member(req.channel, claims.user_id)
        .await;

    let event = crate::chat::events::ServerEvent::MemberLeftChannel {
        channel: req.channel.to_string(),
        user: claims.user_id.to_string(),
    };
    state.hub.broadcast_to_channel(req.channel, &event).await;

    slack::ok(json!({}))
}

pub async fn conversations_invite(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<InviteRequest>,
) -> Response {
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    // Check if channel is archived
    let arch_sql = format!(
        "SELECT archived_at FROM channels WHERE id = {}",
        req.channel
    );
    match state.api.query_router().query_sync(&arch_sql) {
        Ok(r) => {
            if let Some(row) = r.rows.first() {
                let archived = row[0].to_json().as_str().unwrap_or("").to_string();
                if !archived.is_empty() {
                    return slack::err("channel_archived");
                }
            }
        }
        Err(e) => {
            tracing::error!("archived check failed: {e}");
            return slack::err("internal_error");
        }
    }

    // Check if target user exists
    let check = format!("SELECT id FROM users WHERE id = {}", req.user);
    match state.api.query_router().query_sync(&check) {
        Ok(r) if r.rows.is_empty() => return slack::err("user_not_found"),
        Err(e) => {
            tracing::error!("invite user check failed: {e}");
            return slack::err("internal_error");
        }
        _ => {}
    }

    // Lock to prevent TOCTOU race on duplicate membership
    let _join_guard = state.channel_join_lock.lock().await;

    // Check if already a member
    if is_channel_member(&state, req.channel, req.user) {
        return slack::err("already_in_channel");
    }

    let now = now_timestamp();
    let insert = format!(
        "INSERT INTO channel_members (channel_id, user_id, role, joined_at) \
         VALUES ({}, {}, 'member', '{now}')",
        req.channel, req.user
    );

    if let Err(e) = state.api.query_router().query_sync(&insert) {
        tracing::error!("invite insert failed: {e}");
        return slack::err("internal_error");
    }
    state.persist_tables(&["channel_members"]);

    state.hub.add_channel_member(req.channel, req.user).await;

    let event = crate::chat::events::ServerEvent::MemberJoinedChannel {
        channel: req.channel.to_string(),
        user: req.user.to_string(),
    };
    state.hub.broadcast_to_channel(req.channel, &event).await;

    slack::ok(json!({}))
}

#[derive(Deserialize)]
pub struct InviteRequest {
    #[serde(deserialize_with = "deserialize_id")]
    pub channel: i64,
    #[serde(deserialize_with = "deserialize_id")]
    pub user: i64,
}

pub async fn conversations_members(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ChannelIdRequest>,
) -> Response {
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    let sql = format!(
        "SELECT cm.user_id, u.username, cm.role \
         FROM channel_members cm \
         JOIN users u ON cm.user_id = u.id \
         WHERE cm.channel_id = {}",
        req.channel
    );

    let result = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("members list failed: {e}");
            return slack::err("internal_error");
        }
    };

    let members: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            json!({
                "id": row[0].to_json(),
                "username": row[1].to_json(),
                "role": row[2].to_json(),
            })
        })
        .collect();

    slack::ok(json!({"members": members}))
}

pub async fn conversations_open(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ConversationsOpenRequest>,
) -> Response {
    let other_user = req.users.first().copied().unwrap_or(0);
    if other_user == 0 {
        return slack::err("invalid_arguments");
    }
    if other_user == claims.user_id {
        return slack::err("cannot_dm_self");
    }

    // Verify the target user exists
    let user_check = format!("SELECT id FROM users WHERE id = {}", other_user);
    match state.api.query_router().query_sync(&user_check) {
        Ok(r) if r.rows.is_empty() => return slack::err("user_not_found"),
        Err(e) => {
            tracing::error!("user check failed: {e}");
            return slack::err("internal_error");
        }
        _ => {}
    }

    // Serialize DM creation to prevent TOCTOU race (check-then-insert without
    // DB-level unique constraints could create duplicate DM channels).
    let _dm_guard = state.dm_create_lock.lock().await;

    // Look for existing DM between these two users
    // Use subquery instead of double-JOIN (TeideDB doesn't support multiple JOINs on same table)
    let dm_name = format!(
        "dm-{}-{}",
        claims.user_id.min(other_user),
        claims.user_id.max(other_user)
    );
    let sql = format!(
        "SELECT id, name FROM channels WHERE kind = 'dm' AND name = '{}'",
        escape_sql(&dm_name)
    );

    match state.api.query_router().query_sync(&sql) {
        Ok(r) if !r.rows.is_empty() => {
            let row = &r.rows[0];
            let channel_id = row[0].to_json();

            // Repair membership if a prior partial failure left it incomplete
            let channel_id_val: i64 = channel_id
                .as_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            if channel_id_val > 0 {
                let now = now_timestamp();
                for uid in [claims.user_id, other_user] {
                    let check = format!(
                        "SELECT user_id FROM channel_members WHERE channel_id = {channel_id_val} AND user_id = {uid}"
                    );
                    match state.api.query_router().query_sync(&check) {
                        Ok(mr) if mr.rows.is_empty() => {
                            let repair = format!(
                                "INSERT INTO channel_members (channel_id, user_id, role, joined_at) \
                                 VALUES ({channel_id_val}, {uid}, 'member', '{now}')"
                            );
                            if let Err(e) = state.api.query_router().query_sync(&repair) {
                                tracing::error!("dm membership repair failed: {e}");
                                return slack::err("internal_error");
                            }
                        }
                        Err(e) => {
                            tracing::error!("dm membership check failed: {e}");
                            return slack::err("internal_error");
                        }
                        _ => {}
                    }
                }
                state.persist_tables(&["channel_members"]);
            }

            // Sync repaired membership to the in-memory hub cache
            // so that broadcasts and typing checks work immediately
            let mut members = std::collections::HashSet::new();
            members.insert(claims.user_id);
            members.insert(other_user);
            state.hub.set_channel_members(channel_id_val, members).await;

            return slack::ok(json!({
                "channel": {
                    "id": channel_id,
                    "name": row[1].to_json(),
                    "kind": "dm",
                },
                "already_open": true,
            }));
        }
        Err(e) => {
            tracing::error!("dm check failed: {e}");
            return slack::err("internal_error");
        }
        _ => {}
    }

    // Create new DM channel
    let id = next_id();
    let now = now_timestamp();
    let dm_name = format!(
        "dm-{}-{}",
        claims.user_id.min(other_user),
        claims.user_id.max(other_user)
    );

    let insert_channel = format!(
        "INSERT INTO channels (id, name, kind, topic, description, archived_at, created_by, created_at) \
         VALUES ({id}, '{name}', 'dm', '', '', '', {created_by}, '{now}')",
        name = escape_sql(&dm_name),
        created_by = claims.user_id,
    );

    if let Err(e) = state.api.query_router().query_sync(&insert_channel) {
        tracing::error!("dm channel create failed: {e}");
        return slack::err("internal_error");
    }

    // Add both users as members
    for uid in [claims.user_id, other_user] {
        let member_sql = format!(
            "INSERT INTO channel_members (channel_id, user_id, role, joined_at) \
             VALUES ({id}, {uid}, 'member', '{now}')"
        );
        if let Err(e) = state.api.query_router().query_sync(&member_sql) {
            tracing::error!("dm member insert failed: {e}");
            // Clean up member rows first to avoid orphan memberships
            // (channel_members must be deleted before channels)
            let cleanup_members = format!("DELETE FROM channel_members WHERE channel_id = {id}");
            if let Err(ce) = state.api.query_router().query_sync(&cleanup_members) {
                tracing::error!("dm member cleanup also failed: {ce}");
            }
            let cleanup = format!("DELETE FROM channels WHERE id = {id}");
            if let Err(ce) = state.api.query_router().query_sync(&cleanup) {
                tracing::error!("dm channel cleanup also failed: {ce}");
            }
            return slack::err("internal_error");
        }
    }
    state.persist_tables(&["channels", "channel_members"]);

    let mut members = std::collections::HashSet::new();
    members.insert(claims.user_id);
    members.insert(other_user);
    state.hub.set_channel_members(id, members).await;

    slack::created(json!({
        "channel": {
            "id": id.to_string(),
            "name": dm_name,
            "kind": "dm",
        },
        "already_open": false,
    }))
}

#[derive(Deserialize)]
pub struct ConversationsOpenRequest {
    #[serde(deserialize_with = "deserialize_id_vec")]
    pub users: Vec<i64>,
}

#[derive(Deserialize)]
pub struct ChannelIdRequest {
    #[serde(deserialize_with = "deserialize_id")]
    pub channel: i64,
}

// ── Mark Read ──

#[derive(Deserialize)]
pub struct MarkReadRequest {
    #[serde(deserialize_with = "deserialize_id")]
    pub channel: i64,
    #[serde(default)]
    pub ts: Option<String>,
}

pub async fn conversations_mark_read(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<MarkReadRequest>,
) -> Response {
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    let now = now_timestamp();
    let ts = match req.ts {
        Some(t) => {
            let ts_val = match t.parse::<u64>() {
                Ok(v) => v,
                Err(_) => return slack::err("invalid_arguments"),
            };
            let now_val: u64 = now.parse().unwrap_or(0);
            if ts_val > now_val {
                now.clone()
            } else {
                t
            }
        }
        None => now,
    };

    // Upsert channel_reads (locked to prevent TOCTOU duplicates)
    let _reads_guard = state.reads_lock.lock().await;
    let check_sql = format!(
        "SELECT user_id FROM channel_reads WHERE channel_id = {} AND user_id = {}",
        req.channel, claims.user_id
    );
    let has_existing = state
        .api
        .query_router()
        .query_sync(&check_sql)
        .map(|r| !r.rows.is_empty())
        .unwrap_or(false);

    if has_existing {
        let sql =
            format!(
            "UPDATE channel_reads SET last_read_ts = '{}' WHERE channel_id = {} AND user_id = {}",
            escape_sql(&ts), req.channel, claims.user_id
        );
        if let Err(e) = state.api.query_router().query_sync(&sql) {
            tracing::error!("mark read update failed: {e}");
            return slack::err("internal_error");
        }
    } else {
        let sql =
            format!(
            "INSERT INTO channel_reads (channel_id, user_id, last_read_ts) VALUES ({}, {}, '{}')",
            req.channel, claims.user_id, escape_sql(&ts)
        );
        if let Err(e) = state.api.query_router().query_sync(&sql) {
            tracing::error!("mark read insert failed: {e}");
            return slack::err("internal_error");
        }
    }
    state.persist_tables(&["channel_reads"]);

    slack::ok(json!({}))
}

// ── Channel Update ──

#[derive(Deserialize)]
pub struct ConversationsUpdateRequest {
    #[serde(deserialize_with = "deserialize_id")]
    pub channel: i64,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub topic: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

pub async fn conversations_update(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ConversationsUpdateRequest>,
) -> Response {
    // Check caller is owner or admin
    let role_sql = format!(
        "SELECT role FROM channel_members WHERE channel_id = {} AND user_id = {}",
        req.channel, claims.user_id
    );
    let role = match state.api.query_router().query_sync(&role_sql) {
        Ok(r) if !r.rows.is_empty() => r.rows[0][0]
            .to_json()
            .as_str()
            .unwrap_or("member")
            .to_string(),
        _ => return slack::err("not_in_channel"),
    };
    if role != "owner" && role != "admin" {
        return slack::err("not_authorized");
    }

    // Hold lock across both uniqueness check and UPDATE to prevent TOCTOU race
    let _name_guard = if req.name.is_some() {
        Some(state.channel_create_lock.lock().await)
    } else {
        None
    };

    let mut sets = Vec::new();
    if let Some(ref name) = req.name {
        let name_trimmed = name.trim();
        if name_trimmed.is_empty() || name_trimmed.len() > 80 {
            return slack::err("invalid_name");
        }
        if !name_trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return slack::err("invalid_name");
        }
        let check = format!(
            "SELECT id FROM channels WHERE name = '{}' AND id != {}",
            escape_sql(name_trimmed),
            req.channel
        );
        match state.api.query_router().query_sync(&check) {
            Ok(r) if !r.rows.is_empty() => return slack::err("name_taken"),
            Ok(_) => {}
            Err(e) => {
                tracing::error!("name uniqueness check failed: {e}");
                return slack::err("internal_error");
            }
        }
        sets.push(format!("name = '{}'", escape_sql(name_trimmed)));
    }
    if let Some(ref topic) = req.topic {
        sets.push(format!("topic = '{}'", escape_sql(topic)));
    }
    if let Some(ref desc) = req.description {
        sets.push(format!("description = '{}'", escape_sql(desc)));
    }

    if sets.is_empty() {
        return slack::err("no_changes");
    }

    let sql = format!(
        "UPDATE channels SET {} WHERE id = {}",
        sets.join(", "),
        req.channel
    );
    if let Err(e) = state.api.query_router().query_sync(&sql) {
        tracing::error!("channel update failed: {e}");
        return slack::err("internal_error");
    }
    state.persist_tables(&["channels"]);

    // Broadcast
    let event = crate::chat::events::ServerEvent::ChannelUpdated {
        channel: req.channel.to_string(),
        name: req.name.map(|n| n.trim().to_string()),
        topic: req.topic,
        description: req.description,
        archived_at: None,
    };
    state.hub.broadcast_to_channel(req.channel, &event).await;

    slack::ok(json!({}))
}

// ── Archive / Unarchive / SetRole ──

#[derive(Deserialize)]
pub struct ConversationsArchiveRequest {
    #[serde(deserialize_with = "deserialize_id")]
    pub channel: i64,
}

pub async fn conversations_archive(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ConversationsArchiveRequest>,
) -> Response {
    // Only owner can archive
    let role_sql = format!(
        "SELECT role FROM channel_members WHERE channel_id = {} AND user_id = {}",
        req.channel, claims.user_id
    );
    match state.api.query_router().query_sync(&role_sql) {
        Ok(r) if !r.rows.is_empty() => {
            let role = r.rows[0][0].to_json().as_str().unwrap_or("").to_string();
            if role != "owner" {
                return slack::err("not_authorized");
            }
        }
        _ => return slack::err("not_in_channel"),
    }

    let now = now_timestamp();
    let sql = format!(
        "UPDATE channels SET archived_at = '{}' WHERE id = {}",
        now, req.channel
    );
    if let Err(e) = state.api.query_router().query_sync(&sql) {
        tracing::error!("archive failed: {e}");
        return slack::err("internal_error");
    }
    state.persist_tables(&["channels"]);

    let event = crate::chat::events::ServerEvent::ChannelUpdated {
        channel: req.channel.to_string(),
        name: None,
        topic: None,
        description: None,
        archived_at: Some(now),
    };
    state.hub.broadcast_to_channel(req.channel, &event).await;

    slack::ok(json!({}))
}

pub async fn conversations_unarchive(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ConversationsArchiveRequest>,
) -> Response {
    // Only owner can unarchive
    let role_sql = format!(
        "SELECT role FROM channel_members WHERE channel_id = {} AND user_id = {}",
        req.channel, claims.user_id
    );
    match state.api.query_router().query_sync(&role_sql) {
        Ok(r) if !r.rows.is_empty() => {
            let role = r.rows[0][0].to_json().as_str().unwrap_or("").to_string();
            if role != "owner" {
                return slack::err("not_authorized");
            }
        }
        _ => return slack::err("not_in_channel"),
    }

    let sql = format!(
        "UPDATE channels SET archived_at = '' WHERE id = {}",
        req.channel
    );
    if let Err(e) = state.api.query_router().query_sync(&sql) {
        tracing::error!("unarchive failed: {e}");
        return slack::err("internal_error");
    }
    state.persist_tables(&["channels"]);

    let event = crate::chat::events::ServerEvent::ChannelUpdated {
        channel: req.channel.to_string(),
        name: None,
        topic: None,
        description: None,
        archived_at: Some(String::new()),
    };
    state.hub.broadcast_to_channel(req.channel, &event).await;

    slack::ok(json!({}))
}

#[derive(Deserialize)]
pub struct SetRoleRequest {
    #[serde(deserialize_with = "deserialize_id")]
    pub channel: i64,
    #[serde(deserialize_with = "deserialize_id")]
    pub user: i64,
    pub role: String,
}

pub async fn conversations_set_role(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<SetRoleRequest>,
) -> Response {
    if !["admin", "member"].contains(&req.role.as_str()) {
        return slack::err("invalid_role");
    }
    if req.user == claims.user_id {
        return slack::err("cannot_change_own_role");
    }

    // Caller must be owner
    let caller_role_sql = format!(
        "SELECT role FROM channel_members WHERE channel_id = {} AND user_id = {}",
        req.channel, claims.user_id
    );
    match state.api.query_router().query_sync(&caller_role_sql) {
        Ok(r) if !r.rows.is_empty() => {
            let role = r.rows[0][0].to_json().as_str().unwrap_or("").to_string();
            if role != "owner" {
                return slack::err("not_authorized");
            }
        }
        _ => return slack::err("not_in_channel"),
    }

    // Target must be in channel
    let target_sql = format!(
        "SELECT role FROM channel_members WHERE channel_id = {} AND user_id = {}",
        req.channel, req.user
    );
    match state.api.query_router().query_sync(&target_sql) {
        Ok(r) if !r.rows.is_empty() => {
            let current = r.rows[0][0].to_json().as_str().unwrap_or("").to_string();
            if current == "owner" {
                return slack::err("cannot_change_owner");
            }
        }
        _ => return slack::err("user_not_in_channel"),
    }

    let sql = format!(
        "UPDATE channel_members SET role = '{}' WHERE channel_id = {} AND user_id = {}",
        escape_sql(&req.role),
        req.channel,
        req.user
    );
    if let Err(e) = state.api.query_router().query_sync(&sql) {
        tracing::error!("set role failed: {e}");
        return slack::err("internal_error");
    }
    state.persist_tables(&["channel_members"]);

    slack::ok(json!({}))
}

// ── Channel Settings (Mute / Notification) ──

#[derive(Deserialize)]
pub struct MuteRequest {
    #[serde(deserialize_with = "deserialize_id")]
    pub channel: i64,
}

/// Upsert channel_settings row for (channel_id, user_id).
/// Caller must hold `state.settings_lock` to prevent TOCTOU races.
fn upsert_channel_setting(
    state: &AppState,
    channel_id: i64,
    user_id: i64,
    field: &str,
    value: &str,
) -> Result<(), String> {
    // Validate field name against allowlist to prevent SQL injection
    if !matches!(field, "muted" | "notification_level") {
        return Err(format!("invalid field: {field}"));
    }
    let now = now_timestamp();
    let check_sql = format!(
        "SELECT channel_id FROM channel_settings WHERE channel_id = {} AND user_id = {}",
        channel_id, user_id
    );
    let has_existing = state
        .api
        .query_router()
        .query_sync(&check_sql)
        .map(|r| !r.rows.is_empty())
        .unwrap_or(false);

    if has_existing {
        let sql = format!(
            "UPDATE channel_settings SET {} = '{}' WHERE channel_id = {} AND user_id = {}",
            field,
            escape_sql(value),
            channel_id,
            user_id
        );
        state
            .api
            .query_router()
            .query_sync(&sql)
            .map_err(|e| e.to_string())?;
    } else {
        let (muted, notification_level) = if field == "muted" {
            (value, "all")
        } else {
            ("false", value)
        };
        let sql = format!(
            "INSERT INTO channel_settings (channel_id, user_id, muted, notification_level, created_at) VALUES ({}, {}, '{}', '{}', '{}')",
            channel_id, user_id, escape_sql(muted), escape_sql(notification_level), now
        );
        state
            .api
            .query_router()
            .query_sync(&sql)
            .map_err(|e| e.to_string())?;
    }
    state.persist_tables(&["channel_settings"]);
    Ok(())
}

pub async fn conversations_mute(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<MuteRequest>,
) -> Response {
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    let _guard = state.settings_lock.lock().await;
    if let Err(e) = upsert_channel_setting(&state, req.channel, claims.user_id, "muted", "true") {
        tracing::error!("mute failed: {e}");
        return slack::err("internal_error");
    }

    slack::ok(json!({}))
}

pub async fn conversations_unmute(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<MuteRequest>,
) -> Response {
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    let _guard = state.settings_lock.lock().await;
    if let Err(e) = upsert_channel_setting(&state, req.channel, claims.user_id, "muted", "false") {
        tracing::error!("unmute failed: {e}");
        return slack::err("internal_error");
    }

    slack::ok(json!({}))
}

#[derive(Deserialize)]
pub struct SetNotificationRequest {
    #[serde(deserialize_with = "deserialize_id")]
    pub channel: i64,
    pub level: String,
}

pub async fn conversations_set_notification(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<SetNotificationRequest>,
) -> Response {
    if !["all", "mentions", "none"].contains(&req.level.as_str()) {
        return slack::err("invalid_level");
    }

    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    let _guard = state.settings_lock.lock().await;
    if let Err(e) = upsert_channel_setting(
        &state,
        req.channel,
        claims.user_id,
        "notification_level",
        &req.level,
    ) {
        tracing::error!("set notification failed: {e}");
        return slack::err("internal_error");
    }

    slack::ok(json!({}))
}

// ── Messaging ──

pub async fn chat_post_message(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<PostMessageRequest>,
) -> Response {
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    // Check if channel is archived
    let arch_sql = format!(
        "SELECT archived_at FROM channels WHERE id = {}",
        req.channel
    );
    match state.api.query_router().query_sync(&arch_sql) {
        Ok(r) => {
            if let Some(row) = r.rows.first() {
                let archived = row[0].to_json().as_str().unwrap_or("").to_string();
                if !archived.is_empty() {
                    return slack::err("channel_archived");
                }
            }
        }
        Err(e) => {
            tracing::error!("archived check failed: {e}");
            return slack::err("internal_error");
        }
    }

    if req.text.chars().count() > 40_000 {
        return slack::err("msg_too_long");
    }

    let id = next_id();
    let now = now_timestamp();
    let thread_id = req.thread_ts.unwrap_or(0);

    let insert = format!(
        "INSERT INTO messages (id, channel_id, user_id, thread_id, content, deleted_at, edited_at, created_at) \
         VALUES ({id}, {channel}, {user}, {thread}, '{text}', '', '', '{now}')",
        channel = req.channel,
        user = claims.user_id,
        thread = thread_id,
        text = escape_sql(&req.text),
    );

    if let Err(e) = state.api.query_router().query_sync(&insert) {
        tracing::error!("post message failed: {e}");
        return slack::err("internal_error");
    }
    state.persist_tables(&["messages"]);

    // Parse and store mentions
    let mention_regex = regex_lite::Regex::new(r"@(\w+)").unwrap();
    for cap in mention_regex.captures_iter(&req.text) {
        let mentioned_username = &cap[1];
        let user_sql = format!(
            "SELECT id FROM users WHERE username = '{}'",
            escape_sql(mentioned_username)
        );
        if let Ok(r) = state.api.query_router().query_sync(&user_sql) {
            if let Some(row) = r.rows.first() {
                if let crate::connector::Value::Int(uid) = &row[0] {
                    // Check uniqueness before inserting
                    let check = format!(
                        "SELECT message_id FROM mentions WHERE message_id = {} AND user_id = {}",
                        id, uid
                    );
                    if let Ok(existing) = state.api.query_router().query_sync(&check) {
                        if existing.rows.is_empty() {
                            let mention_sql = format!(
                                "INSERT INTO mentions (message_id, user_id) VALUES ({}, {})",
                                id, uid
                            );
                            let _ = state.api.query_router().query_sync(&mention_sql);
                        }
                    }
                }
            }
        }
    }
    state.persist_tables(&["mentions"]);

    // Index message in tantivy for full-text search
    let channel_name =
        crate::chat::models::channel_display_name(state.api.query_router(), req.channel);
    let doc = vec![(
        id.to_string(),
        "chat".to_string(),
        channel_name,
        req.text.clone(),
    )];
    if let Err(e) = state.api.search_engine().index_documents(&doc) {
        tracing::warn!("message search indexing failed: {e}");
    }

    // Broadcast message event
    let event = crate::chat::events::ServerEvent::Message {
        channel: req.channel.to_string(),
        user: claims.user_id.to_string(),
        text: req.text.clone(),
        ts: id.to_string(),
        thread_ts: if thread_id != 0 {
            Some(thread_id.to_string())
        } else {
            None
        },
        files: None,
    };
    state.hub.broadcast_to_channel(req.channel, &event).await;

    slack::ok(json!({
        "message": {
            "ts": id.to_string(),
            "channel": req.channel.to_string(),
            "user": claims.user_id.to_string(),
            "text": req.text,
            "created_at": now,
        }
    }))
}

#[derive(Deserialize)]
pub struct PostMessageRequest {
    #[serde(deserialize_with = "deserialize_id")]
    pub channel: i64,
    pub text: String,
    #[serde(default, deserialize_with = "deserialize_opt_id")]
    pub thread_ts: Option<i64>,
}

pub async fn chat_update(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ChatUpdateRequest>,
) -> Response {
    // Verify the message exists, is not deleted, and belongs to the user
    let check = format!(
        "SELECT user_id, channel_id FROM messages WHERE id = {} AND deleted_at = ''",
        req.ts
    );
    let result = match state.api.query_router().query_sync(&check) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("chat update check failed: {e}");
            return slack::err("internal_error");
        }
    };

    if result.rows.is_empty() {
        return slack::err("message_not_found");
    }

    let msg_user = match &result.rows[0][0] {
        crate::connector::Value::Int(v) => *v,
        _ => return slack::err("internal_error"),
    };
    let channel_id = match &result.rows[0][1] {
        crate::connector::Value::Int(v) => *v,
        _ => return slack::err("internal_error"),
    };

    if msg_user != claims.user_id {
        return slack::err("cant_update_message");
    }

    if req.text.chars().count() > 40_000 {
        return slack::err("msg_too_long");
    }

    let now = now_timestamp();

    let update_sql = format!(
        "UPDATE messages SET content = '{}', edited_at = '{}' WHERE id = {}",
        escape_sql(&req.text),
        now,
        req.ts
    );

    if let Err(e) = state.api.query_router().query_sync(&update_sql) {
        tracing::error!("chat update failed: {e}");
        return slack::err("internal_error");
    }
    state.persist_tables(&["messages"]);

    // Re-index in tantivy: delete old doc and add updated one
    let _ = state
        .api
        .search_engine()
        .delete_documents(&[req.ts.to_string()]);
    let channel_name =
        crate::chat::models::channel_display_name(state.api.query_router(), channel_id);
    let doc = vec![(
        req.ts.to_string(),
        "chat".to_string(),
        channel_name,
        req.text.clone(),
    )];
    if let Err(e) = state.api.search_engine().index_documents(&doc) {
        tracing::warn!("message search re-indexing failed: {e}");
    }

    // Broadcast message_changed
    let event = crate::chat::events::ServerEvent::MessageChanged {
        channel: channel_id.to_string(),
        message: crate::chat::events::MessagePayload {
            user: claims.user_id.to_string(),
            text: req.text.clone(),
            ts: req.ts.to_string(),
            edited_ts: now,
        },
    };
    state.hub.broadcast_to_channel(channel_id, &event).await;

    slack::ok(json!({
        "message": {
            "ts": req.ts.to_string(),
            "text": req.text,
        }
    }))
}

#[derive(Deserialize)]
pub struct ChatUpdateRequest {
    #[serde(deserialize_with = "deserialize_id")]
    pub ts: i64,
    pub text: String,
}

pub async fn chat_delete(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ChatDeleteRequest>,
) -> Response {
    let check = format!(
        "SELECT user_id, channel_id FROM messages WHERE id = {} AND deleted_at = ''",
        req.ts
    );
    let result = match state.api.query_router().query_sync(&check) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("chat delete check failed: {e}");
            return slack::err("internal_error");
        }
    };

    if result.rows.is_empty() {
        return slack::err("message_not_found");
    }

    let msg_user = match &result.rows[0][0] {
        crate::connector::Value::Int(v) => *v,
        _ => return slack::err("internal_error"),
    };
    let channel_id = match &result.rows[0][1] {
        crate::connector::Value::Int(v) => *v,
        _ => return slack::err("internal_error"),
    };

    if msg_user != claims.user_id {
        return slack::err("cant_delete_message");
    }

    let now = now_timestamp();
    let soft_delete = format!(
        "UPDATE messages SET deleted_at = '{}' WHERE id = {}",
        now, req.ts
    );

    if let Err(e) = state.api.query_router().query_sync(&soft_delete) {
        tracing::error!("chat delete failed: {e}");
        return slack::err("internal_error");
    }
    state.persist_tables(&["messages"]);

    // Remove from tantivy search index
    if let Err(e) = state
        .api
        .search_engine()
        .delete_documents(&[req.ts.to_string()])
    {
        tracing::warn!("message search index removal failed: {e}");
    }

    let event = crate::chat::events::ServerEvent::MessageDeleted {
        channel: channel_id.to_string(),
        ts: req.ts.to_string(),
    };
    state.hub.broadcast_to_channel(channel_id, &event).await;

    slack::ok(json!({}))
}

#[derive(Deserialize)]
pub struct ChatDeleteRequest {
    #[serde(deserialize_with = "deserialize_id")]
    pub ts: i64,
}

// ── Reactions ──

pub async fn reactions_add(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ReactionRequest>,
) -> Response {
    // Get channel from message
    let check = format!(
        "SELECT channel_id FROM messages WHERE id = {} AND deleted_at = ''",
        req.timestamp
    );
    let result = match state.api.query_router().query_sync(&check) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("reaction check failed: {e}");
            return slack::err("internal_error");
        }
    };

    if result.rows.is_empty() {
        return slack::err("message_not_found");
    }

    let channel_id = match &result.rows[0][0] {
        crate::connector::Value::Int(v) => *v,
        _ => return slack::err("internal_error"),
    };

    if !is_channel_member(&state, channel_id, claims.user_id) {
        return slack::err("channel_not_found");
    }

    // Hold lock for check-then-insert to prevent duplicate reactions from TOCTOU races
    let _reaction_guard = state.reaction_lock.lock().await;

    // Check uniqueness
    let dup_check = format!(
        "SELECT message_id FROM reactions WHERE message_id = {} AND user_id = {} AND emoji = '{}'",
        req.timestamp,
        claims.user_id,
        escape_sql(&req.name)
    );
    if let Ok(r) = state.api.query_router().query_sync(&dup_check) {
        if !r.rows.is_empty() {
            return slack::err("already_reacted");
        }
    }

    let now = now_timestamp();
    let insert = format!(
        "INSERT INTO reactions (message_id, user_id, emoji, created_at) \
         VALUES ({}, {}, '{}', '{now}')",
        req.timestamp,
        claims.user_id,
        escape_sql(&req.name)
    );

    if let Err(e) = state.api.query_router().query_sync(&insert) {
        tracing::error!("reaction insert failed: {e}");
        return slack::err("internal_error");
    }
    state.persist_tables(&["reactions"]);

    let event = crate::chat::events::ServerEvent::ReactionAdded {
        channel: channel_id.to_string(),
        user: claims.user_id.to_string(),
        reaction: req.name,
        item_ts: req.timestamp.to_string(),
    };
    state.hub.broadcast_to_channel(channel_id, &event).await;

    slack::ok(json!({}))
}

pub async fn reactions_remove(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ReactionRequest>,
) -> Response {
    let check = format!(
        "SELECT channel_id FROM messages WHERE id = {}",
        req.timestamp
    );
    let result = match state.api.query_router().query_sync(&check) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("reaction remove check failed: {e}");
            return slack::err("internal_error");
        }
    };

    if result.rows.is_empty() {
        return slack::err("message_not_found");
    }

    let channel_id = match &result.rows[0][0] {
        crate::connector::Value::Int(v) => *v,
        _ => return slack::err("internal_error"),
    };

    if !is_channel_member(&state, channel_id, claims.user_id) {
        return slack::err("channel_not_found");
    }

    // Atomic delete: single DELETE, parse result to check if any rows were removed.
    // TeideDB returns "Deleted N rows from 'reactions'" for DELETE statements.
    let delete = format!(
        "DELETE FROM reactions WHERE message_id = {} AND user_id = {} AND emoji = '{}'",
        req.timestamp,
        claims.user_id,
        escape_sql(&req.name)
    );

    let delete_result = match state.api.query_router().query_sync(&delete) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("reaction delete failed: {e}");
            return slack::err("internal_error");
        }
    };

    // Only broadcast if rows were actually deleted (status message starts with "Deleted 0" when nothing matched)
    let actually_deleted = delete_result
        .rows
        .first()
        .and_then(|row| row.first())
        .and_then(|v| match v {
            crate::connector::Value::String(s) => Some(!s.starts_with("Deleted 0")),
            _ => None,
        })
        .unwrap_or(false);

    if !actually_deleted {
        return slack::err("no_reaction");
    }
    state.persist_tables(&["reactions"]);

    let event = crate::chat::events::ServerEvent::ReactionRemoved {
        channel: channel_id.to_string(),
        user: claims.user_id.to_string(),
        reaction: req.name,
        item_ts: req.timestamp.to_string(),
    };
    state.hub.broadcast_to_channel(channel_id, &event).await;

    slack::ok(json!({}))
}

#[derive(Deserialize)]
pub struct ReactionRequest {
    pub name: String,
    #[serde(deserialize_with = "deserialize_id")]
    pub timestamp: i64,
}

// ── Search ──

#[derive(Deserialize)]
pub struct SearchMessagesRequest {
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
    #[serde(default, deserialize_with = "deserialize_opt_id")]
    pub user_id: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_opt_id")]
    pub channel_id: Option<i64>,
    #[serde(default)]
    pub date_from: Option<String>,
    #[serde(default)]
    pub date_to: Option<String>,
}

fn default_search_limit() -> usize {
    20
}

pub async fn search_messages(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<SearchMessagesRequest>,
) -> Response {
    if req.query.is_empty() {
        return slack::err("invalid_arguments");
    }

    let limit = req.limit.min(100);
    let filter_user_id = req.user_id;
    let filter_channel_id = req.channel_id;
    // Convert date filter value to epoch seconds. Accepts either a plain epoch-second
    // string (sent by the UI after local-timezone conversion) or a YYYY-MM-DD date string
    // (interpreted as UTC for backwards compatibility with direct API callers).
    fn date_str_to_epoch(d: &str, end_of_day: bool) -> Option<i64> {
        // Try plain epoch seconds first (no dashes)
        if !d.contains('-') {
            return d.parse::<i64>().ok();
        }
        let parts: Vec<&str> = d.split('-').collect();
        if parts.len() != 3 {
            return None;
        }
        let y: i64 = parts[0].parse().ok()?;
        let m: i64 = parts[1].parse().ok()?;
        let day: i64 = parts[2].parse().ok()?;
        // Days from year 0 to Unix epoch (1970-01-01)
        fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
            let y = if m <= 2 { y - 1 } else { y };
            let era = y / 400;
            let yoe = y - era * 400;
            let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
            let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
            era * 146097 + doe - 719468
        }
        let epoch_days = days_from_civil(y, m, day);
        let secs = epoch_days * 86400 + if end_of_day { 86399 } else { 0 };
        Some(secs)
    }
    let filter_date_from_epoch = req
        .date_from
        .as_ref()
        .and_then(|d| date_str_to_epoch(d, false));
    let filter_date_to_epoch = req
        .date_to
        .as_ref()
        .and_then(|d| date_str_to_epoch(d, true));

    // Get the set of channel IDs the user is a member of for filtering
    let member_channel_ids: std::collections::HashSet<i64> = {
        let sql = format!(
            "SELECT channel_id FROM channel_members WHERE user_id = {}",
            claims.user_id
        );
        match state.api.query_router().query_sync(&sql) {
            Ok(r) => r
                .rows
                .iter()
                .filter_map(|row| match &row[0] {
                    crate::connector::Value::Int(v) => Some(*v),
                    _ => None,
                })
                .collect(),
            Err(e) => {
                tracing::error!("membership lookup failed: {e}");
                return slack::err("internal_error");
            }
        }
    };

    // Over-fetch to compensate for post-query auth filtering
    let search_query = crate::search::SearchQuery {
        text: req.query,
        sources: Some(vec!["chat".to_string()]),
        limit: limit * crate::chat::models::SEARCH_OVERFETCH_FACTOR,
        date_from: None,
        date_to: None,
    };

    let results = match state.api.search_engine().search(&search_query) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("search.messages failed: {e}");
            return slack::err("internal_error");
        }
    };

    // Look up channel_id for each message to filter by ID (not name)
    // Parse IDs to i64 to prevent SQL injection from arbitrary tantivy document IDs
    let msg_ids: Vec<i64> = results
        .iter()
        .filter_map(|r| r.id.parse::<i64>().ok())
        .collect();
    // Fetch channel_id, user_id, created_at for each matched message
    struct MsgMeta {
        channel_id: i64,
        user_id: i64,
        created_at: String,
    }
    let msg_meta_map: std::collections::HashMap<String, MsgMeta> = if msg_ids.is_empty() {
        std::collections::HashMap::new()
    } else {
        let id_list = msg_ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT id, channel_id, user_id, created_at FROM messages WHERE id IN ({id_list}) AND deleted_at = ''"
        );
        match state.api.query_router().query_sync(&sql) {
            Ok(r) => r
                .rows
                .iter()
                .filter_map(|row| {
                    let id = match &row[0] {
                        crate::connector::Value::Int(v) => v.to_string(),
                        _ => return None,
                    };
                    let ch_id = match &row[1] {
                        crate::connector::Value::Int(v) => *v,
                        _ => return None,
                    };
                    let user_id = match &row[2] {
                        crate::connector::Value::Int(v) => *v,
                        _ => 0,
                    };
                    let created_at = row[3].to_json().as_str().unwrap_or("").to_string();
                    Some((
                        id,
                        MsgMeta {
                            channel_id: ch_id,
                            user_id,
                            created_at,
                        },
                    ))
                })
                .collect(),
            Err(e) => {
                tracing::error!("message channel lookup failed: {e}");
                return slack::err("internal_error");
            }
        }
    };

    // Filter results: membership + optional user/channel/date filters
    let filtered: Vec<_> = results
        .iter()
        .filter(|r| {
            msg_meta_map.get(&r.id).is_some_and(|meta| {
                if !member_channel_ids.contains(&meta.channel_id) {
                    return false;
                }
                if let Some(uid) = filter_user_id {
                    if meta.user_id != uid {
                        return false;
                    }
                }
                if let Some(cid) = filter_channel_id {
                    if meta.channel_id != cid {
                        return false;
                    }
                }
                if let Some(ref from_epoch) = filter_date_from_epoch {
                    if let Ok(ts) = meta.created_at.parse::<i64>() {
                        if ts < *from_epoch {
                            return false;
                        }
                    }
                }
                if let Some(ref to_epoch) = filter_date_to_epoch {
                    if let Ok(ts) = meta.created_at.parse::<i64>() {
                        if ts > *to_epoch {
                            return false;
                        }
                    }
                }
                true
            })
        })
        .collect();
    let total = filtered.len();
    let matches: Vec<serde_json::Value> = filtered
        .into_iter()
        .take(limit)
        .map(|r| {
            let meta = msg_meta_map.get(&r.id);
            json!({
                "ts": r.id,
                "channel": meta.map(|m| m.channel_id).unwrap_or(0),
                "user": meta.map(|m| m.user_id).unwrap_or(0),
                "created_at": meta.map(|m| m.created_at.as_str()).unwrap_or(""),
                "text": r.snippet,
                "score": r.score,
            })
        })
        .collect();

    slack::ok(json!({
        "messages": {
            "matches": matches,
            "total": total,
        }
    }))
}

// ── Directory ──

#[derive(Deserialize)]
pub struct DirectoryRequest {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
    #[serde(default, deserialize_with = "deserialize_opt_id")]
    pub cursor: Option<i64>,
    #[serde(default)]
    pub archived: bool,
}

pub async fn conversations_directory(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(req): Json<DirectoryRequest>,
) -> Response {
    let limit = req.limit.min(100);

    let mut sql =
        "SELECT id, name, kind, topic, description, created_at FROM channels WHERE kind = 'public'"
            .to_string();

    if !req.archived {
        sql.push_str(" AND archived_at = ''");
    }

    if let Some(ref q) = req.query {
        if !q.is_empty() {
            let escaped_like = escape_sql_like(q);
            sql.push_str(&format!(" AND name LIKE '%{escaped_like}%' ESCAPE '\\'"));
        }
    }

    if let Some(cursor) = req.cursor {
        sql.push_str(&format!(" AND id > {cursor}"));
    }

    sql.push_str(&format!(" ORDER BY id LIMIT {limit}"));

    let rows = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r.rows,
        Err(e) => {
            tracing::error!("conversations.directory failed: {e}");
            return slack::err("internal_error");
        }
    };

    let mut channels = Vec::new();
    for row in &rows {
        let id = match &row[0] {
            crate::connector::Value::Int(v) => *v,
            _ => continue,
        };
        let name = row[1].to_json().as_str().unwrap_or("").to_string();
        let kind = row[2].to_json().as_str().unwrap_or("").to_string();
        let topic = row[3].to_json().as_str().unwrap_or("").to_string();
        let description = row[4].to_json().as_str().unwrap_or("").to_string();
        let created_at = row[5].to_json().as_str().unwrap_or("").to_string();

        // Compute member count
        let count_sql = format!("SELECT COUNT(*) FROM channel_members WHERE channel_id = {id}");
        let member_count = match state.api.query_router().query_sync(&count_sql) {
            Ok(r) => r
                .rows
                .first()
                .and_then(|r| match &r[0] {
                    crate::connector::Value::Int(v) => Some(*v),
                    _ => None,
                })
                .unwrap_or(0),
            Err(_) => 0,
        };

        channels.push(json!({
            "id": id.to_string(),
            "name": name,
            "kind": kind,
            "topic": topic,
            "description": description,
            "created_at": created_at,
            "member_count": member_count,
        }));
    }

    slack::ok(json!({ "channels": channels }))
}

// ── Helpers ──

// ── Pins ──

#[derive(Deserialize)]
pub struct PinAddRequest {
    #[serde(deserialize_with = "deserialize_id")]
    pub channel: i64,
    #[serde(alias = "timestamp", deserialize_with = "deserialize_id")]
    pub message_id: i64,
}

pub async fn pins_add(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<PinAddRequest>,
) -> Response {
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    // Verify message exists in this channel
    let check = format!(
        "SELECT id FROM messages WHERE id = {} AND channel_id = {} AND deleted_at = ''",
        req.message_id, req.channel
    );
    match state.api.query_router().query_sync(&check) {
        Ok(r) if r.rows.is_empty() => return slack::err("message_not_found"),
        Err(e) => {
            tracing::error!("pin check failed: {e}");
            return slack::err("internal_error");
        }
        _ => {}
    }

    // Serialize pin creation to prevent TOCTOU duplicate pins
    let _pin_guard = state.pin_lock.lock().await;

    // Idempotent: check if already pinned
    let dup = format!(
        "SELECT message_id FROM pinned_messages WHERE channel_id = {} AND message_id = {}",
        req.channel, req.message_id
    );
    if let Ok(r) = state.api.query_router().query_sync(&dup) {
        if !r.rows.is_empty() {
            return slack::ok(json!({}));
        }
    }

    let now = now_timestamp();
    let insert = format!(
        "INSERT INTO pinned_messages (channel_id, message_id, user_id, created_at) \
         VALUES ({}, {}, {}, '{now}')",
        req.channel, req.message_id, claims.user_id
    );
    if let Err(e) = state.api.query_router().query_sync(&insert) {
        tracing::error!("pin insert failed: {e}");
        return slack::err("internal_error");
    }
    state.persist_tables(&["pinned_messages"]);

    let event = crate::chat::events::ServerEvent::MessagePinned {
        channel: req.channel.to_string(),
        message_id: req.message_id.to_string(),
        user: claims.user_id.to_string(),
    };
    state.hub.broadcast_to_channel(req.channel, &event).await;

    slack::ok(json!({}))
}

#[derive(Deserialize)]
pub struct PinRemoveRequest {
    #[serde(deserialize_with = "deserialize_id")]
    pub channel: i64,
    #[serde(alias = "timestamp", deserialize_with = "deserialize_id")]
    pub message_id: i64,
}

pub async fn pins_remove(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<PinRemoveRequest>,
) -> Response {
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    let delete = format!(
        "DELETE FROM pinned_messages WHERE channel_id = {} AND message_id = {}",
        req.channel, req.message_id
    );
    if let Err(e) = state.api.query_router().query_sync(&delete) {
        tracing::error!("pin delete failed: {e}");
        return slack::err("internal_error");
    }
    state.persist_tables(&["pinned_messages"]);

    let event = crate::chat::events::ServerEvent::MessageUnpinned {
        channel: req.channel.to_string(),
        message_id: req.message_id.to_string(),
        user: claims.user_id.to_string(),
    };
    state.hub.broadcast_to_channel(req.channel, &event).await;

    slack::ok(json!({}))
}

#[derive(Deserialize)]
pub struct PinListRequest {
    #[serde(deserialize_with = "deserialize_id")]
    pub channel: i64,
}

pub async fn pins_list(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<PinListRequest>,
) -> Response {
    if !is_channel_member(&state, req.channel, claims.user_id) {
        return slack::err("channel_not_found");
    }

    let sql = format!(
        "SELECT message_id, user_id, created_at FROM pinned_messages WHERE channel_id = {} ORDER BY created_at DESC",
        req.channel
    );
    let pins = match state.api.query_router().query_sync(&sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("pins list failed: {e}");
            return slack::err("internal_error");
        }
    };

    let mut items = Vec::new();
    for row in &pins.rows {
        let message_id = match &row[0] {
            crate::connector::Value::Int(v) => *v,
            _ => continue,
        };
        let pinned_by = match &row[1] {
            crate::connector::Value::Int(v) => v.to_string(),
            _ => continue,
        };
        let pinned_at = match &row[2] {
            crate::connector::Value::String(v) => v.clone(),
            _ => continue,
        };

        // Fetch the message content (skip deleted messages)
        let msg_sql = format!(
            "SELECT id, user_id, content, created_at FROM messages WHERE id = {} AND deleted_at = ''",
            message_id
        );
        if let Ok(msg_r) = state.api.query_router().query_sync(&msg_sql) {
            if let Some(msg_row) = msg_r.rows.first() {
                let msg_user = match &msg_row[1] {
                    crate::connector::Value::Int(v) => v.to_string(),
                    _ => continue,
                };
                let msg_text = match &msg_row[2] {
                    crate::connector::Value::String(v) => v.clone(),
                    _ => String::new(),
                };
                let msg_ts = match &msg_row[3] {
                    crate::connector::Value::String(v) => v.clone(),
                    _ => continue,
                };

                items.push(json!({
                    "message": {
                        "ts": message_id.to_string(),
                        "user": msg_user,
                        "text": msg_text,
                        "created_at": msg_ts,
                    },
                    "pinned_by": pinned_by,
                    "pinned_at": pinned_at,
                }));
            }
        }
    }

    slack::ok(json!({ "items": items }))
}

/// Fetch reactions for a list of messages and attach them as a `reactions` array on each message JSON.
/// Each reaction is grouped by emoji: `{ "name": "thumbsup", "count": 2, "users": ["1","3"] }`.
fn enrich_reactions(state: &AppState, messages: &mut [serde_json::Value]) {
    for msg in messages.iter_mut() {
        let msg_id_str = msg["ts"].as_str().unwrap_or("0");
        let msg_id: i64 = msg_id_str.parse().unwrap_or(0);
        if msg_id == 0 {
            continue;
        }
        let sql = format!(
            "SELECT emoji, user_id FROM reactions WHERE message_id = {}",
            msg_id
        );
        if let Ok(result) = state.api.query_router().query_sync(&sql) {
            if result.rows.is_empty() {
                continue;
            }
            // Group by emoji
            let mut groups: std::collections::HashMap<String, Vec<String>> =
                std::collections::HashMap::new();
            for row in &result.rows {
                let emoji = match &row[0] {
                    crate::connector::Value::String(v) => v.clone(),
                    _ => continue,
                };
                let uid = match &row[1] {
                    crate::connector::Value::Int(v) => v.to_string(),
                    _ => continue,
                };
                groups.entry(emoji).or_default().push(uid);
            }
            let reactions: Vec<serde_json::Value> = groups
                .into_iter()
                .map(|(name, users)| {
                    json!({
                        "name": name,
                        "count": users.len(),
                        "users": users,
                    })
                })
                .collect();
            msg.as_object_mut()
                .unwrap()
                .insert("reactions".to_string(), serde_json::json!(reactions));
        }
    }
}

/// Fetch files for a list of messages and attach them as a `files` array on each message JSON.
fn enrich_files(state: &AppState, messages: &mut [serde_json::Value]) {
    for msg in messages.iter_mut() {
        let msg_id_str = msg["ts"].as_str().unwrap_or("0");
        let msg_id: i64 = msg_id_str.parse().unwrap_or(0);
        if msg_id == 0 {
            continue;
        }
        let sql = format!(
            "SELECT id, filename, mime_type, size_bytes FROM files WHERE message_id = {}",
            msg_id
        );
        if let Ok(result) = state.api.query_router().query_sync(&sql) {
            if result.rows.is_empty() {
                continue;
            }
            let files: Vec<serde_json::Value> = result
                .rows
                .iter()
                .map(|row| {
                    let id = match &row[0] {
                        crate::connector::Value::Int(v) => *v,
                        _ => 0,
                    };
                    let filename = match &row[1] {
                        crate::connector::Value::String(v) => v.clone(),
                        _ => String::new(),
                    };
                    let mime_type = match &row[2] {
                        crate::connector::Value::String(v) => v.clone(),
                        _ => String::new(),
                    };
                    let size_bytes = match &row[3] {
                        crate::connector::Value::Int(v) => *v,
                        _ => 0,
                    };
                    json!({
                        "id": id.to_string(),
                        "filename": filename,
                        "mime_type": mime_type,
                        "size_bytes": size_bytes,
                        "url": format!("/files/{}/{}", id, filename),
                    })
                })
                .collect();
            msg.as_object_mut()
                .unwrap()
                .insert("files".to_string(), serde_json::json!(files));
        }
    }
}

fn is_channel_member(state: &AppState, channel_id: i64, user_id: i64) -> bool {
    let sql = format!(
        "SELECT channel_id FROM channel_members WHERE channel_id = {} AND user_id = {}",
        channel_id, user_id
    );
    match state.api.query_router().query_sync(&sql) {
        Ok(r) => !r.rows.is_empty(),
        Err(_) => false,
    }
}

// ── Link Unfurling ──

static UNFURL_CACHE: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, (std::time::Instant, serde_json::Value)>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

#[derive(Deserialize)]
pub struct LinksUnfurlRequest {
    pub url: String,
}

pub async fn links_unfurl(
    State(_state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(req): Json<LinksUnfurlRequest>,
) -> Response {
    // Validate URL
    let url = match url::Url::parse(&req.url) {
        Ok(u) => u,
        Err(_) => return slack::err("invalid_url"),
    };

    if url.scheme() != "http" && url.scheme() != "https" {
        return slack::err("invalid_url");
    }

    // Block private/reserved IPs (SSRF protection)
    // Check both literal IPs in URL and resolved IPs to prevent DNS rebinding
    fn is_blocked_ipv4(ip: &std::net::Ipv4Addr) -> bool {
        let octets = ip.octets();
        octets[0] == 10
            || (octets[0] == 172 && (16..=31).contains(&octets[1]))
            || (octets[0] == 192 && octets[1] == 168)
            || octets[0] == 127
            || octets[0] == 0
            || (octets[0] == 169 && octets[1] == 254)
            || (octets[0] == 100 && (64..=127).contains(&octets[1])) // 100.64.0.0/10 shared/CGNAT
    }

    fn is_blocked_ipv6(ip: &std::net::Ipv6Addr) -> bool {
        ip.is_loopback()
            || ip.is_unspecified()
            || { let s = ip.segments(); s[0] & 0xfe00 == 0xfc00 } // unique local (fc00::/7)
            || { let s = ip.segments(); s[0] & 0xffc0 == 0xfe80 } // link-local (fe80::/10)
            || { let s = ip.segments(); s[0] == 0 && s[1] == 0 && s[2] == 0 && s[3] == 0 && s[4] == 0 && s[5] == 0xffff }
        // IPv4-mapped
    }

    let mut resolved: Vec<std::net::SocketAddr> = Vec::new();
    if let Some(host) = url.host_str() {
        let host_trimmed = host.trim_matches(|c| c == '[' || c == ']');
        if host_trimmed == "localhost" || host_trimmed == "::1" || host_trimmed == "0.0.0.0" {
            return slack::err("blocked_url");
        }
        if let Ok(ip) = host_trimmed.parse::<std::net::Ipv4Addr>() {
            if is_blocked_ipv4(&ip) {
                return slack::err("blocked_url");
            }
        }
        if let Ok(ip) = host_trimmed.parse::<std::net::Ipv6Addr>() {
            if is_blocked_ipv6(&ip) {
                return slack::err("blocked_url");
            }
        }
        // Resolve hostname and check resolved IPs to prevent DNS rebinding
        let port = url
            .port()
            .unwrap_or(if url.scheme() == "https" { 443 } else { 80 });
        let addr_str = format!("{}:{}", host_trimmed, port);
        resolved = match tokio::net::lookup_host(&addr_str).await {
            Ok(addrs) => addrs.collect(),
            Err(_) => return slack::err("fetch_failed"),
        };
        if resolved.is_empty() {
            return slack::err("fetch_failed");
        }
        for addr in &resolved {
            match addr.ip() {
                std::net::IpAddr::V4(ip) if is_blocked_ipv4(&ip) => {
                    return slack::err("blocked_url");
                }
                std::net::IpAddr::V6(ip) if is_blocked_ipv6(&ip) => {
                    return slack::err("blocked_url");
                }
                _ => {}
            }
        }
    }

    // Check cache
    {
        let cache = UNFURL_CACHE.lock().unwrap();
        if let Some((ts, cached)) = cache.get(&req.url) {
            if ts.elapsed() < std::time::Duration::from_secs(3600) {
                return slack::ok(cached.clone());
            }
        }
    }

    // Fetch with timeout; disable automatic redirects and follow manually so we can
    // resolve each redirect target's DNS and block redirects to internal/private IPs.
    // This prevents SSRF via DNS rebinding (e.g. redirect to a hostname that resolves
    // to 127.0.0.1 or 10.x.x.x).
    async fn resolve_and_check_url(
        target: &url::Url,
        is_blocked_v4: fn(&std::net::Ipv4Addr) -> bool,
        is_blocked_v6: fn(&std::net::Ipv6Addr) -> bool,
    ) -> Result<Vec<std::net::SocketAddr>, &'static str> {
        let host = target.host_str().ok_or("no_host")?;
        let h = host.trim_matches(|c| c == '[' || c == ']');
        if h == "localhost" || h == "::1" || h == "0.0.0.0" {
            return Err("blocked_url");
        }
        if let Ok(ip) = h.parse::<std::net::Ipv4Addr>() {
            if is_blocked_v4(&ip) {
                return Err("blocked_url");
            }
        }
        if let Ok(ip) = h.parse::<std::net::Ipv6Addr>() {
            if is_blocked_v6(&ip) {
                return Err("blocked_url");
            }
        }
        let port = target
            .port()
            .unwrap_or(if target.scheme() == "https" { 443 } else { 80 });
        let addr_str = format!("{}:{}", h, port);
        let addrs: Vec<std::net::SocketAddr> = match tokio::net::lookup_host(&addr_str).await {
            Ok(a) => a.collect(),
            Err(_) => return Err("fetch_failed"),
        };
        if addrs.is_empty() {
            return Err("fetch_failed");
        }
        for addr in &addrs {
            match addr.ip() {
                std::net::IpAddr::V4(ip) if is_blocked_v4(&ip) => return Err("blocked_url"),
                std::net::IpAddr::V6(ip) if is_blocked_v6(&ip) => return Err("blocked_url"),
                _ => {}
            }
        }
        Ok(addrs)
    }

    let mut current_url = url.clone();
    let mut current_resolved = resolved;
    let mut resp: Option<reqwest::Response> = None;
    for _redirect in 0..=3 {
        let mut builder = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none());
        if let Some(host) = current_url.host_str() {
            for addr in &current_resolved {
                builder = builder.resolve(host, *addr);
            }
        }
        let client = match builder.build() {
            Ok(c) => c,
            Err(_) => return slack::err("fetch_failed"),
        };
        let r = match client.get(current_url.as_str()).send().await {
            Ok(r) => r,
            Err(_) => return slack::err("fetch_failed"),
        };
        if r.status().is_redirection() {
            let location = match r.headers().get("location").and_then(|v| v.to_str().ok()) {
                Some(loc) => loc,
                None => return slack::err("fetch_failed"),
            };
            let next_url = match current_url.join(location) {
                Ok(u) => u,
                Err(_) => return slack::err("fetch_failed"),
            };
            if next_url.scheme() != "http" && next_url.scheme() != "https" {
                return slack::err("blocked_url");
            }
            match resolve_and_check_url(&next_url, is_blocked_ipv4, is_blocked_ipv6).await {
                Ok(addrs) => {
                    current_resolved = addrs;
                    current_url = next_url;
                }
                Err(_) => return slack::err("blocked_url"),
            }
            continue;
        }
        resp = Some(r);
        break;
    }
    let resp = match resp {
        Some(r) => r,
        None => return slack::err("fetch_failed"),
    };

    // Reject responses that declare a large content-length before downloading
    if let Some(len) = resp.content_length() {
        if len > 1_000_000 {
            return slack::err("fetch_failed");
        }
    }

    let body = match resp.text().await {
        Ok(b) if b.len() <= 1_000_000 => b,
        _ => return slack::err("fetch_failed"),
    };

    // Parse OG tags (simple regex-based, handles both attribute orderings)
    fn og_tag(html: &str, property: &str) -> Option<String> {
        // property before content
        let pattern1 = format!(
            r#"<meta[^>]*property=["']og:{}["'][^>]*content=["']([^"']*)["']"#,
            property
        );
        if let Some(caps) = regex::Regex::new(&pattern1).ok()?.captures(html) {
            return Some(caps[1].to_string());
        }
        // content before property
        let pattern2 = format!(
            r#"<meta[^>]*content=["']([^"']*)["'][^>]*property=["']og:{}["']"#,
            property
        );
        regex::Regex::new(&pattern2)
            .ok()?
            .captures(html)
            .map(|c| c[1].to_string())
    }

    let title = og_tag(&body, "title").or_else(|| {
        let re = regex::Regex::new(r"<title[^>]*>([^<]+)</title>").ok()?;
        re.captures(&body).map(|c| c[1].to_string())
    });

    let result = json!({
        "title": title,
        "description": og_tag(&body, "description"),
        "image": og_tag(&body, "image"),
        "site_name": og_tag(&body, "site_name"),
    });

    // Cache the result
    {
        let mut cache = UNFURL_CACHE.lock().unwrap();
        if cache.len() >= 1000 {
            cache.retain(|_, (ts, _)| ts.elapsed() < std::time::Duration::from_secs(3600));
            if cache.len() >= 1000 {
                let keys: Vec<_> = cache.keys().take(500).cloned().collect();
                for k in keys {
                    cache.remove(&k);
                }
            }
        }
        cache.insert(req.url, (std::time::Instant::now(), result.clone()));
    }

    slack::ok(result)
}

// ── Routes ──

use axum::{middleware, Router};

pub fn chat_routes(state: AppState) -> Router {
    let authed = Router::new()
        // Conversations
        .route(
            "/conversations.create",
            axum::routing::post(conversations_create),
        )
        .route(
            "/conversations.list",
            axum::routing::post(conversations_list),
        )
        .route(
            "/conversations.info",
            axum::routing::post(conversations_info),
        )
        .route(
            "/conversations.history",
            axum::routing::post(conversations_history),
        )
        .route(
            "/conversations.replies",
            axum::routing::post(conversations_replies),
        )
        .route(
            "/conversations.join",
            axum::routing::post(conversations_join),
        )
        .route(
            "/conversations.leave",
            axum::routing::post(conversations_leave),
        )
        .route(
            "/conversations.invite",
            axum::routing::post(conversations_invite),
        )
        .route(
            "/conversations.members",
            axum::routing::post(conversations_members),
        )
        .route(
            "/conversations.open",
            axum::routing::post(conversations_open),
        )
        .route(
            "/conversations.markRead",
            axum::routing::post(conversations_mark_read),
        )
        .route(
            "/conversations.update",
            axum::routing::post(conversations_update),
        )
        .route(
            "/conversations.archive",
            axum::routing::post(conversations_archive),
        )
        .route(
            "/conversations.unarchive",
            axum::routing::post(conversations_unarchive),
        )
        .route(
            "/conversations.setRole",
            axum::routing::post(conversations_set_role),
        )
        .route(
            "/conversations.mute",
            axum::routing::post(conversations_mute),
        )
        .route(
            "/conversations.unmute",
            axum::routing::post(conversations_unmute),
        )
        .route(
            "/conversations.setNotification",
            axum::routing::post(conversations_set_notification),
        )
        // Chat
        .route("/chat.postMessage", axum::routing::post(chat_post_message))
        .route("/chat.update", axum::routing::post(chat_update))
        .route("/chat.delete", axum::routing::post(chat_delete))
        // Users
        .route("/users.list", axum::routing::post(users_list))
        .route("/users.info", axum::routing::post(users_info))
        .route(
            "/users.setPresence",
            axum::routing::post(users_set_presence),
        )
        .route(
            "/users.updateProfile",
            axum::routing::post(users_update_profile),
        )
        .route(
            "/users.changePassword",
            axum::routing::post(users_change_password),
        )
        .route(
            "/users.getSettings",
            axum::routing::post(users_get_settings),
        )
        .route(
            "/users.updateSettings",
            axum::routing::post(users_update_settings),
        )
        // Search / Autocomplete
        .route("/users.search", axum::routing::post(users_search))
        .route(
            "/conversations.autocomplete",
            axum::routing::post(conversations_autocomplete),
        )
        // Reactions
        .route("/reactions.add", axum::routing::post(reactions_add))
        .route("/reactions.remove", axum::routing::post(reactions_remove))
        // Pins
        .route("/pins.add", axum::routing::post(pins_add))
        .route("/pins.remove", axum::routing::post(pins_remove))
        .route("/pins.list", axum::routing::post(pins_list))
        // Links
        .route("/links.unfurl", axum::routing::post(links_unfurl))
        // Search
        .route("/search.messages", axum::routing::post(search_messages))
        // Directory
        .route(
            "/conversations.directory",
            axum::routing::post(conversations_directory),
        )
        // Files
        .route(
            "/files.upload",
            axum::routing::post(crate::chat::files::files_upload),
        )
        .layer(middleware::from_fn(crate::chat::auth::jwt_middleware))
        .with_state(state.clone());

    let public = Router::new()
        .route("/auth.register", axum::routing::post(auth_register))
        .route("/auth.login", axum::routing::post(auth_login))
        .with_state(state);

    Router::new()
        .nest("/api/slack", authed)
        .nest("/api/slack", public)
}
