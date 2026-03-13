<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { showSettings } from '$lib/stores/settings';

	// Settings is now an overlay, not a page. Redirect back and open overlay.
	onMount(() => {
		showSettings.set(true);
		goto('/', { replaceState: true });
	});
</script>
