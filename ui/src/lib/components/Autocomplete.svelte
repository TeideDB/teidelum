<script lang="ts">
	interface AutocompleteItem {
		id: string;
		label: string;
		secondary?: string;
		avatar?: string;
	}

	interface Props {
		items: AutocompleteItem[];
		onSelect: (item: AutocompleteItem) => void;
		visible: boolean;
	}

	let { items, onSelect, visible }: Props = $props();

	let selectedIndex = $state(0);

	// Reset selection when items change
	$effect(() => {
		items; // track
		selectedIndex = 0;
	});

	export function handleKeydown(e: KeyboardEvent): boolean {
		if (!visible || items.length === 0) return false;

		if (e.key === 'ArrowDown') {
			e.preventDefault();
			selectedIndex = (selectedIndex + 1) % items.length;
			return true;
		}
		if (e.key === 'ArrowUp') {
			e.preventDefault();
			selectedIndex = (selectedIndex - 1 + items.length) % items.length;
			return true;
		}
		if (e.key === 'Enter' || e.key === 'Tab') {
			e.preventDefault();
			onSelect(items[selectedIndex]);
			return true;
		}
		if (e.key === 'Escape') {
			e.preventDefault();
			return true;
		}
		return false;
	}
</script>

{#if visible && items.length > 0}
	<div class="absolute bottom-full left-0 right-0 mb-1 max-h-48 overflow-y-auto rounded-lg border border-primary-dark/40 bg-navy-dark shadow-lg">
		{#each items as item, i}
			<button
				class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm transition {i === selectedIndex ? 'bg-primary/30 text-heading' : 'text-body hover:bg-primary/10'}"
				onmousedown={(e: MouseEvent) => { e.preventDefault(); onSelect(item); }}
				onmouseenter={() => (selectedIndex = i)}
			>
				{#if item.avatar}
					<span class="flex h-6 w-6 flex-shrink-0 items-center justify-center rounded bg-primary text-xs font-bold text-white">
						{item.avatar}
					</span>
				{/if}
				<span class="truncate font-medium">{item.label}</span>
				{#if item.secondary}
					<span class="truncate text-xs text-primary-light/40">{item.secondary}</span>
				{/if}
			</button>
		{/each}
	</div>
{/if}
