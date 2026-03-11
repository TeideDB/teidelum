<script lang="ts">
	import { sendTyping } from '$lib/ws';
	import { sendMessage } from '$lib/stores/messages';
	import FileUpload from './FileUpload.svelte';
	import type { Id } from '$lib/types';

	interface Props {
		channelId: Id;
		threadTs?: Id;
		placeholder?: string;
	}

	let { channelId, threadTs, placeholder = 'Type a message...' }: Props = $props();

	let text = $state('');
	let textarea: HTMLTextAreaElement | undefined = $state();
	let lastTypingSent = $state(0);

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !e.shiftKey) {
			e.preventDefault();
			handleSend();
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
	}

	async function handleSend() {
		const trimmed = text.trim();
		if (!trimmed) return;

		text = '';
		if (textarea) {
			textarea.style.height = 'auto';
		}

		await sendMessage(channelId, trimmed, threadTs);
	}
</script>

<div class="border-t border-primary-dark/40 px-4 py-3">
	<div class="flex items-end gap-2 rounded-lg bg-navy-light px-3 py-2">
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
