import { get } from 'svelte/store';
import { channels } from '$lib/stores/channels';
import type { Id } from '$lib/types';

let permissionGranted = false;

export async function requestPermission(): Promise<boolean> {
	if (!('Notification' in window)) return false;
	if (Notification.permission === 'granted') {
		permissionGranted = true;
		return true;
	}
	if (Notification.permission === 'denied') return false;
	const result = await Notification.requestPermission();
	permissionGranted = result === 'granted';
	return permissionGranted;
}

export function showNotification(
	title: string,
	body: string,
	channelId?: Id,
	isMention?: boolean
) {
	if (!permissionGranted || document.hasFocus()) return;

	if (channelId) {
		const ch = get(channels).find((c) => c.id === channelId);
		if (ch?.muted === 'true') return;
		// Respect notification_level: "none" suppresses all, "mentions" only notifies on mentions
		if (ch?.notification_level === 'none') return;
		if (ch?.notification_level === 'mentions' && !isMention) return;
	}

	const notification = new Notification(title, {
		body,
		icon: '/teide-logo.svg',
		tag: channelId ? `teidelum-${channelId}` : 'teidelum'
	});

	notification.onclick = () => {
		window.focus();
		if (channelId) {
			window.location.hash = '';
			window.location.pathname = `/${channelId}`;
		}
		notification.close();
	};
}
