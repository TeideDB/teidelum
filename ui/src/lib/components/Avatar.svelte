<script lang="ts">
	export let url: string = '';
	export let name: string = '';
	export let size: 'sm' | 'md' | 'lg' = 'md';

	const sizeClasses = { sm: 'w-6 h-6 text-xs', md: 'w-8 h-8 text-sm', lg: 'w-12 h-12 text-lg' };

	function initials(name: string): string {
		return name.split(/\s+/).map(w => w[0]).join('').toUpperCase().slice(0, 2) || '?';
	}

	function colorFromName(name: string): string {
		let hash = 0;
		for (let i = 0; i < name.length; i++) hash = name.charCodeAt(i) + ((hash << 5) - hash);
		const hue = Math.abs(hash % 360);
		return `hsl(${hue}, 60%, 45%)`;
	}
</script>

{#if url}
	<img src={url} alt={name} class="rounded-full object-cover {sizeClasses[size]}" />
{:else}
	<div
		class="rounded-full flex items-center justify-center font-semibold text-white {sizeClasses[size]}"
		style="background-color: {colorFromName(name)}"
	>
		{initials(name)}
	</div>
{/if}
