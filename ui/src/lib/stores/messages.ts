import { writable, get } from 'svelte/store';
import * as api from '$lib/api';
import * as ws from '$lib/ws';
import { ensureUser } from '$lib/stores/users';
import type { Message, Id, WsEvent } from '$lib/types';

interface ChannelMessages {
	messages: Message[];
	hasMore: boolean;
	loading: boolean;
}

/** Map of channelId -> messages state */
export const messagesByChannel = writable<Map<Id, ChannelMessages>>(new Map());

export function resetMessages() {
	messagesByChannel.set(new Map());
}

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
		// Re-read current state to preserve any WS messages received during fetch
		const current = getChannelState(channelId);
		setChannelState(channelId, { ...current, loading: false });
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
		// Re-read current state to preserve any WS messages received during fetch
		const current = getChannelState(channelId);
		setChannelState(channelId, {
			messages: [...res.messages.reverse(), ...current.messages],
			hasMore: res.has_more ?? false,
			loading: false
		});
	} else {
		const current = getChannelState(channelId);
		setChannelState(channelId, { ...current, loading: false });
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

/** Initialize WebSocket listeners for real-time message updates. Returns cleanup function. */
export function initMessageWsListeners(): () => void {
	const unsubs: (() => void)[] = [];

	unsubs.push(
		ws.on('message', (event: WsEvent) => {
			// Backend sends: { type, channel, user, text, ts, thread_ts?, files? }
			// Map to Message shape for the store
			const data = event as unknown as {
				channel: Id;
				user: Id;
				text: string;
				ts: Id;
				thread_ts?: Id;
				files?: Array<{ id: string; filename: string; mime_type: string; size_bytes: number }>;
			};
			if (data.channel) {
				// Ensure the sender is in the user store (they may have registered after we loaded)
				ensureUser(data.user);
				const message: Message = {
					id: data.ts,
					channel_id: data.channel,
					user_id: data.user,
					text: data.text,
					ts: data.ts,
					created_at: new Date().toISOString(),
					thread_ts: data.thread_ts,
					files: data.files?.map((f) => ({
						id: f.id,
						filename: f.filename,
						mime_type: f.mime_type,
						size_bytes: f.size_bytes,
						url: `/files/${f.id}/${f.filename}`
					}))
				};
				appendMessage(data.channel, message);
			}
		})
	);

	unsubs.push(
		ws.on('message_changed', (event: WsEvent) => {
			// Backend sends: { type, channel, message: { user, text, ts, edited_ts } }
			const data = event as unknown as {
				message: { user: string; text: string; ts: Id; edited_ts: string };
			};
			if (data.message) {
				updateMessage(data.message.ts, (msg) => ({
					...msg,
					text: data.message.text,
					edited_at: data.message.edited_ts
				}));
			}
		})
	);

	unsubs.push(
		ws.on('message_deleted', (event: WsEvent) => {
			const data = event as unknown as { ts: Id };
			if (data.ts) {
				removeMessage(data.ts);
			}
		})
	);

	unsubs.push(
		ws.on('reaction_added', (event: WsEvent) => {
			// Backend sends flat: { type, channel, user, reaction, item_ts }
			const data = event as unknown as { item_ts: Id; reaction: string; user: Id };
			if (data.item_ts) {
				updateMessage(data.item_ts, (msg) => {
					const reactions = [...(msg.reactions || [])];
					const idx = reactions.findIndex((r) => r.name === data.reaction);
					if (idx !== -1) {
						// Guard against duplicate WS events
						if (!reactions[idx].users.includes(data.user)) {
							reactions[idx] = {
								...reactions[idx],
								count: reactions[idx].count + 1,
								users: [...reactions[idx].users, data.user]
							};
						}
					} else {
						reactions.push({ name: data.reaction, count: 1, users: [data.user] });
					}
					return { ...msg, reactions };
				});
			}
		})
	);

	unsubs.push(
		ws.on('reaction_removed', (event: WsEvent) => {
			// Backend sends flat: { type, channel, user, reaction, item_ts }
			const data = event as unknown as { item_ts: Id; reaction: string; user: Id };
			if (data.item_ts) {
				updateMessage(data.item_ts, (msg) => {
					let reactions = (msg.reactions || []).map((r) =>
						r.name === data.reaction
							? {
									...r,
									count: r.count - 1,
									users: r.users.filter((u) => u !== data.user)
								}
							: r
					);
					reactions = reactions.filter((r) => r.count > 0);
					return { ...msg, reactions };
				});
			}
		})
	);

	return () => unsubs.forEach((fn) => fn());
}
