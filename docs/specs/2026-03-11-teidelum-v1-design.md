# Teidelum v1 — Full Chat Platform Design Spec

**Goal:** Transform Teidelum from a working MVP into a complete, polished chat platform. No new connectors or Pro tier features — focus entirely on making the chat experience feel finished.

**Baseline:** All core chat functionality exists (auth, channels, messages, threads, reactions, mentions, unreads, search, file upload, WebSocket real-time, MCP tools, SvelteKit frontend).

---

## Cross-Cutting Concerns

### Schema Migrations

TeideDB has no `ALTER TABLE ADD COLUMN`. For workstreams that add columns to existing tables (WS2: channels, WS6: users), the approach is:

1. **Drop and recreate** the table with the new schema in `models.rs` DDL.
2. Tables are created at startup via `CREATE TABLE IF NOT EXISTS`. On a fresh start with no data, the new columns are present automatically.
3. For existing deployments with data: provide a one-time migration script that exports data, drops the table, recreates with new schema, and re-imports. This is acceptable for v1 since Teidelum is pre-release and local-first (no shared production databases to worry about).
4. Long-term (post-v1): implement a proper migration system with version tracking.

### Uniqueness Enforcement

TeideDB has no UNIQUE constraints. All new tables requiring uniqueness must use SELECT-before-INSERT at the application layer, consistent with existing patterns:

- **`user_settings`**: unique on `user_id` — SELECT by user_id before INSERT, UPDATE if exists.
- **`pinned_messages`**: unique on `(channel_id, message_id)` — SELECT by both before INSERT. `pins.add` is idempotent (returns ok if already pinned).
- **`channel_settings`**: unique on `(channel_id, user_id)` — SELECT by both before INSERT, UPDATE if exists.

All SELECT-before-INSERT sequences are subject to TOCTOU races under concurrent requests. This is acceptable for v1 given single-server deployment and low collision probability. The worst case is a duplicate row, which is benign (queries use LIMIT 1 or aggregate).

### Admin Role

The `channel_members.role` column supports `owner`, `admin`, and `member` values:

- **Owner**: the channel creator (set automatically on `conversations.create`). Can edit, archive, and manage all members.
- **Admin**: granted by the owner via a new `conversations.setRole` endpoint. Can edit channel name/topic/description. Cannot archive or change other admins.
- **Member**: default role on join/invite. Can read, post, react, pin.

New endpoint: `conversations.setRole` — owner can set any member's role to `admin` or `member`. Cannot change own role. Cannot demote other owners (there is only one owner per channel).

---

## Workstream 1: User Settings & Profile

### Problem
Users cannot edit their profile, change their password, or configure any preferences. No settings page exists.

### Backend

**New table: `user_settings`**

| Column | Type | Notes |
|--------|------|-------|
| user_id | i64 | FK -> users.id, one row per user |
| theme | string | "dark" / "light", default "dark" |
| notification_default | string | "all" / "mentions" / "none", default "all" |
| timezone | string | IANA timezone, default "UTC" |
| created_at | string | |

**New endpoints:**
- `users.updateProfile` — update display_name, avatar_url, email. Validates email uniqueness. Broadcasts `user_profile_updated` WebSocket event to all online users.
- `users.changePassword` — accepts old_password + new_password. Verifies old password with argon2, hashes new password, updates user row.
- `users.getSettings` — returns user_settings row (creates default if missing).
- `users.updateSettings` — partial update of theme, notification_default, timezone.

**New WebSocket event:**
- `user_profile_updated` — `{ user: user_id, display_name, avatar_url, status_text, status_emoji }` — sent to all online users so names/avatars update live.

**Avatar upload:** Reuse existing file upload infrastructure. `users.updateProfile` accepts an avatar_url pointing to `/files/:id/:filename`. Frontend uploads via `files.upload`, then sends the resulting URL to `updateProfile`.

### Frontend

**New route: `/settings`** with tab navigation:
- **Profile tab:** Edit display name, avatar (upload + preview), email. Save button.
- **Account tab:** Change password (old password, new password, confirm). Save button.
- **Notifications tab:** Default notification level (all/mentions/none). DND toggle (on/off, no schedule for v1).
- **Appearance tab:** Dark/light theme toggle with live preview.

**Sidebar changes:**
- User area at bottom: clicking username/avatar opens a menu with "Settings", "Set status", "Sign out".

**Avatar display:**
- Show user avatars in: message list (author headers), sidebar user area, thread panel, search results. Fall back to colored initials when no avatar set.

---

## Workstream 2: Channel Management

### Problem
Channels cannot be edited or archived after creation. No way to view channel details or member list in the UI.

### Backend

**Schema changes to `channels` table:**
- Add `description` column (string, optional longer text beyond topic).
- Add `archived_at` column (string, nullable — null means active).

**New endpoints:**
- `conversations.update` — update channel name, topic, description. Restricted to channel owner or admin role. Validates name uniqueness. Broadcasts `channel_updated` WebSocket event.
- `conversations.archive` — sets `archived_at` timestamp. Restricted to channel owner. Archived channels: visible in sidebar (dimmed), read-only (postMessage returns error), members can still read history. Broadcasts `channel_updated` event.

**New WebSocket event:**
- `channel_updated` — `{ channel: channel_id, name?, topic?, description?, archived_at? }` — sent to all channel members.

- `conversations.unarchive` — clears `archived_at` timestamp. Restricted to channel owner. Re-enables posting. Broadcasts `channel_updated` event.
- `conversations.setRole` — owner sets a member's role to `admin` or `member`. See Cross-Cutting Concerns for role definitions.

**No channel deletion.** Archive is safer and reversible.

### Frontend

**Channel Info panel** (right side, shares space with thread panel — only one visible at a time, like Slack):
- Opening Channel Info closes any open thread panel, and vice versa.
- Triggered by clicking channel name/topic in header.
- Shows: channel name, kind badge (public/private/dm), topic, description, created by, created date, member count.
- Member list with roles (owner/admin/member).
- "Add people" button (opens invite modal).
- "Edit channel" button (owner/admin only) — opens edit modal.
- "Archive channel" button (owner only) with confirmation dialog.

**Edit channel modal:**
- Fields: name, topic, description.
- Save/cancel buttons.

**Sidebar changes:**
- Archived channels shown dimmed with archive icon.
- Archived channel view shows read-only banner at top, message input disabled.

---

## Workstream 3: Message Actions

### Problem
No way to edit, delete, or pin messages from the UI. Backend edit/delete endpoints exist but aren't wired up. No message context menu.

### Backend

**New table: `pinned_messages`**

| Column | Type | Notes |
|--------|------|-------|
| channel_id | i64 | FK -> channels.id |
| message_id | i64 | FK -> messages.id |
| user_id | i64 | FK -> users.id (who pinned it) |
| created_at | string | |

**New endpoints:**
- `pins.add` — pin a message in its channel. One pin per message (idempotent). Any channel member can pin.
- `pins.remove` — unpin a message. Any channel member can unpin.
- `pins.list` — return all pinned messages for a channel, ordered by pin date desc.

**New WebSocket events:**
- `message_pinned` — `{ channel, message_id, user }`.
- `message_unpinned` — `{ channel, message_id, user }`.

### Frontend

**Message context menu** (hover actions bar, expanding on click or right-click):
- **Reply** — opens thread panel (exists).
- **React** — opens emoji picker (currently just sends +1).
- **Edit** — inline edit mode (only shown for own messages).
- **Delete** — confirmation dialog, then soft-delete (only shown for own messages).
- **Pin/Unpin** — toggles pin status.
- **Copy text** — copies message content to clipboard.

**Inline message editing:**
- Clicking "Edit" replaces message content with textarea pre-filled with current text.
- Save (Enter) / Cancel (Escape) buttons below textarea.
- Edited messages show "(edited)" indicator next to timestamp.

**Message deletion:**
- Confirmation dialog: "Delete this message? This can't be undone."
- Deleted messages removed from view (already handled by `message_deleted` WS event).

**Pinned messages:**
- Pin icon + count in channel header.
- Clicking opens a pinned messages dropdown/panel showing all pinned messages.
- Each pinned message shows content preview, author, pin date, "Unpin" action.

---

## Workstream 4: Rich Input & Autocomplete

### Problem
No autocomplete for @mentions or #channels. Emoji reactions limited to 10 hardcoded options. Typing indicators are sent but never displayed.

### Backend

**New endpoints:**
- `users.search` — search users by username or display_name substring match (case-insensitive). Returns top 10 matches. Used for @mention autocomplete. Substring matching ensures "smith" finds "John Smith".
- `conversations.autocomplete` — search channels by name prefix. Returns top 10 matches. Used for #channel autocomplete.

### Frontend

**@mention autocomplete:**
- Triggered when user types `@` followed by characters.
- Dropdown appears above/below cursor with filtered user list (avatar, display_name, username).
- Arrow keys to navigate, Enter/Tab to select, Escape to dismiss.
- Selection inserts `@username` into message input.
- Works in both MessageInput and ThreadPanel reply input.

**#channel autocomplete:**
- Same interaction as @mention but triggered by `#`.
- Shows channel name and topic preview.
- Selection inserts `#channel-name`.

**Full emoji picker:**
- Replace 10 hardcoded emoji with categorized Unicode emoji picker.
- Categories: Smileys, People, Animals, Food, Travel, Activities, Objects, Symbols, Flags.
- Search/filter within picker.
- "Frequently used" section based on local storage history.
- Accessible from: reaction picker on messages + emoji button in message input.
- Use `emoji-mart` (or similar established library) for the picker component — building a full categorized emoji picker with search from raw Unicode data is not worth the effort for v1.

**Typing indicators display:**
- Below message list, above input: "Alice is typing..." or "Alice and Bob are typing..." or "Several people are typing..."
- Fade out after 4 seconds of no typing event.
- Already receiving typing WS events — just need to render them.

---

## Workstream 5: Media & Content

### Problem
File attachments show as download links only. No inline image preview. No code syntax highlighting in messages. No link previews.

### Backend

**New endpoint:**
- `links.unfurl` — accepts a URL, fetches Open Graph metadata (title, description, image, site_name) server-side. Returns JSON. 5-second timeout. Cache results in memory (LRU, 1000 entries, 1 hour TTL). Called by frontend after message render when URLs detected. **SSRF protection:** block private/reserved IP ranges (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 127.0.0.0/8, 169.254.0.0/16, ::1), restrict to HTTP/HTTPS protocols only, limit redirects to 3, reject responses > 1MB.

### Frontend

**Inline image preview:**
- Detect image MIME types (image/jpeg, image/png, image/gif, image/webp) in file attachments.
- Render `<img>` inline in message (max-width 400px, max-height 300px, maintain aspect ratio).
- Click image opens fullscreen lightbox overlay with close button and Escape to dismiss.
- Non-image files continue showing as download links with file icon.

**Code syntax highlighting:**
- Integrate Shiki (lightweight, WASM-based) into markdown renderer.
- Detect language from fenced code block syntax (```js, ```python, etc.).
- Apply syntax highlighting with theme matching dark/light mode.
- Copy button on code blocks.

**Link previews:**
- After message renders, scan for URLs in content.
- Call `links.unfurl` for each URL (max 3 per message, debounced).
- Render preview card below message: site name, title, description, thumbnail image.
- Cache unfurl results in frontend to avoid re-fetching.

---

## Workstream 6: User Profiles & Presence

### Problem
No way to view user details. No custom status. No idle auto-detection.

### Backend

**Schema changes to `users` table:**
- Add `status_text` column (string, e.g., "In a meeting").
- Add `status_emoji` column (string, e.g., "📅").

**Endpoint changes:**
- `users.updateProfile` also accepts `status_text` and `status_emoji`.
- `users.info` and `users.list` return `status_text` and `status_emoji`.
- `presence_change` WebSocket event includes `status_text` and `status_emoji`.

### Frontend

**User profile popover:**
- Triggered by clicking on username or avatar anywhere (messages, member list, sidebar).
- Shows: avatar (large), display name, username, custom status (emoji + text), presence indicator, member since date.
- Action buttons: "Message" (opens DM via `conversations.open`).
- Dismisses on click outside or Escape.

**Custom user status:**
- Quick-set from sidebar user menu: emoji picker + text input.
- "Clear status" button.
- Status shown: next to display name in messages, in profile popover, in member list.
- Predefined quick options: "In a meeting 📅", "Commuting 🚌", "Out sick 🤒", "Vacationing 🌴", "Working remotely 🏠".

**Idle/away auto-detection:**
- Frontend tracks mouse/keyboard activity.
- After 5 minutes of inactivity: send `users.setPresence({ presence: "away" })`.
- On activity resume: send `users.setPresence({ presence: "online" })`.
- Respect manual DND — don't override if user set DND explicitly.

---

## Workstream 7: Notification Preferences

### Problem
No way to mute channels or control notification behavior. No browser notifications. No DND mode.

### Backend

**New table: `channel_settings`**

| Column | Type | Notes |
|--------|------|-------|
| channel_id | i64 | FK -> channels.id |
| user_id | i64 | FK -> users.id |
| muted | string | "true" / "false", default "false" |
| notification_level | string | "all" / "mentions" / "none", default "all" |
| created_at | string | |

**New endpoints:**
- `conversations.setNotification` — set notification_level for a channel.
- `conversations.mute` / `conversations.unmute` — toggle muted flag.

**Response changes:**
- `conversations.list` includes `muted` and `notification_level` per channel from channel_settings table (default values if no row exists).

### Frontend

**Per-channel mute:**
- Right-click channel in sidebar → "Mute channel" / "Unmute channel".
- Also available in Channel Info panel.
- Muted channels: dimmed text in sidebar, no unread badge (unless @mentioned).
- Muted icon next to channel name.

**Browser notifications:**
- On first login: request browser notification permission.
- Show desktop notification for new messages when tab is not focused.
- Respect mute settings and notification level.
- Notification shows: sender avatar, sender name, message preview (truncated), channel name.
- Click notification focuses tab and navigates to channel.

**DND mode:**
- Toggle from sidebar user menu or settings.
- When active: suppress all desktop notifications, show moon icon next to user name.
- Broadcasts DND status via presence system.

**Notification bell:**
- Icon in sidebar header area showing count of unread @mentions across all channels.
- Click opens dropdown listing channels with @mentions, click to navigate.

---

## Workstream 8: Search & Discovery

### Problem
Search is text-only with no filters. No way to discover public channels you haven't joined.

### Backend

**Endpoint changes:**
- `search.messages` extended filters: `user_id` (string), `channel_id` (string), `date_from` (string, ISO date), `date_to` (string, ISO date). Applied as post-query filters on tantivy results.

**New endpoint:**
- `conversations.directory` — returns all public channels (not just user's channels). Includes: name, topic, description, member_count, created_at. Supports optional `query` param for name filtering, `limit` (default 50, max 200), and `cursor` (channel_id for keyset pagination). Allows joining directly.

### Frontend

**Search filters:**
- In search modal, add filter bar above results.
- Filter chips: "From: @user" (user autocomplete), "In: #channel" (channel autocomplete), "Date: range" (date picker).
- Filters combinable.
- Clear individual filters or "Clear all".

**Channel directory:**
- "Browse channels" button in sidebar (below channel list).
- Opens modal/page showing all public channels.
- Each entry: channel name, topic, member count, "Join" button.
- Search/filter within directory.
- Joined channels shown with checkmark instead of "Join" button.

**In-channel search:**
- Cmd+F within a channel scopes search to current channel (prefills channel filter in search modal).

---

## Workstream 9: UI Polish

### Problem
Missing quality-of-life features that make a chat platform feel finished.

### Frontend

**Dark/light theme:**
- CSS custom properties for all colors (background, text, borders, accents).
- Dark theme (current) and light theme.
- Toggle in settings + quick toggle button in sidebar.
- Persisted in user_settings (synced to backend) and localStorage (immediate).
- System preference detection as default (`prefers-color-scheme`).

**Loading skeletons:**
- Shimmer placeholders for: message list (3-4 message-shaped blocks), channel list (6-8 bars), user list.
- Shown during initial data fetch and channel switches.

**Empty states:**
- Empty channel: "No messages yet. Say something!" with wave illustration.
- No search results: "No messages found. Try different keywords."
- No DMs: "No direct messages yet. Start a conversation!"

**Connection status indicator:**
- Fixed bar at top of message area.
- "Reconnecting..." (yellow) when WebSocket disconnects.
- "Connected" (green, auto-hides after 2 seconds) on recovery.
- "Connection lost" (red) after multiple failed reconnect attempts.

**Keyboard shortcuts:**
- `Cmd+K` — Search (exists).
- `Cmd+Shift+A` — Jump to next unread channel.
- `Up arrow` (empty input) — Edit last own message.
- `Escape` — Close thread panel, search modal, or any open panel.
- `Cmd+/` — Show keyboard shortcuts help modal.

**Mobile responsive:**
- Sidebar collapses to hamburger menu on screens < 768px.
- Thread panel opens as full-screen overlay on mobile.
- Touch-friendly tap targets (min 44px).
- Swipe right to open sidebar, swipe left to close.

**Drag-and-drop file upload:**
- Drop zone overlay appears when dragging files over message area.
- Visual feedback: dashed border, "Drop files to upload" text.
- Supports multiple files (uploaded sequentially).

---

## Implementation Order

Recommended sequence based on dependencies and impact:

1. **Workstream 1: User Settings & Profile** — foundation for settings, theme, avatar used everywhere.
2. **Workstream 6: User Profiles & Presence** — depends on avatar/profile from WS1.
3. **Workstream 2: Channel Management** — independent, high impact.
4. **Workstream 3: Message Actions** — independent, high impact (wires up existing backend).
5. **Workstream 4: Rich Input & Autocomplete** — independent, high UX impact.
6. **Workstream 7: Notification Preferences** — depends on settings infrastructure from WS1.
7. **Workstream 5: Media & Content** — independent, moderate complexity.
8. **Workstream 8: Search & Discovery** — extends existing search.
9. **Workstream 9: UI Polish** — finishing touches, depends on theme from WS1.

---

## Out of Scope for v1

- External connectors (Notion, Zulip, kdb+)
- Pro tier features (SSO, SAML/OIDC, audit logs, data retention)
- Password reset via email (no email infrastructure)
- Email verification
- Two-factor authentication
- Multi-server federation
- Slash commands / workflow automation
- Mobile native apps
- Import/export from other platforms
- Custom emoji upload
- Message scheduling
- Read receipts (per-message)
- User blocking
- Granular permissions beyond owner/admin/member channel roles
- DND scheduling (time-based auto-DND)
