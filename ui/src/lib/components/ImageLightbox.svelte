<script lang="ts">
	interface Props {
		src: string;
		alt: string;
		onClose: () => void;
	}

	let { src, alt, onClose }: Props = $props();

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') onClose();
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-sm"
	onclick={onClose}
>
	<button
		class="absolute top-4 right-4 text-white/70 hover:text-white cursor-pointer"
		onclick={onClose}
		aria-label="Close"
	>
		<svg class="h-8 w-8" fill="none" stroke="currentColor" viewBox="0 0 24 24">
			<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
		</svg>
	</button>
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<img
		{src}
		{alt}
		class="max-w-[90vw] max-h-[90vh] object-contain"
		onclick={(e) => e.stopPropagation()}
	/>
</div>
