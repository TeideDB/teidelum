<script lang="ts">
	import { goto } from '$app/navigation';
	import { channels } from '$lib/stores/channels';
	import { onMount } from 'svelte';

	onMount(() => {
		let navigated = false;
		const unsub = channels.subscribe(($channels) => {
			if (!navigated && $channels.length > 0) {
				navigated = true;
				goto(`/${$channels[0].id}`);
				// Defer unsubscribe to avoid calling it during the synchronous callback
				setTimeout(() => unsub(), 0);
			}
		});
		return unsub;
	});
</script>

<div class="flex flex-1 items-center justify-center text-gray-500">
	<p>Select a channel to start chatting</p>
</div>
