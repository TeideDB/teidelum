<script lang="ts">
	import { fileDownloadUrl } from '$lib/api';

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

	/** For /files/ paths, append the current auth token so the request is authenticated. */
	function resolvedUrl(raw: string): string {
		if (!raw) return '';
		// /files/{id}/{filename} — extract id and filename, use fileDownloadUrl for current token
		const m = raw.match(/^\/files\/([^/]+)\/(.+?)(?:\?.*)?$/);
		if (m) {
			return fileDownloadUrl(m[1], decodeURIComponent(m[2]));
		}
		return raw;
	}
</script>

{#if url}
	<img src={resolvedUrl(url)} alt={name} class="rounded-full object-cover {sizeClasses[size]}" />
{:else}
	<div
		class="rounded-full flex items-center justify-center font-semibold text-white {sizeClasses[size]}"
		style="background-color: {colorFromName(name)}"
	>
		{initials(name)}
	</div>
{/if}
