<script lang="ts">
	import { goto } from '$app/navigation';
	import {
		publicChannels,
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
	<div class="flex items-center justify-between border-b border-gray-700 px-4 py-3">
		<h2 class="text-lg font-bold text-white">Teide Chat</h2>
		<button
			onclick={handleLogout}
			class="text-xs text-gray-500 hover:text-gray-300"
			title="Sign out"
		>
			Sign out
		</button>
	</div>

	<!-- User info -->
	{#if $auth.user}
		<div class="border-b border-gray-700 px-4 py-2">
			<span class="text-sm text-gray-300">{$auth.user.display_name || $auth.user.username}</span>
		</div>
	{/if}

	<!-- Channel list -->
	<div class="flex-1 overflow-y-auto">
		<!-- Channels section -->
		<div class="px-2 pt-3">
			<div class="flex items-center justify-between px-2 pb-1">
				<span class="text-xs font-semibold uppercase tracking-wide text-gray-500">Channels</span>
				<button
					onclick={() => (showCreateModal = true)}
					class="text-gray-500 hover:text-gray-300"
					title="Create channel"
				>
					<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16m8-8H4" />
					</svg>
				</button>
			</div>

			{#each $publicChannels as channel}
				<button
					onclick={() => navigateToChannel(channel)}
					class="flex w-full items-center justify-between rounded px-2 py-1 text-left text-sm transition {isActive(channel.id)
						? 'bg-blue-600 text-white'
						: 'text-gray-400 hover:bg-gray-700 hover:text-gray-200'}"
				>
					<span class="truncate">
						<span class="mr-1 text-gray-500">#</span>
						{channel.name}
					</span>
					{#if getUnreadCount(channel.id) > 0}
						<span class="ml-1 rounded-full bg-red-500 px-1.5 text-xs font-bold text-white">
							{getUnreadCount(channel.id)}
						</span>
					{/if}
				</button>
			{/each}
		</div>

		<!-- DMs section -->
		<div class="px-2 pt-4">
			<div class="px-2 pb-1">
				<span class="text-xs font-semibold uppercase tracking-wide text-gray-500">Direct Messages</span>
			</div>

			{#each $dmChannels as channel}
				<button
					onclick={() => navigateToChannel(channel)}
					class="flex w-full items-center justify-between rounded px-2 py-1 text-left text-sm transition {isActive(channel.id)
						? 'bg-blue-600 text-white'
						: 'text-gray-400 hover:bg-gray-700 hover:text-gray-200'}"
				>
					<span class="truncate">{getDmDisplayName(channel)}</span>
					{#if getUnreadCount(channel.id) > 0}
						<span class="ml-1 rounded-full bg-red-500 px-1.5 text-xs font-bold text-white">
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
		<div class="w-full max-w-md rounded-lg bg-gray-800 p-6 shadow-xl">
			<h3 class="mb-4 text-lg font-bold text-white">Create Channel</h3>

			<form
				onsubmit={(e) => {
					e.preventDefault();
					handleCreateChannel();
				}}
				class="space-y-3"
			>
				<div>
					<label for="channelName" class="mb-1 block text-sm text-gray-400">Channel Name</label>
					<input
						id="channelName"
						type="text"
						bind:value={newChannelName}
						class="w-full rounded bg-gray-700 px-3 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
						placeholder="e.g. general"
						required
					/>
				</div>

				<div>
					<label for="channelTopic" class="mb-1 block text-sm text-gray-400">Topic (optional)</label>
					<input
						id="channelTopic"
						type="text"
						bind:value={newChannelTopic}
						class="w-full rounded bg-gray-700 px-3 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
						placeholder="What's this channel about?"
					/>
				</div>

				<div class="flex justify-end gap-2 pt-2">
					<button
						type="button"
						onclick={() => (showCreateModal = false)}
						class="rounded px-4 py-2 text-sm text-gray-400 hover:text-gray-200"
					>
						Cancel
					</button>
					<button
						type="submit"
						class="rounded bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
					>
						Create
					</button>
				</div>
			</form>
		</div>
	</div>
{/if}
