<script lang="ts">
	import { goto } from '$app/navigation';
	import { doLogin } from '$lib/stores/auth';

	let username = $state('');
	let password = $state('');
	let error = $state<string | null>(null);
	let loading = $state(false);

	async function handleSubmit(e: SubmitEvent) {
		e.preventDefault();
		if (!username.trim() || !password) return;

		loading = true;
		error = null;

		const err = await doLogin(username.trim(), password);
		loading = false;

		if (err) {
			error = err;
		} else {
			goto('/');
		}
	}
</script>

<svelte:head>
	<title>Login - Teide Chat</title>
</svelte:head>

<div class="flex min-h-screen items-center justify-center bg-gray-900">
	<div class="w-full max-w-sm rounded-lg bg-gray-800 p-8 shadow-xl">
		<h1 class="mb-6 text-center text-2xl font-bold text-white">Teide Chat</h1>

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
					placeholder="Enter username"
					autocomplete="username"
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
					placeholder="Enter password"
					autocomplete="current-password"
					required
				/>
			</div>

			<button
				type="submit"
				disabled={loading}
				class="w-full rounded bg-blue-600 py-2 font-medium text-white transition hover:bg-blue-700 disabled:opacity-50"
			>
				{loading ? 'Signing in...' : 'Sign In'}
			</button>
		</form>

		<p class="mt-4 text-center text-sm text-gray-500">
			Don't have an account?
			<a href="/register" class="text-blue-400 hover:underline">Register</a>
		</p>
	</div>
</div>
