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

export function initUserWsListeners(): () => void {
	// Backend sends presence: "online" / "offline", map to "active" / "away"
	const unsub = ws.on('presence_change', (event: WsEvent) => {
		const data = event as unknown as { user: Id; presence: string };
		if (data.user) {
			const mapped = data.presence === 'online' ? 'active' : 'away';
			presence.update((map) => {
				const newMap = new Map(map);
				newMap.set(data.user, mapped);
				return newMap;
			});
		}
	});
	return unsub;
}
