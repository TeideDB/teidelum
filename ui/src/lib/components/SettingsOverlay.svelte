<script lang="ts">
	import { onMount } from 'svelte';
	import * as api from '$lib/api';
	import { auth, refreshCurrentUser } from '$lib/stores/auth';
	import { theme as themeStore } from '$lib/stores/theme';
	import { channels, loadChannels } from '$lib/stores/channels';
	import Avatar from '$lib/components/Avatar.svelte';
	import type { UserSettings, Channel } from '$lib/types';

	interface Props {
		onClose: () => void;
	}

	let { onClose }: Props = $props();

	type Section =
		| 'profile'
		| 'account'
		| 'appearance'
		| 'notifications'
		| 'channels';

	let active = $state<Section>('profile');
	let settings = $state<UserSettings | null>(null);

	// Profile
	let displayName = $state('');
	let email = $state('');
	let avatarUrl = $state('');
	let profileMsg = $state('');
	let profileErr = $state('');
	let uploadingAvatar = $state(false);

	// Account
	let oldPassword = $state('');
	let newPassword = $state('');
	let confirmPassword = $state('');
	let accountMsg = $state('');
	let accountErr = $state('');

	// Notifications & Appearance
	let notifDefault = $state<string>('all');
	let theme = $state<string>('dark');
	let timezone = $state<string>('');
	let settingsMsg = $state('');
	let settingsErr = $state('');

	const personalSections: { key: Section; label: string; icon: string }[] = [
		{ key: 'profile', label: 'Profile', icon: 'M16 7a4 4 0 11-8 0 4 4 0 018 0zM12 14a7 7 0 00-7 7h14a7 7 0 00-7-7z' },
		{ key: 'account', label: 'Account & Security', icon: 'M12 15v2m-6 4h12a2 2 0 002-2v-6a2 2 0 00-2-2H6a2 2 0 00-2 2v6a2 2 0 002 2zm10-10V7a4 4 0 00-8 0v4h8z' },
		{ key: 'appearance', label: 'Preferences', icon: 'M7 21a4 4 0 01-4-4V5a2 2 0 012-2h4a2 2 0 012 2v12a4 4 0 01-4 4zm0 0h12a2 2 0 002-2v-4a2 2 0 00-2-2h-2.343M11 7.343l1.657-1.657a2 2 0 012.828 0l2.829 2.829a2 2 0 010 2.828l-8.486 8.485M7 17h.01' }
	];

	const chatSections: { key: Section; label: string; icon: string }[] = [
		{ key: 'notifications', label: 'Notifications', icon: 'M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9' },
		{ key: 'channels', label: 'Channel Settings', icon: 'M7 20l4-16m2 16l4-16M6 9h14M4 15h14' }
	];

	// Channel notification overrides
	let channelOverrides = $derived(
		$channels
			.filter((c) => c.kind !== 'dm')
			.map((c) => ({
				...c,
				notifLevel: c.notification_level || 'default',
				isMuted: c.muted === 'true'
			}))
	);

	onMount(async () => {
		const state = $auth;
		if (state.user) {
			displayName = state.user.display_name || '';
			email = state.user.email || '';
			avatarUrl = state.user.avatar_url || '';
		}
		const res = await api.usersGetSettings();
		if (res.ok && res.settings) {
			settings = res.settings;
			notifDefault = res.settings.notification_default || 'all';
			theme = res.settings.theme || 'dark';
			timezone = res.settings.timezone || '';
		}
	});

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Escape') onClose();
	}

	function clearMessages() {
		settingsMsg = '';
		settingsErr = '';
		profileMsg = '';
		profileErr = '';
		accountMsg = '';
		accountErr = '';
	}

	async function saveProfile() {
		profileMsg = '';
		profileErr = '';
		const res = await api.usersUpdateProfile({
			display_name: displayName,
			email: email,
			avatar_url: avatarUrl
		});
		if (res.ok) {
			await refreshCurrentUser();
			profileMsg = 'Profile updated.';
		} else {
			profileErr = res.error || 'Failed to update profile.';
		}
	}

	async function handleAvatarSelect(e: Event) {
		const target = e.target as HTMLInputElement;
		const file = target.files?.[0];
		if (!file) return;

		uploadingAvatar = true;
		profileErr = '';
		profileMsg = '';

		try {
			const { get } = await import('svelte/store');
			const { nonDmChannels, dmChannels } = await import('$lib/stores/channels');
			const pubChannels = get(nonDmChannels);
			const privChannels = get(dmChannels);
			const channelId = pubChannels[0]?.id || privChannels[0]?.id;

			if (!channelId) {
				throw new Error('Must join at least one channel to upload files.');
			}

			const res = await api.filesUpload(channelId, file);
			if (res.ok && res.file) {
				avatarUrl = api.fileDownloadUrl(res.file.id, res.file.filename);
			} else {
				throw new Error(res.error || 'Upload failed');
			}
		} catch (err: any) {
			profileErr = err.message || 'Error uploading file.';
		} finally {
			uploadingAvatar = false;
			target.value = '';
		}
	}

	async function changePassword() {
		accountMsg = '';
		accountErr = '';
		if (newPassword !== confirmPassword) {
			accountErr = 'Passwords do not match.';
			return;
		}
		if (!newPassword) {
			accountErr = 'New password is required.';
			return;
		}
		const res = await api.usersChangePassword(oldPassword, newPassword);
		if (res.ok) {
			oldPassword = '';
			newPassword = '';
			confirmPassword = '';
			accountMsg = 'Password changed.';
		} else {
			accountErr = res.error || 'Failed to change password.';
		}
	}

	async function saveNotifications() {
		settingsMsg = '';
		settingsErr = '';
		const res = await api.usersUpdateSettings({ notification_default: notifDefault });
		if (res.ok) {
			settingsMsg = 'Notification settings saved.';
		} else {
			settingsErr = res.error || 'Failed to save settings.';
		}
	}

	async function saveAppearance() {
		settingsMsg = '';
		settingsErr = '';
		const updates: { theme?: string; timezone?: string } = { theme };
		if (timezone) updates.timezone = timezone;
		const res = await api.usersUpdateSettings(updates);
		if (res.ok) {
			themeStore.set(theme as 'dark' | 'light');
			settingsMsg = 'Preferences saved.';
		} else {
			settingsErr = res.error || 'Failed to save settings.';
		}
	}

	async function toggleChannelMute(channel: Channel) {
		if (channel.muted === 'true') {
			await api.conversationsUnmute(channel.id);
		} else {
			await api.conversationsMute(channel.id);
		}
		await loadChannels();
	}

	async function setChannelNotification(channelId: string, level: string) {
		await api.conversationsSetNotification(channelId, level);
		await loadChannels();
	}
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- Full-screen overlay -->
<div class="fixed inset-0 z-50 flex bg-navy">
	<!-- Left sidebar navigation -->
	<div class="flex w-64 flex-shrink-0 flex-col border-r border-primary-dark/40 bg-navy-light">
		<!-- Header with back button -->
		<div class="flex items-center gap-3 border-b border-primary-dark/40 px-5 py-4">
			<button
				onclick={onClose}
				class="flex items-center gap-2 text-primary-lighter/70 hover:text-heading transition"
			>
				<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 19l-7-7m0 0l7-7m-7 7h18" />
				</svg>
				<span class="text-sm font-medium">Back</span>
			</button>
		</div>

		<div class="flex-1 overflow-y-auto py-4">
			<!-- Personal settings group -->
			<div class="px-3 pb-4">
				<h3 class="mb-2 px-3 text-xs font-bold uppercase tracking-wider text-primary-light/40">Personal</h3>
				{#each personalSections as section}
					<button
						onclick={() => { active = section.key; clearMessages(); }}
						class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-left text-sm transition {active === section.key
							? 'bg-primary/20 text-heading font-medium border-l-2 border-primary'
							: 'text-primary-lighter/70 hover:bg-primary-darker/40 hover:text-heading border-l-2 border-transparent'}"
					>
						<svg class="h-[18px] w-[18px] flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d={section.icon} />
						</svg>
						{section.label}
					</button>
				{/each}
			</div>

			<!-- Chat settings group -->
			<div class="px-3">
				<h3 class="mb-2 px-3 text-xs font-bold uppercase tracking-wider text-primary-light/40">Chat</h3>
				{#each chatSections as section}
					<button
						onclick={() => { active = section.key; clearMessages(); }}
						class="flex w-full items-center gap-3 rounded-md px-3 py-2 text-left text-sm transition {active === section.key
							? 'bg-primary/20 text-heading font-medium border-l-2 border-primary'
							: 'text-primary-lighter/70 hover:bg-primary-darker/40 hover:text-heading border-l-2 border-transparent'}"
					>
						<svg class="h-[18px] w-[18px] flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d={section.icon} />
						</svg>
						{section.label}
					</button>
				{/each}
			</div>
		</div>
	</div>

	<!-- Content area -->
	<div class="flex flex-1 flex-col overflow-hidden">
		<!-- Top bar -->
		<div class="flex items-center justify-between border-b border-primary-dark/40 px-8 py-4">
			<h1 class="font-[Oswald] text-xl font-semibold text-heading">
				{#if active === 'profile'}Profile
				{:else if active === 'account'}Account & Security
				{:else if active === 'appearance'}Preferences
				{:else if active === 'notifications'}Notifications
				{:else if active === 'channels'}Channel Settings
				{/if}
			</h1>
			<button
				onclick={onClose}
				class="rounded-md p-2 text-primary-light/50 hover:bg-primary-darker/40 hover:text-heading transition"
				aria-label="Close settings"
			>
				<svg class="h-5 w-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
					<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
				</svg>
			</button>
		</div>

		<!-- Scrollable content -->
		<div class="flex-1 overflow-y-auto px-8 py-6">
			{#if active === 'profile'}
				<div class="max-w-lg space-y-6">
					<div class="flex items-center gap-5">
						<Avatar url={avatarUrl} name={displayName || $auth.user?.username || ''} size="lg" />
						<div>
							<p class="text-sm font-medium text-heading">{displayName || $auth.user?.username || ''}</p>
							<p class="text-xs text-primary-light/50">@{$auth.user?.username}</p>
							<label class="mt-2 inline-block cursor-pointer rounded border border-primary-light/30 hover:border-primary-light bg-navy px-3 py-1.5 text-xs font-medium text-primary-lighter transition">
								{uploadingAvatar ? 'Uploading...' : 'Change avatar'}
								<input type="file" accept="image/*" class="hidden" onchange={handleAvatarSelect} disabled={uploadingAvatar} />
							</label>
						</div>
					</div>

					<div class="space-y-4 rounded-lg border border-primary-dark/30 bg-navy-light/50 p-5">
						<div>
							<label for="displayName" class="mb-1.5 block text-sm font-medium text-primary-lighter/80">Display Name</label>
							<input
								id="displayName"
								type="text"
								bind:value={displayName}
								class="w-full rounded-md bg-navy px-3 py-2 text-sm text-white placeholder-primary-light/40 border border-primary-dark/30 focus:outline-none focus:border-primary focus:ring-1 focus:ring-primary"
							/>
						</div>

						<div>
							<label for="email" class="mb-1.5 block text-sm font-medium text-primary-lighter/80">Email</label>
							<input
								id="email"
								type="email"
								bind:value={email}
								class="w-full rounded-md bg-navy px-3 py-2 text-sm text-white placeholder-primary-light/40 border border-primary-dark/30 focus:outline-none focus:border-primary focus:ring-1 focus:ring-primary"
							/>
						</div>
					</div>

					{#if profileMsg}
						<p class="text-sm text-green-400">{profileMsg}</p>
					{/if}
					{#if profileErr}
						<p class="text-sm text-red-400">{profileErr}</p>
					{/if}

					<button
						onclick={saveProfile}
						class="rounded-md bg-primary px-5 py-2 text-sm font-medium text-white hover:bg-primary-light transition"
					>
						Save changes
					</button>
				</div>

			{:else if active === 'account'}
				<div class="max-w-lg space-y-6">
					<div class="space-y-4 rounded-lg border border-primary-dark/30 bg-navy-light/50 p-5">
						<h3 class="text-sm font-semibold text-heading">Change Password</h3>

						<div>
							<label for="oldPassword" class="mb-1.5 block text-sm font-medium text-primary-lighter/80">Current Password</label>
							<input
								id="oldPassword"
								type="password"
								bind:value={oldPassword}
								class="w-full rounded-md bg-navy px-3 py-2 text-sm text-white placeholder-primary-light/40 border border-primary-dark/30 focus:outline-none focus:border-primary focus:ring-1 focus:ring-primary"
							/>
						</div>

						<div>
							<label for="newPassword" class="mb-1.5 block text-sm font-medium text-primary-lighter/80">New Password</label>
							<input
								id="newPassword"
								type="password"
								bind:value={newPassword}
								class="w-full rounded-md bg-navy px-3 py-2 text-sm text-white placeholder-primary-light/40 border border-primary-dark/30 focus:outline-none focus:border-primary focus:ring-1 focus:ring-primary"
							/>
						</div>

						<div>
							<label for="confirmPassword" class="mb-1.5 block text-sm font-medium text-primary-lighter/80">Confirm New Password</label>
							<input
								id="confirmPassword"
								type="password"
								bind:value={confirmPassword}
								class="w-full rounded-md bg-navy px-3 py-2 text-sm text-white placeholder-primary-light/40 border border-primary-dark/30 focus:outline-none focus:border-primary focus:ring-1 focus:ring-primary"
							/>
						</div>

						{#if accountMsg}
							<p class="text-sm text-green-400">{accountMsg}</p>
						{/if}
						{#if accountErr}
							<p class="text-sm text-red-400">{accountErr}</p>
						{/if}

						<button
							onclick={changePassword}
							class="rounded-md bg-primary px-5 py-2 text-sm font-medium text-white hover:bg-primary-light transition"
						>
							Update password
						</button>
					</div>
				</div>

			{:else if active === 'appearance'}
				<div class="max-w-lg space-y-6">
					<div class="space-y-5 rounded-lg border border-primary-dark/30 bg-navy-light/50 p-5">
						<div>
							<h3 class="mb-3 text-sm font-semibold text-heading">Theme</h3>
							<div class="flex gap-3">
								<button
									onclick={() => (theme = 'dark')}
									class="flex items-center gap-2 rounded-md px-4 py-2.5 text-sm border transition {theme === 'dark'
										? 'bg-primary/20 border-primary text-heading'
										: 'bg-navy border-primary-dark/30 text-primary-lighter/70 hover:border-primary-light/30 hover:text-heading'}"
								>
									<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
										<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
									</svg>
									Dark
								</button>
								<button
									onclick={() => (theme = 'light')}
									class="flex items-center gap-2 rounded-md px-4 py-2.5 text-sm border transition {theme === 'light'
										? 'bg-primary/20 border-primary text-heading'
										: 'bg-navy border-primary-dark/30 text-primary-lighter/70 hover:border-primary-light/30 hover:text-heading'}"
								>
									<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
										<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" />
									</svg>
									Light
								</button>
							</div>
						</div>

						<div>
							<label for="timezone" class="mb-1.5 block text-sm font-medium text-primary-lighter/80">Timezone</label>
							<input
								id="timezone"
								type="text"
								bind:value={timezone}
								class="w-full rounded-md bg-navy px-3 py-2 text-sm text-white placeholder-primary-light/40 border border-primary-dark/30 focus:outline-none focus:border-primary focus:ring-1 focus:ring-primary"
								placeholder="e.g. America/New_York"
							/>
							<p class="mt-1 text-xs text-primary-light/40">Used for displaying timestamps in your local time.</p>
						</div>
					</div>

					{#if settingsMsg}
						<p class="text-sm text-green-400">{settingsMsg}</p>
					{/if}
					{#if settingsErr}
						<p class="text-sm text-red-400">{settingsErr}</p>
					{/if}

					<button
						onclick={saveAppearance}
						class="rounded-md bg-primary px-5 py-2 text-sm font-medium text-white hover:bg-primary-light transition"
					>
						Save changes
					</button>
				</div>

			{:else if active === 'notifications'}
				<div class="max-w-lg space-y-6">
					<div class="space-y-4 rounded-lg border border-primary-dark/30 bg-navy-light/50 p-5">
						<div>
							<h3 class="mb-1 text-sm font-semibold text-heading">Default notification level</h3>
							<p class="mb-3 text-xs text-primary-light/40">Applies to all channels unless overridden individually.</p>
							<select
								id="notifDefault"
								bind:value={notifDefault}
								class="w-full rounded-md bg-navy px-3 py-2 text-sm text-white border border-primary-dark/30 focus:outline-none focus:border-primary focus:ring-1 focus:ring-primary"
							>
								<option value="all">All messages</option>
								<option value="mentions">Mentions only</option>
								<option value="none">None</option>
							</select>
						</div>
					</div>

					{#if settingsMsg}
						<p class="text-sm text-green-400">{settingsMsg}</p>
					{/if}
					{#if settingsErr}
						<p class="text-sm text-red-400">{settingsErr}</p>
					{/if}

					<button
						onclick={saveNotifications}
						class="rounded-md bg-primary px-5 py-2 text-sm font-medium text-white hover:bg-primary-light transition"
					>
						Save changes
					</button>
				</div>

			{:else if active === 'channels'}
				<div class="max-w-2xl space-y-4">
					<p class="text-sm text-primary-lighter/50">Override notification levels or mute individual channels.</p>

					{#if channelOverrides.length === 0}
						<p class="text-sm text-primary-light/40">No channels to configure.</p>
					{:else}
						<div class="rounded-lg border border-primary-dark/30 overflow-hidden">
							<!-- Table header -->
							<div class="flex items-center bg-navy-light/80 px-4 py-2 text-xs font-semibold uppercase tracking-wider text-primary-light/40 border-b border-primary-dark/30">
								<span class="flex-1">Channel</span>
								<span class="w-36 text-center">Notifications</span>
								<span class="w-20 text-center">Mute</span>
							</div>
							{#each channelOverrides as ch (ch.id)}
								<div class="flex items-center border-b border-primary-dark/20 last:border-b-0 px-4 py-2.5 hover:bg-navy-light/30 transition">
									<div class="flex flex-1 items-center gap-2 min-w-0">
										<span class="text-primary-light/40">#</span>
										<span class="truncate text-sm text-body">{ch.name}</span>
										{#if ch.isMuted}
											<span class="rounded bg-primary-darker/60 px-1.5 py-0.5 text-[10px] text-primary-light/50">muted</span>
										{/if}
									</div>
									<div class="w-36 text-center">
										<select
											value={ch.notifLevel}
											onchange={(e) => setChannelNotification(ch.id, (e.target as HTMLSelectElement).value)}
											class="rounded-md bg-navy px-2 py-1 text-xs text-white border border-primary-dark/30 focus:outline-none focus:border-primary"
										>
											<option value="default">Default</option>
											<option value="all">All</option>
											<option value="mentions">Mentions</option>
											<option value="none">None</option>
										</select>
									</div>
									<div class="w-20 text-center">
										<button
											onclick={() => toggleChannelMute(ch)}
											class="rounded-md px-2.5 py-1 text-xs transition {ch.isMuted
												? 'bg-primary-darker/60 text-primary-lighter hover:text-heading'
												: 'text-primary-light/40 hover:bg-primary-darker/40 hover:text-primary-lighter'}"
										>
											{ch.isMuted ? 'Unmute' : 'Mute'}
										</button>
									</div>
								</div>
							{/each}
						</div>
					{/if}
				</div>
			{/if}
		</div>
	</div>
</div>
