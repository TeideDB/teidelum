<script lang="ts">
	import { page } from '$app/state';
	import MessageList from '$lib/components/MessageList.svelte';
	import MessageInput from '$lib/components/MessageInput.svelte';
	import ThreadPanel from '$lib/components/ThreadPanel.svelte';
	import { setActiveChannel, activeChannel } from '$lib/stores/channels';
	import { markRead } from '$lib/stores/unreads';
	import type { Message } from '$lib/types';

	const channelId = $derived(page.params.channel ?? '');

	let threadMessage = $state<Message | null>(null);

	$effect(() => {
		if (channelId) {
			setActiveChannel(channelId);
			markRead(channelId);
		}
	});

	function openThread(msg: Message) {
		threadMessage = msg;
	}

	function closeThread() {
		threadMessage = null;
	}
</script>

<svelte:head>
	<title>{$activeChannel ? `#${$activeChannel.name}` : 'Teidelum'} - Teidelum</title>
</svelte:head>

<div class="flex flex-1 overflow-hidden">
	<!-- Main message area -->
	<div class="flex flex-1 flex-col overflow-hidden">
		<!-- Channel header -->
		<div class="flex items-center border-b border-primary-dark/40 px-4 py-3">
			<div>
				<h2 class="text-lg font-bold text-white">
					{#if $activeChannel}
						{#if $activeChannel.kind === 'dm'}
							{$activeChannel.name}
						{:else}
							<span class="text-primary-light/40">#</span> {$activeChannel.name}
						{/if}
					{:else}
						Loading...
					{/if}
				</h2>
				{#if $activeChannel?.topic}
					<p class="text-xs text-primary-light/50">{$activeChannel.topic}</p>
				{/if}
			</div>
		</div>

		<!-- Messages -->
		<MessageList {channelId} onOpenThread={openThread} />

		<!-- Input -->
		<MessageInput
			{channelId}
			placeholder={$activeChannel ? `Message #${$activeChannel.name}` : 'Type a message...'}
		/>
	</div>

	<!-- Thread panel -->
	{#if threadMessage}
		<div class="w-96 flex-shrink-0 border-l border-primary-dark/40">
			<ThreadPanel
				{channelId}
				parentMessage={threadMessage}
				onClose={closeThread}
			/>
		</div>
	{/if}
</div>
