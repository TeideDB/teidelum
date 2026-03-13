import { writable } from 'svelte/store';
import type { WsEvent, WsEventType } from './types';

export type ConnectionState = 'connected' | 'reconnecting' | 'disconnected';
export const connectionState = writable<ConnectionState>('disconnected');

type EventCallback = (event: WsEvent) => void;

const listeners = new Map<WsEventType | '*', Set<EventCallback>>();

let ws: WebSocket | null = null;
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let reconnectDelay = 1000;
const MAX_RECONNECT_DELAY = 5000;
let currentToken: string | null = null;
let intentionalClose = false;
let onReconnectCallback: (() => void) | null = null;

/** Register a callback that fires once after a successful reconnect (not initial connect). */
export function onReconnect(cb: () => void) {
	onReconnectCallback = cb;
}

// When the tab becomes visible again, try to reconnect immediately instead
// of waiting for the current backoff timer to fire.
if (typeof document !== 'undefined') {
	document.addEventListener('visibilitychange', () => {
		if (document.visibilityState === 'visible' && currentToken && !ws) {
			// Reset delay and reconnect now
			reconnectDelay = 1000;
			if (reconnectTimer) {
				clearTimeout(reconnectTimer);
				reconnectTimer = null;
			}
			doConnect();
		}
	});
}

export function connect(token: string) {
	currentToken = token;
	intentionalClose = false;
	reconnectDelay = 1000;
	doConnect();
}

export function disconnect() {
	intentionalClose = true;
	currentToken = null;
	if (reconnectTimer) {
		clearTimeout(reconnectTimer);
		reconnectTimer = null;
	}
	if (ws) {
		ws.close();
		ws = null;
	}
	connectionState.set('disconnected');
	// Don't clear listeners here - they are managed by component cleanup callbacks
	// Clearing them would break re-login without page reload since onMount won't re-fire
}

function getWsUrl(token: string): string {
	// In Tauri, use configured server URL; in browser, use current host
	if (typeof window !== 'undefined' && '__TAURI__' in window) {
		const serverUrl = localStorage.getItem('teidelum_server_url') || 'http://localhost:3000';
		const wsUrl = serverUrl.replace(/^http/, 'ws');
		return `${wsUrl}/ws?token=${token}`;
	}
	const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
	return `${protocol}//${window.location.host}/ws?token=${token}`;
}

function doConnect() {
	if (!currentToken) return;

	const url = getWsUrl(currentToken);

	ws = new WebSocket(url);

	const wasReconnecting = reconnectDelay > 1000 || reconnectTimer !== null;

	ws.onopen = () => {
		const isReconnect = wasReconnecting;
		reconnectDelay = 1000;
		connectionState.set('connected');
		if (isReconnect && onReconnectCallback) {
			onReconnectCallback();
		}
	};

	ws.onmessage = (event) => {
		try {
			const data: WsEvent = JSON.parse(event.data);
			dispatch(data);
		} catch {
			// ignore malformed messages
		}
	};

	ws.onclose = () => {
		ws = null;
		if (!intentionalClose) {
			connectionState.set('reconnecting');
			scheduleReconnect();
		} else {
			connectionState.set('disconnected');
		}
	};

	ws.onerror = () => {
		// onclose will fire after onerror
	};
}

function scheduleReconnect() {
	if (reconnectTimer) return;
	reconnectTimer = setTimeout(() => {
		reconnectTimer = null;
		reconnectDelay = Math.min(reconnectDelay * 2, MAX_RECONNECT_DELAY);
		doConnect();
	}, reconnectDelay);
}

function dispatch(event: WsEvent) {
	const typeListeners = listeners.get(event.type);
	if (typeListeners) {
		for (const cb of typeListeners) cb(event);
	}
	const wildcardListeners = listeners.get('*');
	if (wildcardListeners) {
		for (const cb of wildcardListeners) cb(event);
	}
}

/** Subscribe to a specific event type or '*' for all events. Returns unsubscribe function. */
export function on(type: WsEventType | '*', callback: EventCallback): () => void {
	if (!listeners.has(type)) {
		listeners.set(type, new Set());
	}
	listeners.get(type)!.add(callback);
	return () => {
		listeners.get(type)?.delete(callback);
	};
}

/** Send typing indicator to a channel */
export function sendTyping(channel: string) {
	if (ws && ws.readyState === WebSocket.OPEN) {
		ws.send(JSON.stringify({ type: 'typing', channel }));
	}
}
