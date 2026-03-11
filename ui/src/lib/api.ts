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
