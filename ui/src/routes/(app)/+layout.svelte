<script lang="ts">
	import { onMount } from 'svelte';
	import Sidebar from '$lib/components/Sidebar.svelte';
	import { loadChannels, initChannelWsListeners } from '$lib/stores/channels';
	import { loadUsers, initUserWsListeners } from '$lib/stores/users';
	import { initMessageWsListeners } from '$lib/stores/messages';
	import { initUnreadsWsListeners } from '$lib/stores/unreads';

	let { children } = $props();

	onMount(async () => {
		await Promise.all([loadChannels(), loadUsers()]);
		initChannelWsListeners();
		initUserWsListeners();
		initMessageWsListeners();
		initUnreadsWsListeners();
	});
</script>

<div class="flex h-screen overflow-hidden bg-gray-900">
	<!-- Sidebar -->
	<div class="flex w-64 flex-shrink-0 flex-col border-r border-gray-700 bg-gray-800">
		<Sidebar />
	</div>

	<!-- Main content area -->
	<div class="flex flex-1 overflow-hidden">
		{@render children()}
	</div>
</div>
