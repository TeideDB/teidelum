<script lang="ts">
	import { onMount } from 'svelte';
	import * as api from '$lib/api';
	const { fileDownloadUrl, usersSearch, conversationsAutocomplete } = api;
	import { users } from '$lib/stores/users';
	import { sendMessage } from '$lib/stores/messages';
	import { sendTyping } from '$lib/ws';
	import { renderMarkdown } from '$lib/markdown';
	import Autocomplete from './Autocomplete.svelte';
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
	let replyTextarea: HTMLTextAreaElement | undefined = $state();
	let lastTypingSent = $state(0);
	let loadSeq = 0;

	// Autocomplete state
	let autocompleteVisible = $state(false);
	let autocompleteItems = $state<Array<{ id: string; label: string; secondary?: string; avatar?: string }>>([]);
	let autocompleteTrigger = $state<'@' | '#' | null>(null);
	let triggerStart = $state(0);
	let debounceTimer: ReturnType<typeof setTimeout> | undefined;
	let autocompleteRef: Autocomplete | undefined = $state();

	$effect(() => {
		// Reload replies when parent message changes
		parentMessage.id; // track
		loadReplies();
	});

	async function loadReplies() {
		const seq = ++loadSeq;
		loading = true;
		try {
			const res = await api.conversationsReplies(channelId, parentMessage.id);
			// Discard stale responses if parent changed during fetch
			if (seq !== loadSeq) return;
			if (res.ok && res.messages) {
				// First message is the parent; rest are replies
				replies = res.messages.slice(1);
			}
		} catch (e) {
			if (seq !== loadSeq) return;
			console.error('Failed to load replies:', e);
		} finally {
			if (seq === loadSeq) {
				loading = false;
			}
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

	function handleKeydown(e: KeyboardEvent) {
		if (autocompleteRef && autocompleteRef.handleKeydown(e)) {
			return;
		}

		if (e.key === 'Escape' && autocompleteVisible) {
			e.preventDefault();
			closeAutocomplete();
			return;
		}

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
		checkAutocomplete();
	}

	function checkAutocomplete() {
		if (!replyTextarea) return;

		const cursorPos = replyTextarea.selectionStart;
		const value = replyTextarea.value;

		let i = cursorPos - 1;
		while (i >= 0 && /\S/.test(value[i]) && value[i] !== '@' && value[i] !== '#') {
			i--;
		}

		if (i >= 0 && (value[i] === '@' || value[i] === '#')) {
			if (i === 0 || /\s/.test(value[i - 1])) {
				const trigger = value[i] as '@' | '#';
				const query = value.substring(i + 1, cursorPos);

				if (query.length > 0) {
					autocompleteTrigger = trigger;
					triggerStart = i;
					debouncedSearch(trigger, query);
					return;
				}
			}
		}

		closeAutocomplete();
	}

	function debouncedSearch(trigger: '@' | '#', query: string) {
		if (debounceTimer) clearTimeout(debounceTimer);
		debounceTimer = setTimeout(() => doSearch(trigger, query), 200);
	}

	async function doSearch(trigger: '@' | '#', query: string) {
		try {
			if (trigger === '@') {
				const res = await usersSearch(query);
				if (res.ok && res.users) {
					autocompleteItems = res.users.map((u) => ({
						id: u.id,
						label: u.username,
						secondary: u.display_name !== u.username ? u.display_name : undefined,
						avatar: (u.display_name || u.username)[0]?.toUpperCase()
					}));
				}
			} else {
				const res = await conversationsAutocomplete(query);
				if (res.ok && res.channels) {
					autocompleteItems = res.channels.map((c) => ({
						id: c.id,
						label: c.name,
						secondary: c.topic || undefined
					}));
				}
			}
			autocompleteVisible = autocompleteItems.length > 0;
		} catch {
			closeAutocomplete();
		}
	}

	function handleAutocompleteSelect(item: { id: string; label: string }) {
		if (!replyTextarea) return;

		const before = replyText.substring(0, triggerStart);
		const after = replyText.substring(replyTextarea.selectionStart);
		const prefix = autocompleteTrigger === '@' ? '@' : '#';
		replyText = before + prefix + item.label + ' ' + after;

		closeAutocomplete();

		requestAnimationFrame(() => {
			if (replyTextarea) {
				const cursorPos = before.length + prefix.length + item.label.length + 1;
				replyTextarea.selectionStart = cursorPos;
				replyTextarea.selectionEnd = cursorPos;
				replyTextarea.focus();
			}
		});
	}

	function closeAutocomplete() {
		autocompleteVisible = false;
		autocompleteItems = [];
		autocompleteTrigger = null;
		if (debounceTimer) clearTimeout(debounceTimer);
	}

	async function handleSendReply() {
		const trimmed = replyText.trim();
		if (!trimmed) return;

		replyText = '';
		closeAutocomplete();
		await sendMessage(channelId, trimmed, parentMessage.id);
		// Reload replies to show the new one
		await loadReplies();
	}
</script>

<div class="flex h-full flex-col">
	<!-- Thread header -->
	<div class="flex items-center justify-between border-b border-primary-dark/40 px-4 py-3">
		<h3 class="font-bold text-white">Thread</h3>
		<button onclick={onClose} aria-label="Close thread" class="text-primary-light/50 hover:text-primary-lighter">
			<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
			</svg>
		</button>
	</div>

	<!-- Parent message -->
	<div class="border-b border-primary-dark/40 px-4 py-3">
		<div class="flex gap-3">
			<div class="flex h-9 w-9 flex-shrink-0 items-center justify-center rounded-lg bg-primary text-sm font-bold text-white">
				{getUserAvatar(parentMessage.user_id)}
			</div>
			<div>
				<div class="flex items-baseline gap-2">
					<span class="text-sm font-bold text-gray-200">{getUserName(parentMessage.user_id)}</span>
					<span class="text-xs text-primary-light/40">{formatTime(parentMessage.created_at)}</span>
				</div>
				<div class="prose-chat text-sm text-gray-300 break-words">{@html renderMarkdown(parentMessage.text)}</div>
				{#if parentMessage.files && parentMessage.files.length > 0}
					<div class="mt-1 flex flex-col gap-1">
						{#each parentMessage.files as file}
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
						{/each}
					</div>
				{/if}
			</div>
		</div>
	</div>

	<!-- Replies -->
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div class="flex-1 overflow-y-auto px-4 py-2" onclick={(e: MouseEvent) => {
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
	}}>
		{#if loading}
			<div class="py-4 text-center text-sm text-primary-light/50">Loading replies...</div>
		{:else if replies.length === 0}
			<div class="py-4 text-center text-sm text-primary-light/50">No replies yet</div>
		{:else}
			{#each replies as reply}
				<div class="flex gap-3 py-2">
					<div class="flex h-8 w-8 flex-shrink-0 items-center justify-center rounded-lg bg-primary text-xs font-bold text-white">
						{getUserAvatar(reply.user_id)}
					</div>
					<div>
						<div class="flex items-baseline gap-2">
							<span class="text-sm font-bold text-gray-200">{getUserName(reply.user_id)}</span>
							<span class="text-xs text-primary-light/40">{formatTime(reply.created_at)}</span>
						</div>
						<div class="prose-chat text-sm text-gray-300 break-words">{@html renderMarkdown(reply.text)}</div>
						{#if reply.files && reply.files.length > 0}
							<div class="mt-1 flex flex-col gap-1">
								{#each reply.files as file}
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
								{/each}
							</div>
						{/if}
					</div>
				</div>
			{/each}
		{/if}
	</div>

	<!-- Reply input -->
	<div class="border-t border-primary-dark/40 px-4 py-3">
		<div class="relative flex items-end gap-2 rounded-lg bg-navy px-3 py-2">
			<Autocomplete
				bind:this={autocompleteRef}
				items={autocompleteItems}
				onSelect={handleAutocompleteSelect}
				visible={autocompleteVisible}
			/>
			<textarea
				bind:this={replyTextarea}
				bind:value={replyText}
				onkeydown={handleKeydown}
				oninput={handleInput}
				placeholder="Reply..."
				rows="1"
				class="max-h-[120px] flex-1 resize-none bg-transparent text-sm text-white placeholder-primary-light/40 focus:outline-none"
			></textarea>
			<button
				onclick={handleSendReply}
				disabled={!replyText.trim()}
				aria-label="Send reply"
				class="flex-shrink-0 rounded p-1 text-primary-light/50 transition hover:text-primary-lighter disabled:opacity-30"
			>
				<svg class="h-5 w-5" fill="currentColor" viewBox="0 0 24 24">
					<path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z" />
				</svg>
			</button>
		</div>
	</div>
</div>
