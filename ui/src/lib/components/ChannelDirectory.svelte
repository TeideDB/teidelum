<script lang="ts">
	import { goto } from '$app/navigation';
	import * as api from '$lib/api';
	import { setActiveChannel, loadChannels } from '$lib/stores/channels';
	import type { Channel } from '$lib/types';

	interface Props {
		onClose: () => void;
	}

	let { onClose }: Props = $props();

	let query = $state('');
	let channels = $state<Channel[]>([]);
	let loading = $state(true);
	let searchTimeout: ReturnType<typeof setTimeout> | null = null;

	$effect(() => {
		loadDirectory();
	});

	async function loadDirectory() {
		loading = true;
		try {
			const res = await api.conversationsDirectory(query.trim() || undefined, 50);
			if (res.ok && res.channels) {
				channels = res.channels;
			}
		} catch (e) {
			console.error('Directory load failed:', e);
		} finally {
			loading = false;
		}
	}

	function handleInput() {
		if (searchTimeout) clearTimeout(searchTimeout);
		searchTimeout = setTimeout(loadDirectory, 300);
	}

	async function joinChannel(channel: Channel) {
		try {
			await api.conversationsJoin(channel.id);
			await loadChannels();
			setActiveChannel(channel.id);
			goto(`/${channel.id}`);
			onClose();
		} catch (e) {
			console.error('Join failed:', e);
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			onClose();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="fixed inset-0 z-50 flex items-center justify-center bg-overlay">
	<div class="w-full max-w-lg rounded-lg bg-navy-light shadow-2xl">
		<!-- Header -->
		<div class="flex items-center justify-between border-b border-primary-dark/40 p-4">
			<h3 class="font-[Oswald] text-lg font-semibold text-heading">Browse Channels</h3>
			<button onclick={onClose} aria-label="Close" class="text-primary-light/50 hover:text-primary-lighter">
				<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
				</svg>
			</button>
		</div>

		<!-- Search -->
		<div class="border-b border-primary-dark/40 px-4 py-3">
			<input
				type="text"
				bind:value={query}
				oninput={handleInput}
				placeholder="Search channels..."
				class="w-full rounded bg-navy px-3 py-2 text-sm text-white placeholder-primary-light/40 focus:outline-none focus:ring-1 focus:ring-primary"
				autofocus
			/>
		</div>

		<!-- Channel list -->
		<div class="max-h-80 overflow-y-auto">
			{#if loading}
				<div class="p-4 text-center text-sm text-primary-light/50">Loading...</div>
			{:else if channels.length === 0}
				<div class="p-4 text-center text-sm text-primary-light/50">No channels found</div>
			{:else}
				{#each channels as channel}
					<div class="flex items-center justify-between border-b border-primary-dark/20 px-4 py-3">
						<div class="min-w-0 flex-1">
							<div class="flex items-center gap-2">
								<span class="text-primary-light/40">#</span>
								<span class="text-sm font-medium text-heading">{channel.name}</span>
								{#if channel.member_count !== undefined}
									<span class="text-xs text-primary-light/40">{channel.member_count} members</span>
								{/if}
							</div>
							{#if channel.topic}
								<div class="mt-0.5 truncate text-xs text-primary-lighter/50">{channel.topic}</div>
							{/if}
						</div>
						<button
							onclick={() => joinChannel(channel)}
							class="ml-3 rounded bg-primary px-3 py-1 text-xs font-medium text-white hover:bg-primary-light"
						>
							Join
						</button>
					</div>
				{/each}
			{/if}
		</div>
	</div>
</div>
