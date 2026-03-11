<script lang="ts">
	import { goto } from '$app/navigation';
	import { doRegister } from '$lib/stores/auth';

	let username = $state('');
	let email = $state('');
	let password = $state('');
	let confirmPassword = $state('');
	let error = $state<string | null>(null);
	let loading = $state(false);

	async function handleSubmit(e: SubmitEvent) {
		e.preventDefault();
		if (!username.trim() || !email.trim() || !password) return;

		if (password !== confirmPassword) {
			error = 'Passwords do not match';
			return;
		}

		loading = true;
		error = null;

		const err = await doRegister(username.trim(), password, email.trim());
		loading = false;

		if (err) {
			error = err;
		} else {
			goto('/');
		}
	}
</script>

<svelte:head>
	<title>Register - Teide Chat</title>
</svelte:head>

<div class="flex min-h-screen items-center justify-center bg-gray-900">
	<div class="w-full max-w-sm rounded-lg bg-gray-800 p-8 shadow-xl">
		<h1 class="mb-6 text-center text-2xl font-bold text-white">Create Account</h1>

		<form onsubmit={handleSubmit} class="space-y-4">
			{#if error}
				<div class="rounded bg-red-900/50 px-3 py-2 text-sm text-red-300">{error}</div>
			{/if}

			<div>
				<label for="username" class="mb-1 block text-sm text-gray-400">Username</label>
				<input
					id="username"
					type="text"
					bind:value={username}
					class="w-full rounded bg-gray-700 px-3 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
					placeholder="Choose a username"
					autocomplete="username"
					required
				/>
			</div>

			<div>
				<label for="email" class="mb-1 block text-sm text-gray-400">Email</label>
				<input
					id="email"
					type="email"
					bind:value={email}
					class="w-full rounded bg-gray-700 px-3 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
					placeholder="you@example.com"
					autocomplete="email"
					required
				/>
			</div>

			<div>
				<label for="password" class="mb-1 block text-sm text-gray-400">Password</label>
				<input
					id="password"
					type="password"
					bind:value={password}
					class="w-full rounded bg-gray-700 px-3 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
					placeholder="Choose a password"
					autocomplete="new-password"
					required
				/>
			</div>

			<div>
				<label for="confirmPassword" class="mb-1 block text-sm text-gray-400">Confirm Password</label>
				<input
					id="confirmPassword"
					type="password"
					bind:value={confirmPassword}
					class="w-full rounded bg-gray-700 px-3 py-2 text-white placeholder-gray-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
					placeholder="Confirm password"
					autocomplete="new-password"
					required
				/>
			</div>

			<button
				type="submit"
				disabled={loading}
				class="w-full rounded bg-blue-600 py-2 font-medium text-white transition hover:bg-blue-700 disabled:opacity-50"
			>
				{loading ? 'Creating account...' : 'Create Account'}
			</button>
		</form>

		<p class="mt-4 text-center text-sm text-gray-500">
			Already have an account?
			<a href="/login" class="text-blue-400 hover:underline">Sign in</a>
		</p>
	</div>
</div>
