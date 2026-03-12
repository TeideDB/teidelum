<script lang="ts">
	import { onDestroy } from 'svelte';
	import * as ws from '$lib/ws';
	import { getUser } from '$lib/stores/users';
	import { auth } from '$lib/stores/auth';
	import { get } from 'svelte/store';
	import type { WsEvent, Id } from '$lib/types';

	export let channelId: Id;

	let typingUsers = new Map<Id, number>(); // userId -> timeout handle
	let displayNames: string[] = [];

	const unsub = ws.on('typing', (event: WsEvent) => {
		const data = event as unknown as { channel: Id; user: Id };
		if (data.channel !== channelId) return;
		if (data.user === get(auth).userId) return;

		// Clear existing timeout
		if (typingUsers.has(data.user)) {
			clearTimeout(typingUsers.get(data.user)!);
		}
		// Set new timeout (4 seconds)
		const handle = setTimeout(() => {
			typingUsers.delete(data.user);
			typingUsers = typingUsers; // trigger reactivity
			updateDisplay();
		}, 4000);
		typingUsers.set(data.user, handle as unknown as number);
		typingUsers = typingUsers;
		updateDisplay();
	});

	function updateDisplay() {
		displayNames = Array.from(typingUsers.keys()).map((uid) => {
			const user = getUser(uid);
			return user?.display_name || user?.username || 'Someone';
		});
	}

	onDestroy(() => {
		unsub();
		for (const handle of typingUsers.values()) clearTimeout(handle);
	});
</script>

<div class="h-5" aria-live="polite">
	{#if displayNames.length > 0}
		<span class="text-xs text-gray-400 px-4">
			{#if displayNames.length === 1}
				{displayNames[0]} is typing...
			{:else if displayNames.length === 2}
				{displayNames[0]} and {displayNames[1]} are typing...
			{:else}
				Several people are typing...
			{/if}
		</span>
	{/if}
</div>
