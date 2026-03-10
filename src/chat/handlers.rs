use crate::api::TeidelumApi;
use crate::chat::auth::{self, Claims};
use crate::chat::id::next_id;
use crate::chat::models::{escape_sql, now_timestamp};
use crate::chat::slack;
use axum::{extract::State, response::Response, Extension, Json};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

pub type AppState = Arc<ChatState>;

pub struct ChatState {
    pub api: Arc<TeidelumApi>,
    pub hub: Arc<crate::chat::hub::Hub>,
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
        "INSERT INTO users (id, username, display_name, email, password_hash, avatar_url, status, is_bot, created_at) \
         VALUES ({id}, '{username}', '{display}', '{email}', '{hash}', '', 'offline', false, '{now}')",
        username = escape_sql(&req.username),
        display = escape_sql(&display_name),
        email = escape_sql(&req.email),
        hash = escape_sql(&password_hash),
    );

    if let Err(e) = state.api.query_router().query_sync(&insert_sql) {
        tracing::error!("user insert failed: {e}");
        return slack::err("internal_error");
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
    let sql = "SELECT id, username, display_name, email, avatar_url, status, is_bot FROM users";
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
        "SELECT id, username, display_name, email, avatar_url, status, is_bot \
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
        }
    }))
}

#[derive(Deserialize)]
pub struct UserInfoRequest {
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

    // Broadcast presence change
    let event = crate::chat::events::ServerEvent::PresenceChange {
        user: claims.user_id.to_string(),
        presence: req.presence,
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

    // Check if channel name already exists
    let check = format!(
        "SELECT id FROM channels WHERE name = '{}'",
        escape_sql(&req.name)
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
        "INSERT INTO channels (id, name, kind, topic, created_by, created_at) \
         VALUES ({id}, '{name}', '{kind}', '{topic}', {created_by}, '{now}')",
        name = escape_sql(&req.name),
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

    // Update hub membership cache
    let mut members = std::collections::HashSet::new();
    members.insert(claims.user_id);
    state.hub.set_channel_members(id, members).await;

    slack::created(json!({
        "channel": {
            "id": id.to_string(),
            "name": req.name,
            "kind": req.kind,
        }
    }))
}

pub async fn conversations_list(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Response {
    // List channels the user is a member of
    let sql = format!(
        "SELECT c.id, c.name, c.kind, c.topic, c.created_at \
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

    let channels: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            json!({
                "id": row[0].to_json(),
                "name": row[1].to_json(),
                "kind": row[2].to_json(),
                "topic": row[3].to_json(),
                "created_at": row[4].to_json(),
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
        "SELECT id, name, kind, topic, created_by, created_at \
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
            "created_by": row[4].to_json(),
            "created_at": row[5].to_json(),
        }
    }))
}

#[derive(Deserialize)]
pub struct HistoryRequest {
    pub channel: i64,
    #[serde(default = "default_history_limit")]
    pub limit: usize,
    #[serde(default)]
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
         WHERE m.channel_id = {} AND m.thread_id = 0{} \
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

    let messages: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            let deleted = match &row[5] {
                crate::connector::Value::Null => false,
                crate::connector::Value::String(s) if s.is_empty() => false,
                _ => true,
            };
            json!({
                "ts": row[0].to_json(),
                "channel": row[1].to_json(),
                "user": row[2].to_json(),
                "thread_ts": row[3].to_json(),
                "text": if deleted { serde_json::Value::String("[deleted]".into()) } else { row[4].to_json() },
                "edited_ts": row[6].to_json(),
                "created_at": row[7].to_json(),
                "username": row[8].to_json(),
            })
        })
        .collect();

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
         WHERE m.channel_id = {} AND (m.id = {} OR m.thread_id = {}) \
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

    let messages: Vec<serde_json::Value> = result
        .rows
        .iter()
        .map(|row| {
            let deleted = match &row[5] {
                crate::connector::Value::Null => false,
                crate::connector::Value::String(s) if s.is_empty() => false,
                _ => true,
            };
            json!({
                "ts": row[0].to_json(),
                "channel": row[1].to_json(),
                "user": row[2].to_json(),
                "thread_ts": row[3].to_json(),
                "text": if deleted { serde_json::Value::String("[deleted]".into()) } else { row[4].to_json() },
                "edited_ts": row[6].to_json(),
                "created_at": row[7].to_json(),
                "username": row[8].to_json(),
            })
        })
        .collect();

    slack::ok(json!({"messages": messages}))
}

#[derive(Deserialize)]
pub struct RepliesRequest {
    pub channel: i64,
    pub ts: i64,
}

pub async fn conversations_join(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ChannelIdRequest>,
) -> Response {
    // Check channel exists and is public
    let sql = format!("SELECT kind FROM channels WHERE id = {}", req.channel);
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

    let sql = format!(
        "DELETE FROM channel_members WHERE channel_id = {} AND user_id = {}",
        req.channel, claims.user_id
    );

    if let Err(e) = state.api.query_router().query_sync(&sql) {
        tracing::error!("leave failed: {e}");
        return slack::err("internal_error");
    }

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
    pub channel: i64,
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

    // Look for existing DM between these two users
    let sql = format!(
        "SELECT c.id, c.name FROM channels c \
         JOIN channel_members cm1 ON c.id = cm1.channel_id \
         JOIN channel_members cm2 ON c.id = cm2.channel_id \
         WHERE c.kind = 'dm' AND cm1.user_id = {} AND cm2.user_id = {}",
        claims.user_id, other_user
    );

    match state.api.query_router().query_sync(&sql) {
        Ok(r) if !r.rows.is_empty() => {
            let row = &r.rows[0];
            return slack::ok(json!({
                "channel": {
                    "id": row[0].to_json(),
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
        "INSERT INTO channels (id, name, kind, topic, created_by, created_at) \
         VALUES ({id}, '{name}', 'dm', '', {created_by}, '{now}')",
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
            return slack::err("internal_error");
        }
    }

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
    pub users: Vec<i64>,
}

#[derive(Deserialize)]
pub struct ChannelIdRequest {
    pub channel: i64,
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

    let id = next_id();
    let now = now_timestamp();
    let thread_id = req.thread_ts.unwrap_or(0);

    let insert = format!(
        "INSERT INTO messages (id, channel_id, user_id, thread_id, content, deleted_at, edited_at, created_at) \
         VALUES ({id}, {channel}, {user}, {thread}, '{text}', NULL, NULL, '{now}')",
        channel = req.channel,
        user = claims.user_id,
        thread = thread_id,
        text = escape_sql(&req.text),
    );

    if let Err(e) = state.api.query_router().query_sync(&insert) {
        tracing::error!("post message failed: {e}");
        return slack::err("internal_error");
    }

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
    };
    state.hub.broadcast_to_channel(req.channel, &event).await;

    slack::ok(json!({
        "message": {
            "ts": id.to_string(),
            "channel": req.channel.to_string(),
            "user": claims.user_id.to_string(),
            "text": req.text,
        }
    }))
}

#[derive(Deserialize)]
pub struct PostMessageRequest {
    pub channel: i64,
    pub text: String,
    #[serde(default)]
    pub thread_ts: Option<i64>,
}

pub async fn chat_update(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ChatUpdateRequest>,
) -> Response {
    // Verify the message exists and belongs to the user
    let check = format!(
        "SELECT user_id, channel_id FROM messages WHERE id = {}",
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
    pub ts: i64,
    pub text: String,
}

pub async fn chat_delete(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(req): Json<ChatDeleteRequest>,
) -> Response {
    let check = format!(
        "SELECT user_id, channel_id FROM messages WHERE id = {}",
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

    let event = crate::chat::events::ServerEvent::MessageDeleted {
        channel: channel_id.to_string(),
        ts: req.ts.to_string(),
    };
    state.hub.broadcast_to_channel(channel_id, &event).await;

    slack::ok(json!({}))
}

#[derive(Deserialize)]
pub struct ChatDeleteRequest {
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
        "SELECT channel_id FROM messages WHERE id = {}",
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

    // Check that the reaction actually exists before deleting
    let check_reaction = format!(
        "SELECT message_id FROM reactions WHERE message_id = {} AND user_id = {} AND emoji = '{}'",
        req.timestamp,
        claims.user_id,
        escape_sql(&req.name)
    );
    match state.api.query_router().query_sync(&check_reaction) {
        Ok(r) if r.rows.is_empty() => return slack::err("no_reaction"),
        Err(e) => {
            tracing::error!("reaction existence check failed: {e}");
            return slack::err("internal_error");
        }
        _ => {}
    }

    let delete = format!(
        "DELETE FROM reactions WHERE message_id = {} AND user_id = {} AND emoji = '{}'",
        req.timestamp,
        claims.user_id,
        escape_sql(&req.name)
    );

    if let Err(e) = state.api.query_router().query_sync(&delete) {
        tracing::error!("reaction delete failed: {e}");
        return slack::err("internal_error");
    }

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
    pub timestamp: i64,
}

// ── Search ──

#[derive(Deserialize)]
pub struct SearchMessagesRequest {
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
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

    // Get the set of channel names the user is a member of for filtering
    let member_channels: std::collections::HashSet<String> = {
        let sql = format!(
            "SELECT c.name FROM channels c \
             JOIN channel_members cm ON c.id = cm.channel_id \
             WHERE cm.user_id = {}",
            claims.user_id
        );
        match state.api.query_router().query_sync(&sql) {
            Ok(r) => r
                .rows
                .iter()
                .filter_map(|row| match &row[0] {
                    crate::connector::Value::String(s) => Some(format!("#{s}")),
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

    // Filter results to only channels the user is a member of
    let matches: Vec<serde_json::Value> = results
        .iter()
        .filter(|r| member_channels.contains(&r.title))
        .take(limit)
        .map(|r| {
            json!({
                "ts": r.id,
                "channel": r.title,
                "text": r.snippet,
                "score": r.score,
            })
        })
        .collect();

    slack::ok(json!({
        "messages": {
            "matches": matches,
            "total": matches.len(),
        }
    }))
}

// ── Helpers ──

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
        // Reactions
        .route("/reactions.add", axum::routing::post(reactions_add))
        .route("/reactions.remove", axum::routing::post(reactions_remove))
        // Search
        .route("/search.messages", axum::routing::post(search_messages))
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
