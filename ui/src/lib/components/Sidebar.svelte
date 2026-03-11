<script lang="ts">
	import { goto } from '$app/navigation';
	import {
		nonDmChannels,
		dmChannels,
		activeChannelId,
		setActiveChannel,
		createChannel
	} from '$lib/stores/channels';
	import { unreads } from '$lib/stores/unreads';
	import { auth, doLogout } from '$lib/stores/auth';
	import type { Channel } from '$lib/types';

	let showCreateModal = $state(false);
	let newChannelName = $state('');
	let newChannelTopic = $state('');
	let showUserMenu = $state(false);

	function navigateToChannel(channel: Channel) {
		setActiveChannel(channel.id);
		goto(`/${channel.id}`);
	}

	function getUnreadCount(channelId: string): number {
		return $unreads.get(channelId) ?? 0;
	}

	function isActive(channelId: string): boolean {
		return $activeChannelId === channelId;
	}

	function getDmDisplayName(channel: Channel): string {
		return channel.name || 'Direct Message';
	}

	async function handleCreateChannel() {
		if (!newChannelName.trim()) return;
		const ch = await createChannel(newChannelName.trim(), 'public', newChannelTopic.trim() || undefined);
		if (ch) {
			showCreateModal = false;
			newChannelName = '';
			newChannelTopic = '';
			navigateToChannel(ch);
		}
	}

	function handleLogout() {
		doLogout();
		goto('/login');
	}
</script>

<div class="flex h-full flex-col">
	<!-- Header -->
	<div class="flex items-center justify-between border-b border-primary-dark/40 px-4 py-3">
		<div class="flex items-center gap-2">
			<img src="/teide-logo.svg" alt="Teidelum" class="h-6 w-auto" />
			<h2 class="font-[Oswald] text-lg font-semibold tracking-wide text-white">Teidelum</h2>
		</div>
	</div>

	<!-- User area with menu -->
	{#if $auth.user}
		<div class="relative border-b border-primary-dark/40 px-4 py-2">
			<button
				onclick={() => (showUserMenu = !showUserMenu)}
				class="flex w-full items-center gap-2 rounded px-1 py-1 text-left text-sm text-primary-lighter hover:bg-primary-darker/60"
			>
				<span class="h-2 w-2 rounded-full bg-green-400"></span>
				<span class="truncate">{$auth.user.display_name || $auth.user.username}</span>
				<svg class="ml-auto h-3 w-3 text-primary-light/50" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
				</svg>
			</button>

			{#if showUserMenu}
				<div class="absolute bottom-full left-2 right-2 mb-1 rounded-md bg-navy-light shadow-lg ring-1 ring-primary-dark/60 z-50">
					<button
						onclick={() => {
							showUserMenu = false;
							goto('/settings');
						}}
						class="flex w-full items-center px-3 py-2 text-sm text-primary-lighter/80 hover:bg-primary-darker/60 hover:text-white rounded-t-md"
					>
						Settings
					</button>
					<button
						onclick={() => {
							showUserMenu = false;
							handleLogout();
						}}
						class="flex w-full items-center px-3 py-2 text-sm text-primary-lighter/80 hover:bg-primary-darker/60 hover:text-white rounded-b-md"
					>
						Sign out
					</button>
				</div>
			{/if}
		</div>
	{/if}

	<!-- Channel list -->
	<div class="flex-1 overflow-y-auto">
		<!-- Channels section -->
		<div class="px-2 pt-3">
			<div class="flex items-center justify-between px-2 pb-1">
				<span class="text-xs font-semibold uppercase tracking-wide text-primary-light/50">Channels</span>
				<button
					onclick={() => (showCreateModal = true)}
					class="text-primary-light/50 hover:text-primary-lighter"
					title="Create channel"
				>
					<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
					</svg>
				</button>
			</div>

			{#each $nonDmChannels as channel}
				<button
					onclick={() => navigateToChannel(channel)}
					class="flex w-full items-center justify-between rounded px-2 py-1 text-left text-sm transition {isActive(channel.id)
						? 'bg-primary text-white'
						: 'text-primary-lighter/80 hover:bg-primary-darker/60 hover:text-white'}"
				>
					<span class="truncate">
						<span class="mr-1 text-primary-light/40">#</span>
						{channel.name}
					</span>
					{#if getUnreadCount(channel.id) > 0}
						<span class="ml-1 rounded-full bg-primary-light px-1.5 text-xs font-bold text-white">
							{getUnreadCount(channel.id)}
						</span>
					{/if}
				</button>
			{/each}
		</div>

		<!-- DMs section -->
		<div class="px-2 pt-4">
			<div class="px-2 pb-1">
				<span class="text-xs font-semibold uppercase tracking-wide text-primary-light/50">Direct Messages</span>
			</div>

			{#each $dmChannels as channel}
				<button
					onclick={() => navigateToChannel(channel)}
					class="flex w-full items-center justify-between rounded px-2 py-1 text-left text-sm transition {isActive(channel.id)
						? 'bg-primary text-white'
						: 'text-primary-lighter/80 hover:bg-primary-darker/60 hover:text-white'}"
				>
					<span class="truncate">{getDmDisplayName(channel)}</span>
					{#if getUnreadCount(channel.id) > 0}
						<span class="ml-1 rounded-full bg-primary-light px-1.5 text-xs font-bold text-white">
							{getUnreadCount(channel.id)}
						</span>
					{/if}
				</button>
			{/each}
		</div>
	</div>
</div>

<!-- Create channel modal -->
{#if showCreateModal}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
		<div class="w-full max-w-md rounded-lg bg-navy-light p-6 shadow-xl">
			<h3 class="mb-4 font-[Oswald] text-lg font-semibold text-white">Create Channel</h3>

			<form
				onsubmit={(e) => {
					e.preventDefault();
					handleCreateChannel();
				}}
				class="space-y-3"
			>
				<div>
					<label for="channelName" class="mb-1 block text-sm text-primary-lighter/70">Channel Name</label>
					<input
						id="channelName"
						type="text"
						bind:value={newChannelName}
						class="w-full rounded bg-navy px-3 py-2 text-white placeholder-primary-light/40 focus:outline-none focus:ring-2 focus:ring-primary"
						placeholder="e.g. general"
						required
					/>
				</div>

				<div>
					<label for="channelTopic" class="mb-1 block text-sm text-primary-lighter/70">Topic (optional)</label>
					<input
						id="channelTopic"
						type="text"
						bind:value={newChannelTopic}
						class="w-full rounded bg-navy px-3 py-2 text-white placeholder-primary-light/40 focus:outline-none focus:ring-2 focus:ring-primary"
						placeholder="What's this channel about?"
					/>
				</div>

				<div class="flex justify-end gap-2 pt-2">
					<button
						type="button"
						onclick={() => (showCreateModal = false)}
						class="rounded px-4 py-2 text-sm text-primary-lighter/70 hover:text-white"
					>
						Cancel
					</button>
					<button
						type="submit"
						class="rounded bg-primary px-4 py-2 text-sm font-medium text-white hover:bg-primary-light"
					>
						Create
					</button>
				</div>
			</form>
		</div>
	</div>
{/if}
