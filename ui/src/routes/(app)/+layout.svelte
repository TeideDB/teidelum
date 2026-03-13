<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import SearchModal from '$lib/components/SearchModal.svelte';
	import ShortcutsModal from '$lib/components/ShortcutsModal.svelte';
	import ConnectionStatus from '$lib/components/ConnectionStatus.svelte';
	import SettingsOverlay from '$lib/components/SettingsOverlay.svelte';
	import { showSettings } from '$lib/stores/settings';
	import { loadChannels, initChannelWsListeners, activeChannelId, channels, nonDmChannels, dmChannels } from '$lib/stores/channels';
	import { loadUsers, initUserWsListeners } from '$lib/stores/users';
	import { initMessageWsListeners } from '$lib/stores/messages';
	import { initUnreadsWsListeners } from '$lib/stores/unreads';
	import { unreads } from '$lib/stores/unreads';
	import { usersSetPresence } from '$lib/api';
	import { auth } from '$lib/stores/auth';
	import { onReconnect } from '$lib/ws';
	import { get } from 'svelte/store';
	import type { Id } from '$lib/types';

	type NavView = 'channels' | 'dms';
	let activeView = $state<NavView>('channels');

	const totalUnreads = $derived(
		Array.from($unreads.values()).reduce((sum, n) => sum + n, 0)
	);
	const dmUnreads = $derived(
		$dmChannels.reduce((sum, ch) => sum + ($unreads.get(ch.id) ?? 0), 0)
	);

	function switchView(view: NavView) {
		activeView = view;
		sidebarOpen = true;
	}

	let { children } = $props();
	let showSearch = $state(false);
	let showShortcuts = $state(false);
	let searchInitialChannel = $state<Id | undefined>(undefined);
	let sidebarOpen = $state(false);

	// Close sidebar on navigation (mobile)
	$effect(() => {
		// Track page URL changes
		page.url;
		sidebarOpen = false;
	});

	let idleTimer: ReturnType<typeof setTimeout>;
	let isIdle = false;

	function resetIdle() {
		if (isIdle) {
			isIdle = false;
			usersSetPresence('online');
		}
		clearTimeout(idleTimer);
		idleTimer = setTimeout(() => {
			isIdle = true;
			usersSetPresence('away');
		}, 5 * 60 * 1000);
	}

	onMount(() => {
		Promise.all([loadChannels(), loadUsers()]).catch((e) => {
			console.error('Failed to load initial data:', e);
		});
		const cleanups = [
			initChannelWsListeners(),
			initUserWsListeners(),
			initMessageWsListeners(),
			initUnreadsWsListeners()
		];

		// Re-fetch all data after a reconnect so the UI is fresh
		onReconnect(() => {
			Promise.all([loadChannels(), loadUsers()]).catch((e) => {
				console.error('Failed to reload data after reconnect:', e);
			});
		});

		window.addEventListener('mousemove', resetIdle);
		window.addEventListener('keydown', resetIdle);
		resetIdle();

		return () => {
			cleanups.forEach((fn) => fn());
			window.removeEventListener('mousemove', resetIdle);
			window.removeEventListener('keydown', resetIdle);
			clearTimeout(idleTimer);
		};
	});

	function handleGlobalKeydown(e: KeyboardEvent) {
		if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
			e.preventDefault();
			searchInitialChannel = undefined;
			showSearch = !showSearch;
		}
		if ((e.metaKey || e.ctrlKey) && e.key === 'f') {
			e.preventDefault();
			searchInitialChannel = get(activeChannelId) ?? undefined;
			showSearch = true;
		}
		if ((e.metaKey || e.ctrlKey) && e.key === '/') {
			e.preventDefault();
			showShortcuts = !showShortcuts;
		}
		if ((e.metaKey || e.ctrlKey) && e.shiftKey && (e.key === 'A' || e.key === 'a')) {
			e.preventDefault();
			navigateToNextUnread();
		}
	}

	function navigateToNextUnread() {
		const unreadMap = get(unreads);
		const channelList = get(channels);
		const currentId = get(activeChannelId);

		// Find channels with unreads
		const unreadChannels = channelList.filter((ch) => (unreadMap.get(ch.id) ?? 0) > 0);
		if (unreadChannels.length === 0) return;

		// Find the next unread channel after the current one
		const currentIndex = unreadChannels.findIndex((ch) => ch.id === currentId);
		const nextIndex = currentIndex < 0 ? 0 : (currentIndex + 1) % unreadChannels.length;
		goto(`/${unreadChannels[nextIndex].id}`);
	}
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

<ConnectionStatus />

<div class="flex h-screen overflow-hidden bg-navy">
	<!-- Left vertical icon bar (Zulip-style) -->
	<div class="hidden md:flex w-12 flex-shrink-0 flex-col items-center border-r border-primary-dark/40 bg-navy-light py-3 gap-1">
		<!-- Logo -->
		<div class="mb-3 flex h-8 w-8 items-center justify-center">
			<img src="/teide-logo.svg" alt="Teidelum" class="h-6 w-6" />
		</div>

		<!-- Home / Channels -->
		<button
			onclick={() => switchView('channels')}
			class="relative flex h-9 w-9 items-center justify-center rounded-lg transition {activeView === 'channels' ? 'bg-primary/30 text-heading' : 'text-primary-light/50 hover:bg-primary-darker/40 hover:text-primary-lighter'}"
			title="Channels"
		>
			<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M7 20l4-16m2 16l4-16M6 9h14M4 15h14" />
			</svg>
		</button>

		<!-- DMs -->
		<button
			onclick={() => switchView('dms')}
			class="relative flex h-9 w-9 items-center justify-center rounded-lg transition {activeView === 'dms' ? 'bg-primary/30 text-heading' : 'text-primary-light/50 hover:bg-primary-darker/40 hover:text-primary-lighter'}"
			title="Direct Messages"
		>
			<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
			</svg>
			{#if dmUnreads > 0}
				<span class="absolute -top-0.5 -right-0.5 flex h-4 min-w-4 items-center justify-center rounded-full bg-red-500 px-1 text-[10px] font-bold text-white">{dmUnreads}</span>
			{/if}
		</button>

		<!-- Search -->
		<button
			onclick={() => { searchInitialChannel = undefined; showSearch = true; }}
			class="flex h-9 w-9 items-center justify-center rounded-lg text-primary-light/50 hover:bg-primary-darker/40 hover:text-primary-lighter transition"
			title="Search (Cmd+K)"
		>
			<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
			</svg>
		</button>

		<div class="flex-1"></div>

		<!-- Settings -->
		<button
			onclick={() => showSettings.set(true)}
			class="flex h-9 w-9 items-center justify-center rounded-lg text-primary-light/50 hover:bg-primary-darker/40 hover:text-primary-lighter transition"
			title="Settings"
		>
			<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z" />
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
			</svg>
		</button>
	</div>

	<!-- Mobile sidebar backdrop -->
	{#if sidebarOpen}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div
			class="fixed inset-0 z-30 bg-black/50 md:hidden"
			onclick={() => (sidebarOpen = false)}
			onkeydown={(e) => e.key === 'Escape' && (sidebarOpen = false)}
		></div>
	{/if}

	<!-- Sidebar -->
	<div
		class="fixed inset-y-0 left-0 z-40 flex w-64 flex-shrink-0 flex-col border-r border-primary-dark/40 bg-navy-dark transition-transform duration-200 md:relative md:translate-x-0 {sidebarOpen ? 'translate-x-0' : '-translate-x-full'}"
	>
		<Sidebar />
	</div>

	<!-- Main content area -->
	<div class="flex min-w-0 flex-1 flex-col overflow-hidden">
		<!-- Mobile hamburger bar -->
		<div class="flex items-center border-b border-primary-dark/40 px-2 py-2 md:hidden">
			<button
				onclick={() => (sidebarOpen = !sidebarOpen)}
				class="flex h-11 w-11 items-center justify-center rounded text-primary-lighter hover:bg-primary-darker/60"
				aria-label="Toggle sidebar"
			>
				<svg class="h-6 w-6" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
				</svg>
			</button>
		</div>

		<div class="flex flex-1 overflow-hidden">
			{@render children()}
		</div>
	</div>
</div>

{#if showSearch}
	<SearchModal onClose={() => (showSearch = false)} initialChannel={searchInitialChannel} />
{/if}

{#if showShortcuts}
	<ShortcutsModal onClose={() => (showShortcuts = false)} />
{/if}

{#if $showSettings}
	<SettingsOverlay onClose={() => showSettings.set(false)} />
{/if}
