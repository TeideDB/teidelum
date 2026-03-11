import type { WsEvent, WsEventType } from './types';

type EventCallback = (event: WsEvent) => void;

const listeners = new Map<WsEventType | '*', Set<EventCallback>>();

let ws: WebSocket | null = null;
let reconnectTimer: ReturnType<typeof setTimeout> | null = null;
let reconnectDelay = 1000;
const MAX_RECONNECT_DELAY = 30000;
let currentToken: string | null = null;
let intentionalClose = false;

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

	ws.onopen = () => {
		reconnectDelay = 1000;
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
			scheduleReconnect();
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
