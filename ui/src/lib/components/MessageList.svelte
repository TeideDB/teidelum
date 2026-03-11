<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { messagesByChannel, loadMessages, loadOlderMessages } from '$lib/stores/messages';
	import { users } from '$lib/stores/users';
	import { auth } from '$lib/stores/auth';
	import { reactionsAdd, reactionsRemove } from '$lib/api';
	import type { Message, Id } from '$lib/types';

	interface Props {
		channelId: Id;
		onOpenThread?: (msg: Message) => void;
	}

	let { channelId, onOpenThread }: Props = $props();

	let scrollContainer: HTMLDivElement | undefined = $state();
	let isAtBottom = $state(true);
	let prevMessageCount = $state(0);

	const channelState = $derived($messagesByChannel.get(channelId));
	const messages = $derived(channelState?.messages ?? []);
	const hasMore = $derived(channelState?.hasMore ?? false);
	const loading = $derived(channelState?.loading ?? false);

	$effect(() => {
		// Load messages when channelId changes
		channelId; // track
		loadMessages(channelId);
	});

	$effect(() => {
		// Auto-scroll to bottom when new messages arrive (if already at bottom)
		if (messages.length > prevMessageCount && isAtBottom) {
			tick().then(() => {
				scrollToBottom();
			});
		}
		prevMessageCount = messages.length;
	});

	function scrollToBottom() {
		if (scrollContainer) {
			scrollContainer.scrollTop = scrollContainer.scrollHeight;
		}
	}

	function handleScroll() {
		if (!scrollContainer) return;

		const { scrollTop, scrollHeight, clientHeight } = scrollContainer;
		isAtBottom = scrollHeight - scrollTop - clientHeight < 50;

		// Load older messages when scrolled to top
		if (scrollTop < 100 && hasMore && !loading) {
			loadOlderMessages(channelId);
		}
	}

	function getUserName(userId: Id): string {
		const user = $users.get(userId);
		return user?.display_name || user?.username || userId;
	}

	function getUserAvatar(userId: Id): string {
		const user = $users.get(userId);
		return user?.display_name?.[0]?.toUpperCase() || user?.username?.[0]?.toUpperCase() || '?';
	}

	function formatTime(timestamp: string): string {
		try {
			const date = new Date(timestamp);
			return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
		} catch {
			return '';
		}
	}

	function formatDate(timestamp: string): string {
		try {
			const date = new Date(timestamp);
			const today = new Date();
			if (date.toDateString() === today.toDateString()) return 'Today';
			const yesterday = new Date(today);
			yesterday.setDate(yesterday.getDate() - 1);
			if (date.toDateString() === yesterday.toDateString()) return 'Yesterday';
			return date.toLocaleDateString([], { month: 'short', day: 'numeric', year: 'numeric' });
		} catch {
			return '';
		}
	}

	function shouldShowDateSeparator(idx: number): boolean {
		if (idx === 0) return true;
		const curr = messages[idx];
		const prev = messages[idx - 1];
		if (!curr.created_at || !prev.created_at) return false;
		return formatDate(curr.created_at) !== formatDate(prev.created_at);
	}

	function shouldShowAuthor(idx: number): boolean {
		if (idx === 0) return true;
		const curr = messages[idx];
		const prev = messages[idx - 1];
		return curr.user_id !== prev.user_id || shouldShowDateSeparator(idx);
	}

	async function toggleReaction(msg: Message, emoji: string) {
		const currentUserId = $auth.userId;
		const existing = msg.reactions?.find((r) => r.name === emoji);
		if (existing && currentUserId && existing.users.includes(currentUserId)) {
			await reactionsRemove(emoji, msg.id);
		} else {
			await reactionsAdd(emoji, msg.id);
		}
	}

	onMount(() => {
		tick().then(scrollToBottom);
	});
</script>

<div
	class="flex-1 overflow-y-auto px-4 py-2"
	bind:this={scrollContainer}
	onscroll={handleScroll}
>
	{#if loading && messages.length === 0}
		<div class="flex h-full items-center justify-center text-gray-500">Loading messages...</div>
	{:else if messages.length === 0}
		<div class="flex h-full items-center justify-center text-gray-500">
			No messages yet. Start the conversation!
		</div>
	{:else}
		{#if loading && hasMore}
			<div class="py-2 text-center text-sm text-gray-500">Loading older messages...</div>
		{/if}

		{#each messages as msg, idx}
			{#if shouldShowDateSeparator(idx)}
				<div class="my-4 flex items-center">
					<div class="flex-1 border-t border-gray-700"></div>
					<span class="px-3 text-xs text-gray-500">{formatDate(msg.created_at)}</span>
					<div class="flex-1 border-t border-gray-700"></div>
				</div>
			{/if}

			<div class="group relative flex gap-3 px-1 py-0.5 hover:bg-gray-800/50 {shouldShowAuthor(idx) ? 'mt-3' : ''}">
				{#if shouldShowAuthor(idx)}
					<!-- Avatar -->
					<div class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-lg bg-blue-600 text-sm font-bold text-white">
						{getUserAvatar(msg.user_id)}
					</div>
				{:else}
					<!-- Timestamp on hover (aligned with avatar) -->
					<div class="flex w-9 flex-shrink-0 items-center justify-center">
						<span class="hidden text-xs text-gray-600 group-hover:inline">{formatTime(msg.created_at)}</span>
					</div>
				{/if}

				<div class="min-w-0 flex-1">
					{#if shouldShowAuthor(idx)}
						<div class="flex items-baseline gap-2">
							<span class="text-sm font-bold text-gray-200">{getUserName(msg.user_id)}</span>
							<span class="text-xs text-gray-600">{formatTime(msg.created_at)}</span>
							{#if msg.edited_at}
								<span class="text-xs text-gray-600">(edited)</span>
							{/if}
						</div>
					{/if}

					<div class="text-sm leading-relaxed text-gray-300 break-words">{msg.text}</div>

					<!-- Reactions -->
					{#if msg.reactions && msg.reactions.length > 0}
						<div class="mt-1 flex flex-wrap gap-1">
							{#each msg.reactions as reaction}
								<button
									onclick={() => toggleReaction(msg, reaction.name)}
									class="inline-flex items-center gap-1 rounded-full border border-gray-700 bg-gray-800 px-2 py-0.5 text-xs transition hover:border-blue-500"
								>
									<span>{reaction.name}</span>
									<span class="text-gray-400">{reaction.count}</span>
								</button>
							{/each}
						</div>
					{/if}

					<!-- Thread indicator -->
					{#if msg.reply_count && msg.reply_count > 0}
						<button
							onclick={() => onOpenThread?.(msg)}
							class="mt-1 text-xs text-blue-400 hover:underline"
						>
							{msg.reply_count} {msg.reply_count === 1 ? 'reply' : 'replies'}
						</button>
					{/if}
				</div>

				<!-- Message actions (hover) -->
				<div class="absolute -top-3 right-2 hidden gap-1 rounded border border-gray-700 bg-gray-800 p-0.5 shadow group-hover:flex">
					<button
						onclick={() => toggleReaction(msg, '+1')}
						class="rounded p-1 text-gray-500 hover:bg-gray-700 hover:text-gray-300"
						title="React"
					>
						<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.828 14.828a4 4 0 01-5.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
						</svg>
					</button>
					{#if onOpenThread}
						<button
							onclick={() => onOpenThread?.(msg)}
							class="rounded p-1 text-gray-500 hover:bg-gray-700 hover:text-gray-300"
							title="Reply in thread"
						>
							<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
								<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
							</svg>
						</button>
					{/if}
				</div>
			</div>
		{/each}
	{/if}
</div>
