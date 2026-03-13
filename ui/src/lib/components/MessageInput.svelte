<script lang="ts">
	import { untrack } from 'svelte';
	import { sendTyping } from '$lib/ws';
	import { sendMessage } from '$lib/stores/messages';
	import { usersSearch, conversationsAutocomplete } from '$lib/api';
	import FileUpload from './FileUpload.svelte';
	import Autocomplete from './Autocomplete.svelte';
	import EmojiPicker from './EmojiPicker.svelte';
	import type { Id } from '$lib/types';

	interface Props {
		channelId: Id;
		threadTs?: Id;
		placeholder?: string;
		onEditLast?: () => void;
	}

	let { channelId, threadTs, placeholder = 'Type a message...', onEditLast }: Props = $props();

	let text = $state('');
	let textarea: HTMLTextAreaElement | undefined = $state();
	let lastTypingSent = $state(0);
	let showEmojiPicker = $state(false);

	function draftKey(id: Id): string {
		return `draft:${id}`;
	}

	// Save/load drafts when channelId changes
	let prevChannelId: Id | undefined;
	$effect(() => {
		const currentId = channelId; // only track channelId
		untrack(() => {
			// Save draft for previous channel
			if (prevChannelId !== undefined && prevChannelId !== currentId) {
				const trimmed = text.trim();
				if (trimmed) {
					localStorage.setItem(draftKey(prevChannelId), text);
				} else {
					localStorage.removeItem(draftKey(prevChannelId));
				}
			}
			// Load draft for new channel
			text = localStorage.getItem(draftKey(currentId)) || '';
			if (textarea) {
				textarea.style.height = 'auto';
				if (text) {
					requestAnimationFrame(() => {
						if (textarea) {
							textarea.style.height = Math.min(textarea.scrollHeight, 200) + 'px';
						}
					});
				}
			}
			prevChannelId = currentId;
		});
	});

	// Autocomplete state
	let autocompleteVisible = $state(false);
	let autocompleteItems = $state<Array<{ id: string; label: string; secondary?: string; avatar?: string }>>([]);
	let autocompleteTrigger = $state<'@' | '#' | null>(null);
	let triggerStart = $state(0);
	let debounceTimer: ReturnType<typeof setTimeout> | undefined;
	let autocompleteRef: Autocomplete | undefined = $state();

	function handleKeydown(e: KeyboardEvent) {
		// Let autocomplete handle keys first
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
			handleSend();
		}

		if (e.key === 'ArrowUp' && !text.trim() && onEditLast) {
			e.preventDefault();
			onEditLast();
		}
	}

	function handleInput() {
		// Auto-resize textarea
		if (textarea) {
			textarea.style.height = 'auto';
			textarea.style.height = Math.min(textarea.scrollHeight, 200) + 'px';
		}

		// Send typing indicator (throttled to once per 3 seconds)
		const now = Date.now();
		if (now - lastTypingSent > 3000) {
			sendTyping(channelId);
			lastTypingSent = now;
		}

		// Check for autocomplete triggers
		checkAutocomplete();
	}

	function checkAutocomplete() {
		if (!textarea) return;

		const cursorPos = textarea.selectionStart;
		const value = textarea.value;

		// Look backwards from cursor for @ or # trigger
		let i = cursorPos - 1;
		while (i >= 0 && /\S/.test(value[i]) && value[i] !== '@' && value[i] !== '#') {
			i--;
		}

		if (i >= 0 && (value[i] === '@' || value[i] === '#')) {
			// Check that trigger is at start of input or preceded by whitespace
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
		if (!textarea) return;

		const before = text.substring(0, triggerStart);
		const after = text.substring(textarea.selectionStart);
		const prefix = autocompleteTrigger === '@' ? '@' : '#';
		text = before + prefix + item.label + ' ' + after;

		closeAutocomplete();

		// Refocus and position cursor
		requestAnimationFrame(() => {
			if (textarea) {
				const cursorPos = before.length + prefix.length + item.label.length + 1;
				textarea.selectionStart = cursorPos;
				textarea.selectionEnd = cursorPos;
				textarea.focus();
			}
		});
	}

	function closeAutocomplete() {
		autocompleteVisible = false;
		autocompleteItems = [];
		autocompleteTrigger = null;
		if (debounceTimer) clearTimeout(debounceTimer);
	}

	// (Removed unused nameToEmoji)

	function handleEmojiSelect(emoji: { id: string; native: string }) {
		const native = emoji.native;
		if (textarea) {
			const start = textarea.selectionStart;
			const end = textarea.selectionEnd;
			text = text.substring(0, start) + native + text.substring(end);
			showEmojiPicker = false;
			requestAnimationFrame(() => {
				if (textarea) {
					const pos = start + native.length;
					textarea.selectionStart = pos;
					textarea.selectionEnd = pos;
					textarea.focus();
				}
			});
		} else {
			text += native;
			showEmojiPicker = false;
		}
	}

	export function insertText(insert: string) {
		text = insert + text;
		requestAnimationFrame(() => {
			if (textarea) {
				textarea.style.height = 'auto';
				textarea.style.height = Math.min(textarea.scrollHeight, 200) + 'px';
				textarea.focus();
				const pos = insert.length;
				textarea.selectionStart = pos;
				textarea.selectionEnd = pos;
			}
		});
	}

	async function handleSend() {
		const trimmed = text.trim();
		if (!trimmed) return;

		text = '';
		localStorage.removeItem(draftKey(channelId));
		closeAutocomplete();
		if (textarea) {
			textarea.style.height = 'auto';
		}

		await sendMessage(channelId, trimmed, threadTs);
	}
</script>

<div class="border-t border-primary-dark/40 px-4 py-3">
	<div class="relative flex items-end gap-2 rounded-lg bg-navy-light px-3 py-2">
		<Autocomplete
			bind:this={autocompleteRef}
			items={autocompleteItems}
			onSelect={handleAutocompleteSelect}
			visible={autocompleteVisible}
		/>

		<FileUpload {channelId} {threadTs} />

		<textarea
			bind:this={textarea}
			bind:value={text}
			onkeydown={handleKeydown}
			oninput={handleInput}
			{placeholder}
			rows="1"
			class="max-h-[200px] flex-1 resize-none bg-transparent text-sm text-white placeholder-primary-light/40 focus:outline-none"
		></textarea>

		<div class="relative">
			<button
				onclick={() => (showEmojiPicker = !showEmojiPicker)}
				class="flex-shrink-0 rounded p-1 text-primary-light/50 transition hover:text-primary-lighter"
				title="Add emoji"
			>
				<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14.828 14.828a4 4 0 01-5.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
				</svg>
			</button>
			{#if showEmojiPicker}
				<div class="absolute bottom-full right-0 z-50 mb-2">
					<EmojiPicker onSelect={handleEmojiSelect} />
				</div>
			{/if}
		</div>

		<button
			onclick={handleSend}
			disabled={!text.trim()}
			class="flex-shrink-0 rounded p-1 text-primary-light/50 transition hover:text-primary-lighter disabled:opacity-30"
			title="Send message"
		>
			<svg class="h-5 w-5" fill="currentColor" viewBox="0 0 24 24">
				<path d="M2.01 21L23 12 2.01 3 2 10l15 2-15 2z" />
			</svg>
		</button>
	</div>
</div>
