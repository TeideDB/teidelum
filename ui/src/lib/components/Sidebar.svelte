<script lang="ts">
	import { goto } from '$app/navigation';
	import {
		nonDmChannels,
		dmChannels,
		activeChannelId,
		setActiveChannel,
		createChannel,
		loadChannels,
		channelsLoaded
	} from '$lib/stores/channels';
	import { unreads } from '$lib/stores/unreads';
	import { auth, doLogout } from '$lib/stores/auth';
	import { theme, toggleTheme } from '$lib/stores/theme';
	import { usersUpdateProfile, conversationsMute, conversationsUnmute } from '$lib/api';
	import Avatar from '$lib/components/Avatar.svelte';
	import EmojiPicker from '$lib/components/EmojiPicker.svelte';
	import ChannelDirectory from '$lib/components/ChannelDirectory.svelte';
	import Skeleton from '$lib/components/Skeleton.svelte';
	import type { Channel } from '$lib/types';

	let showCreateModal = $state(false);
	let newChannelName = $state('');
	let newChannelTopic = $state('');
	let showUserMenu = $state(false);
	let showStatusModal = $state(false);
	let statusText = $state('');
	let statusEmoji = $state('');
	let showStatusEmojiPicker = $state(false);
	let showDirectory = $state(false);
	let contextMenu = $state<{ x: number; y: number; channel: Channel } | null>(null);

	// Map short names back to native emoji
	const nameToEmoji: Record<string, string> = {
		'+1': '\u{1F44D}',
		'-1': '\u{1F44E}',
		heart: '\u{2764}\u{FE0F}',
		laughing: '\u{1F606}',
		eyes: '\u{1F440}',
		tada: '\u{1F389}',
		fire: '\u{1F525}',
		rocket: '\u{1F680}',
		'100': '\u{1F4AF}',
		thinking: '\u{1F914}',
		calendar: '\u{1F4C5}',
		palm_tree: '\u{1F334}',
		house: '\u{1F3E0}',
		face_with_thermometer: '\u{1F912}'
	};

	const quickStatuses = [
		{ emoji: '\u{1F4C5}', text: 'In a meeting' },
		{ emoji: '\u{1F3E0}', text: 'Working remotely' },
		{ emoji: '\u{1F334}', text: 'On vacation' },
		{ emoji: '\u{1F912}', text: 'Out sick' }
	];

	function handleContextMenu(e: MouseEvent, channel: Channel) {
		e.preventDefault();
		contextMenu = { x: e.clientX, y: e.clientY, channel };
	}

	function closeContextMenu() {
		contextMenu = null;
	}

	async function toggleMute(channel: Channel) {
		if (channel.muted === 'true') {
			await conversationsUnmute(channel.id);
		} else {
			await conversationsMute(channel.id);
		}
		await loadChannels();
		contextMenu = null;
	}

	function isMuted(channel: Channel): boolean {
		return channel.muted === 'true';
	}

	function openStatusModal() {
		showUserMenu = false;
		statusText = $auth.user?.status_text ?? '';
		statusEmoji = $auth.user?.status_emoji ?? '';
		showStatusModal = true;
	}

	function handleStatusEmojiSelect(name: string) {
		statusEmoji = nameToEmoji[name] || name;
		showStatusEmojiPicker = false;
	}

	async function saveStatus() {
		await usersUpdateProfile({
			status_text: statusText,
			status_emoji: statusEmoji
		});
		if ($auth.user) {
			$auth.user.status_text = statusText;
			$auth.user.status_emoji = statusEmoji;
		}
		showStatusModal = false;
	}

	function applyQuickStatus(emoji: string, text: string) {
		statusEmoji = emoji;
		statusText = text;
	}

	async function clearStatus() {
		statusEmoji = '';
		statusText = '';
		await usersUpdateProfile({ status_text: '', status_emoji: '' });
		if ($auth.user) {
			$auth.user.status_text = '';
			$auth.user.status_emoji = '';
		}
		showStatusModal = false;
	}

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

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="flex h-full flex-col" onclick={closeContextMenu} onkeydown={closeContextMenu}>
	<!-- Header -->
	<div class="flex items-center justify-between border-b border-primary-dark/40 px-4 py-3">
		<div class="flex items-center gap-2">
			<img src="/teide-logo.svg" alt="Teidelum" class="h-6 w-auto" />
			<h2 class="font-[Oswald] text-lg font-semibold tracking-wide text-heading">Teidelum</h2>
		</div>
		<button
			onclick={toggleTheme}
			class="rounded p-1 text-primary-light/50 hover:text-primary-lighter"
			title="Toggle theme"
		>
			{#if $theme === 'dark'}
				<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" />
				</svg>
			{:else}
				<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
				</svg>
			{/if}
		</button>
	</div>

	<!-- User area with menu -->
	{#if $auth.user}
		<div class="relative border-b border-primary-dark/40 px-4 py-2">
			<button
				onclick={() => (showUserMenu = !showUserMenu)}
				class="flex w-full items-center gap-2 rounded px-1 py-2.5 md:py-1 text-left text-sm text-primary-lighter hover:bg-primary-darker/60"
			>
				<Avatar url={$auth.user.avatar_url ?? ''} name={$auth.user.display_name || $auth.user.username || ''} size="sm" />
				<span class="truncate">{$auth.user.display_name || $auth.user.username}</span>
				<svg class="ml-auto h-3 w-3 text-primary-light/50" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
				</svg>
			</button>

			{#if showUserMenu}
				<div class="absolute bottom-full left-2 right-2 mb-1 rounded-md bg-navy-light shadow-lg ring-1 ring-primary-dark/60 z-50">
					<button
						onclick={openStatusModal}
						class="flex w-full items-center gap-2 px-3 py-3 md:py-2 text-sm text-primary-lighter/80 hover:bg-primary-darker/60 hover:text-heading rounded-t-md"
					>
						{#if $auth.user?.status_emoji}
							<span>{$auth.user.status_emoji}</span>
						{:else}
							<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.828 14.828a4 4 0 01-5.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
							</svg>
						{/if}
						{$auth.user?.status_text || 'Set status'}
					</button>
					<button
						onclick={() => {
							showUserMenu = false;
							goto('/settings');
						}}
						class="flex w-full items-center px-3 py-3 md:py-2 text-sm text-primary-lighter/80 hover:bg-primary-darker/60 hover:text-heading"
					>
						Settings
					</button>
					<button
						onclick={() => {
							showUserMenu = false;
							handleLogout();
						}}
						class="flex w-full items-center px-3 py-3 md:py-2 text-sm text-primary-lighter/80 hover:bg-primary-darker/60 hover:text-heading rounded-b-md"
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
				<div class="flex items-center gap-1">
					<button
						onclick={() => (showDirectory = true)}
						class="text-primary-light/50 hover:text-primary-lighter"
						title="Browse channels"
					>
						<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
						</svg>
					</button>
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
			</div>

			{#if !$channelsLoaded}
				<Skeleton variant="channel" count={5} />
			{:else if $nonDmChannels.length === 0}
				<p class="px-3 py-2 text-xs text-primary-light/40">No channels yet</p>
			{:else}
			{#each $nonDmChannels as channel (channel.id)}
				<button
					onclick={() => navigateToChannel(channel)}
					oncontextmenu={(e) => handleContextMenu(e, channel)}
					class="flex w-full items-center justify-between rounded px-2 py-2.5 md:py-1 text-left text-sm transition {isActive(channel.id)
						? 'bg-primary text-white'
						: channel.archived_at
							? 'text-primary-light/30 hover:bg-primary-darker/60 hover:text-primary-lighter/60'
							: isMuted(channel)
								? 'text-primary-light/40 hover:bg-primary-darker/60 hover:text-primary-lighter/60'
								: 'text-primary-lighter/80 hover:bg-primary-darker/60 hover:text-heading'}"
				>
					<span class="flex items-center truncate">
						{#if channel.archived_at}
							<svg class="mr-1 h-3 w-3 text-primary-light/30" fill="none" stroke="currentColor" viewBox="0 0 24 24">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4" />
							</svg>
						{:else if isMuted(channel)}
							<svg class="mr-1 h-3 w-3 text-primary-light/30" fill="none" stroke="currentColor" viewBox="0 0 24 24">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5.586 15H4a1 1 0 01-1-1v-4a1 1 0 011-1h1.586l4.707-4.707C10.923 3.663 12 4.109 12 5v14c0 .891-1.077 1.337-1.707.707L5.586 15z" />
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2" />
							</svg>
						{:else}
							<span class="mr-1 text-primary-light/40">#</span>
						{/if}
						{channel.name}
					</span>
					{#if !isMuted(channel) && getUnreadCount(channel.id) > 0}
						<span class="ml-1 rounded-full bg-primary-light px-1.5 text-xs font-bold text-white">
							{getUnreadCount(channel.id)}
						</span>
					{/if}
				</button>
			{/each}
			{/if}
		</div>

		<!-- DMs section -->
		<div class="px-2 pt-4">
			<div class="px-2 pb-1">
				<span class="text-xs font-semibold uppercase tracking-wide text-primary-light/50">Direct Messages</span>
			</div>

			{#if $channelsLoaded && $dmChannels.length === 0}
				<p class="px-3 py-2 text-xs text-primary-light/40">No direct messages yet</p>
			{:else}
				{#each $dmChannels as channel (channel.id)}
					<button
						onclick={() => navigateToChannel(channel)}
						class="flex w-full items-center justify-between rounded px-2 py-2.5 md:py-1 text-left text-sm transition {isActive(channel.id)
							? 'bg-primary text-white'
							: 'text-primary-lighter/80 hover:bg-primary-darker/60 hover:text-heading'}"
					>
						<span class="truncate">{getDmDisplayName(channel)}</span>
						{#if getUnreadCount(channel.id) > 0}
							<span class="ml-1 rounded-full bg-primary-light px-1.5 text-xs font-bold text-white">
								{getUnreadCount(channel.id)}
							</span>
						{/if}
					</button>
				{/each}
			{/if}
		</div>
	</div>
</div>

<!-- Context menu for channels -->
{#if contextMenu}
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="fixed inset-0 z-50"
		onclick={closeContextMenu}
		onkeydown={closeContextMenu}
	>
		<div
			class="absolute rounded-md bg-navy-light py-1 shadow-lg ring-1 ring-primary-dark/60"
			style="left: {contextMenu.x}px; top: {contextMenu.y}px;"
		>
			<button
				onclick={() => contextMenu && toggleMute(contextMenu.channel)}
				class="flex w-full items-center gap-2 px-4 py-2 text-sm text-primary-lighter/80 hover:bg-primary-darker/60 hover:text-heading"
			>
				{#if isMuted(contextMenu.channel)}
					<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.536 8.464a5 5 0 010 7.072M18.364 5.636a9 9 0 010 12.728M5.586 15H4a1 1 0 01-1-1v-4a1 1 0 011-1h1.586l4.707-4.707C10.923 3.663 12 4.109 12 5v14c0 .891-1.077 1.337-1.707.707L5.586 15z" />
					</svg>
					Unmute channel
				{:else}
					<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5.586 15H4a1 1 0 01-1-1v-4a1 1 0 011-1h1.586l4.707-4.707C10.923 3.663 12 4.109 12 5v14c0 .891-1.077 1.337-1.707.707L5.586 15z" />
						<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2" />
					</svg>
					Mute channel
				{/if}
			</button>
		</div>
	</div>
{/if}

<!-- Create channel modal -->
{#if showCreateModal}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-overlay">
		<div class="w-full max-w-md rounded-lg bg-navy-light p-6 shadow-xl">
			<h3 class="mb-4 font-[Oswald] text-lg font-semibold text-heading">Create Channel</h3>

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
						class="rounded px-4 py-2 text-sm text-primary-lighter/70 hover:text-heading"
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

<!-- Set status modal -->
{#if showStatusModal}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-overlay">
		<div class="w-full max-w-sm rounded-lg bg-navy-light p-6 shadow-xl">
			<h3 class="mb-4 font-[Oswald] text-lg font-semibold text-heading">Set a status</h3>

			<div class="mb-3 flex items-center gap-2">
				<div class="relative">
					<button
						onclick={() => (showStatusEmojiPicker = !showStatusEmojiPicker)}
						class="flex h-10 w-10 items-center justify-center rounded bg-navy text-lg hover:bg-navy/80"
						title="Pick emoji"
					>
						{statusEmoji || '\u{1F642}'}
					</button>
					{#if showStatusEmojiPicker}
						<div class="absolute bottom-full left-0 z-50 mb-2">
							<EmojiPicker onSelect={handleStatusEmojiSelect} />
						</div>
					{/if}
				</div>
				<input
					type="text"
					bind:value={statusText}
					class="flex-1 rounded bg-navy px-3 py-2 text-sm text-white placeholder-primary-light/40 focus:outline-none focus:ring-2 focus:ring-primary"
					placeholder="What's your status?"
					maxlength="100"
				/>
			</div>

			<!-- Quick statuses -->
			<div class="mb-4 space-y-1">
				{#each quickStatuses as qs}
					<button
						onclick={() => applyQuickStatus(qs.emoji, qs.text)}
						class="flex w-full items-center gap-2 rounded px-2 py-1.5 text-sm text-primary-lighter/80 hover:bg-primary-darker/60 hover:text-heading"
					>
						<span>{qs.emoji}</span>
						<span>{qs.text}</span>
					</button>
				{/each}
			</div>

			<div class="flex justify-between pt-2">
				<button
					onclick={clearStatus}
					class="rounded px-3 py-2 text-sm text-primary-lighter/70 hover:text-heading"
				>
					Clear status
				</button>
				<div class="flex gap-2">
					<button
						onclick={() => (showStatusModal = false)}
						class="rounded px-4 py-2 text-sm text-primary-lighter/70 hover:text-heading"
					>
						Cancel
					</button>
					<button
						onclick={saveStatus}
						class="rounded bg-primary px-4 py-2 text-sm font-medium text-white hover:bg-primary-light"
					>
						Save
					</button>
				</div>
			</div>
		</div>
	</div>
{/if}

{#if showDirectory}
	<ChannelDirectory onClose={() => (showDirectory = false)} />
{/if}
