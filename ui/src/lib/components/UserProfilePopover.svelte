<script lang="ts">
	import { goto } from '$app/navigation';
	import { users, presence } from '$lib/stores/users';
	import { openDm, setActiveChannel } from '$lib/stores/channels';
	import { auth } from '$lib/stores/auth';
	import Avatar from '$lib/components/Avatar.svelte';
	import type { Id } from '$lib/types';

	interface Props {
		userId: Id;
		anchorRect: { top: number; left: number; bottom: number; right: number };
		onClose: () => void;
	}

	let { userId, anchorRect, onClose }: Props = $props();

	const user = $derived($users.get(userId));
	const userPresence = $derived($presence.get(userId) ?? 'away');
	const isOwnProfile = $derived($auth.userId === userId);

	function formatDate(dateStr: string): string {
		try {
			const n = parseInt(dateStr, 10);
			const date = !isNaN(n) && String(n) === dateStr ? new Date(n * 1000) : new Date(dateStr);
			return date.toLocaleDateString([], { year: 'numeric', month: 'long', day: 'numeric' });
		} catch {
			return dateStr;
		}
	}

	async function handleMessage() {
		const ch = await openDm([userId]);
		if (ch) {
			setActiveChannel(ch.id);
			goto(`/${ch.id}`);
		}
		onClose();
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') onClose();
	}

	// Position: prefer below the anchor, but flip up if near bottom of viewport
	const popoverStyle = $derived(() => {
		const spaceBelow = window.innerHeight - anchorRect.bottom;
		const top = spaceBelow > 320 ? anchorRect.bottom + 4 : anchorRect.top - 320;
		const left = Math.min(anchorRect.left, window.innerWidth - 300);
		return `top: ${Math.max(4, top)}px; left: ${Math.max(4, left)}px;`;
	});
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Backdrop -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="fixed inset-0 z-50" onclick={onClose}>
	<!-- Popover card -->
	<!-- svelte-ignore a11y_click_events_have_key_events -->
	<!-- svelte-ignore a11y_no_static_element_interactions -->
	<div
		class="absolute z-50 w-72 rounded-lg bg-navy-light shadow-xl border border-primary-dark/40"
		style={popoverStyle()}
		onclick={(e) => e.stopPropagation()}
	>
		<!-- Header with avatar -->
		<div class="flex items-center gap-3 p-4">
			<div class="relative flex-shrink-0">
				<Avatar url={user?.avatar_url ?? ''} name={user?.display_name || user?.username || ''} size="lg" />
				<!-- Presence dot -->
				<span
					class="absolute -bottom-0.5 -right-0.5 h-3.5 w-3.5 rounded-full border-2 border-navy-light {userPresence === 'active' ? 'bg-green-500' : 'bg-gray-500'}"
				></span>
			</div>
			<div class="min-w-0">
				<p class="truncate font-bold text-white">{user?.display_name || user?.username || userId}</p>
				{#if user?.display_name && user.username !== user.display_name}
					<p class="truncate text-xs text-primary-light/50">@{user.username}</p>
				{/if}
			</div>
		</div>

		<!-- Status -->
		{#if user?.status_emoji || user?.status_text}
			<div class="mx-4 mb-3 flex items-center gap-1.5 rounded bg-navy px-2.5 py-1.5 text-sm">
				{#if user.status_emoji}
					<span>{user.status_emoji}</span>
				{/if}
				{#if user.status_text}
					<span class="truncate text-primary-lighter/80">{user.status_text}</span>
				{/if}
			</div>
		{/if}

		<!-- Info -->
		<div class="border-t border-primary-dark/40 px-4 py-3 space-y-1">
			<div class="flex items-center gap-2 text-xs text-primary-light/50">
				<span class="h-1.5 w-1.5 rounded-full {userPresence === 'active' ? 'bg-green-500' : 'bg-gray-500'}"></span>
				{userPresence === 'active' ? 'Active' : 'Away'}
			</div>
			{#if user?.created_at}
				<p class="text-xs text-primary-light/40">Member since {formatDate(user.created_at)}</p>
			{/if}
		</div>

		<!-- Actions -->
		{#if !isOwnProfile}
			<div class="border-t border-primary-dark/40 p-3">
				<button
					onclick={handleMessage}
					class="w-full rounded bg-primary px-3 py-1.5 text-sm font-medium text-white hover:bg-primary-light"
				>
					Message
				</button>
			</div>
		{/if}
	</div>
</div>
