<script lang="ts">
	import ReactionPicker from '$lib/components/ReactionPicker.svelte';
	import type { Message, Id } from '$lib/types';

	interface Props {
		message: Message;
		currentUserId: Id;
		isPinned?: boolean;
		onReply?: () => void;
		onReact: (emoji: string) => void;
		onEdit?: () => void;
		onDelete?: () => void;
		onPin?: () => void;
		onUnpin?: () => void;
	}

	let {
		message,
		currentUserId,
		isPinned = false,
		onReply,
		onReact,
		onEdit,
		onDelete,
		onPin,
		onUnpin
	}: Props = $props();

	let showReactionPicker = $state(false);

	const isOwnMessage = $derived(message.user_id === currentUserId);

	function copyText() {
		navigator.clipboard.writeText(message.text);
	}
</script>

<div class="flex gap-0.5 rounded border border-primary-dark/40 bg-navy-light p-0.5 shadow-lg">
	<!-- React -->
	<div class="relative">
		<button
			onclick={() => (showReactionPicker = !showReactionPicker)}
			class="rounded p-1 text-primary-light/50 hover:bg-navy-mid hover:text-primary-lighter"
			title="Add reaction"
		>
			<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.828 14.828a4 4 0 01-5.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
			</svg>
		</button>
		{#if showReactionPicker}
			<div class="absolute -top-2 right-0 z-50 -translate-y-full">
				<ReactionPicker
					onSelect={(emoji) => {
						onReact(emoji);
						showReactionPicker = false;
					}}
					onClose={() => (showReactionPicker = false)}
				/>
			</div>
		{/if}
	</div>

	<!-- Reply in thread -->
	{#if onReply}
		<button
			onclick={onReply}
			class="rounded p-1 text-primary-light/50 hover:bg-navy-mid hover:text-primary-lighter"
			title="Reply in thread"
		>
			<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
			</svg>
		</button>
	{/if}

	<!-- Pin/Unpin -->
	{#if isPinned && onUnpin}
		<button
			onclick={onUnpin}
			class="rounded p-1 text-primary-light/50 hover:bg-navy-mid hover:text-primary-lighter"
			title="Unpin message"
		>
			<svg class="h-4 w-4" fill="currentColor" viewBox="0 0 24 24">
				<path d="M16 12V4h1V2H7v2h1v8l-2 2v2h5.2v6h1.6v-6H18v-2l-2-2z" />
			</svg>
		</button>
	{:else if onPin}
		<button
			onclick={onPin}
			class="rounded p-1 text-primary-light/50 hover:bg-navy-mid hover:text-primary-lighter"
			title="Pin message"
		>
			<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 12V4h1V2H7v2h1v8l-2 2v2h5.2v6h1.6v-6H18v-2l-2-2z" />
			</svg>
		</button>
	{/if}

	<!-- Edit (own messages only) -->
	{#if isOwnMessage && onEdit}
		<button
			onclick={onEdit}
			class="rounded p-1 text-primary-light/50 hover:bg-navy-mid hover:text-primary-lighter"
			title="Edit message"
		>
			<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
			</svg>
		</button>
	{/if}

	<!-- Delete (own messages only) -->
	{#if isOwnMessage && onDelete}
		<button
			onclick={onDelete}
			class="rounded p-1 text-primary-light/50 hover:bg-navy-mid hover:text-red-400"
			title="Delete message"
		>
			<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
			</svg>
		</button>
	{/if}

	<!-- Copy text -->
	<button
		onclick={copyText}
		class="rounded p-1 text-primary-light/50 hover:bg-navy-mid hover:text-primary-lighter"
		title="Copy text"
	>
		<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
			<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 16H6a2 2 0 01-2-2V6a2 2 0 012-2h8a2 2 0 012 2v2m-6 12h8a2 2 0 002-2v-8a2 2 0 00-2-2h-8a2 2 0 00-2 2v8a2 2 0 002 2z" />
		</svg>
	</button>
</div>
