<script lang="ts">
	import { onMount } from 'svelte';
	import { linksUnfurl } from '$lib/api';

	interface Props {
		url: string;
	}

	let { url }: Props = $props();

	// Module-level cache shared across all instances
	const cache = LinkPreviewCache.instance;

	let title = $state<string | undefined>();
	let description = $state<string | undefined>();
	let image = $state<string | undefined>();
	let siteName = $state<string | undefined>();
	let loaded = $state(false);
	let failed = $state(false);

	onMount(() => {
		const cached = cache.get(url);
		if (cached) {
			title = cached.title;
			description = cached.description;
			image = cached.image;
			siteName = cached.site_name;
			loaded = true;
			if (!title && !description && !image) failed = true;
			return;
		}

		linksUnfurl(url)
			.then((res) => {
				if (res.ok) {
					const entry = {
						title: res.title,
						description: res.description,
						image: res.image,
						site_name: res.site_name
					};
					cache.set(url, entry);
					title = entry.title;
					description = entry.description;
					image = entry.image;
					siteName = entry.site_name;
					if (!title && !description && !image) failed = true;
				} else {
					cache.set(url, {});
					failed = true;
				}
				loaded = true;
			})
			.catch(() => {
				cache.set(url, {});
				failed = true;
				loaded = true;
			});
	});
</script>

<script lang="ts" module>
	interface CacheEntry {
		title?: string;
		description?: string;
		image?: string;
		site_name?: string;
	}

	class LinkPreviewCache {
		static instance = new LinkPreviewCache();
		private map = new Map<string, CacheEntry>();

		get(url: string): CacheEntry | undefined {
			return this.map.get(url);
		}

		set(url: string, entry: CacheEntry) {
			this.map.set(url, entry);
		}
	}
</script>

{#if loaded && !failed && (title || description || image)}
	<a
		href={url}
		target="_blank"
		rel="noopener noreferrer"
		class="mt-1 flex max-w-md overflow-hidden rounded border border-primary-dark/40 bg-navy-light/50 hover:bg-navy-light/80 transition"
	>
		{#if image}
			<img
				src={image}
				alt={title ?? ''}
				class="h-20 w-20 flex-shrink-0 object-cover"
			/>
		{/if}
		<div class="min-w-0 flex-1 px-3 py-2">
			{#if siteName}
				<div class="text-xs text-primary-lighter/60 truncate">{siteName}</div>
			{/if}
			{#if title}
				<div class="text-sm font-medium text-primary-lighter truncate">{title}</div>
			{/if}
			{#if description}
				<div class="text-xs text-primary-light/60 line-clamp-2">{description}</div>
			{/if}
		</div>
	</a>
{/if}
