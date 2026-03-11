import { writable, get } from 'svelte/store';
import * as ws from '$lib/ws';
import { activeChannelId } from './channels';
import { auth } from './auth';
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

export function initUnreadsWsListeners(): () => void {
	const unsub = ws.on('message', (event: WsEvent) => {
		// Backend sends channel as `channel` field
		const data = event as unknown as { channel?: Id; user?: Id };
		const channelId = data.channel;
		// Don't increment unreads for own messages
		if (channelId && data.user !== get(auth).userId) {
			incrementUnread(channelId);
		}
	});
	return unsub;
}
