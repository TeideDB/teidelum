<script lang="ts">
	import { goto } from '$app/navigation';
	import DOMPurify from 'dompurify';
	import * as api from '$lib/api';
	import { users } from '$lib/stores/users';
	import type { Message, Id } from '$lib/types';

	function sanitizeSnippet(html: string): string {
		return DOMPurify.sanitize(html, { ALLOWED_TAGS: ['b'] });
	}

	interface Props {
		onClose: () => void;
		initialChannel?: Id;
	}

	let { onClose, initialChannel }: Props = $props();

	let query = $state('');
	let results = $state<Message[]>([]);
	let loading = $state(false);
	let searchTimeout: ReturnType<typeof setTimeout> | null = null;
	let searchSeq = 0;

	// Filters
	let showFilters = $state(!!initialChannel);
	let filterUserId = $state<Id | undefined>(undefined);
	let filterChannelId = $state<Id | undefined>(initialChannel);
	let filterDateFrom = $state('');
	let filterDateTo = $state('');

	// Autocomplete state
	let userQuery = $state('');
	let userResults = $state<Array<{ id: Id; username: string; display_name: string; avatar_url: string }>>([]);
	let channelQuery = $state('');
	let channelResults = $state<Array<{ id: Id; name: string; topic: string }>>([]);
	let selectedUserName = $state('');
	let selectedChannelName = $state('');

	// Initialize selected channel name if initialChannel is set
	$effect(() => {
		const chId = initialChannel;
		if (chId) {
			api.conversationsAutocomplete('').then((res) => {
				const ch = res.channels?.find((c) => c.id === chId);
				if (ch) selectedChannelName = ch.name;
			});
		}
	});

	let userSearchTimeout: ReturnType<typeof setTimeout> | null = null;
	let channelSearchTimeout: ReturnType<typeof setTimeout> | null = null;

	function handleUserInput() {
		if (userSearchTimeout) clearTimeout(userSearchTimeout);
		userSearchTimeout = setTimeout(async () => {
			if (!userQuery.trim()) {
				userResults = [];
				return;
			}
			const res = await api.usersSearch(userQuery);
			if (res.ok && res.users) {
				userResults = res.users;
			}
		}, 200);
	}

	function selectUser(user: { id: Id; username: string; display_name: string }) {
		filterUserId = user.id;
		selectedUserName = user.display_name || user.username;
		userQuery = '';
		userResults = [];
		triggerSearch();
	}

	function clearUserFilter() {
		filterUserId = undefined;
		selectedUserName = '';
		triggerSearch();
	}

	function handleChannelInput() {
		if (channelSearchTimeout) clearTimeout(channelSearchTimeout);
		channelSearchTimeout = setTimeout(async () => {
			if (!channelQuery.trim()) {
				channelResults = [];
				return;
			}
			const res = await api.conversationsAutocomplete(channelQuery);
			if (res.ok && res.channels) {
				channelResults = res.channels;
			}
		}, 200);
	}

	function selectChannel(ch: { id: Id; name: string }) {
		filterChannelId = ch.id;
		selectedChannelName = ch.name;
		channelQuery = '';
		channelResults = [];
		triggerSearch();
	}

	function clearChannelFilter() {
		filterChannelId = undefined;
		selectedChannelName = '';
		triggerSearch();
	}

	function triggerSearch() {
		if (searchTimeout) clearTimeout(searchTimeout);
		searchTimeout = setTimeout(doSearch, 300);
	}

	function handleInput() {
		triggerSearch();
	}

	async function doSearch() {
		const q = query.trim();
		if (!q) {
			++searchSeq;
			results = [];
			return;
		}

		const seq = ++searchSeq;
		loading = true;
		try {
			// Convert local date strings to epoch-second strings so the backend
		// compares against the user's local day boundaries, not UTC midnight.
		let dfFrom: string | undefined;
		let dfTo: string | undefined;
		if (filterDateFrom) {
			dfFrom = String(Math.floor(new Date(filterDateFrom + 'T00:00:00').getTime() / 1000));
		}
		if (filterDateTo) {
			dfTo = String(Math.floor(new Date(filterDateTo + 'T23:59:59').getTime() / 1000));
		}
		const res = await api.searchMessages(q, filterChannelId, 20, filterUserId, dfFrom, dfTo);
			// Discard stale responses
			if (seq !== searchSeq) return;
			if (res.ok && res.messages) {
				results = res.messages;
			}
		} catch (e) {
			if (seq !== searchSeq) return;
			console.error('Search failed:', e);
		} finally {
			if (seq === searchSeq) {
				loading = false;
			}
		}
	}

	function getUserName(userId: Id): string {
		const user = $users.get(userId);
		return user?.display_name || user?.username || userId;
	}

	function parseTimestamp(timestamp: string): Date {
		const n = parseInt(timestamp, 10);
		if (!isNaN(n) && String(n) === timestamp) {
			return new Date(n * 1000);
		}
		return new Date(timestamp);
	}

	function formatTime(timestamp: string): string {
		try {
			const date = parseTimestamp(timestamp);
			return date.toLocaleDateString([], { month: 'short', day: 'numeric' }) +
				' ' + date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
		} catch {
			return '';
		}
	}

	function navigateToMessage(msg: Message) {
		goto(`/${msg.channel_id}`);
		onClose();
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			onClose();
		}
	}

	function hasActiveFilters(): boolean {
		return !!filterUserId || !!filterChannelId || !!filterDateFrom || !!filterDateTo;
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="fixed inset-0 z-50 flex items-start justify-center bg-overlay pt-20">
	<div class="w-full max-w-2xl rounded-lg bg-navy-light shadow-2xl">
		<!-- Search input -->
		<div class="border-b border-primary-dark/40 p-4">
			<div class="flex items-center gap-3">
				<svg class="h-5 w-5 text-primary-light/50" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
				</svg>
				<input
					type="text"
					bind:value={query}
					oninput={handleInput}
					placeholder="Search messages..."
					class="flex-1 bg-transparent text-white placeholder-primary-light/40 focus:outline-none"
					autofocus
				/>
				<button
					onclick={() => (showFilters = !showFilters)}
					class="rounded p-1 text-primary-light/50 hover:text-primary-lighter {hasActiveFilters() ? 'text-primary-light' : ''}"
					title="Toggle filters"
				>
					<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 4a1 1 0 011-1h16a1 1 0 011 1v2.586a1 1 0 01-.293.707l-6.414 6.414a1 1 0 00-.293.707V17l-4 4v-6.586a1 1 0 00-.293-.707L3.293 7.293A1 1 0 013 6.586V4z" />
					</svg>
				</button>
				<button onclick={onClose} aria-label="Close search" class="text-primary-light/50 hover:text-primary-lighter">
					<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
					</svg>
				</button>
			</div>
		</div>

		<!-- Filter bar -->
		{#if showFilters}
			<div class="border-b border-primary-dark/40 px-4 py-3 space-y-2">
				<div class="flex flex-wrap gap-3">
					<!-- User filter -->
					<div class="relative flex-1 min-w-[140px]">
						{#if selectedUserName}
							<div class="flex items-center gap-1 rounded bg-navy px-2 py-1.5 text-sm text-white">
								<span class="truncate">From: {selectedUserName}</span>
								<button onclick={clearUserFilter} class="ml-1 text-primary-light/50 hover:text-heading">
									<svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" /></svg>
								</button>
							</div>
						{:else}
							<input
								type="text"
								bind:value={userQuery}
								oninput={handleUserInput}
								placeholder="Filter by user..."
								class="w-full rounded bg-navy px-2 py-1.5 text-sm text-white placeholder-primary-light/40 focus:outline-none focus:ring-1 focus:ring-primary"
							/>
							{#if userResults.length > 0}
								<div class="absolute top-full left-0 right-0 z-10 mt-1 max-h-32 overflow-y-auto rounded bg-navy shadow-lg ring-1 ring-primary-dark/60">
									{#each userResults as user}
										<button
											onclick={() => selectUser(user)}
											class="flex w-full items-center gap-2 px-2 py-1.5 text-sm text-primary-lighter/80 hover:bg-primary-darker/60 hover:text-heading"
										>
											{user.display_name || user.username}
										</button>
									{/each}
								</div>
							{/if}
						{/if}
					</div>

					<!-- Channel filter -->
					<div class="relative flex-1 min-w-[140px]">
						{#if selectedChannelName}
							<div class="flex items-center gap-1 rounded bg-navy px-2 py-1.5 text-sm text-white">
								<span class="truncate">In: #{selectedChannelName}</span>
								<button onclick={clearChannelFilter} class="ml-1 text-primary-light/50 hover:text-heading">
									<svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" /></svg>
								</button>
							</div>
						{:else}
							<input
								type="text"
								bind:value={channelQuery}
								oninput={handleChannelInput}
								placeholder="Filter by channel..."
								class="w-full rounded bg-navy px-2 py-1.5 text-sm text-white placeholder-primary-light/40 focus:outline-none focus:ring-1 focus:ring-primary"
							/>
							{#if channelResults.length > 0}
								<div class="absolute top-full left-0 right-0 z-10 mt-1 max-h-32 overflow-y-auto rounded bg-navy shadow-lg ring-1 ring-primary-dark/60">
									{#each channelResults as ch}
										<button
											onclick={() => selectChannel(ch)}
											class="flex w-full items-center gap-2 px-2 py-1.5 text-sm text-primary-lighter/80 hover:bg-primary-darker/60 hover:text-heading"
										>
											#{ch.name}
										</button>
									{/each}
								</div>
							{/if}
						{/if}
					</div>

					<!-- Date from -->
					<input
						type="date"
						bind:value={filterDateFrom}
						onchange={() => triggerSearch()}
						class="rounded bg-navy px-2 py-1.5 text-sm text-white focus:outline-none focus:ring-1 focus:ring-primary"
						title="From date"
					/>

					<!-- Date to -->
					<input
						type="date"
						bind:value={filterDateTo}
						onchange={() => triggerSearch()}
						class="rounded bg-navy px-2 py-1.5 text-sm text-white focus:outline-none focus:ring-1 focus:ring-primary"
						title="To date"
					/>
				</div>
			</div>
		{/if}

		<!-- Results -->
		<div class="max-h-96 overflow-y-auto">
			{#if loading}
				<div class="p-4 text-center text-sm text-primary-light/50">Searching...</div>
			{:else if query.trim() && results.length === 0}
				<div class="p-4 text-center text-sm text-primary-light/50">No results found</div>
			{:else}
				{#each results as msg (msg.id)}
					<button
						onclick={() => navigateToMessage(msg)}
						class="flex w-full gap-3 border-b border-primary-dark/20 px-4 py-3 text-left transition hover:bg-navy-mid/50"
					>
						<div class="min-w-0 flex-1">
							<div class="flex items-baseline gap-2">
								<span class="text-sm font-bold text-body">{getUserName(msg.user_id)}</span>
								<span class="text-xs text-primary-light/40">{formatTime(msg.created_at)}</span>
							</div>
							<div class="truncate text-sm text-primary-lighter/70">{@html sanitizeSnippet(msg.text)}</div>
						</div>
					</button>
				{/each}
			{/if}
		</div>
	</div>
</div>
