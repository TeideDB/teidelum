<script lang="ts">
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import MessageList from '$lib/components/MessageList.svelte';
	import MessageInput from '$lib/components/MessageInput.svelte';
	import ThreadPanel from '$lib/components/ThreadPanel.svelte';
	import ChannelInfoPanel from '$lib/components/ChannelInfoPanel.svelte';
	import { setActiveChannel, activeChannel } from '$lib/stores/channels';
	import { markRead } from '$lib/stores/unreads';
	import { pinsList, pinsRemove } from '$lib/api';
	import { renderMarkdown } from '$lib/markdown';
	import { users } from '$lib/stores/users';
	import * as ws from '$lib/ws';
	import type { Message, Id } from '$lib/types';

	const channelId = $derived(page.params.channel ?? '');

	let threadMessage = $state<Message | null>(null);
	let showChannelInfo = $state(false);
	let pinnedMessages = $state<Message[]>([]);
	let showPinnedDropdown = $state(false);

	const pinnedMessageIds = $derived(new Set(pinnedMessages.map((m) => m.id)));

	$effect(() => {
		if (channelId) {
			setActiveChannel(channelId);
			markRead(channelId);
			loadPins(channelId);
		}
	});

	async function loadPins(chId: Id) {
		const res = await pinsList(chId);
		if (res.ok && res.pins) {
			pinnedMessages = res.pins;
		} else {
			pinnedMessages = [];
		}
	}

	// Listen for pin WS events
	onMount(() => {
		const unsubs: (() => void)[] = [];
		unsubs.push(
			ws.on('message_pinned', () => {
				loadPins(channelId);
			})
		);
		unsubs.push(
			ws.on('message_unpinned', () => {
				loadPins(channelId);
			})
		);
		return () => unsubs.forEach((fn) => fn());
	});

	function openThread(msg: Message) {
		showChannelInfo = false;
		showPinnedDropdown = false;
		threadMessage = msg;
	}

	function closeThread() {
		threadMessage = null;
	}

	function toggleChannelInfo() {
		showChannelInfo = !showChannelInfo;
		if (showChannelInfo) {
			threadMessage = null;
			showPinnedDropdown = false;
		}
	}

	function togglePinnedDropdown() {
		showPinnedDropdown = !showPinnedDropdown;
	}

	async function handleUnpin(msgId: Id) {
		await pinsRemove(channelId, msgId);
		pinnedMessages = pinnedMessages.filter((m) => m.id !== msgId);
	}

	function getUserName(userId: Id): string {
		const user = $users.get(userId);
		return user?.display_name || user?.username || userId;
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

			<!-- Pinned messages indicator -->
			{#if pinnedMessages.length > 0}
				<div class="relative ml-auto">
					<button
						onclick={togglePinnedDropdown}
						class="flex items-center gap-1 rounded px-2 py-1 text-primary-light/60 hover:bg-navy-light hover:text-primary-lighter"
						title="Pinned messages"
					>
						<svg class="h-4 w-4" fill="currentColor" viewBox="0 0 24 24">
							<path d="M16 12V4h1V2H7v2h1v8l-2 2v2h5.2v6h1.6v-6H18v-2l-2-2z" />
						</svg>
						<span class="text-xs">{pinnedMessages.length}</span>
					</button>

					{#if showPinnedDropdown}
						<!-- svelte-ignore a11y_no_static_element_interactions -->
						<div
							class="absolute right-0 top-full z-40 mt-1 w-80 rounded-lg border border-primary-dark/40 bg-navy-light shadow-xl"
							onkeydown={(e) => e.key === 'Escape' && (showPinnedDropdown = false)}
						>
							<div class="border-b border-primary-dark/40 px-4 py-2">
								<h4 class="text-sm font-semibold text-white">Pinned Messages</h4>
							</div>
							<div class="max-h-80 overflow-y-auto">
								{#each pinnedMessages as pin}
									<div class="border-b border-primary-dark/20 px-4 py-3 last:border-b-0">
										<div class="flex items-start justify-between gap-2">
											<div class="min-w-0 flex-1">
												<span class="text-xs font-medium text-gray-200">{getUserName(pin.user_id)}</span>
												<div class="mt-0.5 text-sm text-gray-300 break-words line-clamp-3">{@html renderMarkdown(pin.text)}</div>
											</div>
											<button
												onclick={() => handleUnpin(pin.id)}
												class="flex-shrink-0 rounded p-1 text-primary-light/40 hover:bg-navy-mid hover:text-red-400"
												title="Unpin"
											>
												<svg class="h-3.5 w-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
													<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
												</svg>
											</button>
										</div>
									</div>
								{/each}
							</div>
						</div>
					{/if}
				</div>
			{/if}
		</div>

		{#if isArchived}
			<div class="border-b border-yellow-700/30 bg-yellow-900/20 px-4 py-2 text-sm text-yellow-200/80">
				This channel is archived. No new messages can be posted.
			</div>
		{/if}

		<!-- Messages -->
		<MessageList {channelId} onOpenThread={openThread} {pinnedMessageIds} />

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
