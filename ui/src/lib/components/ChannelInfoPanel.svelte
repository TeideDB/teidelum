<script lang="ts">
	import { onMount } from 'svelte';
	import * as api from '$lib/api';
	import { users } from '$lib/stores/users';
	import { auth } from '$lib/stores/auth';
	import { loadChannels } from '$lib/stores/channels';
	import Avatar from '$lib/components/Avatar.svelte';
	import UserProfilePopover from '$lib/components/UserProfilePopover.svelte';
	import type { Channel, ChannelMember, Id } from '$lib/types';

	interface Props {
		channel: Channel;
		onClose: () => void;
	}

	let { channel, onClose }: Props = $props();

	let members = $state<ChannelMember[]>([]);
	let loading = $state(true);
	let editing = $state(false);
	let editName = $state('');
	let editTopic = $state('');
	let editDescription = $state('');
	let saving = $state(false);
	let showArchiveConfirm = $state(false);
	let showInviteModal = $state(false);
	let inviteUserId = $state('');
	let popoverUserId = $state<Id | null>(null);
	let popoverAnchorRect = $state<{ top: number; left: number; bottom: number; right: number } | null>(null);

	function openProfilePopover(userId: Id, e: MouseEvent) {
		const el = e.currentTarget as HTMLElement;
		const rect = el.getBoundingClientRect();
		popoverAnchorRect = { top: rect.top, left: rect.left, bottom: rect.bottom, right: rect.right };
		popoverUserId = userId;
	}

	const myRole = $derived(members.find(m => m.id === $auth.user?.id)?.role);
	const isOwnerOrAdmin = $derived(myRole === 'owner' || myRole === 'admin');
	const isArchived = $derived(!!channel.archived_at);

	onMount(() => {
		loadMembers();
	});

	async function loadMembers() {
		loading = true;
		try {
			const res = await api.conversationsMembers(channel.id);
			if (res.ok && res.members) {
				members = res.members;
			}
		} catch (e) {
			console.error('Failed to load members:', e);
		} finally {
			loading = false;
		}
	}

	function startEdit() {
		editName = channel.name;
		editTopic = channel.topic || '';
		editDescription = channel.description || '';
		editing = true;
	}

	async function saveEdit() {
		saving = true;
		try {
			const updates: { name?: string; topic?: string; description?: string } = {};
			if (editName !== channel.name) updates.name = editName;
			if (editTopic !== (channel.topic || '')) updates.topic = editTopic;
			if (editDescription !== (channel.description || '')) updates.description = editDescription;

			if (Object.keys(updates).length > 0) {
				const res = await api.conversationsUpdate(channel.id, updates);
				if (res.ok) {
					await loadChannels();
				}
			}
			editing = false;
		} catch (e) {
			console.error('Failed to update channel:', e);
		} finally {
			saving = false;
		}
	}

	async function handleArchive() {
		try {
			const res = await api.conversationsArchive(channel.id);
			if (res.ok) {
				await loadChannels();
				showArchiveConfirm = false;
			}
		} catch (e) {
			console.error('Failed to archive channel:', e);
		}
	}

	async function handleUnarchive() {
		try {
			const res = await api.conversationsUnarchive(channel.id);
			if (res.ok) {
				await loadChannels();
			}
		} catch (e) {
			console.error('Failed to unarchive channel:', e);
		}
	}

	async function handleInvite() {
		if (!inviteUserId.trim()) return;
		try {
			const res = await api.conversationsInvite(channel.id, inviteUserId.trim());
			if (res.ok) {
				await loadMembers();
				showInviteModal = false;
				inviteUserId = '';
			}
		} catch (e) {
			console.error('Failed to invite user:', e);
		}
	}

	function getUserName(userId: Id): string {
		const user = $users.get(userId);
		return user?.display_name || user?.username || userId;
	}

	function getUserAvatar(userId: Id): string {
		const user = $users.get(userId);
		return user?.avatar_url || '';
	}

	function formatDate(dateStr: string): string {
		try {
			const n = parseInt(dateStr, 10);
			const date = !isNaN(n) && String(n) === dateStr ? new Date(n * 1000) : new Date(dateStr);
			return date.toLocaleDateString([], { year: 'numeric', month: 'short', day: 'numeric' });
		} catch {
			return dateStr;
		}
	}
</script>

<div class="flex h-full flex-col">
	<!-- Header -->
	<div class="flex items-center justify-between border-b border-primary-dark/40 px-4 py-3">
		<h3 class="font-bold text-heading">Channel Details</h3>
		<button onclick={onClose} aria-label="Close panel" class="text-primary-light/50 hover:text-primary-lighter">
			<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
				<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
			</svg>
		</button>
	</div>

	<div class="flex-1 overflow-y-auto px-4 py-4 space-y-4">
		{#if editing}
			<!-- Edit form -->
			<div class="space-y-3">
				<div>
					<label for="editChannelName" class="mb-1 block text-xs text-primary-lighter/70">Name</label>
					<input
						id="editChannelName"
						type="text"
						bind:value={editName}
						class="w-full rounded bg-navy px-3 py-2 text-sm text-white placeholder-primary-light/40 focus:outline-none focus:ring-2 focus:ring-primary"
					/>
				</div>
				<div>
					<label for="editChannelTopic" class="mb-1 block text-xs text-primary-lighter/70">Topic</label>
					<input
						id="editChannelTopic"
						type="text"
						bind:value={editTopic}
						class="w-full rounded bg-navy px-3 py-2 text-sm text-white placeholder-primary-light/40 focus:outline-none focus:ring-2 focus:ring-primary"
						placeholder="Channel topic"
					/>
				</div>
				<div>
					<label for="editChannelDesc" class="mb-1 block text-xs text-primary-lighter/70">Description</label>
					<textarea
						id="editChannelDesc"
						bind:value={editDescription}
						rows="3"
						class="w-full rounded bg-navy px-3 py-2 text-sm text-white placeholder-primary-light/40 focus:outline-none focus:ring-2 focus:ring-primary resize-none"
						placeholder="What's this channel for?"
					></textarea>
				</div>
				<div class="flex gap-2">
					<button
						onclick={saveEdit}
						disabled={saving}
						class="rounded bg-primary px-3 py-1.5 text-sm font-medium text-white hover:bg-primary-light disabled:opacity-50"
					>
						{saving ? 'Saving...' : 'Save'}
					</button>
					<button
						onclick={() => (editing = false)}
						class="rounded px-3 py-1.5 text-sm text-primary-lighter/70 hover:text-heading"
					>
						Cancel
					</button>
				</div>
			</div>
		{:else}
			<!-- Channel info display -->
			<div>
				<div class="flex items-center gap-2">
					<h4 class="text-lg font-bold text-heading">
						{#if channel.kind === 'dm'}
							{channel.name}
						{:else}
							<span class="text-primary-light/40">#</span> {channel.name}
						{/if}
					</h4>
					<span class="rounded bg-primary-darker/60 px-1.5 py-0.5 text-xs text-primary-lighter/70">
						{channel.kind}
					</span>
					{#if isArchived}
						<span class="rounded bg-red-900/40 px-1.5 py-0.5 text-xs text-red-300">
							archived
						</span>
					{/if}
				</div>

				{#if channel.topic}
					<p class="mt-1 text-sm text-primary-lighter/70">{channel.topic}</p>
				{/if}

				{#if channel.description}
					<p class="mt-2 text-sm text-gray-300">{channel.description}</p>
				{/if}

				<p class="mt-2 text-xs text-primary-light/40">
					Created by {getUserName(channel.created_by)} on {formatDate(channel.created_at)}
				</p>
			</div>

			<!-- Actions -->
			{#if channel.kind !== 'dm'}
				<div class="flex flex-wrap gap-2">
					{#if isOwnerOrAdmin && !isArchived}
						<button
							onclick={startEdit}
							class="rounded bg-primary-darker/60 px-3 py-1.5 text-xs text-primary-lighter hover:bg-primary-darker hover:text-heading"
						>
							Edit
						</button>
					{/if}
					{#if !isArchived}
						<button
							onclick={() => (showInviteModal = true)}
							class="rounded bg-primary-darker/60 px-3 py-1.5 text-xs text-primary-lighter hover:bg-primary-darker hover:text-heading"
						>
							Add people
						</button>
					{/if}
					{#if isOwnerOrAdmin}
						{#if isArchived}
							<button
								onclick={handleUnarchive}
								class="rounded bg-primary-darker/60 px-3 py-1.5 text-xs text-primary-lighter hover:bg-primary-darker hover:text-heading"
							>
								Unarchive
							</button>
						{:else}
							<button
								onclick={() => (showArchiveConfirm = true)}
								class="rounded bg-red-900/40 px-3 py-1.5 text-xs text-red-300 hover:bg-red-900/60"
							>
								Archive
							</button>
						{/if}
					{/if}
				</div>
			{/if}
		{/if}

		<!-- Members -->
		<div>
			<h4 class="mb-2 text-xs font-semibold uppercase tracking-wide text-primary-light/50">
				Members ({members.length})
			</h4>
			{#if loading}
				<p class="text-sm text-primary-light/50">Loading...</p>
			{:else}
				<div class="space-y-1">
					{#each members as member}
						<button
							type="button"
							class="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left hover:bg-navy-light/50 cursor-pointer"
							onclick={(e) => openProfilePopover(member.id, e)}
						>
							<Avatar url={getUserAvatar(member.id)} name={getUserName(member.id)} size="sm" />
							<span class="text-sm text-gray-300">{getUserName(member.id)}</span>
							{#if member.role === 'owner' || member.role === 'admin'}
								<span class="text-xs text-primary-light/40">{member.role}</span>
							{/if}
						</button>
					{/each}
				</div>
			{/if}
		</div>
	</div>
</div>

<!-- Archive confirm dialog -->
{#if showArchiveConfirm}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-overlay">
		<div class="w-full max-w-sm rounded-lg bg-navy-light p-6 shadow-xl">
			<h3 class="mb-2 text-lg font-semibold text-heading">Archive Channel</h3>
			<p class="mb-4 text-sm text-primary-lighter/70">
				Are you sure you want to archive #{channel.name}? No new messages can be posted.
			</p>
			<div class="flex justify-end gap-2">
				<button
					onclick={() => (showArchiveConfirm = false)}
					class="rounded px-4 py-2 text-sm text-primary-lighter/70 hover:text-heading"
				>
					Cancel
				</button>
				<button
					onclick={handleArchive}
					class="rounded bg-red-600 px-4 py-2 text-sm font-medium text-white hover:bg-red-500"
				>
					Archive
				</button>
			</div>
		</div>
	</div>
{/if}

<!-- Invite modal -->
{#if showInviteModal}
	<div class="fixed inset-0 z-50 flex items-center justify-center bg-overlay">
		<div class="w-full max-w-sm rounded-lg bg-navy-light p-6 shadow-xl">
			<h3 class="mb-4 text-lg font-semibold text-heading">Add People</h3>
			<form
				onsubmit={(e) => {
					e.preventDefault();
					handleInvite();
				}}
			>
				<input
					type="text"
					bind:value={inviteUserId}
					class="mb-4 w-full rounded bg-navy px-3 py-2 text-sm text-white placeholder-primary-light/40 focus:outline-none focus:ring-2 focus:ring-primary"
					placeholder="User ID"
				/>
				<div class="flex justify-end gap-2">
					<button
						type="button"
						onclick={() => (showInviteModal = false)}
						class="rounded px-4 py-2 text-sm text-primary-lighter/70 hover:text-heading"
					>
						Cancel
					</button>
					<button
						type="submit"
						class="rounded bg-primary px-4 py-2 text-sm font-medium text-white hover:bg-primary-light"
					>
						Invite
					</button>
				</div>
			</form>
		</div>
	</div>
{/if}

{#if popoverUserId && popoverAnchorRect}
	<UserProfilePopover
		userId={popoverUserId}
		anchorRect={popoverAnchorRect}
		onClose={() => { popoverUserId = null; popoverAnchorRect = null; }}
	/>
{/if}
