<script lang="ts">
	import { onMount } from 'svelte';
	import * as api from '$lib/api';
	import { auth, refreshCurrentUser } from '$lib/stores/auth';
	import type { UserSettings } from '$lib/types';

	type Tab = 'profile' | 'account' | 'notifications' | 'appearance';

	let activeTab = $state<Tab>('profile');
	let settings = $state<UserSettings | null>(null);

	// Profile
	let displayName = $state('');
	let email = $state('');
	let profileMsg = $state('');
	let profileErr = $state('');

	// Account
	let oldPassword = $state('');
	let newPassword = $state('');
	let confirmPassword = $state('');
	let accountMsg = $state('');
	let accountErr = $state('');

	// Notifications & Appearance
	let notifDefault = $state<string>('all');
	let theme = $state<string>('dark');
	let settingsMsg = $state('');
	let settingsErr = $state('');

	const tabs: { key: Tab; label: string }[] = [
		{ key: 'profile', label: 'Profile' },
		{ key: 'account', label: 'Account' },
		{ key: 'notifications', label: 'Notifications' },
		{ key: 'appearance', label: 'Appearance' }
	];

	onMount(async () => {
		// Load current user info into profile fields
		const state = $auth;
		if (state.user) {
			displayName = state.user.display_name || '';
			email = state.user.email || '';
		}
		// Load settings
		const res = await api.usersGetSettings();
		if (res.ok && res.settings) {
			settings = res.settings;
			notifDefault = res.settings.notification_default || 'all';
			theme = res.settings.theme || 'dark';
		}
	});

	async function saveProfile() {
		profileMsg = '';
		profileErr = '';
		const res = await api.usersUpdateProfile({
			display_name: displayName,
			email: email
		});
		if (res.ok) {
			await refreshCurrentUser();
			profileMsg = 'Profile updated.';
		} else {
			profileErr = res.error || 'Failed to update profile.';
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
		const res = await api.usersUpdateSettings({ theme });
		if (res.ok) {
			settingsMsg = 'Appearance settings saved.';
		} else {
			settingsErr = res.error || 'Failed to save settings.';
		}
	}
</script>

<div class="flex h-full flex-col overflow-y-auto">
	<div class="border-b border-primary-dark/40 px-6 py-4">
		<h1 class="font-[Oswald] text-xl font-semibold text-heading">Settings</h1>
	</div>

	<div class="flex flex-1">
		<!-- Tab navigation -->
		<div class="w-48 border-r border-primary-dark/40 px-2 py-4">
			{#each tabs as tab}
				<button
					onclick={() => {
						activeTab = tab.key;
						settingsMsg = '';
						settingsErr = '';
					}}
					class="w-full rounded px-3 py-2 text-left text-sm transition {activeTab === tab.key
						? 'bg-primary text-white'
						: 'text-primary-lighter/70 hover:bg-primary-darker/60 hover:text-heading'}"
				>
					{tab.label}
				</button>
			{/each}
		</div>

		<!-- Tab content -->
		<div class="flex-1 p-6">
			{#if activeTab === 'profile'}
				<div class="max-w-md space-y-4">
					<h2 class="text-lg font-semibold text-heading">Profile</h2>

					<div>
						<label for="displayName" class="mb-1 block text-sm text-primary-lighter/70">Display Name</label>
						<input
							id="displayName"
							type="text"
							bind:value={displayName}
							class="w-full rounded bg-navy px-3 py-2 text-white placeholder-primary-light/40 focus:outline-none focus:ring-2 focus:ring-primary"
						/>
					</div>

					<div>
						<label for="email" class="mb-1 block text-sm text-primary-lighter/70">Email</label>
						<input
							id="email"
							type="email"
							bind:value={email}
							class="w-full rounded bg-navy px-3 py-2 text-white placeholder-primary-light/40 focus:outline-none focus:ring-2 focus:ring-primary"
						/>
					</div>

					{#if profileMsg}
						<p class="text-sm text-green-400">{profileMsg}</p>
					{/if}
					{#if profileErr}
						<p class="text-sm text-red-400">{profileErr}</p>
					{/if}

					<button
						onclick={saveProfile}
						class="rounded bg-primary px-4 py-2 text-sm font-medium text-white hover:bg-primary-light"
					>
						Save Profile
					</button>
				</div>
			{:else if activeTab === 'account'}
				<div class="max-w-md space-y-4">
					<h2 class="text-lg font-semibold text-heading">Change Password</h2>

					<div>
						<label for="oldPassword" class="mb-1 block text-sm text-primary-lighter/70">Current Password</label>
						<input
							id="oldPassword"
							type="password"
							bind:value={oldPassword}
							class="w-full rounded bg-navy px-3 py-2 text-white placeholder-primary-light/40 focus:outline-none focus:ring-2 focus:ring-primary"
						/>
					</div>

					<div>
						<label for="newPassword" class="mb-1 block text-sm text-primary-lighter/70">New Password</label>
						<input
							id="newPassword"
							type="password"
							bind:value={newPassword}
							class="w-full rounded bg-navy px-3 py-2 text-white placeholder-primary-light/40 focus:outline-none focus:ring-2 focus:ring-primary"
						/>
					</div>

					<div>
						<label for="confirmPassword" class="mb-1 block text-sm text-primary-lighter/70">Confirm New Password</label>
						<input
							id="confirmPassword"
							type="password"
							bind:value={confirmPassword}
							class="w-full rounded bg-navy px-3 py-2 text-white placeholder-primary-light/40 focus:outline-none focus:ring-2 focus:ring-primary"
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
						class="rounded bg-primary px-4 py-2 text-sm font-medium text-white hover:bg-primary-light"
					>
						Change Password
					</button>
				</div>
			{:else if activeTab === 'notifications'}
				<div class="max-w-md space-y-4">
					<h2 class="text-lg font-semibold text-heading">Notifications</h2>

					<div>
						<label for="notifDefault" class="mb-1 block text-sm text-primary-lighter/70">Default Notification Level</label>
						<select
							id="notifDefault"
							bind:value={notifDefault}
							class="w-full rounded bg-navy px-3 py-2 text-white focus:outline-none focus:ring-2 focus:ring-primary"
						>
							<option value="all">All messages</option>
							<option value="mentions">Mentions only</option>
							<option value="none">None</option>
						</select>
					</div>

					{#if settingsMsg}
						<p class="text-sm text-green-400">{settingsMsg}</p>
					{/if}
					{#if settingsErr}
						<p class="text-sm text-red-400">{settingsErr}</p>
					{/if}

					<button
						onclick={saveNotifications}
						class="rounded bg-primary px-4 py-2 text-sm font-medium text-white hover:bg-primary-light"
					>
						Save
					</button>
				</div>
			{:else if activeTab === 'appearance'}
				<div class="max-w-md space-y-4">
					<h2 class="text-lg font-semibold text-heading">Appearance</h2>

					<div>
						<span class="mb-2 block text-sm text-primary-lighter/70">Theme</span>
						<div class="flex gap-3">
							<button
								onclick={() => (theme = 'dark')}
								class="rounded px-4 py-2 text-sm transition {theme === 'dark'
									? 'bg-primary text-white'
									: 'bg-navy text-primary-lighter/70 hover:text-heading'}"
							>
								Dark
							</button>
							<button
								onclick={() => (theme = 'light')}
								class="rounded px-4 py-2 text-sm transition {theme === 'light'
									? 'bg-primary text-white'
									: 'bg-navy text-primary-lighter/70 hover:text-heading'}"
							>
								Light
							</button>
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
						class="rounded bg-primary px-4 py-2 text-sm font-medium text-white hover:bg-primary-light"
					>
						Save
					</button>
				</div>
			{/if}
		</div>
	</div>
</div>
