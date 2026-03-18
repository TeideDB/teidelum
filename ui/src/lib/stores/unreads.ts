import { writable, get } from 'svelte/store';
import * as ws from '$lib/ws';
import { activeChannelId } from './channels';
import { auth } from './auth';
import { showNotification } from '$lib/notifications';
import { users } from './users';
import { conversationsMarkRead } from '$lib/api';
import type { Id, WsEvent } from '$lib/types';

/** Map of channelId -> unread count */
export const unreads = writable<Map<Id, number>>(new Map());

export function resetUnreads() {
	unreads.set(new Map());
}

/**
 * Mark a channel as read locally AND on the server.
 * Call this AFTER conversations.history has completed so the server
 * records the correct last_read_ts.
 */
export function markRead(channelId: Id) {
	unreads.update((map) => {
		const newMap = new Map(map);
		newMap.delete(channelId);
		return newMap;
	});
	// Fire-and-forget server sync so the next loadChannels() sees 0 unreads
	conversationsMarkRead(channelId).catch(() => {});
}

/** Clear a channel's unread count in the local store only (no server call). */
export function clearLocalUnread(channelId: Id) {
	unreads.update((map) => {
		const newMap = new Map(map);
		newMap.delete(channelId);
		return newMap;
	});
}

/** Sync unread counts from the server channel list response into the store. */
export function syncUnreadsFromChannels(channelUnreads: Map<Id, number>) {
	unreads.set(channelUnreads);
}

export async function markAllRead() {
	const current = get(unreads);
	const channelIds = Array.from(current.keys());
	await Promise.all(channelIds.map((id) => conversationsMarkRead(id)));
	unreads.set(new Map());
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

export function initUnreadsWsListeners(): () => void {
	const unsub = ws.on('message', (event: WsEvent) => {
		// Backend sends channel as `channel` field
		const data = event as unknown as { channel?: Id; user?: Id; text?: string };
		const channelId = data.channel;
		// Don't increment unreads for own messages
		if (channelId && data.user !== get(auth).userId) {
			incrementUnread(channelId);
			// Desktop notification when tab not focused
			const senderUser = data.user
				? get(users).get(data.user)
				: undefined;
			const senderName = senderUser?.display_name || senderUser?.username || 'Someone';
			const currentUser = get(auth).user;
			const currentUsername = currentUser?.username;
			const escapedUsername = currentUsername?.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
			const isMention = !!(data.text && escapedUsername && new RegExp(`@${escapedUsername}\\b`).test(data.text));
			showNotification(senderName, data.text || 'New message', channelId, isMention);
		}
	});
	return unsub;
}
