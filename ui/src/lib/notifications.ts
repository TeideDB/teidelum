import { get } from 'svelte/store';
import { channels } from '$lib/stores/channels';
import type { Id } from '$lib/types';

let permissionGranted = false;
let audioCtx: AudioContext | null = null;

function getAudioContext(): AudioContext | null {
	if (!audioCtx) {
		try {
			audioCtx = new AudioContext();
		} catch {
			return null;
		}
	}
	return audioCtx;
}

function playNotificationSound() {
	if (localStorage.getItem('notification_sound') === 'false') return;
	const ctx = getAudioContext();
	if (!ctx) return;
	const oscillator = ctx.createOscillator();
	const gain = ctx.createGain();
	oscillator.connect(gain);
	gain.connect(ctx.destination);
	oscillator.type = 'sine';
	oscillator.frequency.setValueAtTime(880, ctx.currentTime);
	oscillator.frequency.setValueAtTime(660, ctx.currentTime + 0.1);
	gain.gain.setValueAtTime(0.3, ctx.currentTime);
	gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.3);
	oscillator.start(ctx.currentTime);
	oscillator.stop(ctx.currentTime + 0.3);
}

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

	playNotificationSound();

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
