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
	unread_count?: number;
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
	last_reply_ts?: string;
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
