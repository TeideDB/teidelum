<script lang="ts">
	import * as api from '$lib/api';
	import type { Id } from '$lib/types';

	interface Props {
		channelId: Id;
		threadTs?: Id;
	}

	let { channelId, threadTs }: Props = $props();

	let fileInput: HTMLInputElement | undefined = $state();
	let uploading = $state(false);

	function triggerUpload() {
		fileInput?.click();
	}

	async function handleFileSelect(e: Event) {
		const input = e.target as HTMLInputElement;
		const file = input.files?.[0];
		if (!file) return;

		uploading = true;
		try {
			await api.filesUpload(channelId, file, threadTs);
		} catch (err) {
			console.error('File upload failed:', err);
		}
		uploading = false;

		// Reset input
		if (fileInput) fileInput.value = '';
	}
</script>

<input
	bind:this={fileInput}
	type="file"
	class="hidden"
	onchange={handleFileSelect}
/>

<button
	onclick={triggerUpload}
	disabled={uploading}
	class="rounded p-1 text-gray-500 transition hover:text-gray-300 disabled:opacity-50"
	title="Upload file"
>
	{#if uploading}
		<svg class="h-5 w-5 animate-spin" fill="none" stroke="currentColor" viewBox="0 0 24 24">
			<circle cx="12" cy="12" r="10" stroke-width="2" class="opacity-25" />
			<path stroke-width="2" d="M4 12a8 8 0 018-8" class="opacity-75" />
		</svg>
	{:else}
		<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
			<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15.172 7l-6.586 6.586a2 2 0 102.828 2.828l6.414-6.586a4 4 0 00-5.656-5.656l-6.415 6.585a6 6 0 108.486 8.486L20.5 13" />
		</svg>
	{/if}
</button>
