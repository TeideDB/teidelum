<script lang="ts">
	import { connectionState } from '$lib/ws';
	import { onMount } from 'svelte';

	let showConnected = $state(false);
	let connectedTimer: ReturnType<typeof setTimeout> | undefined;

	// Track previous state to detect reconnection
	let prevState = $state<string>('disconnected');

	onMount(() => {
		const unsub = connectionState.subscribe((state) => {
			if (state === 'connected' && prevState !== 'connected') {
				// Flash green briefly when reconnected (not on initial connect)
				if (prevState === 'reconnecting') {
					showConnected = true;
					clearTimeout(connectedTimer);
					connectedTimer = setTimeout(() => {
						showConnected = false;
					}, 2000);
				}
			}
			prevState = state;
		});
		return () => {
			unsub();
			clearTimeout(connectedTimer);
		};
	});
</script>

{#if $connectionState === 'reconnecting'}
	<div
		class="fixed top-0 left-0 right-0 z-50 flex items-center justify-center bg-yellow-500 px-3 py-1.5 text-xs font-medium text-yellow-900"
		aria-live="assertive"
		role="alert"
	>
		Reconnecting...
	</div>
{:else if $connectionState === 'disconnected'}
	<div
		class="fixed top-0 left-0 right-0 z-50 flex items-center justify-center bg-red-500 px-3 py-1.5 text-xs font-medium text-white"
		aria-live="assertive"
		role="alert"
	>
		Disconnected
	</div>
{:else if showConnected}
	<div
		class="fixed top-0 left-0 right-0 z-50 flex items-center justify-center bg-green-500 px-3 py-1.5 text-xs font-medium text-white transition-opacity duration-500"
		aria-live="assertive"
		role="alert"
	>
		Connected
	</div>
{/if}
