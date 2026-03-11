import { writable, derived, get } from 'svelte/store';
import * as api from '$lib/api';
import * as ws from '$lib/ws';
import { requestPermission } from '$lib/notifications';
import { resetChannels } from './channels';
import { resetMessages } from './messages';
import { resetUsers } from './users';
import { resetUnreads } from './unreads';
import type { User, Id } from '$lib/types';

interface AuthState {
	token: string | null;
	userId: Id | null;
	user: User | null;
	loading: boolean;
}

const initial: AuthState = {
	token: typeof localStorage !== 'undefined' ? localStorage.getItem('teide_token') : null,
	userId: typeof localStorage !== 'undefined' ? localStorage.getItem('teide_user_id') : null,
	user: null,
	loading: false
};

export const auth = writable<AuthState>(initial);

export const isAuthenticated = derived(auth, ($auth) => !!$auth.token);

/** Initialize from persisted token. Call on app start. */
export async function initAuth() {
	const state = get(auth);
	if (state.token) {
		api.setToken(state.token);
		ws.connect(state.token);
		if (state.userId) {
			try {
				const res = await api.usersInfo(state.userId);
				if (res.ok && res.user) {
					auth.update((s) => ({ ...s, user: res.user! }));
				} else {
					// Token invalid, clear
					doLogout();
				}
			} catch {
				doLogout();
			}
		}
	}
}

export async function doLogin(username: string, password: string): Promise<string | null> {
	auth.update((s) => ({ ...s, loading: true }));
	try {
		const res = await api.login(username, password);
		if (res.ok && res.token && res.user_id) {
			localStorage.setItem('teide_token', res.token);
			localStorage.setItem('teide_user_id', res.user_id);
			api.setToken(res.token);
			ws.connect(res.token);

			const userRes = await api.usersInfo(res.user_id);
			auth.set({
				token: res.token,
				userId: res.user_id,
				user: userRes.ok ? userRes.user! : null,
				loading: false
			});
			requestPermission();
			return null;
		}
		auth.update((s) => ({ ...s, loading: false }));
		return res.error || 'Login failed';
	} catch (e) {
		auth.update((s) => ({ ...s, loading: false }));
		return (e as Error).message;
	}
}

export async function doRegister(
	username: string,
	password: string,
	email: string
): Promise<string | null> {
	auth.update((s) => ({ ...s, loading: true }));
	try {
		const res = await api.register(username, password, email);
		if (res.ok && res.token && res.user_id) {
			localStorage.setItem('teide_token', res.token);
			localStorage.setItem('teide_user_id', res.user_id);
			api.setToken(res.token);
			ws.connect(res.token);

			const userRes = await api.usersInfo(res.user_id);
			auth.set({
				token: res.token,
				userId: res.user_id,
				user: userRes.ok ? userRes.user! : null,
				loading: false
			});
			return null;
		}
		auth.update((s) => ({ ...s, loading: false }));
		return res.error || 'Registration failed';
	} catch (e) {
		auth.update((s) => ({ ...s, loading: false }));
		return (e as Error).message;
	}
}

export async function refreshCurrentUser() {
	const state = get(auth);
	if (state.userId) {
		const res = await api.usersInfo(state.userId);
		if (res.ok && res.user) {
			auth.update((s) => ({ ...s, user: res.user! }));
		}
	}
}

export function doLogout() {
	localStorage.removeItem('teide_token');
	localStorage.removeItem('teide_user_id');
	api.setToken(null);
	ws.disconnect();
	auth.set({ token: null, userId: null, user: null, loading: false });
	resetChannels();
	resetMessages();
	resetUsers();
	resetUnreads();
}
