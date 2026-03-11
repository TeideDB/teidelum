<script lang="ts">
	import type { Message, Id } from '$lib/types';

	interface Props {
		channelId: Id;
		parentMessage: Message;
		onClose: () => void;
	}

	let { channelId, parentMessage, onClose }: Props = $props();
</script>

<div class="flex h-full flex-col">
	<div class="flex items-center justify-between border-b border-gray-700 px-4 py-3">
		<h3 class="font-bold text-white">Thread</h3>
		<button onclick={onClose} aria-label="Close thread" class="text-gray-500 hover:text-gray-300">
			<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
			</svg>
		</button>
	</div>
	<div class="flex-1 p-4 text-center text-sm text-gray-500">Thread panel placeholder</div>
</div>
