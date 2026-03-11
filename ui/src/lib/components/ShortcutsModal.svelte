<script lang="ts">
	interface Props {
		onClose: () => void;
	}

	let { onClose }: Props = $props();

	const shortcuts = [
		{ keys: ['Cmd', 'K'], description: 'Open search' },
		{ keys: ['Cmd', 'F'], description: 'Search in current channel' },
		{ keys: ['Cmd', 'Shift', 'A'], description: 'Jump to next unread channel' },
		{ keys: ['Cmd', '/'], description: 'Show keyboard shortcuts' },
		{ keys: ['Enter'], description: 'Send message' },
		{ keys: ['Shift', 'Enter'], description: 'New line in message' },
		{ keys: ['Up'], description: 'Edit last message (empty input)' },
		{ keys: ['Escape'], description: 'Close modal / cancel' }
	];

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') {
			e.preventDefault();
			onClose();
		}
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
	onclick={onClose}
	onkeydown={() => {}}
>
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="w-full max-w-md rounded-lg border border-primary-dark/40 bg-navy-light shadow-2xl"
		onclick={(e) => e.stopPropagation()}
		onkeydown={() => {}}
	>
		<div class="flex items-center justify-between border-b border-primary-dark/40 px-5 py-3">
			<h3 class="text-lg font-semibold text-heading">Keyboard Shortcuts</h3>
			<button
				onclick={onClose}
				class="rounded p-1 text-primary-light/50 hover:text-primary-lighter"
				title="Close"
			>
				<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
				</svg>
			</button>
		</div>
		<div class="px-5 py-3">
			{#each shortcuts as shortcut}
				<div class="flex items-center justify-between py-2">
					<span class="text-sm text-primary-light/70">{shortcut.description}</span>
					<div class="flex items-center gap-1">
						{#each shortcut.keys as key}
							<kbd class="rounded border border-primary-dark/50 bg-navy px-2 py-0.5 text-xs font-mono text-primary-light/80">{key}</kbd>
						{/each}
					</div>
				</div>
			{/each}
		</div>
	</div>
</div>
