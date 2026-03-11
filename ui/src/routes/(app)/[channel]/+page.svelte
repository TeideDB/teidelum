<script lang="ts">
	import { page } from '$app/state';
	import MessageList from '$lib/components/MessageList.svelte';
	import MessageInput from '$lib/components/MessageInput.svelte';
	import ThreadPanel from '$lib/components/ThreadPanel.svelte';
	import ChannelInfoPanel from '$lib/components/ChannelInfoPanel.svelte';
	import { setActiveChannel, activeChannel } from '$lib/stores/channels';
	import { markRead } from '$lib/stores/unreads';
	import type { Message } from '$lib/types';

	const channelId = $derived(page.params.channel ?? '');

	let threadMessage = $state<Message | null>(null);
	let showChannelInfo = $state(false);

	$effect(() => {
		if (channelId) {
			setActiveChannel(channelId);
			markRead(channelId);
		}
	});

	function openThread(msg: Message) {
		showChannelInfo = false;
		threadMessage = msg;
	}

	function closeThread() {
		threadMessage = null;
	}

	function toggleChannelInfo() {
		showChannelInfo = !showChannelInfo;
		if (showChannelInfo) {
			threadMessage = null;
		}
	}

	const isArchived = $derived(!!$activeChannel?.archived_at);
</script>

<svelte:head>
	<title>{$activeChannel ? `#${$activeChannel.name}` : 'Teidelum'} - Teidelum</title>
</svelte:head>

<div class="flex flex-1 overflow-hidden">
	<!-- Main message area -->
	<div class="flex flex-1 flex-col overflow-hidden">
		<!-- Channel header -->
		<div class="flex items-center border-b border-primary-dark/40 px-4 py-3">
			<button onclick={toggleChannelInfo} class="text-left hover:opacity-80">
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
			</button>
			{#if isArchived}
				<span class="ml-3 rounded bg-red-900/40 px-2 py-0.5 text-xs text-red-300">archived</span>
			{/if}
		</div>

		{#if isArchived}
			<div class="border-b border-yellow-700/30 bg-yellow-900/20 px-4 py-2 text-sm text-yellow-200/80">
				This channel is archived. No new messages can be posted.
			</div>
		{/if}

		<!-- Messages -->
		<MessageList {channelId} onOpenThread={openThread} />

		<!-- Input -->
		{#if !isArchived}
			<MessageInput
				{channelId}
				placeholder={$activeChannel ? `Message #${$activeChannel.name}` : 'Type a message...'}
			/>
		{/if}
	</div>

	<!-- Side panel: Thread or Channel Info -->
	{#if threadMessage}
		<div class="w-96 flex-shrink-0 border-l border-primary-dark/40">
			<ThreadPanel
				{channelId}
				parentMessage={threadMessage}
				onClose={closeThread}
			/>
		</div>
	{:else if showChannelInfo && $activeChannel}
		<div class="w-96 flex-shrink-0 border-l border-primary-dark/40">
			<ChannelInfoPanel
				channel={$activeChannel}
				onClose={() => (showChannelInfo = false)}
			/>
		</div>
	{/if}
</div>
