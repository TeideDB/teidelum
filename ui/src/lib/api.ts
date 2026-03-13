import type {
	AuthResponse,
	Channel,
	ChannelListResponse,
	ChannelResponse,
	FileUploadResponse,
	HistoryResponse,
	Id,
	MembersResponse,
	Message,
	MessageResponse,
	OkResponse,
	SearchResponse,
	UserInfoResponse,
	UserSettingsResponse,
	UsersListResponse
} from './types';

/**
 * Map a backend message object (ts/channel/user) to the frontend Message shape (id/channel_id/user_id).
 */
// eslint-disable-next-line @typescript-eslint/no-explicit-any
function mapMessage(raw: any): Message {
	return {
		id: String(raw.ts ?? raw.id ?? ''),
		ts: String(raw.ts ?? raw.id ?? ''),
		channel_id: String(raw.channel ?? raw.channel_id ?? ''),
		user_id: String(raw.user ?? raw.user_id ?? ''),
		user: raw.username ?? raw.user_name,
		text: raw.text ?? '',
		thread_ts: raw.thread_ts && String(raw.thread_ts) !== '0' ? String(raw.thread_ts) : undefined,
		reply_count: raw.reply_count,
		last_reply_ts: raw.last_reply_ts,
		reactions: raw.reactions,
		files: raw.files,
		edited_at: raw.edited_ts ?? raw.edited_at,
		created_at: raw.created_at ?? new Date().toISOString()
	};
}

let token: string | null = null;

export function setToken(t: string | null) {
	token = t;
}

function getBaseUrl(): string {
	// In Tauri, use configured server URL; in browser, use relative paths
	if (typeof window !== 'undefined' && '__TAURI__' in window) {
		return localStorage.getItem('teidelum_server_url') || 'http://localhost:3000';
	}
	return '';
}

/** Callback invoked when an API call returns 401 (token expired/invalid). */
let onAuthExpired: (() => void) | null = null;
export function setOnAuthExpired(cb: () => void) {
	onAuthExpired = cb;
}

async function call<T>(method: string, body: Record<string, unknown> = {}): Promise<T> {
	const headers: Record<string, string> = { 'Content-Type': 'application/json' };
	if (token) headers['Authorization'] = `Bearer ${token}`;

	const res = await fetch(`${getBaseUrl()}/api/slack/${method}`, {
		method: 'POST',
		headers,
		body: JSON.stringify(body)
	});

	if (res.status === 401 && onAuthExpired) {
		onAuthExpired();
	}

	if (!res.ok) {
		throw new Error(`API ${method}: HTTP ${res.status}`);
	}

	return res.json();
}

// === Auth ===

export function register(username: string, password: string, email: string, display_name?: string): Promise<AuthResponse> {
	const body: Record<string, string> = { username, password, email };
	if (display_name) body.display_name = display_name;
	return call('auth.register', body);
}

export function login(username: string, password: string): Promise<AuthResponse> {
	return call('auth.login', { username, password });
}

export function refreshToken(): Promise<AuthResponse> {
	return call('auth.refresh', {});
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

export async function conversationsHistory(
	channel: Id,
	limit?: number,
	before?: Id
): Promise<HistoryResponse> {
	const body: Record<string, unknown> = { channel };
	if (limit !== undefined) body.limit = limit;
	if (before !== undefined) body.before = before;
	const res = await call<HistoryResponse>('conversations.history', body);
	if (res.ok && res.messages) {
		res.messages = res.messages.map(mapMessage);
	}
	return res;
}

export async function conversationsReplies(channel: Id, ts: Id): Promise<HistoryResponse> {
	const res = await call<HistoryResponse>('conversations.replies', { channel, ts });
	if (res.ok && res.messages) {
		res.messages = res.messages.map(mapMessage);
	}
	return res;
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

export function conversationsMarkRead(channel: Id, ts?: string): Promise<OkResponse> {
	const body: Record<string, unknown> = { channel };
	if (ts !== undefined) body.ts = ts;
	return call('conversations.markRead', body);
}

export function conversationsUpdate(
	channel: Id,
	updates: { name?: string; topic?: string; description?: string }
): Promise<OkResponse> {
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

export function conversationsMute(channel: Id): Promise<OkResponse> {
	return call('conversations.mute', { channel });
}

export function conversationsUnmute(channel: Id): Promise<OkResponse> {
	return call('conversations.unmute', { channel });
}

export function conversationsSetNotification(channel: Id, level: string): Promise<OkResponse> {
	return call('conversations.setNotification', { channel, level });
}

// === Chat ===

export async function chatPostMessage(
	channel: Id,
	text: string,
	thread_ts?: Id
): Promise<MessageResponse> {
	const body: Record<string, unknown> = { channel, text };
	if (thread_ts !== undefined) body.thread_ts = thread_ts;
	const res = await call<MessageResponse>('chat.postMessage', body);
	if (res.ok && res.message) {
		res.message = mapMessage(res.message);
	}
	return res;
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

// === Search / Autocomplete ===

export function usersSearch(
	query: string
): Promise<{
	ok: boolean;
	users?: Array<{ id: Id; username: string; display_name: string; avatar_url: string }>;
}> {
	return call('users.search', { query });
}

export function conversationsAutocomplete(
	query: string
): Promise<{ ok: boolean; channels?: Array<{ id: Id; name: string; topic: string }> }> {
	return call('conversations.autocomplete', { query });
}

// === Directory ===

export function conversationsDirectory(query?: string, limit?: number, cursor?: Id): Promise<{
	ok: boolean;
	channels?: Channel[];
}> {
	const body: Record<string, unknown> = {};
	if (query !== undefined) body.query = query;
	if (limit !== undefined) body.limit = limit;
	if (cursor !== undefined) body.cursor = cursor;
	return call('conversations.directory', body);
}

// === Reactions ===

export function reactionsAdd(name: string, timestamp: Id): Promise<OkResponse> {
	return call('reactions.add', { name, timestamp });
}

export function reactionsRemove(name: string, timestamp: Id): Promise<OkResponse> {
	return call('reactions.remove', { name, timestamp });
}

// === Search ===

export async function searchMessages(
	query: string,
	channel?: Id,
	limit?: number,
	user_id?: Id,
	date_from?: string,
	date_to?: string
): Promise<SearchResponse> {
	const body: Record<string, unknown> = { query };
	if (channel !== undefined) body.channel_id = channel;
	if (limit !== undefined) body.limit = limit;
	if (user_id !== undefined) body.user_id = user_id;
	if (date_from !== undefined) body.date_from = date_from;
	if (date_to !== undefined) body.date_to = date_to;
	// Backend returns { messages: { matches: [...], total: N } }
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	const res = await call<any>('search.messages', body);
	const matches = res?.messages?.matches ?? [];
	return {
		ok: res.ok ?? false,
		messages: matches.map(mapMessage),
		error: res.error
	};
}

// === Pins ===

export function pinsAdd(channel: Id, message_id: Id): Promise<OkResponse> {
	return call('pins.add', { channel, message_id });
}

export function pinsRemove(channel: Id, message_id: Id): Promise<OkResponse> {
	return call('pins.remove', { channel, message_id });
}

export async function pinsList(channel: Id): Promise<{ ok: boolean; pins?: Message[]; error?: string }> {
	// eslint-disable-next-line @typescript-eslint/no-explicit-any
	const res = await call<any>('pins.list', { channel });
	if (res.ok && res.items) {
		// Backend returns Slack-compatible {items: [{message: {...}, pinned_by, pinned_at}]}
		// Extract and flatten to Message[] for frontend use
		res.pins = res.items.map((item: { message: Record<string, unknown> }) => mapMessage(item.message));
		delete res.items;
	}
	return res;
}

// === Links ===

export function linksUnfurl(url: string): Promise<{
	ok: boolean;
	title?: string;
	description?: string;
	image?: string;
	site_name?: string;
	error?: string;
}> {
	return call('links.unfurl', { url });
}

// === Files ===

export function fileDownloadUrl(fileId: Id, filename: string): string {
	const url = `${getBaseUrl()}/files/${fileId}/${encodeURIComponent(filename)}`;
	return token ? `${url}?token=${encodeURIComponent(token)}` : url;
}

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

	const res = await fetch(`${getBaseUrl()}/api/slack/files.upload`, {
		method: 'POST',
		headers,
		body: formData
	});

	if (!res.ok) {
		throw new Error(`API files.upload: HTTP ${res.status}`);
	}

	return res.json();
}
