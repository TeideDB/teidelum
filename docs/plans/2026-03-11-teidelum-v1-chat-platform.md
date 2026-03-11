# Teidelum v1 — Full Chat Platform Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform Teidelum from a working MVP into a complete, polished chat platform with user settings, channel management, message actions, rich input, media previews, notifications, search, and UI polish.

**Architecture:** 9 workstreams, each adding backend endpoints (Rust/Axum handlers in `src/chat/handlers.rs`), new tables (in `src/chat/models.rs`), WebSocket events (in `src/chat/events.rs`), frontend API calls (in `ui/src/lib/api.ts`), types (in `ui/src/lib/types.ts`), stores (in `ui/src/lib/stores/`), and Svelte components (in `ui/src/lib/components/`).

**Tech Stack:** Rust (Axum, TeideDB, tantivy, argon2, jsonwebtoken), SvelteKit (TypeScript, Tailwind CSS, Svelte 5 stores), emoji-mart, Shiki.

**Spec:** `docs/specs/2026-03-11-teidelum-v1-design.md`

**Test pattern:** Integration tests in `tests/` using `tower::ServiceExt::oneshot` on in-memory API. Run with `cargo test --test <test_file> -- --test-threads=1`.

**Frontend test pattern:** `cd ui && npx svelte-check` for type checking. Manual browser testing for UI components.

---

## Chunk 1: User Settings & Profile (Workstream 1)

### Task 1.1: Backend — user_settings table and DDL

**Files:**
- Modify: `src/chat/models.rs`

- [x] **Step 1: Add user_settings table to CREATE_TABLES**

In `src/chat/models.rs`, add to the `CREATE_TABLES` array:

```rust
"CREATE TABLE user_settings (
    user_id BIGINT, theme VARCHAR, notification_default VARCHAR,
    timezone VARCHAR, created_at VARCHAR
)",
```

- [x] **Step 2: Add FK relationship for user_settings**

In `chat_relationships()`, add:

```rust
Relationship {
    from_table: "user_settings".into(),
    from_col: "user_id".into(),
    to_table: "users".into(),
    to_col: "id".into(),
    relation: "settings_for".into(),
},
```

- [x] **Step 3: Update relationship count test**

Update `test_chat_relationships_valid` assertion from 13 to 14.

- [x] **Step 4: Run tests**

Run: `cargo test --lib chat::models -- --test-threads=1`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add src/chat/models.rs
git commit -m "feat: add user_settings table schema"
```

---

### Task 1.2: Backend — users.updateProfile endpoint

**Files:**
- Modify: `src/chat/handlers.rs`
- Modify: `src/chat/events.rs`

- [x] **Step 1: Add UserProfileUpdated event to ServerEvent**

In `src/chat/events.rs`, add variant to `ServerEvent`:

```rust
#[serde(rename = "user_profile_updated")]
UserProfileUpdated {
    user: String,
    display_name: String,
    avatar_url: String,
    status_text: String,
    status_emoji: String,
},
```

- [x] **Step 2: Add WsEventType to frontend types**

In `ui/src/lib/types.ts`, add `'user_profile_updated'` to the `WsEventType` union.

- [x] **Step 3: Write the handler**

In `src/chat/handlers.rs`, add:

```rust
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
    // Build dynamic UPDATE
    let mut sets = Vec::new();
    if let Some(ref name) = req.display_name {
        sets.push(format!("display_name = '{}'", escape_sql(name)));
    }
    if let Some(ref url) = req.avatar_url {
        sets.push(format!("avatar_url = '{}'", escape_sql(url)));
    }
    if let Some(ref email) = req.email {
        // Check email uniqueness
        let check = format!(
            "SELECT id FROM users WHERE email = '{}' AND id != {}",
            escape_sql(email), claims.user_id
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
```

**Note:** This handler references `status_text` and `status_emoji` columns on users. These columns are added in Workstream 6. For now, the UPDATE will silently skip if the columns don't exist, or you can add the columns to the users DDL now (see Task 6.1). The recommended approach: add the columns to users DDL in this task to avoid issues.

Update the users DDL in `CREATE_TABLES`:
```rust
"CREATE TABLE users (
    id BIGINT, username VARCHAR, display_name VARCHAR, email VARCHAR,
    password_hash VARCHAR, avatar_url VARCHAR, status VARCHAR,
    status_text VARCHAR, status_emoji VARCHAR,
    is_bot BOOLEAN, created_at VARCHAR
)",
```

**Important:** Also update `auth_register`'s INSERT statement to include the two new columns with empty defaults:
```rust
"INSERT INTO users (id, username, display_name, email, password_hash, avatar_url, status, status_text, status_emoji, is_bot, created_at) \
 VALUES ({id}, '{username}', '{display}', '{email}', '{hash}', '', 'offline', '', '', false, '{now}')",
```

Also update `users_list` SELECT query (line ~250) to include `status_text, status_emoji` (columns 7 and 8) and add them to the JSON response. Same for `users_info` SELECT query.

Update `PresenceChange` event in `events.rs` to include optional status fields:
```rust
#[serde(rename = "presence_change")]
PresenceChange {
    user: String,
    presence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status_emoji: Option<String>,
},
```

Update `users_set_presence` handler to fetch and include `status_text`/`status_emoji` in the broadcast.

- [x] **Step 4: Register the route**

In `chat_routes()`, add to the authed router:

```rust
.route("/users.updateProfile", axum::routing::post(users_update_profile))
```

- [x] **Step 5: Run `cargo check`**

Run: `cargo check`
Expected: PASS (compiles)

- [x] **Step 6: Commit**

```bash
git add src/chat/handlers.rs src/chat/events.rs src/chat/models.rs
git commit -m "feat: add users.updateProfile endpoint with WS broadcast"
```

---

### Task 1.3: Backend — users.changePassword endpoint

**Files:**
- Modify: `src/chat/handlers.rs`

- [x] **Step 1: Write the handler**

```rust
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

    slack::ok(json!({}))
}
```

- [x] **Step 2: Register the route**

```rust
.route("/users.changePassword", axum::routing::post(users_change_password))
```

- [x] **Step 3: Commit**

```bash
git add src/chat/handlers.rs
git commit -m "feat: add users.changePassword endpoint"
```

---

### Task 1.4: Backend — user settings endpoints

**Files:**
- Modify: `src/chat/handlers.rs`

- [x] **Step 1: Write getSettings handler**

```rust
#[derive(Deserialize)]
pub struct GetSettingsRequest {}

pub async fn users_get_settings(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(_req): Json<serde_json::Value>,
) -> Response {
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
            // Create default settings
            let now = now_timestamp();
            let insert = format!(
                "INSERT INTO user_settings (user_id, theme, notification_default, timezone, created_at) \
                 VALUES ({}, 'dark', 'all', 'UTC', '{now}')",
                claims.user_id
            );
            let _ = state.api.query_router().query_sync(&insert);
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
```

- [x] **Step 2: Write updateSettings handler**

```rust
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
    // Ensure settings row exists (upsert pattern)
    let check = format!(
        "SELECT user_id FROM user_settings WHERE user_id = {}",
        claims.user_id
    );
    let exists = state.api.query_router().query_sync(&check)
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

    slack::ok(json!({}))
}
```

- [x] **Step 3: Register routes**

```rust
.route("/users.getSettings", axum::routing::post(users_get_settings))
.route("/users.updateSettings", axum::routing::post(users_update_settings))
```

- [x] **Step 4: Commit**

```bash
git add src/chat/handlers.rs
git commit -m "feat: add users.getSettings and users.updateSettings endpoints"
```

---

### Task 1.5: Integration tests for user settings

**Files:**
- Modify: `tests/chat_integration.rs`

- [x] **Step 1: Write test for updateProfile**

```rust
#[tokio::test]
async fn test_update_profile() {
    let (app, _tmp) = setup().await;
    let token = register_and_login(&app, "profileuser", "pass123", "profile@test.com").await;

    // Update display_name
    let resp = app.clone().oneshot(post_json(
        "/api/slack/users.updateProfile",
        json!({"display_name": "New Name", "avatar_url": "https://example.com/avatar.png"}),
        Some(&token),
    )).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // Verify via users.info
    let resp = app.clone().oneshot(post_json(
        "/api/slack/users.info",
        json!({"user": get_user_id(&app, &token).await}),
        Some(&token),
    )).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["user"]["display_name"], "New Name");
    assert_eq!(body["user"]["avatar_url"], "https://example.com/avatar.png");
}
```

Note: `setup()` and `register_and_login()` are helpers — extract from existing test pattern (create tmp dir, init API, create chat state, return app). If these helpers don't exist, create them by refactoring the common setup pattern from existing tests.

- [x] **Step 2: Write test for changePassword**

```rust
#[tokio::test]
async fn test_change_password() {
    let (app, _tmp) = setup().await;
    let token = register_and_login(&app, "pwuser", "oldpass", "pw@test.com").await;

    // Change password
    let resp = app.clone().oneshot(post_json(
        "/api/slack/users.changePassword",
        json!({"old_password": "oldpass", "new_password": "newpass"}),
        Some(&token),
    )).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // Login with new password should work
    let resp = app.clone().oneshot(post_json(
        "/api/slack/auth.login",
        json!({"username": "pwuser", "password": "newpass"}),
        None,
    )).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // Login with old password should fail
    let resp = app.clone().oneshot(post_json(
        "/api/slack/auth.login",
        json!({"username": "pwuser", "password": "oldpass"}),
        None,
    )).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], false);
}
```

- [x] **Step 3: Write test for settings**

```rust
#[tokio::test]
async fn test_user_settings() {
    let (app, _tmp) = setup().await;
    let token = register_and_login(&app, "settingsuser", "pass", "settings@test.com").await;

    // Get default settings
    let resp = app.clone().oneshot(post_json(
        "/api/slack/users.getSettings",
        json!({}),
        Some(&token),
    )).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);
    assert_eq!(body["settings"]["theme"], "dark");

    // Update theme
    let resp = app.clone().oneshot(post_json(
        "/api/slack/users.updateSettings",
        json!({"theme": "light"}),
        Some(&token),
    )).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["ok"], true);

    // Verify
    let resp = app.clone().oneshot(post_json(
        "/api/slack/users.getSettings",
        json!({}),
        Some(&token),
    )).await.unwrap();
    let body = body_json(resp).await;
    assert_eq!(body["settings"]["theme"], "light");
}
```

- [x] **Step 4: Run tests**

Run: `cargo test --test chat_integration -- --test-threads=1`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add tests/chat_integration.rs
git commit -m "test: add integration tests for user profile and settings endpoints"
```

---

### Task 1.6: Frontend — API client and types for settings

**Files:**
- Modify: `ui/src/lib/api.ts`
- Modify: `ui/src/lib/types.ts`

- [x] **Step 1: Add types**

In `ui/src/lib/types.ts`, add:

```typescript
export interface UserSettings {
	theme: 'dark' | 'light';
	notification_default: 'all' | 'mentions' | 'none';
	timezone: string;
}

export interface UserSettingsResponse {
	ok: boolean;
	settings?: UserSettings;
	error?: string;
}
```

Add `status_text` and `status_emoji` to the `User` interface:

```typescript
export interface User {
	id: Id;
	username: string;
	display_name: string;
	email: string;
	avatar_url: string;
	status: string;
	status_text?: string;
	status_emoji?: string;
	is_bot: boolean;
	created_at: string;
}
```

- [x] **Step 2: Add API functions**

In `ui/src/lib/api.ts`, add:

```typescript
export function usersUpdateProfile(profile: {
	display_name?: string;
	avatar_url?: string;
	email?: string;
	status_text?: string;
	status_emoji?: string;
}): Promise<OkResponse> {
	return call('users.updateProfile', profile);
}

export function usersChangePassword(old_password: string, new_password: string): Promise<OkResponse> {
	return call('users.changePassword', { old_password, new_password });
}

export function usersGetSettings(): Promise<UserSettingsResponse> {
	return call('users.getSettings', {});
}

export function usersUpdateSettings(settings: {
	theme?: string;
	notification_default?: string;
	timezone?: string;
}): Promise<OkResponse> {
	return call('users.updateSettings', settings);
}
```

- [x] **Step 3: Run type check**

Run: `cd ui && npx svelte-check`
Expected: PASS

- [x] **Step 4: Commit**

```bash
git add ui/src/lib/api.ts ui/src/lib/types.ts
git commit -m "feat: add API client functions for user profile and settings"
```

---

### Task 1.7: Frontend — Settings page

**Files:**
- Create: `ui/src/routes/(app)/settings/+page.svelte`
- Modify: `ui/src/lib/stores/auth.ts`

- [x] **Step 1: Add settings store helper to auth**

In `ui/src/lib/stores/auth.ts`, add function to refresh current user:

```typescript
export async function refreshCurrentUser() {
	const state = get(auth);
	if (state.userId) {
		const res = await api.usersInfo(state.userId);
		if (res.ok && res.user) {
			auth.update((s) => ({ ...s, user: res.user! }));
		}
	}
}
```

- [x] **Step 2: Create settings page**

Create `ui/src/routes/(app)/settings/+page.svelte` with tab navigation (Profile, Account, Notifications, Appearance). Each tab is a section within the same page, switched by a local `activeTab` variable.

The component should:
- Load settings via `api.usersGetSettings()` on mount
- Profile tab: form with display_name, email inputs, avatar upload button (uses existing `api.filesUpload`), save calls `api.usersUpdateProfile()`
- Account tab: old password, new password, confirm password fields, save calls `api.usersChangePassword()`
- Notifications tab: select for notification_default (all/mentions/none), save calls `api.usersUpdateSettings()`
- Appearance tab: dark/light toggle, save calls `api.usersUpdateSettings({ theme })`
- After successful profile update, call `refreshCurrentUser()` to update the auth store
- Show success/error messages for each action

- [x] **Step 3: Add settings link to sidebar**

Modify `ui/src/lib/components/Sidebar.svelte`:
- Replace the simple logout button at the bottom with a user menu
- Clicking the user area shows a dropdown with "Settings", "Set status" (placeholder for WS6), "Sign out"
- "Settings" navigates to `/settings`

- [x] **Step 4: Add user_profile_updated WS listener**

In `ui/src/lib/stores/users.ts`, add listener for `user_profile_updated` events:

```typescript
unsubs.push(
    ws.on('user_profile_updated', (event: WsEvent) => {
        const data = event as unknown as {
            user: Id;
            display_name: string;
            avatar_url: string;
        };
        if (data.user) {
            users.update((map) => {
                const newMap = new Map(map);
                const existing = newMap.get(data.user);
                if (existing) {
                    newMap.set(data.user, { ...existing, display_name: data.display_name, avatar_url: data.avatar_url });
                }
                return newMap;
            });
        }
    })
);
```

Update `initUserWsListeners` to include this new listener (change from single unsub to array pattern matching `initMessageWsListeners`).

- [x] **Step 5: Run type check**

Run: `cd ui && npx svelte-check`
Expected: PASS

- [x] **Step 6: Commit**

```bash
git add ui/src/routes/\(app\)/settings/+page.svelte ui/src/lib/components/Sidebar.svelte ui/src/lib/stores/auth.ts ui/src/lib/stores/users.ts
git commit -m "feat: add settings page with profile, account, notification, and appearance tabs"
```

---

### Task 1.8: Frontend — Avatar component and display

**Files:**
- Create: `ui/src/lib/components/Avatar.svelte`
- Modify: `ui/src/lib/components/MessageList.svelte`
- Modify: `ui/src/lib/components/Sidebar.svelte`

- [x] **Step 1: Create Avatar component**

Create `ui/src/lib/components/Avatar.svelte`:

```svelte
<script lang="ts">
    export let url: string = '';
    export let name: string = '';
    export let size: 'sm' | 'md' | 'lg' = 'md';

    const sizeClasses = { sm: 'w-6 h-6 text-xs', md: 'w-8 h-8 text-sm', lg: 'w-12 h-12 text-lg' };

    function initials(name: string): string {
        return name.split(/\s+/).map(w => w[0]).join('').toUpperCase().slice(0, 2) || '?';
    }

    function colorFromName(name: string): string {
        let hash = 0;
        for (let i = 0; i < name.length; i++) hash = name.charCodeAt(i) + ((hash << 5) - hash);
        const hue = Math.abs(hash % 360);
        return `hsl(${hue}, 60%, 45%)`;
    }
</script>

{#if url}
    <img src={url} alt={name} class="rounded-full object-cover {sizeClasses[size]}" />
{:else}
    <div
        class="rounded-full flex items-center justify-center font-semibold text-white {sizeClasses[size]}"
        style="background-color: {colorFromName(name)}"
    >
        {initials(name)}
    </div>
{/if}
```

- [x] **Step 2: Use Avatar in MessageList**

Replace the existing avatar placeholder in `MessageList.svelte` with `<Avatar url={user?.avatar_url} name={user?.display_name || msg.user || ''} size="md" />`.

- [x] **Step 3: Use Avatar in Sidebar**

Show the current user's avatar in the sidebar user area.

- [x] **Step 4: Run type check**

Run: `cd ui && npx svelte-check`
Expected: PASS

- [x] **Step 5: Commit**

```bash
git add ui/src/lib/components/Avatar.svelte ui/src/lib/components/MessageList.svelte ui/src/lib/components/Sidebar.svelte
git commit -m "feat: add Avatar component with colored initials fallback"
```

---

## Chunk 2: Channel Management (Workstream 2)

### Task 2.1: Backend — Update channels schema

**Files:**
- Modify: `src/chat/models.rs`

- [x] **Step 1: Add description and archived_at columns to channels DDL**

```rust
"CREATE TABLE channels (
    id BIGINT, name VARCHAR, kind VARCHAR, topic VARCHAR,
    description VARCHAR, archived_at VARCHAR,
    created_by BIGINT, created_at VARCHAR
)",
```

- [x] **Step 2: Run `cargo check`**

Expected: PASS

- [x] **Step 3: Commit**

```bash
git add src/chat/models.rs
git commit -m "feat: add description and archived_at columns to channels schema"
```

---

### Task 2.2: Backend — conversations.update endpoint

**Files:**
- Modify: `src/chat/handlers.rs`
- Modify: `src/chat/events.rs`

- [x] **Step 1: Add ChannelUpdated event**

In `src/chat/events.rs`:

```rust
#[serde(rename = "channel_updated")]
ChannelUpdated {
    channel: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    topic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    archived_at: Option<String>,
},
```

- [x] **Step 2: Write conversations_update handler**

In `src/chat/handlers.rs`:

```rust
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
        Ok(r) if !r.rows.is_empty() => {
            r.rows[0][0].to_json().as_str().unwrap_or("member").to_string()
        }
        _ => return slack::err("not_in_channel"),
    };
    if role != "owner" && role != "admin" {
        return slack::err("not_authorized");
    }

    let mut sets = Vec::new();
    if let Some(ref name) = req.name {
        // Check name uniqueness
        let check = format!(
            "SELECT id FROM channels WHERE name = '{}' AND id != {}",
            escape_sql(name), req.channel
        );
        match state.api.query_router().query_sync(&check) {
            Ok(r) if !r.rows.is_empty() => return slack::err("name_taken"),
            _ => {}
        }
        sets.push(format!("name = '{}'", escape_sql(name)));
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
        sets.join(", "), req.channel
    );
    if let Err(e) = state.api.query_router().query_sync(&sql) {
        tracing::error!("channel update failed: {e}");
        return slack::err("internal_error");
    }

    // Broadcast
    let event = crate::chat::events::ServerEvent::ChannelUpdated {
        channel: req.channel.to_string(),
        name: req.name,
        topic: req.topic,
        description: req.description,
        archived_at: None,
    };
    state.hub.broadcast_to_channel(req.channel, &event).await;

    slack::ok(json!({}))
}
```

- [x] **Step 3: Register route**

```rust
.route("/conversations.update", axum::routing::post(conversations_update))
```

- [x] **Step 4: Commit**

```bash
git add src/chat/handlers.rs src/chat/events.rs
git commit -m "feat: add conversations.update endpoint with owner/admin check"
```

---

### Task 2.3: Backend — archive, unarchive, setRole endpoints

**Files:**
- Modify: `src/chat/handlers.rs`

- [x] **Step 1: Write conversations_archive handler**

```rust
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

    let event = crate::chat::events::ServerEvent::ChannelUpdated {
        channel: req.channel.to_string(),
        name: None, topic: None, description: None,
        archived_at: Some(now),
    };
    state.hub.broadcast_to_channel(req.channel, &event).await;

    slack::ok(json!({}))
}
```

- [x] **Step 2: Write conversations_unarchive handler**

Same structure as archive but sets `archived_at = ''` (empty string = active). Same owner-only role check. Broadcasts `ChannelUpdated` with `archived_at: Some(String::new())`. **Convention:** all archive checks use `!archived.is_empty()` — empty string means active.

- [x] **Step 3: Write conversations_set_role handler**

```rust
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
        escape_sql(&req.role), req.channel, req.user
    );
    if let Err(e) = state.api.query_router().query_sync(&sql) {
        tracing::error!("set role failed: {e}");
        return slack::err("internal_error");
    }

    slack::ok(json!({}))
}
```

- [x] **Step 4: Register routes**

```rust
.route("/conversations.archive", axum::routing::post(conversations_archive))
.route("/conversations.unarchive", axum::routing::post(conversations_unarchive))
.route("/conversations.setRole", axum::routing::post(conversations_set_role))
```

- [x] **Step 5: Block posting to archived channels**

In `chat_post_message`, add a check at the start:

```rust
// Check if channel is archived
let arch_sql = format!("SELECT archived_at FROM channels WHERE id = {}", req.channel);
if let Ok(r) = state.api.query_router().query_sync(&arch_sql) {
    if let Some(row) = r.rows.first() {
        let archived = row[0].to_json().as_str().unwrap_or("").to_string();
        if !archived.is_empty() {
            return slack::err("channel_archived");
        }
    }
}
```

- [x] **Step 6: Run `cargo check`**

- [x] **Step 7: Commit**

```bash
git add src/chat/handlers.rs
git commit -m "feat: add archive, unarchive, setRole endpoints and block posting to archived channels"
```

---

### Task 2.4: Integration tests for channel management

**Files:**
- Modify: `tests/chat_integration.rs`

- [x] **Step 1: Write test for conversations.update**

Test: create channel, update name/topic as owner, verify via conversations.info. Try update as non-owner, verify failure.

- [x] **Step 2: Write test for archive/unarchive**

Test: archive channel as owner, try posting (should fail), unarchive, post again (should work).

- [x] **Step 3: Write test for setRole**

Test: owner sets member to admin, admin can update channel topic, member cannot.

- [x] **Step 4: Run tests**

Run: `cargo test --test chat_integration -- --test-threads=1`

- [x] **Step 5: Commit**

```bash
git add tests/chat_integration.rs
git commit -m "test: add integration tests for channel management endpoints"
```

---

### Task 2.5: Frontend — Channel Info panel and API

**Files:**
- Modify: `ui/src/lib/api.ts`
- Modify: `ui/src/lib/types.ts`
- Create: `ui/src/lib/components/ChannelInfoPanel.svelte`
- Modify: `ui/src/routes/(app)/[channel]/+page.svelte`
- Modify: `ui/src/lib/stores/channels.ts`

- [x] **Step 1: Add API functions and types**

Add to `types.ts`: `description` and `archived_at` to `Channel` interface. Add `'channel_updated'` to `WsEventType`.

Add to `api.ts`:

```typescript
export function conversationsUpdate(channel: Id, updates: {
    name?: string; topic?: string; description?: string;
}): Promise<OkResponse> {
    return call('conversations.update', { channel, ...updates });
}

export function conversationsArchive(channel: Id): Promise<OkResponse> {
    return call('conversations.archive', { channel });
}

export function conversationsUnarchive(channel: Id): Promise<OkResponse> {
    return call('conversations.unarchive', { channel });
}

export function conversationsSetRole(channel: Id, user: Id, role: string): Promise<OkResponse> {
    return call('conversations.setRole', { channel, user, role });
}
```

- [x] **Step 2: Add channel_updated WS listener in channels store**

```typescript
unsubs.push(
    ws.on('channel_updated', (event: WsEvent) => {
        const data = event as unknown as {
            channel: Id; name?: string; topic?: string;
            description?: string; archived_at?: string;
        };
        if (data.channel) {
            channels.update((list) =>
                list.map((ch) =>
                    ch.id === data.channel
                        ? { ...ch, ...(data.name && { name: data.name }), ...(data.topic !== undefined && { topic: data.topic }), ...(data.description !== undefined && { description: data.description }), ...(data.archived_at !== undefined && { archived_at: data.archived_at }) }
                        : ch
                )
            );
        }
    })
);
```

- [x] **Step 3: Create ChannelInfoPanel component**

Create `ui/src/lib/components/ChannelInfoPanel.svelte`:
- Shows channel name, kind badge, topic, description, created by, created date
- Loads members via `api.conversationsMembers()` and displays with roles
- "Edit" button (owner/admin) opens inline edit form for name, topic, description
- "Archive" button (owner only) with confirm dialog
- "Add people" button opens invite modal
- Close button (X)

- [x] **Step 4: Wire into channel page**

In `ui/src/routes/(app)/[channel]/+page.svelte`:
- Add state: `let showChannelInfo = false`
- Make channel name/topic in header clickable to toggle `showChannelInfo`
- When `showChannelInfo` is true, show ChannelInfoPanel instead of ThreadPanel
- When ThreadPanel opens, close ChannelInfoPanel and vice versa

- [x] **Step 5: Show archived state in sidebar**

In `Sidebar.svelte`: dim archived channels, show archive icon.
In channel page: if channel is archived, show read-only banner and disable MessageInput.

- [x] **Step 6: Run type check and commit**

```bash
cd ui && npx svelte-check
git add ui/src/lib/api.ts ui/src/lib/types.ts ui/src/lib/components/ChannelInfoPanel.svelte ui/src/routes/\(app\)/\[channel\]/+page.svelte ui/src/lib/stores/channels.ts ui/src/lib/components/Sidebar.svelte
git commit -m "feat: add ChannelInfoPanel with edit, archive, and member management"
```

---

## Chunk 3: Message Actions (Workstream 3)

### Task 3.1: Backend — pinned_messages table and endpoints

**Files:**
- Modify: `src/chat/models.rs`
- Modify: `src/chat/handlers.rs`
- Modify: `src/chat/events.rs`

- [x] **Step 1: Add pinned_messages table**

In `models.rs` `CREATE_TABLES`:

```rust
"CREATE TABLE pinned_messages (
    channel_id BIGINT, message_id BIGINT, user_id BIGINT, created_at VARCHAR
)",
```

Add FK relationships:

```rust
Relationship { from_table: "pinned_messages".into(), from_col: "message_id".into(), to_table: "messages".into(), to_col: "id".into(), relation: "pinned".into() },
Relationship { from_table: "pinned_messages".into(), from_col: "channel_id".into(), to_table: "channels".into(), to_col: "id".into(), relation: "pinned_in".into() },
Relationship { from_table: "pinned_messages".into(), from_col: "user_id".into(), to_table: "users".into(), to_col: "id".into(), relation: "pinned_by".into() },
```

Update relationship count in test.

- [x] **Step 2: Add WebSocket events**

```rust
#[serde(rename = "message_pinned")]
MessagePinned { channel: String, message_id: String, user: String },

#[serde(rename = "message_unpinned")]
MessageUnpinned { channel: String, message_id: String, user: String },
```

- [x] **Step 3: Write pins.add, pins.remove, pins.list handlers**

Follow the existing handler patterns. `pins.add`: check membership, SELECT-before-INSERT for idempotency, broadcast `MessagePinned`. `pins.remove`: check membership, DELETE, broadcast `MessageUnpinned`. `pins.list`: SELECT with JOIN-like pattern (separate queries for pin metadata + message content).

- [x] **Step 4: Register routes**

```rust
.route("/pins.add", axum::routing::post(pins_add))
.route("/pins.remove", axum::routing::post(pins_remove))
.route("/pins.list", axum::routing::post(pins_list))
```

- [x] **Step 5: Run `cargo check`, commit**

---

### Task 3.2: Integration tests for pins

**Files:**
- Modify: `tests/chat_integration.rs`

- [x] **Step 1: Write test**

Pin a message, list pins, verify. Pin same message again (idempotent). Unpin, verify removed.

- [x] **Step 2: Run tests, commit**

---

### Task 3.3: Frontend — Message context menu

**Files:**
- Create: `ui/src/lib/components/MessageContextMenu.svelte`
- Modify: `ui/src/lib/components/MessageList.svelte`
- Modify: `ui/src/lib/api.ts`
- Modify: `ui/src/lib/types.ts`

- [ ] **Step 1: Add pin API functions**

```typescript
export function pinsAdd(channel: Id, message_id: Id): Promise<OkResponse> {
    return call('pins.add', { channel, message_id });
}
export function pinsRemove(channel: Id, message_id: Id): Promise<OkResponse> {
    return call('pins.remove', { channel, message_id });
}
export function pinsList(channel: Id): Promise<{ ok: boolean; pins?: Message[]; error?: string }> {
    return call('pins.list', { channel });
}
```

- [ ] **Step 2: Create MessageContextMenu component**

Shows on hover (extended actions bar) with buttons: Reply, React (opens emoji picker), Edit (own messages only), Delete (own messages only), Pin/Unpin, Copy text.

- [ ] **Step 3: Add inline edit mode to MessageList**

When editing: replace message text with textarea, show Save/Cancel buttons. Save calls `editMessage()` from messages store. Cancel reverts.

- [ ] **Step 4: Add delete confirmation dialog**

Simple modal: "Delete this message? This can't be undone." with Delete/Cancel buttons.

- [ ] **Step 5: Add pinned messages indicator in channel header**

Pin icon + count. Clicking opens a dropdown showing pinned messages with Unpin action.

- [ ] **Step 6: Add WS event types and listeners for pins**

Add `'message_pinned' | 'message_unpinned'` to `WsEventType`.

- [ ] **Step 7: Type check, commit**

---

## Chunk 4: Rich Input & Autocomplete (Workstream 4)

### Task 4.1: Backend — users.search and conversations.autocomplete

**Files:**
- Modify: `src/chat/handlers.rs`

- [ ] **Step 1: Write users_search handler**

```rust
#[derive(Deserialize)]
pub struct UsersSearchRequest {
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

fn default_search_limit() -> usize { 10 }

pub async fn users_search(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(req): Json<UsersSearchRequest>,
) -> Response {
    let query_lower = req.query.to_lowercase();
    let sql = "SELECT id, username, display_name, avatar_url FROM users";
    let result = match state.api.query_router().query_sync(sql) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("users search failed: {e}");
            return slack::err("internal_error");
        }
    };

    // Filter in-app (substring, case-insensitive)
    let mut matches: Vec<serde_json::Value> = result.rows.iter().filter_map(|row| {
        let username = row[1].to_json().as_str().unwrap_or("").to_string();
        let display_name = row[2].to_json().as_str().unwrap_or("").to_string();
        if username.to_lowercase().contains(&query_lower) || display_name.to_lowercase().contains(&query_lower) {
            Some(json!({
                "id": row[0].to_json(),
                "username": username,
                "display_name": display_name,
                "avatar_url": row[3].to_json(),
            }))
        } else {
            None
        }
    }).collect();

    matches.truncate(req.limit);
    slack::ok(json!({"users": matches}))
}
```

- [ ] **Step 2: Write conversations_autocomplete handler**

Same pattern but searches channels by name prefix.

- [ ] **Step 3: Register routes**

```rust
.route("/users.search", axum::routing::post(users_search))
.route("/conversations.autocomplete", axum::routing::post(conversations_autocomplete))
```

- [ ] **Step 4: Run `cargo check`, commit**

---

### Task 4.2: Frontend — Autocomplete component

**Files:**
- Create: `ui/src/lib/components/Autocomplete.svelte`
- Modify: `ui/src/lib/components/MessageInput.svelte`
- Modify: `ui/src/lib/api.ts`

- [ ] **Step 1: Add API functions**

```typescript
export function usersSearch(query: string): Promise<{ ok: boolean; users?: Array<{ id: Id; username: string; display_name: string; avatar_url: string }>; }> {
    return call('users.search', { query });
}

export function conversationsAutocomplete(query: string): Promise<{ ok: boolean; channels?: Array<{ id: Id; name: string; topic: string }>; }> {
    return call('conversations.autocomplete', { query });
}
```

- [ ] **Step 2: Create Autocomplete component**

Generic autocomplete dropdown. Props: `trigger` (string like '@' or '#'), `items` (array), `onSelect` callback. Handles keyboard navigation (arrow keys, Enter, Escape).

- [ ] **Step 3: Integrate into MessageInput**

Watch for `@` and `#` triggers in textarea value. When detected, show Autocomplete dropdown above cursor position. On select, insert the completed text. Debounce API calls (200ms).

- [ ] **Step 4: Also integrate into ThreadPanel reply input**

Same autocomplete in thread replies.

- [ ] **Step 5: Type check, commit**

---

### Task 4.3: Frontend — Emoji picker (emoji-mart)

**Files:**
- Modify: `ui/package.json` (add emoji-mart dependency)
- Create: `ui/src/lib/components/EmojiPicker.svelte`
- Modify: `ui/src/lib/components/ReactionPicker.svelte`
- Modify: `ui/src/lib/components/MessageInput.svelte`

- [ ] **Step 1: Install emoji-mart**

```bash
cd ui && npm install emoji-mart @emoji-mart/data
```

- [ ] **Step 2: Create EmojiPicker wrapper component**

Wraps the emoji-mart picker with Svelte. Props: `onSelect(emoji: string)`. Positioned as a popover.

- [ ] **Step 3: Replace ReactionPicker with EmojiPicker**

MessageContextMenu's "React" button opens EmojiPicker instead of the hardcoded 10-emoji grid.

- [ ] **Step 4: Add emoji button to MessageInput**

Small smiley icon button that opens EmojiPicker, inserts selected emoji at cursor.

- [ ] **Step 5: Type check, commit**

---

### Task 4.4: Frontend — Typing indicators display

**Files:**
- Create: `ui/src/lib/components/TypingIndicator.svelte`
- Modify: `ui/src/routes/(app)/[channel]/+page.svelte`

- [ ] **Step 1: Create TypingIndicator component**

```svelte
<script lang="ts">
    import { onDestroy } from 'svelte';
    import * as ws from '$lib/ws';
    import { getUser } from '$lib/stores/users';
    import { auth } from '$lib/stores/auth';
    import { get } from 'svelte/store';
    import type { WsEvent, Id } from '$lib/types';

    export let channelId: Id;

    let typingUsers = new Map<Id, number>(); // userId -> timeout handle
    let displayNames: string[] = [];

    const unsub = ws.on('typing', (event: WsEvent) => {
        const data = event as unknown as { channel: Id; user: Id };
        if (data.channel !== channelId) return;
        if (data.user === get(auth).userId) return;

        // Clear existing timeout
        if (typingUsers.has(data.user)) {
            clearTimeout(typingUsers.get(data.user)!);
        }
        // Set new timeout (4 seconds)
        const handle = setTimeout(() => {
            typingUsers.delete(data.user);
            typingUsers = typingUsers; // trigger reactivity
            updateDisplay();
        }, 4000);
        typingUsers.set(data.user, handle as unknown as number);
        typingUsers = typingUsers;
        updateDisplay();
    });

    function updateDisplay() {
        displayNames = Array.from(typingUsers.keys()).map((uid) => {
            const user = getUser(uid);
            return user?.display_name || user?.username || 'Someone';
        });
    }

    onDestroy(() => {
        unsub();
        for (const handle of typingUsers.values()) clearTimeout(handle);
    });
</script>

{#if displayNames.length > 0}
    <div class="text-xs text-gray-400 px-4 h-5">
        {#if displayNames.length === 1}
            {displayNames[0]} is typing...
        {:else if displayNames.length === 2}
            {displayNames[0]} and {displayNames[1]} are typing...
        {:else}
            Several people are typing...
        {/if}
    </div>
{:else}
    <div class="h-5"></div>
{/if}
```

- [ ] **Step 2: Add to channel page**

Place `<TypingIndicator channelId={$activeChannelId} />` between MessageList and MessageInput.

- [ ] **Step 3: Type check, commit**

---

### Task 4.5: Integration tests for search endpoints

**Files:**
- Modify: `tests/chat_integration.rs`

- [ ] **Step 1: Write test for users.search**

Register 3 users (alice, bob, alice_b). Search for "alice" — should return 2 matches. Search for "bob" — should return 1. Search for "zzz" — should return 0.

- [ ] **Step 2: Write test for conversations.autocomplete**

Create channels (general, general-dev, random). Autocomplete "gen" — should return 2. Autocomplete "ran" — should return 1.

- [ ] **Step 3: Run tests, commit**

```bash
cargo test --test chat_integration -- --test-threads=1
git commit -m "test: add integration tests for users.search and conversations.autocomplete"
```

---

## Chunk 5: Media & Content (Workstream 5)

### Task 5.1: Backend — links.unfurl endpoint

**Files:**
- Modify: `src/chat/handlers.rs`

- [ ] **Step 1: Add reqwest dependency**

In `Cargo.toml`, add `reqwest = { version = "0.12", features = ["json"] }` if not already present.

- [ ] **Step 2: Write links_unfurl handler**

```rust
pub async fn links_unfurl(
    State(_state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(req): Json<LinksUnfurlRequest>,
) -> Response {
    // SSRF protection: validate URL
    let url = match url::Url::parse(&req.url) {
        Ok(u) => u,
        Err(_) => return slack::err("invalid_url"),
    };

    if url.scheme() != "http" && url.scheme() != "https" {
        return slack::err("invalid_url");
    }

    // Block private/reserved IPs (SSRF protection)
    if let Some(host) = url.host_str() {
        if host == "localhost" || host == "::1" {
            return slack::err("blocked_url");
        }
        if let Ok(ip) = host.parse::<std::net::Ipv4Addr>() {
            let octets = ip.octets();
            let blocked = octets[0] == 10                              // 10.0.0.0/8
                || (octets[0] == 172 && (16..=31).contains(&octets[1])) // 172.16.0.0/12
                || (octets[0] == 192 && octets[1] == 168)              // 192.168.0.0/16
                || octets[0] == 127                                     // 127.0.0.0/8
                || (octets[0] == 169 && octets[1] == 254);             // 169.254.0.0/16
            if blocked {
                return slack::err("blocked_url");
            }
        }
    }

    // LRU cache (module-level static)
    // Add at the top of the handler module:
    // use std::sync::Mutex;
    // use std::collections::HashMap;
    // static UNFURL_CACHE: std::sync::LazyLock<Mutex<HashMap<String, (std::time::Instant, serde_json::Value)>>> =
    //     std::sync::LazyLock::new(|| Mutex::new(HashMap::new()));
    //
    // Check cache first:
    {
        let cache = UNFURL_CACHE.lock().unwrap();
        if let Some((ts, cached)) = cache.get(&req.url) {
            if ts.elapsed() < std::time::Duration::from_secs(3600) {
                return slack::ok(cached.clone());
            }
        }
    }

    // Fetch with timeout
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .unwrap_or_default();

    let resp = match client.get(req.url.clone()).send().await {
        Ok(r) => r,
        Err(_) => return slack::err("fetch_failed"),
    };

    let body = match resp.text().await {
        Ok(b) if b.len() <= 1_000_000 => b,
        _ => return slack::err("fetch_failed"),
    };

    // Parse OG tags (simple regex-based)
    fn og_tag(html: &str, property: &str) -> Option<String> {
        let pattern = format!(r#"<meta[^>]*property=["']og:{}["'][^>]*content=["']([^"']*)["']"#, property);
        regex::Regex::new(&pattern).ok()?.captures(html).map(|c| c[1].to_string())
    }

    let title = og_tag(&body, "title").or_else(|| {
        // Fallback: parse <title> tag
        let re = regex::Regex::new(r"<title[^>]*>([^<]+)</title>").ok()?;
        re.captures(&body).map(|c| c[1].to_string())
    });

    let result = json!({
        "title": title,
        "description": og_tag(&body, "description"),
        "image": og_tag(&body, "image"),
        "site_name": og_tag(&body, "site_name"),
    });

    // Cache the result (evict entries > 1000 by clearing oldest)
    {
        let mut cache = UNFURL_CACHE.lock().unwrap();
        if cache.len() >= 1000 {
            // Simple eviction: remove expired entries, or clear half if still full
            cache.retain(|_, (ts, _)| ts.elapsed() < std::time::Duration::from_secs(3600));
            if cache.len() >= 1000 {
                let keys: Vec<_> = cache.keys().take(500).cloned().collect();
                for k in keys { cache.remove(&k); }
            }
        }
        cache.insert(req.url, (std::time::Instant::now(), result.clone()));
    }

    slack::ok(result)
}

#[derive(Deserialize)]
pub struct LinksUnfurlRequest {
    pub url: String,
}
```

- [ ] **Step 3: Register route**

```rust
.route("/links.unfurl", axum::routing::post(links_unfurl))
```

- [ ] **Step 4: Add regex dependency if not present**

In `Cargo.toml`: `regex = "1"`

- [ ] **Step 5: Run `cargo check`, commit**

---

### Task 5.2: Frontend — Inline image preview

**Files:**
- Modify: `ui/src/lib/components/MessageList.svelte`
- Create: `ui/src/lib/components/ImageLightbox.svelte`

- [ ] **Step 1: Create ImageLightbox component**

Fullscreen overlay, click or Escape to close. Shows image at native resolution.

- [ ] **Step 2: Modify file attachment rendering in MessageList**

For files where `mime_type` starts with `image/`: render `<img>` with `max-w-[400px] max-h-[300px] object-contain cursor-pointer` classes. Click opens ImageLightbox. Non-image files keep existing download link behavior.

- [ ] **Step 3: Type check, commit**

---

### Task 5.3: Frontend — Code syntax highlighting

**Files:**
- Modify: `ui/package.json`
- Modify: `ui/src/lib/markdown.ts`

- [ ] **Step 1: Install Shiki**

```bash
cd ui && npm install shiki
```

- [ ] **Step 2: Integrate Shiki into markdown renderer**

In `ui/src/lib/markdown.ts`, configure marked with a custom renderer for code blocks that uses Shiki's `codeToHtml()`. Load Shiki asynchronously on first use. Add a "Copy" button wrapper around code blocks.

- [ ] **Step 3: Type check, commit**

---

### Task 5.4: Frontend — Link previews

**Files:**
- Create: `ui/src/lib/components/LinkPreview.svelte`
- Modify: `ui/src/lib/components/MessageList.svelte`
- Modify: `ui/src/lib/api.ts`

- [ ] **Step 1: Add API function**

```typescript
export function linksUnfurl(url: string): Promise<{
    ok: boolean;
    title?: string;
    description?: string;
    image?: string;
    site_name?: string;
}> {
    return call('links.unfurl', { url });
}
```

- [ ] **Step 2: Create LinkPreview component**

Takes a URL prop. Calls `api.linksUnfurl()` on mount. Renders a card with title, description, thumbnail. Caches results in a module-level Map.

- [ ] **Step 3: Integrate into MessageList**

After rendering message text, detect URLs with regex. For each URL (max 3), render a `<LinkPreview>` component below the message.

- [ ] **Step 4: Type check, commit**

---

### Task 5.5: Integration test for links.unfurl

**Files:**
- Modify: `tests/chat_integration.rs`

- [ ] **Step 1: Write test for links.unfurl**

Test with a blocked URL (localhost) — should return error. Test with an invalid URL — should return error. (Can't easily test a real URL in CI, so test the safety checks.)

- [ ] **Step 2: Run tests, commit**

---

## Chunk 6: User Profiles & Presence (Workstream 6)

### Task 6.1: Backend — status_text and status_emoji (if not done in Task 1.2)

If these columns were already added to the users DDL in Task 1.2, this task is just verification. Otherwise:

- [ ] **Step 1: Update users DDL to include status_text and status_emoji**
- [ ] **Step 2: Update users_list and users_info to return these fields**
- [ ] **Step 3: Update presence_change event to include status_text and status_emoji**
- [ ] **Step 4: Commit**

---

### Task 6.2: Frontend — User profile popover

**Files:**
- Create: `ui/src/lib/components/UserProfilePopover.svelte`
- Modify: `ui/src/lib/components/MessageList.svelte`

- [ ] **Step 1: Create UserProfilePopover component**

Props: `userId`, `anchorElement` (for positioning).
Shows: large Avatar, display name, username, custom status (emoji + text), presence indicator, "Member since" date.
Action: "Message" button opens DM via `openDm()`.
Dismisses on click outside or Escape.
Positioned relative to anchor element.

- [ ] **Step 2: Wire into MessageList**

Clicking on a username or avatar in the message list opens UserProfilePopover for that user.

- [ ] **Step 3: Wire into ChannelInfoPanel member list**

Clicking a member name opens the popover.

- [ ] **Step 4: Type check, commit**

---

### Task 6.3: Frontend — Custom user status

**Files:**
- Modify: `ui/src/lib/components/Sidebar.svelte`

- [ ] **Step 1: Add status setter to sidebar user menu**

When clicking "Set status" in the user menu:
- Show a small modal with emoji selector (from EmojiPicker) + text input
- Predefined quick options as buttons
- "Clear status" button
- Save calls `api.usersUpdateProfile({ status_text, status_emoji })`

- [ ] **Step 2: Display status in messages and popover**

Show status emoji next to display name in MessageList and UserProfilePopover.

- [ ] **Step 3: Type check, commit**

---

### Task 6.4: Frontend — Idle auto-detection

**Files:**
- Modify: `ui/src/routes/(app)/+layout.svelte`

- [ ] **Step 1: Add idle detection**

In the app layout, track mouse/keyboard activity:

```typescript
let idleTimer: ReturnType<typeof setTimeout>;
let isIdle = false;

function resetIdle() {
    if (isIdle) {
        isIdle = false;
        api.usersSetPresence('online');
    }
    clearTimeout(idleTimer);
    idleTimer = setTimeout(() => {
        isIdle = true;
        api.usersSetPresence('away');
    }, 5 * 60 * 1000); // 5 minutes
}

onMount(() => {
    window.addEventListener('mousemove', resetIdle);
    window.addEventListener('keydown', resetIdle);
    resetIdle();
    return () => {
        window.removeEventListener('mousemove', resetIdle);
        window.removeEventListener('keydown', resetIdle);
        clearTimeout(idleTimer);
    };
});
```

- [ ] **Step 2: Commit**

---

## Chunk 7: Notification Preferences (Workstream 7)

### Task 7.1: Backend — channel_settings table and endpoints

**Files:**
- Modify: `src/chat/models.rs`
- Modify: `src/chat/handlers.rs`

- [ ] **Step 1: Add channel_settings table**

```rust
"CREATE TABLE channel_settings (
    channel_id BIGINT, user_id BIGINT, muted VARCHAR, notification_level VARCHAR, created_at VARCHAR
)",
```

Add FK relationships.

- [ ] **Step 2: Write conversations_mute/unmute handlers**

Use SELECT-before-INSERT upsert pattern for `(channel_id, user_id)`.

- [ ] **Step 3: Write conversations_set_notification handler**

- [ ] **Step 4: Update conversations_list to include muted/notification_level**

For each channel, query channel_settings to get muted/notification_level (default to "false"/"all" if no row).

- [ ] **Step 5: Register routes, run `cargo check`, commit**

---

### Task 7.2: Frontend — Mute, notifications, DND

**Files:**
- Modify: `ui/src/lib/api.ts`
- Modify: `ui/src/lib/stores/channels.ts`
- Modify: `ui/src/lib/components/Sidebar.svelte`
- Create: `ui/src/lib/notifications.ts`

- [ ] **Step 1: Add API functions**

```typescript
export function conversationsMute(channel: Id): Promise<OkResponse> { return call('conversations.mute', { channel }); }
export function conversationsUnmute(channel: Id): Promise<OkResponse> { return call('conversations.unmute', { channel }); }
export function conversationsSetNotification(channel: Id, level: string): Promise<OkResponse> {
    return call('conversations.setNotification', { channel, level });
}
```

- [ ] **Step 2: Add muted/notification_level to Channel type**

- [ ] **Step 3: Add context menu to sidebar channels**

Right-click on channel → "Mute" / "Unmute". Muted channels: dimmed + mute icon, no unread badge.

- [ ] **Step 4: Create notifications module**

`ui/src/lib/notifications.ts`:
- `requestPermission()` — ask browser for notification permission
- `showNotification(title, body, channelId)` — show desktop notification when tab is not focused, respect mute/DND
- Call `requestPermission()` after first login

- [ ] **Step 5: Wire notifications to WS message events**

In unreads store or a new listener: when a message arrives and tab is hidden, call `showNotification()`.

- [ ] **Step 6: Type check, commit**

---

### Task 7.3: Integration tests for notification preferences

**Files:**
- Modify: `tests/chat_integration.rs`

- [ ] **Step 1: Write test for mute/unmute**

Create channel, mute it, verify `conversations.list` returns `muted: "true"`. Unmute, verify `muted: "false"`.

- [ ] **Step 2: Write test for setNotification**

Set notification level to "mentions", verify returned in `conversations.list`.

- [ ] **Step 3: Run tests, commit**

---

## Chunk 8: Search & Discovery (Workstream 8)

### Task 8.1: Backend — Search filters and channel directory

**Files:**
- Modify: `src/chat/handlers.rs`

- [ ] **Step 1: Extend search.messages with filters**

Add optional fields to `SearchMessagesRequest`: `user_id`, `channel_id`, `date_from`, `date_to`. Apply as post-query filters after tantivy search results are retrieved.

- [ ] **Step 2: Write conversations_directory handler**

Returns all public channels with member counts. Supports `query`, `limit`, `cursor` params.

```rust
pub async fn conversations_directory(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Json(req): Json<DirectoryRequest>,
) -> Response {
    let mut sql = "SELECT id, name, kind, topic, description, created_at FROM channels WHERE kind = 'public'".to_string();

    if !req.archived {
        sql.push_str(" AND (archived_at = '' OR archived_at IS NULL)");
    }

    // ... apply query filter, cursor, limit, compute member_count per channel
}
```

- [ ] **Step 3: Register route, run `cargo check`, commit**

---

### Task 8.2: Frontend — Search filters and channel directory

**Files:**
- Modify: `ui/src/lib/components/SearchModal.svelte`
- Create: `ui/src/lib/components/ChannelDirectory.svelte`
- Modify: `ui/src/lib/components/Sidebar.svelte`
- Modify: `ui/src/lib/api.ts`

- [ ] **Step 1: Add filter UI to SearchModal**

Add filter bar with: user dropdown (uses users.search), channel dropdown (uses conversations.autocomplete), date range picker. Pass filters to `api.searchMessages()`.

- [ ] **Step 2: Add API for directory**

```typescript
export function conversationsDirectory(query?: string, limit?: number, cursor?: Id): Promise<{
    ok: boolean;
    channels?: Channel[];
}> {
    return call('conversations.directory', { query, limit, cursor });
}
```

- [ ] **Step 3: Create ChannelDirectory component**

Modal showing all public channels. Search/filter. Each entry: name, topic, member count, Join button. Called from "Browse channels" button in Sidebar.

- [ ] **Step 4: Add Cmd+F for in-channel search**

Intercept Cmd+F, open SearchModal with channel filter pre-set to current channel.

- [ ] **Step 5: Type check, commit**

---

### Task 8.3: Integration tests for search filters and directory

**Files:**
- Modify: `tests/chat_integration.rs`

- [ ] **Step 1: Write test for search.messages with filters**

Post messages from two users. Search with `user_id` filter — should return only that user's messages.

- [ ] **Step 2: Write test for conversations.directory**

Create public and private channels. Directory should return only public channels. Test `query` filter.

- [ ] **Step 3: Run tests, commit**

---

## Chunk 9: UI Polish (Workstream 9)

### Task 9.1: Dark/light theme

**Files:**
- Modify: `ui/src/app.css`
- Create: `ui/src/lib/stores/theme.ts`
- Modify: `ui/src/routes/+layout.svelte`

- [ ] **Step 1: Create theme store**

```typescript
import { writable } from 'svelte/store';

const stored = typeof localStorage !== 'undefined' ? localStorage.getItem('teide_theme') : null;
const systemPreference = typeof window !== 'undefined'
    ? window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
    : 'dark';

export const theme = writable<'dark' | 'light'>(
    (stored as 'dark' | 'light') || systemPreference
);

theme.subscribe((value) => {
    if (typeof localStorage !== 'undefined') {
        localStorage.setItem('teide_theme', value);
    }
    if (typeof document !== 'undefined') {
        document.documentElement.classList.toggle('dark', value === 'dark');
        document.documentElement.classList.toggle('light', value === 'light');
    }
});
```

- [ ] **Step 2: Extract colors to CSS custom properties**

In `app.css`, define `--bg-primary`, `--bg-secondary`, `--text-primary`, `--text-secondary`, `--accent`, etc. under `.dark` and `.light` classes. Replace hardcoded colors throughout components.

- [ ] **Step 3: Add theme toggle to sidebar and settings**

- [ ] **Step 4: Commit**

---

### Task 9.2: Loading skeletons and empty states

**Files:**
- Create: `ui/src/lib/components/Skeleton.svelte`
- Modify: `ui/src/lib/components/MessageList.svelte`
- Modify: `ui/src/lib/components/Sidebar.svelte`

- [ ] **Step 1: Create Skeleton component**

Shimmer animation CSS + shaped placeholders (message block, channel bar).

- [ ] **Step 2: Show skeletons during loading**

In MessageList: show 4 message skeletons while `loading` is true and messages are empty.
In Sidebar: show channel list skeletons while channels haven't loaded.

- [ ] **Step 3: Add empty states**

Empty channel: friendly "No messages yet" message.
No search results: "No messages found."
No DMs: "No direct messages yet."

- [ ] **Step 4: Commit**

---

### Task 9.3: Connection status indicator

**Files:**
- Create: `ui/src/lib/components/ConnectionStatus.svelte`
- Modify: `ui/src/lib/ws.ts`
- Modify: `ui/src/routes/(app)/+layout.svelte`

- [ ] **Step 1: Export connection state from ws.ts**

```typescript
export const connectionState = writable<'connected' | 'reconnecting' | 'disconnected'>('disconnected');
```

Update in `doConnect.onopen`, `onclose`, etc.

- [ ] **Step 2: Create ConnectionStatus component**

Fixed bar at top. Yellow for reconnecting, green flash for connected, red for disconnected.

- [ ] **Step 3: Add to app layout, commit**

---

### Task 9.4: Keyboard shortcuts

**Files:**
- Modify: `ui/src/routes/(app)/+layout.svelte`
- Create: `ui/src/lib/components/ShortcutsModal.svelte`

- [ ] **Step 1: Add keyboard listeners**

- `Cmd+Shift+A`: navigate to next channel with unreads
- `Up arrow` (empty input): trigger edit of last own message
- `Cmd+/`: show shortcuts help modal

- [ ] **Step 2: Create ShortcutsModal**

Simple modal listing all keyboard shortcuts.

- [ ] **Step 3: Commit**

---

### Task 9.5: Mobile responsive

**Files:**
- Modify: `ui/src/routes/(app)/+layout.svelte`
- Modify: `ui/src/lib/components/Sidebar.svelte`

- [ ] **Step 1: Add responsive layout**

- Sidebar: hidden by default on `<768px`, hamburger button to toggle
- Thread/ChannelInfo panel: full-screen overlay on mobile
- Touch-friendly tap targets (min 44px)

- [ ] **Step 2: Commit**

---

### Task 9.6: Drag-and-drop file upload

**Files:**
- Modify: `ui/src/routes/(app)/[channel]/+page.svelte`

- [ ] **Step 1: Add drop zone**

Listen for `dragover`/`dragleave`/`drop` events on the message area. Show overlay "Drop files to upload" on drag. On drop, call `filesUpload()` for each file.

- [ ] **Step 2: Commit**

---

## Final: Integration Verification

### Task F.1: Full test run

- [ ] **Step 1: Run all backend tests**

```bash
cargo test -- --test-threads=1
```

- [ ] **Step 2: Run clippy**

```bash
cargo clippy -- -D warnings
```

- [ ] **Step 3: Run frontend type check**

```bash
cd ui && npx svelte-check
```

- [ ] **Step 4: Build production**

```bash
cd ui && npm run build
cargo build --release
```

- [ ] **Step 5: Manual smoke test**

Start server, create accounts, test: settings, channel management, message edit/delete/pin, autocomplete, emoji picker, image preview, code highlighting, link previews, typing indicators, notifications, search filters, channel directory, theme toggle, mobile layout, drag-drop upload.

---

## Summary

| Chunk | Workstream | Tasks | Key Deliverables |
|-------|-----------|-------|-----------------|
| 1 | User Settings & Profile | 1.1-1.8 | user_settings table, 4 endpoints, settings page, Avatar component |
| 2 | Channel Management | 2.1-2.5 | channels schema update, 4 endpoints, ChannelInfoPanel |
| 3 | Message Actions | 3.1-3.3 | pinned_messages table, 3 endpoints, context menu, inline edit |
| 4 | Rich Input & Autocomplete | 4.1-4.4 | 2 search endpoints, Autocomplete, EmojiPicker, TypingIndicator |
| 5 | Media & Content | 5.1-5.4 | links.unfurl endpoint, image preview, Shiki, LinkPreview |
| 6 | User Profiles & Presence | 6.1-6.4 | status columns, UserProfilePopover, idle detection |
| 7 | Notification Preferences | 7.1-7.2 | channel_settings table, mute/DND, browser notifications |
| 8 | Search & Discovery | 8.1-8.2 | search filters, channel directory |
| 9 | UI Polish | 9.1-9.6 | theme toggle, skeletons, connection status, shortcuts, responsive, drag-drop |
