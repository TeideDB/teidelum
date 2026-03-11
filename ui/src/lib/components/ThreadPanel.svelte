<script lang="ts">
	import { onMount } from 'svelte';
	import * as api from '$lib/api';
	import { users } from '$lib/stores/users';
	import { sendMessage } from '$lib/stores/messages';
	import { sendTyping } from '$lib/ws';
	import type { Message, Id } from '$lib/types';

	interface Props {
		channelId: Id;
		parentMessage: Message;
		onClose: () => void;
	}

	let { channelId, parentMessage, onClose }: Props = $props();

	let replies = $state<Message[]>([]);
	let loading = $state(true);
	let replyText = $state('');
	let lastTypingSent = $state(0);

	$effect(() => {
		// Reload replies when parent message changes
		parentMessage.id; // track
		loadReplies();
	});

	async function loadReplies() {
		loading = true;
		const res = await api.conversationsReplies(channelId, parentMessage.id);
		if (res.ok && res.messages) {
			// First message is the parent; rest are replies
			replies = res.messages.slice(1);
		}
		loading = false;
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

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSendReply();
		}
	}

	function handleInput() {
		const now = Date.now();
		if (now - lastTypingSent > 3000) {
			sendTyping(channelId);
			lastTypingSent = now;
		}
	}

	async function handleSendReply() {
		const trimmed = replyText.trim();
		if (!trimmed) return;

		replyText = '';
		await sendMessage(channelId, trimmed, parentMessage.id);
		// Reload replies to show the new one
		await loadReplies();
	}
</script>

<div class="flex h-full flex-col">
	<!-- Thread header -->
	<div class="flex items-center justify-between border-b border-gray-700 px-4 py-3">
		<h3 class="font-bold text-white">Thread</h3>
		<button onclick={onClose} aria-label="Close thread" class="text-gray-500 hover:text-gray-300">
			<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
			</svg>
		</button>
	</div>

	<!-- Parent message -->
	<div class="border-b border-gray-700 px-4 py-3">
		<div class="flex gap-3">
			<div class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-lg bg-blue-600 text-sm font-bold text-white">
				{getUserAvatar(parentMessage.user_id)}
			</div>
			<div>
				<div class="flex items-baseline gap-2">
					<span class="text-sm font-bold text-gray-200">{getUserName(parentMessage.user_id)}</span>
					<span class="text-xs text-gray-600">{formatTime(parentMessage.created_at)}</span>
				</div>
				<div class="text-sm text-gray-300">{parentMessage.text}</div>
			</div>
		</div>
	</div>

	<!-- Replies -->
	<div class="flex-1 overflow-y-auto px-4 py-2">
		{#if loading}
			<div class="py-4 text-center text-sm text-gray-500">Loading replies...</div>
		{:else if replies.length === 0}
			<div class="py-4 text-center text-sm text-gray-500">No replies yet</div>
		{:else}
			{#each replies as reply}
				<div class="flex gap-3 py-2">
					<div class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-lg bg-blue-600 text-xs font-bold text-white">
						{getUserAvatar(reply.user_id)}
					</div>
					<div>
						<div class="flex items-baseline gap-2">
							<span class="text-sm font-bold text-gray-200">{getUserName(reply.user_id)}</span>
							<span class="text-xs text-gray-600">{formatTime(reply.created_at)}</span>
						</div>
						<div class="text-sm text-gray-300">{reply.text}</div>
					</div>
				</div>
			{/each}
		{/if}
	</div>

	<!-- Reply input -->
	<div class="border-t border-gray-700 px-4 py-3">
		<div class="flex items-end gap-2 rounded-lg bg-gray-700 px-3 py-2">
			<textarea
				bind:value={replyText}
				onkeydown={handleKeydown}
				oninput={handleInput}
				placeholder="Reply..."
				rows="1"
				class="max-h-[120px] flex-1 resize-none bg-transparent text-sm text-white placeholder-gray-500 focus:outline-none"
			></textarea>
			<button
				onclick={handleSendReply}
				disabled={!replyText.trim()}
				aria-label="Send reply"
				class="flex-shrink-0 rounded p-1 text-gray-500 transition hover:text-blue-400 disabled:opacity-30"
			>
				<svg class="h-5 w-5" fill="currentColor" viewBox="0 0 24 24">
					<path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z" />
				</svg>
			</button>
		</div>
	</div>
</div>
