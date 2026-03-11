<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { messagesByChannel, loadMessages, loadOlderMessages, editMessage, deleteMessage } from '$lib/stores/messages';
	import { users } from '$lib/stores/users';
	import { auth } from '$lib/stores/auth';
	import { reactionsAdd, reactionsRemove, pinsAdd, pinsRemove, fileDownloadUrl } from '$lib/api';
	import { renderMarkdown, onHighlightReady } from '$lib/markdown';
	import Avatar from '$lib/components/Avatar.svelte';
	import MessageContextMenu from '$lib/components/MessageContextMenu.svelte';
	import ImageLightbox from '$lib/components/ImageLightbox.svelte';
	import LinkPreview from '$lib/components/LinkPreview.svelte';
	import UserProfilePopover from '$lib/components/UserProfilePopover.svelte';
	import Skeleton from '$lib/components/Skeleton.svelte';
	import type { Message, Id } from '$lib/types';

	const URL_REGEX = /https?:\/\/[^\s<>"')\]]+/g;

	interface Props {
		channelId: Id;
		onOpenThread?: (msg: Message) => void;
		pinnedMessageIds?: Set<Id>;
	}

	let { channelId, onOpenThread, pinnedMessageIds = new Set() }: Props = $props();

	let scrollContainer: HTMLDivElement | undefined = $state();
	let isAtBottom = $state(true);
	let prevMessageCount = $state(0);

	// Inline edit state
	let editingMessageId = $state<Id | null>(null);
	let editText = $state('');

	// Delete confirmation state
	let deletingMessage = $state<Message | null>(null);

	// Image lightbox state
	let lightboxSrc = $state<string | null>(null);
	let lightboxAlt = $state('');

	// Re-render trigger for async Shiki highlighting
	let highlightVersion = $state(0);
	$effect(() => {
		return onHighlightReady(() => { highlightVersion++; });
	});
	function renderMd(text: string): string {
		void highlightVersion;
		return renderMarkdown(text);
	}

	// User profile popover state
	let popoverUserId = $state<Id | null>(null);
	let popoverAnchorRect = $state<{ top: number; left: number; bottom: number; right: number } | null>(null);

	function openProfilePopover(userId: Id, e: MouseEvent) {
		const el = e.currentTarget as HTMLElement;
		const rect = el.getBoundingClientRect();
		popoverAnchorRect = { top: rect.top, left: rect.left, bottom: rect.bottom, right: rect.right };
		popoverUserId = userId;
	}

	const channelState = $derived($messagesByChannel.get(channelId));
	const messages = $derived(channelState?.messages ?? []);
	const hasMore = $derived(channelState?.hasMore ?? false);
	const loading = $derived(channelState?.loading ?? false);
	const currentUserId = $derived($auth.userId ?? '');

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

	function getUser(userId: Id) {
		return $users.get(userId);
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
			return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
		} catch {
			return '';
		}
	}

	function formatDate(timestamp: string): string {
		try {
			const date = parseTimestamp(timestamp);
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
		const existing = msg.reactions?.find((r) => r.name === emoji);
		if (existing && currentUserId && existing.users.includes(currentUserId)) {
			await reactionsRemove(emoji, msg.id);
		} else {
			await reactionsAdd(emoji, msg.id);
		}
	}

	// Inline edit handlers
	export function editLastOwnMessage() {
		const ownMessages = messages.filter(
			(m) => m.user_id === currentUserId && !m.thread_ts
		);
		if (ownMessages.length > 0) {
			startEdit(ownMessages[ownMessages.length - 1]);
		}
	}

	function startEdit(msg: Message) {
		editingMessageId = msg.id;
		editText = msg.text;
	}

	function cancelEdit() {
		editingMessageId = null;
		editText = '';
	}

	async function saveEdit() {
		if (!editingMessageId || !editText.trim()) return;
		await editMessage(editingMessageId, editText.trim());
		editingMessageId = null;
		editText = '';
	}

	function handleEditKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			saveEdit();
		} else if (e.key === 'Escape') {
			cancelEdit();
		}
	}

	// Delete handlers
	async function confirmDelete() {
		if (!deletingMessage) return;
		await deleteMessage(deletingMessage.id);
		deletingMessage = null;
	}

	// Pin handlers
	async function pinMessage(msg: Message) {
		await pinsAdd(channelId, msg.id);
	}

	async function unpinMessage(msg: Message) {
		await pinsRemove(channelId, msg.id);
	}

	function extractUrls(text: string): string[] {
		const matches = text.match(URL_REGEX);
		if (!matches) return [];
		// Deduplicate, max 3
		return [...new Set(matches)].slice(0, 3);
	}

	onMount(() => {
		tick().then(scrollToBottom);
	});
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="flex-1 overflow-y-auto px-4 py-2"
	bind:this={scrollContainer}
	onscroll={handleScroll}
	onclick={(e: MouseEvent) => {
		const btn = (e.target as HTMLElement).closest('.code-copy-btn') as HTMLElement | null;
		if (btn) {
			const code = btn.getAttribute('data-code');
			if (code) {
				const decoded = code.replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"');
				navigator.clipboard.writeText(decoded);
				const prev = btn.textContent;
				btn.textContent = 'Copied!';
				setTimeout(() => { btn.textContent = prev; }, 2000);
			}
		}
	}}
>
	{#if loading && messages.length === 0}
		<div class="px-1 py-2">
			<Skeleton variant="message" count={5} />
		</div>
	{:else if messages.length === 0}
		<div class="flex h-full flex-col items-center justify-center gap-2 text-primary-light/50">
			<svg class="h-10 w-10 opacity-40" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
			</svg>
			<span class="text-sm">No messages yet. Start the conversation!</span>
		</div>
	{:else}
		{#if loading && hasMore}
			<div class="py-2 text-center text-sm text-primary-light/50">Loading older messages...</div>
		{/if}

		{#each messages as msg, idx}
			{#if shouldShowDateSeparator(idx)}
				<div class="my-4 flex items-center">
					<div class="flex-1 border-t border-primary-dark/40"></div>
					<span class="px-3 text-xs text-primary-light/50">{formatDate(msg.created_at)}</span>
					<div class="flex-1 border-t border-primary-dark/40"></div>
				</div>
			{/if}

			<div class="group relative flex gap-3 px-1 py-0.5 hover:bg-navy-light/50 {shouldShowAuthor(idx) ? 'mt-3' : ''}">
				{#if shouldShowAuthor(idx)}
					<!-- Avatar -->
					<button
						type="button"
						class="flex-shrink-0 pt-0.5 cursor-pointer"
						onclick={(e) => openProfilePopover(msg.user_id, e)}
					>
						<Avatar url={getUser(msg.user_id)?.avatar_url ?? ''} name={getUser(msg.user_id)?.display_name || getUser(msg.user_id)?.username || ''} size="md" />
					</button>
				{:else}
					<!-- Timestamp on hover (aligned with avatar) -->
					<div class="flex w-9 flex-shrink-0 items-center justify-center">
						<span class="hidden text-xs text-primary-light/40 group-hover:inline">{formatTime(msg.created_at)}</span>
					</div>
				{/if}

				<div class="min-w-0 flex-1">
					{#if shouldShowAuthor(idx)}
						<div class="flex items-baseline gap-2">
							<button
								type="button"
								class="text-sm font-bold text-gray-200 hover:underline cursor-pointer"
								onclick={(e) => openProfilePopover(msg.user_id, e)}
							>{getUserName(msg.user_id)}</button>
							{#if getUser(msg.user_id)?.status_emoji}
								<span class="text-sm" title={getUser(msg.user_id)?.status_text || ''}>{getUser(msg.user_id)?.status_emoji}</span>
							{/if}
							<span class="text-xs text-primary-light/40">{formatTime(msg.created_at)}</span>
							{#if msg.edited_at}
								<span class="text-xs text-primary-light/40">(edited)</span>
							{/if}
						</div>
					{/if}

					<!-- Message text or inline edit -->
					{#if editingMessageId === msg.id}
						<div class="mt-1">
							<textarea
								bind:value={editText}
								onkeydown={handleEditKeydown}
								class="w-full resize-none rounded border border-primary-dark/40 bg-navy px-3 py-2 text-sm text-white placeholder-primary-light/40 focus:border-primary focus:outline-none"
								rows="2"
							></textarea>
							<div class="mt-1 flex gap-2">
								<button
									onclick={saveEdit}
									class="rounded bg-primary px-3 py-1 text-xs font-medium text-white hover:bg-primary-light"
								>
									Save
								</button>
								<button
									onclick={cancelEdit}
									class="rounded px-3 py-1 text-xs text-primary-lighter/70 hover:text-heading"
								>
									Cancel
								</button>
								<span class="ml-auto text-xs text-primary-light/40">Enter to save, Esc to cancel</span>
							</div>
						</div>
					{:else}
						<div class="prose-chat text-sm leading-relaxed text-gray-300 break-words">{@html renderMd(msg.text)}</div>
					{/if}

					<!-- Link previews -->
					{#if !editingMessageId || editingMessageId !== msg.id}
						{#each extractUrls(msg.text) as linkUrl}
							<LinkPreview url={linkUrl} />
						{/each}
					{/if}

					<!-- File attachments -->
					{#if msg.files && msg.files.length > 0}
						<div class="mt-1 flex flex-col gap-2">
							{#each msg.files as file}
								{#if file.mime_type.startsWith('image/')}
									<button
										type="button"
										class="cursor-pointer"
										onclick={() => { lightboxSrc = fileDownloadUrl(file.id, file.filename); lightboxAlt = file.filename; }}
									>
										<img
											src={fileDownloadUrl(file.id, file.filename)}
											alt={file.filename}
											class="max-w-[400px] max-h-[300px] object-contain rounded"
										/>
									</button>
								{:else}
									<a
										href={fileDownloadUrl(file.id, file.filename)}
										target="_blank"
										rel="noopener noreferrer"
										class="inline-flex items-center gap-1.5 text-xs text-primary-lighter hover:underline"
									>
										<svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
											<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13" />
										</svg>
										{file.filename}
										<span class="text-primary-light/40">({Math.round(file.size_bytes / 1024)}KB)</span>
									</a>
								{/if}
							{/each}
						</div>
					{/if}

					<!-- Reactions -->
					{#if msg.reactions && msg.reactions.length > 0}
						<div class="mt-1 flex flex-wrap gap-1">
							{#each msg.reactions as reaction}
								<button
									onclick={() => toggleReaction(msg, reaction.name)}
									class="inline-flex items-center gap-1 rounded-full border border-primary-dark/40 bg-navy-light px-2 py-0.5 text-xs transition hover:border-primary"
								>
									<span>{reaction.name}</span>
									<span class="text-primary-light/60">{reaction.count}</span>
								</button>
							{/each}
						</div>
					{/if}

					<!-- Thread indicator -->
					{#if msg.reply_count && msg.reply_count > 0}
						<button
							onclick={() => onOpenThread?.(msg)}
							class="mt-1 text-xs text-primary-lighter hover:underline"
						>
							{msg.reply_count} {msg.reply_count === 1 ? 'reply' : 'replies'}
						</button>
					{/if}
				</div>

				<!-- Message context menu (hover) -->
				<div class="absolute -top-3 right-2 hidden group-hover:block">
					<MessageContextMenu
						message={msg}
						{currentUserId}
						isPinned={pinnedMessageIds.has(msg.id)}
						onReply={() => onOpenThread?.(msg)}
						onReact={(emoji) => toggleReaction(msg, emoji)}
						onEdit={() => startEdit(msg)}
						onDelete={() => (deletingMessage = msg)}
						onPin={() => pinMessage(msg)}
						onUnpin={() => unpinMessage(msg)}
					/>
				</div>
			</div>
		{/each}
	{/if}
</div>

<!-- Delete confirmation dialog -->
{#if deletingMessage}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-overlay">
		<div class="w-full max-w-sm rounded-lg bg-navy-light p-6 shadow-xl">
			<h3 class="mb-2 text-lg font-semibold text-heading">Delete Message</h3>
			<p class="mb-4 text-sm text-primary-lighter/70">
				Delete this message? This can't be undone.
			</p>
			<div class="mb-4 rounded bg-navy p-3 text-sm text-gray-300">
				{deletingMessage.text.length > 100 ? deletingMessage.text.slice(0, 100) + '...' : deletingMessage.text}
			</div>
			<div class="flex justify-end gap-2">
				<button
					onclick={() => (deletingMessage = null)}
					class="rounded px-4 py-2 text-sm text-primary-lighter/70 hover:text-heading"
				>
					Cancel
				</button>
				<button
					onclick={confirmDelete}
					class="rounded bg-red-600 px-4 py-2 text-sm font-medium text-white hover:bg-red-500"
				>
					Delete
				</button>
			</div>
		</div>
	</div>
{/if}

{#if lightboxSrc}
	<ImageLightbox src={lightboxSrc} alt={lightboxAlt} onClose={() => { lightboxSrc = null; }} />
{/if}

{#if popoverUserId && popoverAnchorRect}
	<UserProfilePopover
		userId={popoverUserId}
		anchorRect={popoverAnchorRect}
		onClose={() => { popoverUserId = null; popoverAnchorRect = null; }}
	/>
{/if}
