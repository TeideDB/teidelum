import { writable } from 'svelte/store';

const stored = typeof localStorage !== 'undefined' ? localStorage.getItem('teide_theme') : null;
const systemPreference =
	typeof window !== 'undefined'
		? window.matchMedia('(prefers-color-scheme: dark)').matches
			? 'dark'
			: 'light'
		: 'dark';

export const theme = writable<'dark' | 'light'>((stored as 'dark' | 'light') || systemPreference);

theme.subscribe((value) => {
	if (typeof localStorage !== 'undefined') {
		localStorage.setItem('teide_theme', value);
	}
	if (typeof document !== 'undefined') {
		document.documentElement.classList.toggle('dark', value === 'dark');
		document.documentElement.classList.toggle('light', value === 'light');
	}
});

export function toggleTheme() {
	theme.update((t) => (t === 'dark' ? 'light' : 'dark'));
}
