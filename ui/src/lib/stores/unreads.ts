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
