<script lang="ts">
	import { goto } from '$app/navigation';
	import * as api from '$lib/api';
	import { users } from '$lib/stores/users';
	import type { Message, Id } from '$lib/types';

	interface Props {
		onClose: () => void;
	}

	let { onClose }: Props = $props();

	let query = $state('');
	let results = $state<Message[]>([]);
	let loading = $state(false);
	let searchTimeout: ReturnType<typeof setTimeout> | null = null;

	function handleInput() {
		if (searchTimeout) clearTimeout(searchTimeout);
		searchTimeout = setTimeout(doSearch, 300);
	}

	async function doSearch() {
		const q = query.trim();
		if (!q) {
			results = [];
			return;
		}

		loading = true;
		const res = await api.searchMessages(q, undefined, 20);
		if (res.ok && res.messages) {
			results = res.messages;
		}
		loading = false;
	}

	function getUserName(userId: Id): string {
		const user = $users.get(userId);
		return user?.display_name || user?.username || userId;
	}

	function formatTime(timestamp: string): string {
		try {
			const date = new Date(timestamp);
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
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="fixed inset-0 z-50 flex items-start justify-center bg-black/60 pt-20">
	<div class="w-full max-w-2xl rounded-lg bg-gray-800 shadow-2xl">
		<!-- Search input -->
		<div class="border-b border-gray-700 p-4">
			<div class="flex items-center gap-3">
				<svg class="h-5 w-5 text-gray-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
				</svg>
				<input
					type="text"
					bind:value={query}
					oninput={handleInput}
					placeholder="Search messages..."
					class="flex-1 bg-transparent text-white placeholder-gray-500 focus:outline-none"
					autofocus
				/>
				<button onclick={onClose} aria-label="Close search" class="text-gray-500 hover:text-gray-300">
					<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
					</svg>
				</button>
			</div>
		</div>

		<!-- Results -->
		<div class="max-h-96 overflow-y-auto">
			{#if loading}
				<div class="p-4 text-center text-sm text-gray-500">Searching...</div>
			{:else if query.trim() && results.length === 0}
				<div class="p-4 text-center text-sm text-gray-500">No results found</div>
			{:else}
				{#each results as msg}
					<button
						onclick={() => navigateToMessage(msg)}
						class="flex w-full gap-3 border-b border-gray-700/50 px-4 py-3 text-left transition hover:bg-gray-700/50"
					>
						<div class="min-w-0 flex-1">
							<div class="flex items-baseline gap-2">
								<span class="text-sm font-bold text-gray-300">{getUserName(msg.user_id)}</span>
								<span class="text-xs text-gray-600">{formatTime(msg.created_at)}</span>
							</div>
							<div class="truncate text-sm text-gray-400">{msg.text}</div>
						</div>
					</button>
				{/each}
			{/if}
		</div>
	</div>
</div>
