<script lang="ts">
	import { goto } from '$app/navigation';
	import { channels } from '$lib/stores/channels';
	import { onMount } from 'svelte';

	onMount(() => {
		const unsub = channels.subscribe(($channels) => {
			if ($channels.length > 0) {
				goto(`/${$channels[0].id}`);
				unsub();
			}
		});
		return unsub;
	});
</script>

<div class="flex flex-1 items-center justify-center text-gray-500">
	<p>Select a channel to start chatting</p>
</div>
