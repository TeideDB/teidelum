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
