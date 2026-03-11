import { writable, derived, get } from 'svelte/store';
import * as api from '$lib/api';
import * as ws from '$lib/ws';
import type { Channel, Id, WsEvent } from '$lib/types';
import { unreads } from './unreads';

export const channels = writable<Channel[]>([]);
export const activeChannelId = writable<Id | null>(null);

export function resetChannels() {
	channels.set([]);
	activeChannelId.set(null);
}

export const activeChannel = derived(
	[channels, activeChannelId],
	([$channels, $activeChannelId]) => $channels.find((c) => c.id === $activeChannelId) ?? null
);

export const nonDmChannels = derived(channels, ($channels) =>
	$channels.filter((c) => c.kind === 'public' || c.kind === 'private')
);

export const dmChannels = derived(channels, ($channels) =>
	$channels.filter((c) => c.kind === 'dm')
);

export async function loadChannels() {
	const res = await api.conversationsList();
	if (res.ok && res.channels) {
		channels.set(res.channels);
		// Seed unread counts from server on initial load
		const unreadMap = new Map<Id, number>();
		for (const ch of res.channels) {
			if (ch.unread_count && ch.unread_count > 0) {
				unreadMap.set(ch.id, ch.unread_count);
			}
		}
		unreads.set(unreadMap);
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
export function initChannelWsListeners(): () => void {
	const unsubs: (() => void)[] = [];
	unsubs.push(
		ws.on('member_joined_channel', () => {
			loadChannels();
		})
	);
	unsubs.push(
		ws.on('member_left_channel', () => {
			loadChannels();
		})
	);
	unsubs.push(
		ws.on('channel_updated', (event: WsEvent) => {
			const data = event as unknown as {
				channel: Id;
				name?: string;
				topic?: string;
				description?: string;
				archived_at?: string;
			};
			if (data.channel) {
				channels.update((list) =>
					list.map((ch) =>
						ch.id === data.channel
							? {
									...ch,
									...(data.name && { name: data.name }),
									...(data.topic !== undefined && { topic: data.topic }),
									...(data.description !== undefined && {
										description: data.description
									}),
									...(data.archived_at !== undefined && {
										archived_at: data.archived_at
									})
								}
							: ch
					)
				);
			}
		})
	);
	return () => unsubs.forEach((fn) => fn());
}
