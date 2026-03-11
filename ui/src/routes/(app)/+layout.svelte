<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import SearchModal from '$lib/components/SearchModal.svelte';
	import ShortcutsModal from '$lib/components/ShortcutsModal.svelte';
	import ConnectionStatus from '$lib/components/ConnectionStatus.svelte';
	import { loadChannels, initChannelWsListeners, activeChannelId, channels } from '$lib/stores/channels';
	import { loadUsers, initUserWsListeners } from '$lib/stores/users';
	import { initMessageWsListeners } from '$lib/stores/messages';
	import { initUnreadsWsListeners } from '$lib/stores/unreads';
	import { unreads } from '$lib/stores/unreads';
	import { usersSetPresence } from '$lib/api';
	import { get } from 'svelte/store';
	import type { Id } from '$lib/types';

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
		class="fixed inset-y-0 left-0 z-40 flex w-64 flex-shrink-0 flex-col border-r border-primary-dark/40 bg-navy-light transition-transform duration-200 md:relative md:translate-x-0 {sidebarOpen ? 'translate-x-0' : '-translate-x-full'}"
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
