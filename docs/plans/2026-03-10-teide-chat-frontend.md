# Teide Chat Plan 3: SvelteKit Frontend — Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the SvelteKit web frontend for Teide Chat with Slack-familiar layout.

**Architecture:** SvelteKit SPA with Svelte stores for state management, a typed API client, and WebSocket integration for real-time updates.

**Tech Stack:** SvelteKit, TypeScript, Tailwind CSS

**Spec:** `docs/superpowers/specs/2026-03-10-teide-chat-design.md` (section 6)

---

## File Structure

```
teidelum/ui/
├── package.json
├── svelte.config.js
├── tailwind.config.ts
├── tsconfig.json
├── vite.config.ts
├── src/
│   ├── app.html
│   ├── app.css                 — Tailwind directives + global styles
│   ├── lib/
│   │   ├── api.ts              — Typed Slack API client (all endpoints)
│   │   ├── ws.ts               — WebSocket client (reconnect, event dispatch)
│   │   ├── types.ts            — Shared TypeScript types (User, Channel, Message, etc.)
│   │   ├── stores/
│   │   │   ├── auth.ts         — JWT persistence, current user, login/register
│   │   │   ├── channels.ts     — Channel list, active channel, create/join/leave
│   │   │   ├── messages.ts     — Per-channel message cache, send/edit/delete, pagination
│   │   │   ├── users.ts        — User list, presence tracking
│   │   │   └── unreads.ts      — Unread counts per channel
│   │   └── components/
│   │       ├── Sidebar.svelte
│   │       ├── MessageList.svelte
│   │       ├── MessageInput.svelte
│   │       ├── ThreadPanel.svelte
│   │       ├── ReactionPicker.svelte
│   │       ├── FileUpload.svelte
│   │       ├── SearchModal.svelte
│   │       └── UserPresence.svelte
│   └── routes/
│       ├── +layout.svelte      — Root layout (auth guard, WS init)
│       ├── +layout.ts          — Disable SSR
│       ├── login/+page.svelte
│       ├── register/+page.svelte
│       └── (app)/
│           ├── +layout.svelte  — App shell (sidebar + main area)
│           └── [channel]/
│               └── +page.svelte
```

---

## Chunk 1: Project Setup

### Task 1: Scaffold SvelteKit project with TypeScript and Tailwind CSS

**Files:**
- Create: `teidelum/ui/` (entire scaffold)

- [x] **Step 1: Create SvelteKit project**

Run from `/Users/antonkundenko/data/work/teidedb`:

```bash
cd /Users/antonkundenko/data/work/teidedb
npx sv create teidelum/ui --template minimal --types ts --no-add-ons --no-install
```

If `sv` is not available, use:

```bash
npm create svelte@latest teidelum/ui
```

Select: Skeleton project, TypeScript, no additional options.

- [x] **Step 2: Install dependencies**

```bash
cd /Users/antonkundenko/data/work/teidedb/teidelum/ui
npm install
npm install -D tailwindcss @tailwindcss/vite
npm install marked dompurify
npm install -D @types/dompurify
```

- [x] **Step 3: Configure Tailwind CSS**

Add the Tailwind Vite plugin to `vite.config.ts`:

```ts
import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

export default defineConfig({
	plugins: [tailwindcss(), sveltekit()]
});
```

Create `src/app.css`:

```css
@import 'tailwindcss';

:root {
	color-scheme: dark;
}

body {
	@apply bg-gray-900 text-gray-100;
	font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
	margin: 0;
	height: 100vh;
	overflow: hidden;
}
```

Import it in `src/routes/+layout.svelte`:

```svelte
<script>
	import '../app.css';
	let { children } = $props();
</script>

{@render children()}
```

- [x] **Step 4: Disable SSR (SPA mode)**

Create `src/routes/+layout.ts`:

```ts
export const ssr = false;
export const prerender = false;
```

- [x] **Step 5: Configure API proxy for development**

Add proxy config to `vite.config.ts` so `/api` and `/ws` requests go to the backend at `localhost:3000`:

```ts
export default defineConfig({
	plugins: [tailwindcss(), sveltekit()],
	server: {
		proxy: {
			'/api': 'http://localhost:3000',
			'/ws': {
				target: 'ws://localhost:3000',
				ws: true
			},
			'/files': 'http://localhost:3000'
		}
	}
});
```

- [x] **Step 6: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`
Expected: builds with no errors.

- [x] **Step 7: Commit**

```bash
cd /Users/antonkundenko/data/work/teidedb
git add teidelum/ui/
git commit -m "feat(chat-ui): scaffold SvelteKit project with TypeScript and Tailwind CSS"
```

---

### Task 2: Shared types

**Files:**
- Create: `teidelum/ui/src/lib/types.ts`

- [x] **Step 1: Define all shared TypeScript types**

Create `src/lib/types.ts`:

```ts
/** All IDs from the backend are i64 serialized as strings */
export type Id = string;

export interface User {
	id: Id;
	username: string;
	display_name: string;
	email: string;
	avatar_url: string;
	status: string;
	is_bot: boolean;
	created_at: string;
}

export interface Channel {
	id: Id;
	name: string;
	kind: 'public' | 'private' | 'dm';
	topic: string;
	created_by: Id;
	created_at: string;
	member_count?: number;
}

export interface Message {
	id: Id;
	ts: Id; // alias for id, used in Slack-compat responses
	channel_id: Id;
	user_id: Id;
	user?: string; // username, populated by API
	text: string;
	thread_ts?: Id;
	reply_count?: number;
	reactions?: Reaction[];
	files?: FileAttachment[];
	edited_at?: string;
	created_at: string;
}

export interface Reaction {
	name: string;
	count: number;
	users: Id[];
}

export interface FileAttachment {
	id: Id;
	filename: string;
	mime_type: string;
	size_bytes: number;
	url: string;
}

export interface AuthResponse {
	ok: boolean;
	user_id?: Id;
	token?: string;
	error?: string;
}

export interface ChannelListResponse {
	ok: boolean;
	channels?: Channel[];
	error?: string;
}

export interface ChannelResponse {
	ok: boolean;
	channel?: Channel;
	already_open?: boolean;
	error?: string;
}

export interface HistoryResponse {
	ok: boolean;
	messages?: Message[];
	has_more?: boolean;
	error?: string;
}

export interface MessageResponse {
	ok: boolean;
	message?: Message;
	error?: string;
}

export interface MembersResponse {
	ok: boolean;
	members?: Id[];
	error?: string;
}

export interface UsersListResponse {
	ok: boolean;
	members?: User[];
	error?: string;
}

export interface UserInfoResponse {
	ok: boolean;
	user?: User;
	error?: string;
}

export interface SearchResponse {
	ok: boolean;
	messages?: Message[];
	error?: string;
}

export interface FileUploadResponse {
	ok: boolean;
	file?: FileAttachment;
	error?: string;
}

export interface OkResponse {
	ok: boolean;
	error?: string;
}

/** WebSocket event types sent by server */
export type WsEventType =
	| 'hello'
	| 'message'
	| 'message_changed'
	| 'message_deleted'
	| 'reaction_added'
	| 'reaction_removed'
	| 'typing'
	| 'presence_change'
	| 'member_joined_channel'
	| 'member_left_channel';

export interface WsEvent {
	type: WsEventType;
	[key: string]: unknown;
}
```

- [x] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`
Expected: compiles with no errors.

- [x] **Step 3: Commit**

```bash
git add teidelum/ui/src/lib/types.ts
git commit -m "feat(chat-ui): add shared TypeScript types for API and WebSocket"
```

---

### Task 3: API client

**Files:**
- Create: `teidelum/ui/src/lib/api.ts`

- [x] **Step 1: Implement typed API client**

Create `src/lib/api.ts`. This is the single entry point for all API calls. Every method calls `POST /api/slack/<method>` with JSON body and Bearer token.

```ts
import type {
	AuthResponse,
	ChannelListResponse,
	ChannelResponse,
	FileUploadResponse,
	HistoryResponse,
	Id,
	MembersResponse,
	MessageResponse,
	OkResponse,
	SearchResponse,
	UserInfoResponse,
	UsersListResponse
} from './types';

let token: string | null = null;

export function setToken(t: string | null) {
	token = t;
}

export function getToken(): string | null {
	return token;
}

async function call<T>(method: string, body: Record<string, unknown> = {}): Promise<T> {
	const headers: Record<string, string> = { 'Content-Type': 'application/json' };
	if (token) headers['Authorization'] = `Bearer ${token}`;

	const res = await fetch(`/api/slack/${method}`, {
		method: 'POST',
		headers,
		body: JSON.stringify(body)
	});

	if (!res.ok) {
		throw new Error(`API ${method}: HTTP ${res.status}`);
	}

	return res.json();
}

// === Auth ===

export function register(username: string, password: string, email: string): Promise<AuthResponse> {
	return call('auth.register', { username, password, email });
}

export function login(username: string, password: string): Promise<AuthResponse> {
	return call('auth.login', { username, password });
}

// === Conversations ===

export function conversationsCreate(name: string, kind?: string, topic?: string): Promise<ChannelResponse> {
	return call('conversations.create', { name, kind, topic });
}

export function conversationsList(): Promise<ChannelListResponse> {
	return call('conversations.list', {});
}

export function conversationsInfo(channel: Id): Promise<ChannelResponse> {
	return call('conversations.info', { channel });
}

export function conversationsHistory(
	channel: Id,
	limit?: number,
	before?: Id
): Promise<HistoryResponse> {
	const body: Record<string, unknown> = { channel };
	if (limit !== undefined) body.limit = limit;
	if (before !== undefined) body.before = before;
	return call('conversations.history', body);
}

export function conversationsReplies(channel: Id, ts: Id): Promise<HistoryResponse> {
	return call('conversations.replies', { channel, ts });
}

export function conversationsJoin(channel: Id): Promise<OkResponse> {
	return call('conversations.join', { channel });
}

export function conversationsLeave(channel: Id): Promise<OkResponse> {
	return call('conversations.leave', { channel });
}

export function conversationsInvite(channel: Id, user: Id): Promise<OkResponse> {
	return call('conversations.invite', { channel, user });
}

export function conversationsMembers(channel: Id): Promise<MembersResponse> {
	return call('conversations.members', { channel });
}

export function conversationsOpen(users: Id[]): Promise<ChannelResponse> {
	return call('conversations.open', { users });
}

// === Chat ===

export function chatPostMessage(
	channel: Id,
	text: string,
	thread_ts?: Id
): Promise<MessageResponse> {
	const body: Record<string, unknown> = { channel, text };
	if (thread_ts !== undefined) body.thread_ts = thread_ts;
	return call('chat.postMessage', body);
}

export function chatUpdate(ts: Id, text: string): Promise<MessageResponse> {
	return call('chat.update', { ts, text });
}

export function chatDelete(ts: Id): Promise<OkResponse> {
	return call('chat.delete', { ts });
}

// === Users ===

export function usersList(): Promise<UsersListResponse> {
	return call('users.list', {});
}

export function usersInfo(user: Id): Promise<UserInfoResponse> {
	return call('users.info', { user });
}

export function usersSetPresence(presence: string): Promise<OkResponse> {
	return call('users.setPresence', { presence });
}

// === Reactions ===

export function reactionsAdd(name: string, timestamp: Id): Promise<OkResponse> {
	return call('reactions.add', { name, timestamp });
}

export function reactionsRemove(name: string, timestamp: Id): Promise<OkResponse> {
	return call('reactions.remove', { name, timestamp });
}

// === Search ===

export function searchMessages(
	query: string,
	channel?: Id,
	limit?: number
): Promise<SearchResponse> {
	const body: Record<string, unknown> = { query };
	if (channel !== undefined) body.channel = channel;
	if (limit !== undefined) body.limit = limit;
	return call('search.messages', body);
}

// === Files ===

export async function filesUpload(
	channel: Id,
	file: File,
	thread_ts?: Id
): Promise<FileUploadResponse> {
	const formData = new FormData();
	formData.append('file', file);
	formData.append('channel', channel);
	if (thread_ts) formData.append('thread_ts', thread_ts);

	const headers: Record<string, string> = {};
	if (token) headers['Authorization'] = `Bearer ${token}`;

	const res = await fetch('/api/slack/files.upload', {
		method: 'POST',
		headers,
		body: formData
	});

	if (!res.ok) {
		throw new Error(`API files.upload: HTTP ${res.status}`);
	}

	return res.json();
}

export function fileDownloadUrl(fileId: Id, filename: string): string {
	return `/files/${fileId}/${filename}`;
}
```

- [x] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`
Expected: compiles with no errors.

- [x] **Step 3: Commit**

```bash
git add teidelum/ui/src/lib/api.ts
git commit -m "feat(chat-ui): add typed API client for all Slack-compatible endpoints"
```

---

### Task 4: WebSocket client

**Files:**
- Create: `teidelum/ui/src/lib/ws.ts`

- [x] **Step 1: Implement WebSocket client with reconnect and event dispatch**

Create `src/lib/ws.ts`:

```ts
import type { WsEvent, WsEventType } from './types';

type EventCallback = (event: WsEvent) => void;

const listeners = new Map<WsEventType | '*', Set<EventCallback>>();

let ws: WebSocket | null = null;
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let reconnectDelay = 1000;
const MAX_RECONNECT_DELAY = 30000;
let currentToken: string | null = null;
let intentionalClose = false;

export function connect(token: string) {
	currentToken = token;
	intentionalClose = false;
	reconnectDelay = 1000;
	doConnect();
}

export function disconnect() {
	intentionalClose = true;
	currentToken = null;
	if (reconnectTimer) {
		clearTimeout(reconnectTimer);
		reconnectTimer = null;
	}
	if (ws) {
		ws.close();
		ws = null;
	}
}

function doConnect() {
	if (!currentToken) return;

	const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
	const url = `${protocol}//${window.location.host}/ws?token=${currentToken}`;

	ws = new WebSocket(url);

	ws.onopen = () => {
		reconnectDelay = 1000;
	};

	ws.onmessage = (event) => {
		try {
			const data: WsEvent = JSON.parse(event.data);
			dispatch(data);
		} catch {
			// ignore malformed messages
		}
	};

	ws.onclose = () => {
		ws = null;
		if (!intentionalClose) {
			scheduleReconnect();
		}
	};

	ws.onerror = () => {
		// onclose will fire after onerror
	};
}

function scheduleReconnect() {
	if (reconnectTimer) return;
	reconnectTimer = setTimeout(() => {
		reconnectTimer = null;
		reconnectDelay = Math.min(reconnectDelay * 2, MAX_RECONNECT_DELAY);
		doConnect();
	}, reconnectDelay);
}

function dispatch(event: WsEvent) {
	const typeListeners = listeners.get(event.type);
	if (typeListeners) {
		for (const cb of typeListeners) cb(event);
	}
	const wildcardListeners = listeners.get('*');
	if (wildcardListeners) {
		for (const cb of wildcardListeners) cb(event);
	}
}

/** Subscribe to a specific event type or '*' for all events. Returns unsubscribe function. */
export function on(type: WsEventType | '*', callback: EventCallback): () => void {
	if (!listeners.has(type)) {
		listeners.set(type, new Set());
	}
	listeners.get(type)!.add(callback);
	return () => {
		listeners.get(type)?.delete(callback);
	};
}

/** Send typing indicator to a channel */
export function sendTyping(channel: string) {
	if (ws && ws.readyState === WebSocket.OPEN) {
		ws.send(`typing ${channel}`);
	}
}

/** Send ping to keep connection alive */
export function sendPing() {
	if (ws && ws.readyState === WebSocket.OPEN) {
		ws.send('ping');
	}
}
```

- [x] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`
Expected: compiles with no errors.

- [x] **Step 3: Commit**

```bash
git add teidelum/ui/src/lib/ws.ts
git commit -m "feat(chat-ui): add WebSocket client with auto-reconnect and event dispatch"
```

---

## Chunk 2: Stores

### Task 5: Auth store

**Files:**
- Create: `teidelum/ui/src/lib/stores/auth.ts`

- [x] **Step 1: Implement auth store**

Create `src/lib/stores/auth.ts`:

```ts
import { writable, derived, get } from 'svelte/store';
import * as api from '$lib/api';
import * as ws from '$lib/ws';
import type { User, Id } from '$lib/types';

interface AuthState {
	token: string | null;
	userId: Id | null;
	user: User | null;
	loading: boolean;
}

const initial: AuthState = {
	token: typeof localStorage !== 'undefined' ? localStorage.getItem('teide_token') : null,
	userId: typeof localStorage !== 'undefined' ? localStorage.getItem('teide_user_id') : null,
	user: null,
	loading: false
};

export const auth = writable<AuthState>(initial);

export const isAuthenticated = derived(auth, ($auth) => !!$auth.token);

/** Initialize from persisted token. Call on app start. */
export async function initAuth() {
	const state = get(auth);
	if (state.token) {
		api.setToken(state.token);
		ws.connect(state.token);
		if (state.userId) {
			try {
				const res = await api.usersInfo(state.userId);
				if (res.ok && res.user) {
					auth.update((s) => ({ ...s, user: res.user! }));
				} else {
					// Token invalid, clear
					doLogout();
				}
			} catch {
				doLogout();
			}
		}
	}
}

export async function doLogin(username: string, password: string): Promise<string | null> {
	auth.update((s) => ({ ...s, loading: true }));
	try {
		const res = await api.login(username, password);
		if (res.ok && res.token && res.user_id) {
			localStorage.setItem('teide_token', res.token);
			localStorage.setItem('teide_user_id', res.user_id);
			api.setToken(res.token);
			ws.connect(res.token);

			const userRes = await api.usersInfo(res.user_id);
			auth.set({
				token: res.token,
				userId: res.user_id,
				user: userRes.ok ? userRes.user! : null,
				loading: false
			});
			return null;
		}
		auth.update((s) => ({ ...s, loading: false }));
		return res.error || 'Login failed';
	} catch (e) {
		auth.update((s) => ({ ...s, loading: false }));
		return (e as Error).message;
	}
}

export async function doRegister(
	username: string,
	password: string,
	email: string
): Promise<string | null> {
	auth.update((s) => ({ ...s, loading: true }));
	try {
		const res = await api.register(username, password, email);
		if (res.ok && res.token && res.user_id) {
			localStorage.setItem('teide_token', res.token);
			localStorage.setItem('teide_user_id', res.user_id);
			api.setToken(res.token);
			ws.connect(res.token);

			const userRes = await api.usersInfo(res.user_id);
			auth.set({
				token: res.token,
				userId: res.user_id,
				user: userRes.ok ? userRes.user! : null,
				loading: false
			});
			return null;
		}
		auth.update((s) => ({ ...s, loading: false }));
		return res.error || 'Registration failed';
	} catch (e) {
		auth.update((s) => ({ ...s, loading: false }));
		return (e as Error).message;
	}
}

export function doLogout() {
	localStorage.removeItem('teide_token');
	localStorage.removeItem('teide_user_id');
	api.setToken(null);
	ws.disconnect();
	auth.set({ token: null, userId: null, user: null, loading: false });
}
```

- [x] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [x] **Step 3: Commit**

```bash
git add teidelum/ui/src/lib/stores/auth.ts
git commit -m "feat(chat-ui): add auth store with JWT persistence and login/register"
```

---

### Task 6: Channels store

**Files:**
- Create: `teidelum/ui/src/lib/stores/channels.ts`

- [x] **Step 1: Implement channels store**

Create `src/lib/stores/channels.ts`:

```ts
import { writable, derived, get } from 'svelte/store';
import * as api from '$lib/api';
import * as ws from '$lib/ws';
import type { Channel, Id } from '$lib/types';

export const channels = writable<Channel[]>([]);
export const activeChannelId = writable<Id | null>(null);

export const activeChannel = derived(
	[channels, activeChannelId],
	([$channels, $activeChannelId]) => $channels.find((c) => c.id === $activeChannelId) ?? null
);

export const publicChannels = derived(channels, ($channels) =>
	$channels.filter((c) => c.kind === 'public' || c.kind === 'private')
);

export const dmChannels = derived(channels, ($channels) =>
	$channels.filter((c) => c.kind === 'dm')
);

export async function loadChannels() {
	const res = await api.conversationsList();
	if (res.ok && res.channels) {
		channels.set(res.channels);
	}
}

export async function createChannel(
	name: string,
	kind?: string,
	topic?: string
): Promise<Channel | null> {
	const res = await api.conversationsCreate(name, kind, topic);
	if (res.ok && res.channel) {
		channels.update((list) => [...list, res.channel!]);
		return res.channel;
	}
	return null;
}

export async function joinChannel(channelId: Id) {
	const res = await api.conversationsJoin(channelId);
	if (res.ok) {
		await loadChannels();
	}
}

export async function leaveChannel(channelId: Id) {
	const res = await api.conversationsLeave(channelId);
	if (res.ok) {
		channels.update((list) => list.filter((c) => c.id !== channelId));
		if (get(activeChannelId) === channelId) {
			activeChannelId.set(null);
		}
	}
}

export async function openDm(userIds: Id[]): Promise<Channel | null> {
	const res = await api.conversationsOpen(userIds);
	if (res.ok && res.channel) {
		if (!res.already_open) {
			channels.update((list) => [...list, res.channel!]);
		}
		return res.channel;
	}
	return null;
}

export function setActiveChannel(channelId: Id) {
	activeChannelId.set(channelId);
}

// WebSocket: update channel list when membership changes
export function initChannelWsListeners() {
	ws.on('member_joined_channel', () => {
		loadChannels();
	});
	ws.on('member_left_channel', () => {
		loadChannels();
	});
}
```

- [x] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [x] **Step 3: Commit**

```bash
git add teidelum/ui/src/lib/stores/channels.ts
git commit -m "feat(chat-ui): add channels store with CRUD and WebSocket listeners"
```

---

### Task 7: Messages store

**Files:**
- Create: `teidelum/ui/src/lib/stores/messages.ts`

- [x] **Step 1: Implement messages store**

Create `src/lib/stores/messages.ts`:

```ts
import { writable, get } from 'svelte/store';
import * as api from '$lib/api';
import * as ws from '$lib/ws';
import type { Message, Id, WsEvent } from '$lib/types';

interface ChannelMessages {
	messages: Message[];
	hasMore: boolean;
	loading: boolean;
}

/** Map of channelId -> messages state */
export const messagesByChannel = writable<Map<Id, ChannelMessages>>(new Map());

function getChannelState(channelId: Id): ChannelMessages {
	const map = get(messagesByChannel);
	return map.get(channelId) ?? { messages: [], hasMore: true, loading: false };
}

function setChannelState(channelId: Id, state: ChannelMessages) {
	messagesByChannel.update((map) => {
		const newMap = new Map(map);
		newMap.set(channelId, state);
		return newMap;
	});
}

/** Load initial messages for a channel */
export async function loadMessages(channelId: Id) {
	const state = getChannelState(channelId);
	if (state.loading) return;

	setChannelState(channelId, { ...state, loading: true });

	const res = await api.conversationsHistory(channelId, 50);
	if (res.ok && res.messages) {
		setChannelState(channelId, {
			messages: res.messages.reverse(), // API returns newest first, we want oldest first
			hasMore: res.has_more ?? false,
			loading: false
		});
	} else {
		setChannelState(channelId, { ...state, loading: false });
	}
}

/** Load older messages (infinite scroll up) */
export async function loadOlderMessages(channelId: Id) {
	const state = getChannelState(channelId);
	if (state.loading || !state.hasMore) return;

	setChannelState(channelId, { ...state, loading: true });

	const oldestMsg = state.messages[0];
	const before = oldestMsg?.id;

	const res = await api.conversationsHistory(channelId, 50, before);
	if (res.ok && res.messages) {
		setChannelState(channelId, {
			messages: [...res.messages.reverse(), ...state.messages],
			hasMore: res.has_more ?? false,
			loading: false
		});
	} else {
		setChannelState(channelId, { ...state, loading: false });
	}
}

/** Send a message (optimistic UI) */
export async function sendMessage(channelId: Id, text: string, threadTs?: Id) {
	const res = await api.chatPostMessage(channelId, text, threadTs);
	if (res.ok && res.message) {
		// The WebSocket will deliver the message, but in case it's slow, add it now
		appendMessage(channelId, res.message);
	}
}

/** Edit a message */
export async function editMessage(ts: Id, text: string) {
	await api.chatUpdate(ts, text);
	// WebSocket message_changed event will update the store
}

/** Delete a message */
export async function deleteMessage(ts: Id) {
	await api.chatDelete(ts);
	// WebSocket message_deleted event will update the store
}

function appendMessage(channelId: Id, message: Message) {
	messagesByChannel.update((map) => {
		const newMap = new Map(map);
		const state = newMap.get(channelId) ?? { messages: [], hasMore: false, loading: false };
		// Avoid duplicates
		if (!state.messages.find((m) => m.id === message.id)) {
			newMap.set(channelId, {
				...state,
				messages: [...state.messages, message]
			});
		}
		return newMap;
	});
}

function updateMessage(messageId: Id, updater: (msg: Message) => Message) {
	messagesByChannel.update((map) => {
		const newMap = new Map(map);
		for (const [channelId, state] of newMap) {
			const idx = state.messages.findIndex((m) => m.id === messageId || m.ts === messageId);
			if (idx !== -1) {
				const newMessages = [...state.messages];
				newMessages[idx] = updater(newMessages[idx]);
				newMap.set(channelId, { ...state, messages: newMessages });
				break;
			}
		}
		return newMap;
	});
}

function removeMessage(messageId: Id) {
	messagesByChannel.update((map) => {
		const newMap = new Map(map);
		for (const [channelId, state] of newMap) {
			const filtered = state.messages.filter((m) => m.id !== messageId && m.ts !== messageId);
			if (filtered.length !== state.messages.length) {
				newMap.set(channelId, { ...state, messages: filtered });
				break;
			}
		}
		return newMap;
	});
}

/** Initialize WebSocket listeners for real-time message updates */
export function initMessageWsListeners() {
	ws.on('message', (event: WsEvent) => {
		const msg = event as unknown as { channel: Id } & Message;
		if (msg.channel_id || msg.channel) {
			appendMessage(msg.channel_id || (msg.channel as Id), msg as Message);
		}
	});

	ws.on('message_changed', (event: WsEvent) => {
		const data = event as unknown as { message: Message };
		if (data.message) {
			updateMessage(data.message.id || data.message.ts, () => data.message);
		}
	});

	ws.on('message_deleted', (event: WsEvent) => {
		const data = event as unknown as { ts: Id };
		if (data.ts) {
			removeMessage(data.ts);
		}
	});

	ws.on('reaction_added', (event: WsEvent) => {
		const data = event as unknown as { item: { ts: Id }; reaction: string; user: Id };
		if (data.item?.ts) {
			updateMessage(data.item.ts, (msg) => {
				const reactions = [...(msg.reactions || [])];
				const existing = reactions.find((r) => r.name === data.reaction);
				if (existing) {
					existing.count++;
					existing.users = [...existing.users, data.user];
				} else {
					reactions.push({ name: data.reaction, count: 1, users: [data.user] });
				}
				return { ...msg, reactions };
			});
		}
	});

	ws.on('reaction_removed', (event: WsEvent) => {
		const data = event as unknown as { item: { ts: Id }; reaction: string; user: Id };
		if (data.item?.ts) {
			updateMessage(data.item.ts, (msg) => {
				let reactions = [...(msg.reactions || [])];
				const existing = reactions.find((r) => r.name === data.reaction);
				if (existing) {
					existing.count--;
					existing.users = existing.users.filter((u) => u !== data.user);
					if (existing.count <= 0) {
						reactions = reactions.filter((r) => r.name !== data.reaction);
					}
				}
				return { ...msg, reactions };
			});
		}
	});
}
```

- [x] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [x] **Step 3: Commit**

```bash
git add teidelum/ui/src/lib/stores/messages.ts
git commit -m "feat(chat-ui): add messages store with pagination, optimistic send, and WebSocket updates"
```

---

### Task 8: Users store

**Files:**
- Create: `teidelum/ui/src/lib/stores/users.ts`

- [x] **Step 1: Implement users store**


Create `src/lib/stores/users.ts`:

```ts
import { writable, derived, get } from 'svelte/store';
import * as api from '$lib/api';
import * as ws from '$lib/ws';
import type { User, Id, WsEvent } from '$lib/types';

export const users = writable<Map<Id, User>>(new Map());

/** Presence: maps userId -> 'active' | 'away' */
export const presence = writable<Map<Id, string>>(new Map());

export const userList = derived(users, ($users) => Array.from($users.values()));

export async function loadUsers() {
	const res = await api.usersList();
	if (res.ok && res.members) {
		const map = new Map<Id, User>();
		for (const u of res.members) {
			map.set(u.id, u);
		}
		users.set(map);
	}
}

export function getUser(userId: Id): User | undefined {
	return get(users).get(userId);
}

export function getUserPresence(userId: Id): string {
	return get(presence).get(userId) ?? 'away';
}

export function initUserWsListeners() {
	ws.on('presence_change', (event: WsEvent) => {
		const data = event as unknown as { user: Id; presence: string };
		if (data.user) {
			presence.update((map) => {
				const newMap = new Map(map);
				newMap.set(data.user, data.presence);
				return newMap;
			});
		}
	});
}
```

- [x] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [x] **Step 3: Commit**

```bash
git add teidelum/ui/src/lib/stores/users.ts
git commit -m "feat(chat-ui): add users store with presence tracking"
```

---

### Task 9: Unreads store

**Files:**
- Create: `teidelum/ui/src/lib/stores/unreads.ts`

- [x] **Step 1: Implement unreads store**

Create `src/lib/stores/unreads.ts`:

```ts
import { writable, get } from 'svelte/store';
import * as ws from '$lib/ws';
import { activeChannelId } from './channels';
import type { Id, WsEvent } from '$lib/types';

/** Map of channelId -> unread count */
export const unreads = writable<Map<Id, number>>(new Map());

export function markRead(channelId: Id) {
	unreads.update((map) => {
		const newMap = new Map(map);
		newMap.delete(channelId);
		return newMap;
	});
}

export function incrementUnread(channelId: Id) {
	// Don't increment for the currently active channel
	if (get(activeChannelId) === channelId) return;

	unreads.update((map) => {
		const newMap = new Map(map);
		newMap.set(channelId, (newMap.get(channelId) ?? 0) + 1);
		return newMap;
	});
}

export function getUnreadCount(channelId: Id): number {
	return get(unreads).get(channelId) ?? 0;
}

export function initUnreadsWsListeners() {
	ws.on('message', (event: WsEvent) => {
		const data = event as unknown as { channel_id?: Id; channel?: Id };
		const channelId = data.channel_id || data.channel;
		if (channelId) {
			incrementUnread(channelId);
		}
	});
}
```

- [x] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [x] **Step 3: Commit**

```bash
git add teidelum/ui/src/lib/stores/unreads.ts
git commit -m "feat(chat-ui): add unreads store with per-channel unread counts"
```

---

## Chunk 3: Auth Pages

### Task 10: Login page

**Files:**
- Create: `teidelum/ui/src/routes/login/+page.svelte`

- [x] **Step 1: Implement login page**

Create `src/routes/login/+page.svelte`:

```svelte
<script lang="ts">
	import { goto } from '$app/navigation';
	import { doLogin } from '$lib/stores/auth';

	let username = $state('');
	let password = $state('');
	let error = $state<string | null>(null);
	let loading = $state(false);

	async function handleSubmit(e: SubmitEvent) {
		e.preventDefault();
		if (!username.trim() || !password) return;

		loading = true;
		error = null;

		const err = await doLogin(username.trim(), password);
		loading = false;

		if (err) {
			error = err;
		} else {
			goto('/');
		}
	}
</script>

<svelte:head>
	<title>Login - Teide Chat</title>
</svelte:head>

<div class="flex min-h-screen items-center justify-center bg-gray-900">
	<div class="w-full max-w-sm rounded-lg bg-gray-800 p-8 shadow-xl">
		<h1 class="mb-6 text-center text-2xl font-bold text-white">Teide Chat</h1>

		<form onsubmit={handleSubmit} class="space-y-4">
			{#if error}
				<div class="rounded bg-red-900/50 px-3 py-2 text-sm text-red-300">{error}</div>
			{/if}

			<div>
				<label for="username" class="mb-1 block text-sm text-gray-400">Username</label>
				<input
					id="username"
					type="text"
					bind:value={username}
					class="w-full rounded bg-gray-700 px-3 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
					placeholder="Enter username"
					autocomplete="username"
					required
				/>
			</div>

			<div>
				<label for="password" class="mb-1 block text-sm text-gray-400">Password</label>
				<input
					id="password"
					type="password"
					bind:value={password}
					class="w-full rounded bg-gray-700 px-3 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
					placeholder="Enter password"
					autocomplete="current-password"
					required
				/>
			</div>

			<button
				type="submit"
				disabled={loading}
				class="w-full rounded bg-blue-600 py-2 font-medium text-white transition hover:bg-blue-700 disabled:opacity-50"
			>
				{loading ? 'Signing in...' : 'Sign In'}
			</button>
		</form>

		<p class="mt-4 text-center text-sm text-gray-500">
			Don't have an account?
			<a href="/register" class="text-blue-400 hover:underline">Register</a>
		</p>
	</div>
</div>
```

- [x] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [x] **Step 3: Commit**

```bash
git add teidelum/ui/src/routes/login/+page.svelte
git commit -m "feat(chat-ui): add login page"
```

---

### Task 11: Register page

**Files:**
- Create: `teidelum/ui/src/routes/register/+page.svelte`

- [x] **Step 1: Implement register page**

Create `src/routes/register/+page.svelte`:

```svelte
<script lang="ts">
	import { goto } from '$app/navigation';
	import { doRegister } from '$lib/stores/auth';

	let username = $state('');
	let email = $state('');
	let password = $state('');
	let confirmPassword = $state('');
	let error = $state<string | null>(null);
	let loading = $state(false);

	async function handleSubmit(e: SubmitEvent) {
		e.preventDefault();
		if (!username.trim() || !email.trim() || !password) return;

		if (password !== confirmPassword) {
			error = 'Passwords do not match';
			return;
		}

		loading = true;
		error = null;

		const err = await doRegister(username.trim(), password, email.trim());
		loading = false;

		if (err) {
			error = err;
		} else {
			goto('/');
		}
	}
</script>

<svelte:head>
	<title>Register - Teide Chat</title>
</svelte:head>

<div class="flex min-h-screen items-center justify-center bg-gray-900">
	<div class="w-full max-w-sm rounded-lg bg-gray-800 p-8 shadow-xl">
		<h1 class="mb-6 text-center text-2xl font-bold text-white">Create Account</h1>

		<form onsubmit={handleSubmit} class="space-y-4">
			{#if error}
				<div class="rounded bg-red-900/50 px-3 py-2 text-sm text-red-300">{error}</div>
			{/if}

			<div>
				<label for="username" class="mb-1 block text-sm text-gray-400">Username</label>
				<input
					id="username"
					type="text"
					bind:value={username}
					class="w-full rounded bg-gray-700 px-3 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
					placeholder="Choose a username"
					autocomplete="username"
					required
				/>
			</div>

			<div>
				<label for="email" class="mb-1 block text-sm text-gray-400">Email</label>
				<input
					id="email"
					type="email"
					bind:value={email}
					class="w-full rounded bg-gray-700 px-3 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
					placeholder="you@example.com"
					autocomplete="email"
					required
				/>
			</div>

			<div>
				<label for="password" class="mb-1 block text-sm text-gray-400">Password</label>
				<input
					id="password"
					type="password"
					bind:value={password}
					class="w-full rounded bg-gray-700 px-3 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
					placeholder="Choose a password"
					autocomplete="new-password"
					required
				/>
			</div>

			<div>
				<label for="confirmPassword" class="mb-1 block text-sm text-gray-400">Confirm Password</label>
				<input
					id="confirmPassword"
					type="password"
					bind:value={confirmPassword}
					class="w-full rounded bg-gray-700 px-3 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
					placeholder="Confirm password"
					autocomplete="new-password"
					required
				/>
			</div>

			<button
				type="submit"
				disabled={loading}
				class="w-full rounded bg-blue-600 py-2 font-medium text-white transition hover:bg-blue-700 disabled:opacity-50"
			>
				{loading ? 'Creating account...' : 'Create Account'}
			</button>
		</form>

		<p class="mt-4 text-center text-sm text-gray-500">
			Already have an account?
			<a href="/login" class="text-blue-400 hover:underline">Sign in</a>
		</p>
	</div>
</div>
```

- [x] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [x] **Step 3: Commit**

```bash
git add teidelum/ui/src/routes/register/+page.svelte
git commit -m "feat(chat-ui): add register page"
```

---

### Task 12: Auth guard in root layout

**Files:**
- Modify: `teidelum/ui/src/routes/+layout.svelte`

- [x] **Step 1: Add auth guard to root layout**

Update `src/routes/+layout.svelte` to check auth state and redirect unauthenticated users to `/login`:

```svelte
<script lang="ts">
	import '../app.css';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { auth, isAuthenticated, initAuth } from '$lib/stores/auth';
	import { onMount } from 'svelte';

	let { children } = $props();
	let initialized = $state(false);

	const publicRoutes = ['/login', '/register'];

	onMount(async () => {
		await initAuth();
		initialized = true;
	});

	$effect(() => {
		if (!initialized) return;
		const isPublic = publicRoutes.includes(page.url.pathname);

		if (!$isAuthenticated && !isPublic) {
			goto('/login');
		} else if ($isAuthenticated && isPublic) {
			goto('/');
		}
	});
</script>

{#if !initialized}
	<div class="flex min-h-screen items-center justify-center bg-gray-900">
		<div class="text-gray-500">Loading...</div>
	</div>
{:else}
	{@render children()}
{/if}
```

- [x] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [x] **Step 3: Commit**

```bash
git add teidelum/ui/src/routes/+layout.svelte
git commit -m "feat(chat-ui): add auth guard to root layout with redirect logic"
```

---

## Chunk 4: Main Layout and Core Components

### Task 13: App layout shell

**Files:**
- Create: `teidelum/ui/src/routes/(app)/+layout.svelte`
- Create: `teidelum/ui/src/routes/(app)/+page.svelte` (redirect to first channel)

- [x] **Step 1: Create app layout with sidebar + main area**

Create `src/routes/(app)/+layout.svelte`:

```svelte
<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import { loadChannels, initChannelWsListeners } from '$lib/stores/channels';
	import { loadUsers, initUserWsListeners } from '$lib/stores/users';
	import { initMessageWsListeners } from '$lib/stores/messages';
	import { initUnreadsWsListeners } from '$lib/stores/unreads';

	let { children } = $props();
	let threadChannelId = $state<string | null>(null);
	let threadTs = $state<string | null>(null);

	onMount(async () => {
		await Promise.all([loadChannels(), loadUsers()]);
		initChannelWsListeners();
		initUserWsListeners();
		initMessageWsListeners();
		initUnreadsWsListeners();
	});
</script>

<div class="flex h-screen overflow-hidden bg-gray-900">
	<!-- Sidebar -->
	<div class="flex w-64 flex-shrink-0 flex-col border-r border-gray-700 bg-gray-800">
		<Sidebar />
	</div>

	<!-- Main content area -->
	<div class="flex flex-1 overflow-hidden">
		{@render children()}
	</div>
</div>
```

Create `src/routes/(app)/+page.svelte` (landing redirect):

```svelte
<script lang="ts">
	import { goto } from '$app/navigation';
	import { channels } from '$lib/stores/channels';
	import { onMount } from 'svelte';

	onMount(() => {
		const unsub = channels.subscribe(($channels) => {
			if ($channels.length > 0) {
				goto(`/${$channels[0].id}`);
				unsub();
			}
		});
		return unsub;
	});
</script>

<div class="flex flex-1 items-center justify-center text-gray-500">
	<p>Select a channel to start chatting</p>
</div>
```

- [x] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [x] **Step 3: Commit**

```bash
git add teidelum/ui/src/routes/\(app\)/
git commit -m "feat(chat-ui): add app layout shell with sidebar and main content area"
```

---

### Task 14: Sidebar component

**Files:**
- Create: `teidelum/ui/src/lib/components/Sidebar.svelte`

- [x] **Step 1: Implement sidebar with channel list, DM list, unread badges, create channel**

Create `src/lib/components/Sidebar.svelte`:

```svelte
<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import {
		publicChannels,
		dmChannels,
		activeChannelId,
		setActiveChannel,
		createChannel
	} from '$lib/stores/channels';
	import { unreads } from '$lib/stores/unreads';
	import { auth, doLogout } from '$lib/stores/auth';
	import { users } from '$lib/stores/users';
	import type { Channel } from '$lib/types';

	let showCreateModal = $state(false);
	let newChannelName = $state('');
	let newChannelTopic = $state('');

	function navigateToChannel(channel: Channel) {
		setActiveChannel(channel.id);
		goto(`/${channel.id}`);
	}

	function getUnreadCount(channelId: string): number {
		return $unreads.get(channelId) ?? 0;
	}

	function isActive(channelId: string): boolean {
		return $activeChannelId === channelId;
	}

	function getDmDisplayName(channel: Channel): string {
		// DM channel names are typically formatted; show the name as-is
		return channel.name || 'Direct Message';
	}

	async function handleCreateChannel() {
		if (!newChannelName.trim()) return;
		const ch = await createChannel(newChannelName.trim(), 'public', newChannelTopic.trim() || undefined);
		if (ch) {
			showCreateModal = false;
			newChannelName = '';
			newChannelTopic = '';
			navigateToChannel(ch);
		}
	}

	function handleLogout() {
		doLogout();
		goto('/login');
	}
</script>

<div class="flex h-full flex-col">
	<!-- Header -->
	<div class="flex items-center justify-between border-b border-gray-700 px-4 py-3">
		<h2 class="text-lg font-bold text-white">Teide Chat</h2>
		<button
			onclick={handleLogout}
			class="text-xs text-gray-500 hover:text-gray-300"
			title="Sign out"
		>
			Sign out
		</button>
	</div>

	<!-- User info -->
	{#if $auth.user}
		<div class="border-b border-gray-700 px-4 py-2">
			<span class="text-sm text-gray-300">{$auth.user.display_name || $auth.user.username}</span>
		</div>
	{/if}

	<!-- Channel list -->
	<div class="flex-1 overflow-y-auto">
		<!-- Channels section -->
		<div class="px-2 pt-3">
			<div class="flex items-center justify-between px-2 pb-1">
				<span class="text-xs font-semibold uppercase tracking-wide text-gray-500">Channels</span>
				<button
					onclick={() => (showCreateModal = true)}
					class="text-gray-500 hover:text-gray-300"
					title="Create channel"
				>
					<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
					</svg>
				</button>
			</div>

			{#each $publicChannels as channel}
				<button
					onclick={() => navigateToChannel(channel)}
					class="flex w-full items-center justify-between rounded px-2 py-1 text-left text-sm transition {isActive(channel.id)
						? 'bg-blue-600 text-white'
						: 'text-gray-400 hover:bg-gray-700 hover:text-gray-200'}"
				>
					<span class="truncate">
						<span class="mr-1 text-gray-500">#</span>
						{channel.name}
					</span>
					{#if getUnreadCount(channel.id) > 0}
						<span class="ml-1 rounded-full bg-red-500 px-1.5 text-xs font-bold text-white">
							{getUnreadCount(channel.id)}
						</span>
					{/if}
				</button>
			{/each}
		</div>

		<!-- DMs section -->
		<div class="px-2 pt-4">
			<div class="px-2 pb-1">
				<span class="text-xs font-semibold uppercase tracking-wide text-gray-500">Direct Messages</span>
			</div>

			{#each $dmChannels as channel}
				<button
					onclick={() => navigateToChannel(channel)}
					class="flex w-full items-center justify-between rounded px-2 py-1 text-left text-sm transition {isActive(channel.id)
						? 'bg-blue-600 text-white'
						: 'text-gray-400 hover:bg-gray-700 hover:text-gray-200'}"
				>
					<span class="truncate">{getDmDisplayName(channel)}</span>
					{#if getUnreadCount(channel.id) > 0}
						<span class="ml-1 rounded-full bg-red-500 px-1.5 text-xs font-bold text-white">
							{getUnreadCount(channel.id)}
						</span>
					{/if}
				</button>
			{/each}
		</div>
	</div>
</div>

<!-- Create channel modal -->
{#if showCreateModal}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
		<div class="w-full max-w-md rounded-lg bg-gray-800 p-6 shadow-xl">
			<h3 class="mb-4 text-lg font-bold text-white">Create Channel</h3>

			<form
				onsubmit={(e) => {
					e.preventDefault();
					handleCreateChannel();
				}}
				class="space-y-3"
			>
				<div>
					<label for="channelName" class="mb-1 block text-sm text-gray-400">Channel Name</label>
					<input
						id="channelName"
						type="text"
						bind:value={newChannelName}
						class="w-full rounded bg-gray-700 px-3 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
						placeholder="e.g. general"
						required
					/>
				</div>

				<div>
					<label for="channelTopic" class="mb-1 block text-sm text-gray-400">Topic (optional)</label>
					<input
						id="channelTopic"
						type="text"
						bind:value={newChannelTopic}
						class="w-full rounded bg-gray-700 px-3 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
						placeholder="What's this channel about?"
					/>
				</div>

				<div class="flex justify-end gap-2 pt-2">
					<button
						type="button"
						onclick={() => (showCreateModal = false)}
						class="rounded px-4 py-2 text-sm text-gray-400 hover:text-gray-200"
					>
						Cancel
					</button>
					<button
						type="submit"
						class="rounded bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
					>
						Create
					</button>
				</div>
			</form>
		</div>
	</div>
{/if}
```

- [x] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [x] **Step 3: Commit**

```bash
git add teidelum/ui/src/lib/components/Sidebar.svelte
git commit -m "feat(chat-ui): add Sidebar component with channel list, DMs, unreads, and create channel modal"
```

---

### Task 15: MessageList component

**Files:**
- Create: `teidelum/ui/src/lib/components/MessageList.svelte`

- [x] **Step 1: Implement message list with infinite scroll and auto-scroll**

Create `src/lib/components/MessageList.svelte`:

```svelte
<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { messagesByChannel, loadMessages, loadOlderMessages } from '$lib/stores/messages';
	import { users } from '$lib/stores/users';
	import { auth } from '$lib/stores/auth';
	import { reactionsAdd, reactionsRemove } from '$lib/api';
	import type { Message, Id } from '$lib/types';

	interface Props {
		channelId: Id;
		onOpenThread?: (msg: Message) => void;
	}

	let { channelId, onOpenThread }: Props = $props();

	let scrollContainer: HTMLDivElement | undefined = $state();
	let isAtBottom = $state(true);
	let prevMessageCount = $state(0);

	const channelState = $derived($messagesByChannel.get(channelId));
	const messages = $derived(channelState?.messages ?? []);
	const hasMore = $derived(channelState?.hasMore ?? false);
	const loading = $derived(channelState?.loading ?? false);

	$effect(() => {
		// Load messages when channelId changes
		channelId; // track
		loadMessages(channelId);
	});

	$effect(() => {
		// Auto-scroll to bottom when new messages arrive (if already at bottom)
		if (messages.length > prevMessageCount && isAtBottom) {
			tick().then(() => {
				scrollToBottom();
			});
		}
		prevMessageCount = messages.length;
	});

	function scrollToBottom() {
		if (scrollContainer) {
			scrollContainer.scrollTop = scrollContainer.scrollHeight;
		}
	}

	function handleScroll() {
		if (!scrollContainer) return;

		const { scrollTop, scrollHeight, clientHeight } = scrollContainer;
		isAtBottom = scrollHeight - scrollTop - clientHeight < 50;

		// Load older messages when scrolled to top
		if (scrollTop < 100 && hasMore && !loading) {
			loadOlderMessages(channelId);
		}
	}

	function getUserName(userId: Id): string {
		const user = $users.get(userId);
		return user?.display_name || user?.username || userId;
	}

	function getUserAvatar(userId: Id): string {
		const user = $users.get(userId);
		return user?.display_name?.[0]?.toUpperCase() || user?.username?.[0]?.toUpperCase() || '?';
	}

	function formatTime(timestamp: string): string {
		try {
			const date = new Date(timestamp);
			return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
		} catch {
			return '';
		}
	}

	function formatDate(timestamp: string): string {
		try {
			const date = new Date(timestamp);
			const today = new Date();
			if (date.toDateString() === today.toDateString()) return 'Today';
			const yesterday = new Date(today);
			yesterday.setDate(yesterday.getDate() - 1);
			if (date.toDateString() === yesterday.toDateString()) return 'Yesterday';
			return date.toLocaleDateString([], { month: 'short', day: 'numeric', year: 'numeric' });
		} catch {
			return '';
		}
	}

	function shouldShowDateSeparator(idx: number): boolean {
		if (idx === 0) return true;
		const curr = messages[idx];
		const prev = messages[idx - 1];
		if (!curr.created_at || !prev.created_at) return false;
		return formatDate(curr.created_at) !== formatDate(prev.created_at);
	}

	function shouldShowAuthor(idx: number): boolean {
		if (idx === 0) return true;
		const curr = messages[idx];
		const prev = messages[idx - 1];
		return curr.user_id !== prev.user_id || shouldShowDateSeparator(idx);
	}

	async function toggleReaction(msg: Message, emoji: string) {
		const currentUserId = $auth.userId;
		const existing = msg.reactions?.find((r) => r.name === emoji);
		if (existing && currentUserId && existing.users.includes(currentUserId)) {
			await reactionsRemove(emoji, msg.id);
		} else {
			await reactionsAdd(emoji, msg.id);
		}
	}

	onMount(() => {
		tick().then(scrollToBottom);
	});
</script>

<div
	class="flex-1 overflow-y-auto px-4 py-2"
	bind:this={scrollContainer}
	onscroll={handleScroll}
>
	{#if loading && messages.length === 0}
		<div class="flex h-full items-center justify-center text-gray-500">Loading messages...</div>
	{:else if messages.length === 0}
		<div class="flex h-full items-center justify-center text-gray-500">
			No messages yet. Start the conversation!
		</div>
	{:else}
		{#if loading && hasMore}
			<div class="py-2 text-center text-sm text-gray-500">Loading older messages...</div>
		{/if}

		{#each messages as msg, idx}
			{#if shouldShowDateSeparator(idx)}
				<div class="my-4 flex items-center">
					<div class="flex-1 border-t border-gray-700"></div>
					<span class="px-3 text-xs text-gray-500">{formatDate(msg.created_at)}</span>
					<div class="flex-1 border-t border-gray-700"></div>
				</div>
			{/if}

			<div class="group relative flex gap-3 px-1 py-0.5 hover:bg-gray-800/50 {shouldShowAuthor(idx) ? 'mt-3' : ''}">
				{#if shouldShowAuthor(idx)}
					<!-- Avatar -->
					<div class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-lg bg-blue-600 text-sm font-bold text-white">
						{getUserAvatar(msg.user_id)}
					</div>
				{:else}
					<!-- Timestamp on hover (aligned with avatar) -->
					<div class="flex w-9 flex-shrink-0 items-center justify-center">
						<span class="hidden text-xs text-gray-600 group-hover:inline">{formatTime(msg.created_at)}</span>
					</div>
				{/if}

				<div class="min-w-0 flex-1">
					{#if shouldShowAuthor(idx)}
						<div class="flex items-baseline gap-2">
							<span class="text-sm font-bold text-gray-200">{getUserName(msg.user_id)}</span>
							<span class="text-xs text-gray-600">{formatTime(msg.created_at)}</span>
							{#if msg.edited_at}
								<span class="text-xs text-gray-600">(edited)</span>
							{/if}
						</div>
					{/if}

					<div class="text-sm leading-relaxed text-gray-300 break-words">{msg.text}</div>

					<!-- Reactions -->
					{#if msg.reactions && msg.reactions.length > 0}
						<div class="mt-1 flex flex-wrap gap-1">
							{#each msg.reactions as reaction}
								<button
									onclick={() => toggleReaction(msg, reaction.name)}
									class="inline-flex items-center gap-1 rounded-full border border-gray-700 bg-gray-800 px-2 py-0.5 text-xs transition hover:border-blue-500"
								>
									<span>{reaction.name}</span>
									<span class="text-gray-400">{reaction.count}</span>
								</button>
							{/each}
						</div>
					{/if}

					<!-- Thread indicator -->
					{#if msg.reply_count && msg.reply_count > 0}
						<button
							onclick={() => onOpenThread?.(msg)}
							class="mt-1 text-xs text-blue-400 hover:underline"
						>
							{msg.reply_count} {msg.reply_count === 1 ? 'reply' : 'replies'}
						</button>
					{/if}
				</div>

				<!-- Message actions (hover) -->
				<div class="absolute -top-3 right-2 hidden gap-1 rounded border border-gray-700 bg-gray-800 p-0.5 shadow group-hover:flex">
					<button
						onclick={() => toggleReaction(msg, '+1')}
						class="rounded p-1 text-gray-500 hover:bg-gray-700 hover:text-gray-300"
						title="React"
					>
						<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.828 14.828a4 4 0 01-5.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
						</svg>
					</button>
					{#if onOpenThread}
						<button
							onclick={() => onOpenThread?.(msg)}
							class="rounded p-1 text-gray-500 hover:bg-gray-700 hover:text-gray-300"
							title="Reply in thread"
						>
							<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
							</svg>
						</button>
					{/if}
				</div>
			</div>
		{/each}
	{/if}
</div>
```

- [x] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [x] **Step 3: Commit**

```bash
git add teidelum/ui/src/lib/components/MessageList.svelte
git commit -m "feat(chat-ui): add MessageList component with infinite scroll, reactions, and thread indicators"
```

---

### Task 16: MessageInput component

**Files:**
- Create: `teidelum/ui/src/lib/components/MessageInput.svelte`

- [x] **Step 1: Implement message input with send on Enter and typing indicator**

Create `src/lib/components/MessageInput.svelte`:

```svelte
<script lang="ts">
	import { sendTyping } from '$lib/ws';
	import { sendMessage } from '$lib/stores/messages';
	import type { Id } from '$lib/types';

	interface Props {
		channelId: Id;
		threadTs?: Id;
		placeholder?: string;
	}

	let { channelId, threadTs, placeholder = 'Type a message...' }: Props = $props();

	let text = $state('');
	let textarea: HTMLTextAreaElement | undefined = $state();
	let lastTypingSent = $state(0);

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSend();
		}
	}

	function handleInput() {
		// Auto-resize textarea
		if (textarea) {
			textarea.style.height = 'auto';
			textarea.style.height = Math.min(textarea.scrollHeight, 200) + 'px';
		}

		// Send typing indicator (throttled to once per 3 seconds)
		const now = Date.now();
		if (now - lastTypingSent > 3000) {
			sendTyping(channelId);
			lastTypingSent = now;
		}
	}

	async function handleSend() {
		const trimmed = text.trim();
		if (!trimmed) return;

		text = '';
		if (textarea) {
			textarea.style.height = 'auto';
		}

		await sendMessage(channelId, trimmed, threadTs);
	}
</script>

<div class="border-t border-gray-700 px-4 py-3">
	<div class="flex items-end gap-2 rounded-lg bg-gray-700 px-3 py-2">
		<textarea
			bind:this={textarea}
			bind:value={text}
			onkeydown={handleKeydown}
			oninput={handleInput}
			{placeholder}
			rows="1"
			class="max-h-[200px] flex-1 resize-none bg-transparent text-sm text-white placeholder-gray-500 focus:outline-none"
		></textarea>

		<button
			onclick={handleSend}
			disabled={!text.trim()}
			class="flex-shrink-0 rounded p-1 text-gray-500 transition hover:text-blue-400 disabled:opacity-30"
			title="Send message"
		>
			<svg class="h-5 w-5" fill="currentColor" viewBox="0 0 24 24">
				<path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z" />
			</svg>
		</button>
	</div>
</div>
```

- [x] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [x] **Step 3: Commit**

```bash
git add teidelum/ui/src/lib/components/MessageInput.svelte
git commit -m "feat(chat-ui): add MessageInput component with typing indicator and auto-resize"
```

---

### Task 17: Channel page

**Files:**
- Create: `teidelum/ui/src/routes/(app)/[channel]/+page.svelte`

- [x] **Step 1: Implement channel page combining MessageList and MessageInput**

Create `src/routes/(app)/[channel]/+page.svelte`:

```svelte
<script lang="ts">
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import MessageList from '$lib/components/MessageList.svelte';
	import MessageInput from '$lib/components/MessageInput.svelte';
	import ThreadPanel from '$lib/components/ThreadPanel.svelte';
	import { setActiveChannel, activeChannel } from '$lib/stores/channels';
	import { markRead } from '$lib/stores/unreads';
	import { conversationsJoin } from '$lib/api';
	import type { Message } from '$lib/types';

	const channelId = $derived(page.params.channel);

	let threadMessage = $state<Message | null>(null);

	$effect(() => {
		if (channelId) {
			setActiveChannel(channelId);
			markRead(channelId);
		}
	});

	function openThread(msg: Message) {
		threadMessage = msg;
	}

	function closeThread() {
		threadMessage = null;
	}
</script>

<svelte:head>
	<title>{$activeChannel ? `#${$activeChannel.name}` : 'Teide Chat'} - Teide Chat</title>
</svelte:head>

<div class="flex flex-1 overflow-hidden">
	<!-- Main message area -->
	<div class="flex flex-1 flex-col overflow-hidden">
		<!-- Channel header -->
		<div class="flex items-center border-b border-gray-700 px-4 py-3">
			<div>
				<h2 class="text-lg font-bold text-white">
					{#if $activeChannel}
						{#if $activeChannel.kind === 'dm'}
							{$activeChannel.name}
						{:else}
							<span class="text-gray-500">#</span> {$activeChannel.name}
						{/if}
					{:else}
						Loading...
					{/if}
				</h2>
				{#if $activeChannel?.topic}
					<p class="text-xs text-gray-500">{$activeChannel.topic}</p>
				{/if}
			</div>
		</div>

		<!-- Messages -->
		<MessageList {channelId} onOpenThread={openThread} />

		<!-- Input -->
		<MessageInput
			{channelId}
			placeholder={$activeChannel ? `Message #${$activeChannel.name}` : 'Type a message...'}
		/>
	</div>

	<!-- Thread panel -->
	{#if threadMessage}
		<div class="w-96 flex-shrink-0 border-l border-gray-700">
			<ThreadPanel
				{channelId}
				parentMessage={threadMessage}
				onClose={closeThread}
			/>
		</div>
	{/if}
</div>
```

- [x] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [x] **Step 3: Commit**

```bash
git add teidelum/ui/src/routes/\(app\)/\[channel\]/+page.svelte
git commit -m "feat(chat-ui): add channel page with message list, input, and thread panel"
```

---

## Chunk 5: Features

### Task 18: ThreadPanel component

**Files:**
- Create: `teidelum/ui/src/lib/components/ThreadPanel.svelte`

- [ ] **Step 1: Implement thread panel**

Create `src/lib/components/ThreadPanel.svelte`:

```svelte
<script lang="ts">
	import { onMount } from 'svelte';
	import * as api from '$lib/api';
	import { users } from '$lib/stores/users';
	import { sendMessage } from '$lib/stores/messages';
	import { sendTyping } from '$lib/ws';
	import type { Message, Id } from '$lib/types';

	interface Props {
		channelId: Id;
		parentMessage: Message;
		onClose: () => void;
	}

	let { channelId, parentMessage, onClose }: Props = $props();

	let replies = $state<Message[]>([]);
	let loading = $state(true);
	let replyText = $state('');
	let lastTypingSent = $state(0);

	$effect(() => {
		// Reload replies when parent message changes
		parentMessage.id; // track
		loadReplies();
	});

	async function loadReplies() {
		loading = true;
		const res = await api.conversationsReplies(channelId, parentMessage.id);
		if (res.ok && res.messages) {
			// First message is the parent; rest are replies
			replies = res.messages.slice(1);
		}
		loading = false;
	}

	function getUserName(userId: Id): string {
		const user = $users.get(userId);
		return user?.display_name || user?.username || userId;
	}

	function getUserAvatar(userId: Id): string {
		const user = $users.get(userId);
		return user?.display_name?.[0]?.toUpperCase() || user?.username?.[0]?.toUpperCase() || '?';
	}

	function formatTime(timestamp: string): string {
		try {
			const date = new Date(timestamp);
			return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
		} catch {
			return '';
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSendReply();
		}
	}

	function handleInput() {
		const now = Date.now();
		if (now - lastTypingSent > 3000) {
			sendTyping(channelId);
			lastTypingSent = now;
		}
	}

	async function handleSendReply() {
		const trimmed = replyText.trim();
		if (!trimmed) return;

		replyText = '';
		await sendMessage(channelId, trimmed, parentMessage.id);
		// Reload replies to show the new one
		await loadReplies();
	}
</script>

<div class="flex h-full flex-col">
	<!-- Thread header -->
	<div class="flex items-center justify-between border-b border-gray-700 px-4 py-3">
		<h3 class="font-bold text-white">Thread</h3>
		<button onclick={onClose} class="text-gray-500 hover:text-gray-300">
			<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
			</svg>
		</button>
	</div>

	<!-- Parent message -->
	<div class="border-b border-gray-700 px-4 py-3">
		<div class="flex gap-3">
			<div class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-lg bg-blue-600 text-sm font-bold text-white">
				{getUserAvatar(parentMessage.user_id)}
			</div>
			<div>
				<div class="flex items-baseline gap-2">
					<span class="text-sm font-bold text-gray-200">{getUserName(parentMessage.user_id)}</span>
					<span class="text-xs text-gray-600">{formatTime(parentMessage.created_at)}</span>
				</div>
				<div class="text-sm text-gray-300">{parentMessage.text}</div>
			</div>
		</div>
	</div>

	<!-- Replies -->
	<div class="flex-1 overflow-y-auto px-4 py-2">
		{#if loading}
			<div class="py-4 text-center text-sm text-gray-500">Loading replies...</div>
		{:else if replies.length === 0}
			<div class="py-4 text-center text-sm text-gray-500">No replies yet</div>
		{:else}
			{#each replies as reply}
				<div class="flex gap-3 py-2">
					<div class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-lg bg-blue-600 text-xs font-bold text-white">
						{getUserAvatar(reply.user_id)}
					</div>
					<div>
						<div class="flex items-baseline gap-2">
							<span class="text-sm font-bold text-gray-200">{getUserName(reply.user_id)}</span>
							<span class="text-xs text-gray-600">{formatTime(reply.created_at)}</span>
						</div>
						<div class="text-sm text-gray-300">{reply.text}</div>
					</div>
				</div>
			{/each}
		{/if}
	</div>

	<!-- Reply input -->
	<div class="border-t border-gray-700 px-4 py-3">
		<div class="flex items-end gap-2 rounded-lg bg-gray-700 px-3 py-2">
			<textarea
				bind:value={replyText}
				onkeydown={handleKeydown}
				oninput={handleInput}
				placeholder="Reply..."
				rows="1"
				class="max-h-[120px] flex-1 resize-none bg-transparent text-sm text-white placeholder-gray-500 focus:outline-none"
			></textarea>
			<button
				onclick={handleSendReply}
				disabled={!replyText.trim()}
				class="flex-shrink-0 rounded p-1 text-gray-500 transition hover:text-blue-400 disabled:opacity-30"
			>
				<svg class="h-5 w-5" fill="currentColor" viewBox="0 0 24 24">
					<path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z" />
				</svg>
			</button>
		</div>
	</div>
</div>
```

- [ ] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [ ] **Step 3: Commit**

```bash
git add teidelum/ui/src/lib/components/ThreadPanel.svelte
git commit -m "feat(chat-ui): add ThreadPanel component for threaded replies"
```

---

### Task 19: ReactionPicker component

**Files:**
- Create: `teidelum/ui/src/lib/components/ReactionPicker.svelte`

- [ ] **Step 1: Implement simple reaction picker**

Create `src/lib/components/ReactionPicker.svelte`. A minimal picker with common reactions (no full emoji picker for MVP):

```svelte
<script lang="ts">
	interface Props {
		onSelect: (emoji: string) => void;
		onClose: () => void;
	}

	let { onSelect, onClose }: Props = $props();

	const commonReactions = [
		'+1', '-1', 'heart', 'laughing', 'eyes',
		'tada', 'fire', 'rocket', '100', 'thinking'
	];

	const emojiMap: Record<string, string> = {
		'+1': '\u{1F44D}',
		'-1': '\u{1F44E}',
		'heart': '\u{2764}\u{FE0F}',
		'laughing': '\u{1F606}',
		'eyes': '\u{1F440}',
		'tada': '\u{1F389}',
		'fire': '\u{1F525}',
		'rocket': '\u{1F680}',
		'100': '\u{1F4AF}',
		'thinking': '\u{1F914}'
	};

	function handleSelect(name: string) {
		onSelect(name);
		onClose();
	}
</script>

<div class="rounded-lg border border-gray-700 bg-gray-800 p-2 shadow-xl">
	<div class="grid grid-cols-5 gap-1">
		{#each commonReactions as name}
			<button
				onclick={() => handleSelect(name)}
				class="rounded p-1.5 text-lg transition hover:bg-gray-700"
				title={name}
			>
				{emojiMap[name] ?? name}
			</button>
		{/each}
	</div>
</div>
```

- [ ] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [ ] **Step 3: Commit**

```bash
git add teidelum/ui/src/lib/components/ReactionPicker.svelte
git commit -m "feat(chat-ui): add ReactionPicker component with common emoji reactions"
```

---

### Task 20: SearchModal component

**Files:**
- Create: `teidelum/ui/src/lib/components/SearchModal.svelte`

- [ ] **Step 1: Implement search modal**

Create `src/lib/components/SearchModal.svelte`:

```svelte
<script lang="ts">
	import { goto } from '$app/navigation';
	import * as api from '$lib/api';
	import { users } from '$lib/stores/users';
	import type { Message, Id } from '$lib/types';

	interface Props {
		onClose: () => void;
	}

	let { onClose }: Props = $props();

	let query = $state('');
	let results = $state<Message[]>([]);
	let loading = $state(false);
	let searchTimeout: ReturnType<typeof setTimeout> | null = null;

	function handleInput() {
		if (searchTimeout) clearTimeout(searchTimeout);
		searchTimeout = setTimeout(doSearch, 300);
	}

	async function doSearch() {
		const q = query.trim();
		if (!q) {
			results = [];
			return;
		}

		loading = true;
		const res = await api.searchMessages(q, undefined, 20);
		if (res.ok && res.messages) {
			results = res.messages;
		}
		loading = false;
	}

	function getUserName(userId: Id): string {
		const user = $users.get(userId);
		return user?.display_name || user?.username || userId;
	}

	function formatTime(timestamp: string): string {
		try {
			const date = new Date(timestamp);
			return date.toLocaleDateString([], { month: 'short', day: 'numeric' }) +
				' ' + date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
		} catch {
			return '';
		}
	}

	function navigateToMessage(msg: Message) {
		goto(`/${msg.channel_id}`);
		onClose();
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			onClose();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="fixed inset-0 z-50 flex items-start justify-center bg-black/60 pt-20">
	<div class="w-full max-w-2xl rounded-lg bg-gray-800 shadow-2xl">
		<!-- Search input -->
		<div class="border-b border-gray-700 p-4">
			<div class="flex items-center gap-3">
				<svg class="h-5 w-5 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
				</svg>
				<input
					type="text"
					bind:value={query}
					oninput={handleInput}
					placeholder="Search messages..."
					class="flex-1 bg-transparent text-white placeholder-gray-500 focus:outline-none"
					autofocus
				/>
				<button onclick={onClose} class="text-gray-500 hover:text-gray-300">
					<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
					</svg>
				</button>
			</div>
		</div>

		<!-- Results -->
		<div class="max-h-96 overflow-y-auto">
			{#if loading}
				<div class="p-4 text-center text-sm text-gray-500">Searching...</div>
			{:else if query.trim() && results.length === 0}
				<div class="p-4 text-center text-sm text-gray-500">No results found</div>
			{:else}
				{#each results as msg}
					<button
						onclick={() => navigateToMessage(msg)}
						class="flex w-full gap-3 border-b border-gray-700/50 px-4 py-3 text-left transition hover:bg-gray-700/50"
					>
						<div class="min-w-0 flex-1">
							<div class="flex items-baseline gap-2">
								<span class="text-sm font-bold text-gray-300">{getUserName(msg.user_id)}</span>
								<span class="text-xs text-gray-600">{formatTime(msg.created_at)}</span>
							</div>
							<div class="truncate text-sm text-gray-400">{msg.text}</div>
						</div>
					</button>
				{/each}
			{/if}
		</div>
	</div>
</div>
```

- [ ] **Step 2: Add search button to Sidebar**

Update `src/lib/components/Sidebar.svelte` to include a search trigger. Add a search button in the header area (next to the "Sign out" button). When clicked, it should emit an event or use a shared state. For simplicity, add a global search modal toggle.

Actually, add the search button and modal to the app layout instead. Update `src/routes/(app)/+layout.svelte` to include the SearchModal with a keyboard shortcut (Ctrl+K / Cmd+K):

Add to the `<script>` section:

```ts
import SearchModal from '$lib/components/SearchModal.svelte';

let showSearch = $state(false);

function handleGlobalKeydown(e: KeyboardEvent) {
	if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
		e.preventDefault();
		showSearch = !showSearch;
	}
}
```

Add to the template:

```svelte
<svelte:window onkeydown={handleGlobalKeydown} />

{#if showSearch}
	<SearchModal onClose={() => (showSearch = false)} />
{/if}
```

The full updated `src/routes/(app)/+layout.svelte`:

```svelte
<script lang="ts">
	import { onMount } from 'svelte';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import SearchModal from '$lib/components/SearchModal.svelte';
	import { loadChannels, initChannelWsListeners } from '$lib/stores/channels';
	import { loadUsers, initUserWsListeners } from '$lib/stores/users';
	import { initMessageWsListeners } from '$lib/stores/messages';
	import { initUnreadsWsListeners } from '$lib/stores/unreads';

	let { children } = $props();
	let showSearch = $state(false);

	onMount(async () => {
		await Promise.all([loadChannels(), loadUsers()]);
		initChannelWsListeners();
		initUserWsListeners();
		initMessageWsListeners();
		initUnreadsWsListeners();
	});

	function handleGlobalKeydown(e: KeyboardEvent) {
		if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
			e.preventDefault();
			showSearch = !showSearch;
		}
	}
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<div class="flex h-screen overflow-hidden bg-gray-900">
	<!-- Sidebar -->
	<div class="flex w-64 flex-shrink-0 flex-col border-r border-gray-700 bg-gray-800">
		<Sidebar />
	</div>

	<!-- Main content area -->
	<div class="flex flex-1 overflow-hidden">
		{@render children()}
	</div>
</div>

{#if showSearch}
	<SearchModal onClose={() => (showSearch = false)} />
{/if}
```

- [ ] **Step 3: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [ ] **Step 4: Commit**

```bash
git add teidelum/ui/src/lib/components/SearchModal.svelte teidelum/ui/src/routes/\(app\)/+layout.svelte
git commit -m "feat(chat-ui): add SearchModal component with Cmd+K shortcut"
```

---

### Task 21: UserPresence component

**Files:**
- Create: `teidelum/ui/src/lib/components/UserPresence.svelte`

- [ ] **Step 1: Implement presence indicator**

Create `src/lib/components/UserPresence.svelte`:

```svelte
<script lang="ts">
	import { presence } from '$lib/stores/users';
	import type { Id } from '$lib/types';

	interface Props {
		userId: Id;
		size?: 'sm' | 'md';
	}

	let { userId, size = 'sm' }: Props = $props();

	const userPresence = $derived($presence.get(userId) ?? 'away');
	const isActive = $derived(userPresence === 'active');

	const sizeClasses = $derived(
		size === 'sm' ? 'h-2.5 w-2.5' : 'h-3 w-3'
	);
</script>

<span
	class="inline-block rounded-full {sizeClasses} {isActive ? 'bg-green-500' : 'border-2 border-gray-500 bg-transparent'}"
	title={isActive ? 'Active' : 'Away'}
></span>
```

- [ ] **Step 2: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [ ] **Step 3: Commit**

```bash
git add teidelum/ui/src/lib/components/UserPresence.svelte
git commit -m "feat(chat-ui): add UserPresence component with active/away indicator"
```

---

### Task 22: FileUpload component

**Files:**
- Create: `teidelum/ui/src/lib/components/FileUpload.svelte`

- [ ] **Step 1: Implement file upload button**

Create `src/lib/components/FileUpload.svelte`:

```svelte
<script lang="ts">
	import * as api from '$lib/api';
	import type { Id } from '$lib/types';

	interface Props {
		channelId: Id;
		threadTs?: Id;
	}

	let { channelId, threadTs }: Props = $props();

	let fileInput: HTMLInputElement | undefined = $state();
	let uploading = $state(false);

	function triggerUpload() {
		fileInput?.click();
	}

	async function handleFileSelect(e: Event) {
		const input = e.target as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;

		uploading = true;
		try {
			await api.filesUpload(channelId, file, threadTs);
		} catch (err) {
			console.error('File upload failed:', err);
		}
		uploading = false;

		// Reset input
		if (fileInput) fileInput.value = '';
	}
</script>

<input
	bind:this={fileInput}
	type="file"
	class="hidden"
	onchange={handleFileSelect}
/>

<button
	onclick={triggerUpload}
	disabled={uploading}
	class="rounded p-1 text-gray-500 transition hover:text-gray-300 disabled:opacity-50"
	title="Upload file"
>
	{#if uploading}
		<svg class="h-5 w-5 animate-spin" fill="none" stroke="currentColor" viewBox="0 0 24 24">
			<circle cx="12" cy="12" r="10" stroke-width="2" class="opacity-25" />
			<path stroke-width="2" d="M4 12a8 8 0 018-8" class="opacity-75" />
		</svg>
	{:else}
		<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
			<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13" />
		</svg>
	{/if}
</button>
```

- [ ] **Step 2: Add FileUpload to MessageInput**

Update `src/lib/components/MessageInput.svelte` to include the FileUpload button. Add the import and place the component before the textarea in the input area:

```svelte
<script lang="ts">
	import FileUpload from './FileUpload.svelte';
	// ... rest of existing script
</script>
```

In the template, add `<FileUpload {channelId} {threadTs} />` before the textarea inside the input bar:

```svelte
<div class="flex items-end gap-2 rounded-lg bg-gray-700 px-3 py-2">
	<FileUpload {channelId} {threadTs} />
	<textarea ...></textarea>
	<button ...>...</button>
</div>
```

The full updated `src/lib/components/MessageInput.svelte`:

```svelte
<script lang="ts">
	import { sendTyping } from '$lib/ws';
	import { sendMessage } from '$lib/stores/messages';
	import FileUpload from './FileUpload.svelte';
	import type { Id } from '$lib/types';

	interface Props {
		channelId: Id;
		threadTs?: Id;
		placeholder?: string;
	}

	let { channelId, threadTs, placeholder = 'Type a message...' }: Props = $props();

	let text = $state('');
	let textarea: HTMLTextAreaElement | undefined = $state();
	let lastTypingSent = $state(0);

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSend();
		}
	}

	function handleInput() {
		if (textarea) {
			textarea.style.height = 'auto';
			textarea.style.height = Math.min(textarea.scrollHeight, 200) + 'px';
		}

		const now = Date.now();
		if (now - lastTypingSent > 3000) {
			sendTyping(channelId);
			lastTypingSent = now;
		}
	}

	async function handleSend() {
		const trimmed = text.trim();
		if (!trimmed) return;

		text = '';
		if (textarea) {
			textarea.style.height = 'auto';
		}

		await sendMessage(channelId, trimmed, threadTs);
	}
</script>

<div class="border-t border-gray-700 px-4 py-3">
	<div class="flex items-end gap-2 rounded-lg bg-gray-700 px-3 py-2">
		<FileUpload {channelId} {threadTs} />

		<textarea
			bind:this={textarea}
			bind:value={text}
			onkeydown={handleKeydown}
			oninput={handleInput}
			{placeholder}
			rows="1"
			class="max-h-[200px] flex-1 resize-none bg-transparent text-sm text-white placeholder-gray-500 focus:outline-none"
		></textarea>

		<button
			onclick={handleSend}
			disabled={!text.trim()}
			class="flex-shrink-0 rounded p-1 text-gray-500 transition hover:text-blue-400 disabled:opacity-30"
			title="Send message"
		>
			<svg class="h-5 w-5" fill="currentColor" viewBox="0 0 24 24">
				<path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z" />
			</svg>
		</button>
	</div>
</div>
```

- [ ] **Step 3: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [ ] **Step 4: Commit**

```bash
git add teidelum/ui/src/lib/components/FileUpload.svelte teidelum/ui/src/lib/components/MessageInput.svelte
git commit -m "feat(chat-ui): add FileUpload component and integrate into MessageInput"
```

---

## Chunk 6: Polish

### Task 23: Markdown rendering in messages

**Files:**
- Modify: `teidelum/ui/src/lib/components/MessageList.svelte`

- [ ] **Step 1: Add markdown rendering utility**

Create `src/lib/markdown.ts`:

```ts
import { marked } from 'marked';
import DOMPurify from 'dompurify';

// Configure marked for chat messages
marked.setOptions({
	breaks: true, // Convert \n to <br>
	gfm: true    // GitHub-flavored markdown
});

/** Render markdown text to sanitized HTML */
export function renderMarkdown(text: string): string {
	const html = marked.parse(text, { async: false }) as string;
	return DOMPurify.sanitize(html, {
		ALLOWED_TAGS: [
			'p', 'br', 'strong', 'em', 'del', 'code', 'pre',
			'a', 'ul', 'ol', 'li', 'blockquote', 'h1', 'h2', 'h3'
		],
		ALLOWED_ATTR: ['href', 'target', 'rel']
	});
}
```

- [ ] **Step 2: Update MessageList to use markdown rendering**

In `src/lib/components/MessageList.svelte`, replace the plain text display `{msg.text}` with rendered markdown. Import the utility:

```ts
import { renderMarkdown } from '$lib/markdown';
```

Replace the message text line:

```svelte
<!-- Before -->
<div class="text-sm leading-relaxed text-gray-300 break-words">{msg.text}</div>

<!-- After -->
<div class="prose-chat text-sm leading-relaxed text-gray-300 break-words">{@html renderMarkdown(msg.text)}</div>
```

- [ ] **Step 3: Add prose-chat styles to app.css**

Add to `src/app.css`:

```css
/* Markdown styles for chat messages */
.prose-chat p { margin: 0; }
.prose-chat p + p { margin-top: 0.25rem; }
.prose-chat code {
	@apply rounded bg-gray-700 px-1 py-0.5 text-xs text-pink-300;
}
.prose-chat pre {
	@apply my-1 overflow-x-auto rounded bg-gray-800 p-2;
}
.prose-chat pre code {
	@apply bg-transparent p-0 text-gray-300;
}
.prose-chat a {
	@apply text-blue-400 underline;
}
.prose-chat blockquote {
	@apply my-1 border-l-4 border-gray-600 pl-3 text-gray-400;
}
.prose-chat ul, .prose-chat ol {
	@apply my-1 pl-5;
}
.prose-chat li {
	@apply my-0.5;
}
```

- [ ] **Step 4: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [ ] **Step 5: Commit**

```bash
git add teidelum/ui/src/lib/markdown.ts teidelum/ui/src/lib/components/MessageList.svelte teidelum/ui/src/app.css
git commit -m "feat(chat-ui): add markdown rendering for messages with sanitization"
```

---

### Task 24: Mention highlighting (@username)

**Files:**
- Modify: `teidelum/ui/src/lib/markdown.ts`

- [ ] **Step 1: Add mention highlighting to markdown processing**

Update `src/lib/markdown.ts` to detect `@username` patterns and wrap them in a highlight span before markdown rendering:

```ts
import { marked } from 'marked';
import DOMPurify from 'dompurify';

marked.setOptions({
	breaks: true,
	gfm: true
});

/** Highlight @mentions before markdown rendering */
function highlightMentions(text: string): string {
	return text.replace(
		/@(\w+)/g,
		'<span class="mention">@$1</span>'
	);
}

/** Render markdown text to sanitized HTML */
export function renderMarkdown(text: string): string {
	const withMentions = highlightMentions(text);
	const html = marked.parse(withMentions, { async: false }) as string;
	return DOMPurify.sanitize(html, {
		ALLOWED_TAGS: [
			'p', 'br', 'strong', 'em', 'del', 'code', 'pre',
			'a', 'ul', 'ol', 'li', 'blockquote', 'h1', 'h2', 'h3',
			'span'
		],
		ALLOWED_ATTR: ['href', 'target', 'rel', 'class']
	});
}
```

- [ ] **Step 2: Add mention styles to app.css**

Add to `src/app.css`:

```css
.mention {
	@apply rounded bg-blue-500/20 px-0.5 text-blue-300;
}
```

- [ ] **Step 3: Verify build**

Run: `cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run build`

- [ ] **Step 4: Commit**

```bash
git add teidelum/ui/src/lib/markdown.ts teidelum/ui/src/app.css
git commit -m "feat(chat-ui): add @mention highlighting in messages"
```

---

### Task 25: Final integration test

- [ ] **Step 1: Verify complete build**

```bash
cd /Users/antonkundenko/data/work/teidedb/teidelum/ui
npm run build
```

Expected: no errors, clean build.

- [ ] **Step 2: Manual smoke test checklist**

Start the backend and frontend:

```bash
# Terminal 1: Backend
cd /Users/antonkundenko/data/work/teidedb/teidelum && cargo run

# Terminal 2: Frontend
cd /Users/antonkundenko/data/work/teidedb/teidelum/ui && npm run dev
```

Open `http://localhost:5173` in browser and verify:

1. Redirected to `/login` when not authenticated
2. Can register a new account at `/register`
3. Can log in with registered credentials
4. After login, redirected to app with sidebar visible
5. Can create a new channel via the "+" button in sidebar
6. Can send a message in the channel (Enter to send)
7. Messages appear in the message list
8. Can open a thread by clicking reply count or thread icon
9. Can reply in a thread
10. Can add a reaction to a message (hover actions)
11. Search works via Cmd+K / Ctrl+K
12. Typing indicator fires (check network/WS tab)
13. File upload button opens file picker
14. Markdown renders in messages (try `**bold**`, `` `code` ``, etc.)
15. @mentions are highlighted
16. Unread badges appear when messages arrive in other channels
17. Sign out returns to login page
18. Page refresh preserves auth (token in localStorage)

- [ ] **Step 3: Final commit**

```bash
git add -A teidelum/ui/
git commit -m "feat(chat-ui): complete Teide Chat SvelteKit frontend"
```
