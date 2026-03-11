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
	<title>Login - Teidelum</title>
</svelte:head>

<div class="flex min-h-screen items-center justify-center bg-navy">
	<div class="w-full max-w-sm rounded-lg bg-navy-light p-8 shadow-xl">
		<div class="mb-6 flex flex-col items-center gap-2">
			<img src="/teide-logo.svg" alt="Teidelum" class="h-10 w-auto" />
			<h1 class="font-[Oswald] text-2xl font-semibold tracking-wide text-heading">Teidelum</h1>
		</div>

		<form onsubmit={handleSubmit} class="space-y-4">
			{#if error}
				<div class="rounded bg-red-900/50 px-3 py-2 text-sm text-red-300">{error}</div>
			{/if}

			<div>
				<label for="username" class="mb-1 block text-sm text-primary-lighter/70">Username</label>
				<input
					id="username"
					type="text"
					bind:value={username}
					class="w-full rounded bg-navy px-3 py-2 text-white placeholder-primary-light/40 focus:outline-none focus:ring-2 focus:ring-primary"
					placeholder="Enter username"
					autocomplete="username"
					required
				/>
			</div>

			<div>
				<label for="password" class="mb-1 block text-sm text-primary-lighter/70">Password</label>
				<input
					id="password"
					type="password"
					bind:value={password}
					class="w-full rounded bg-navy px-3 py-2 text-white placeholder-primary-light/40 focus:outline-none focus:ring-2 focus:ring-primary"
					placeholder="Enter password"
					autocomplete="current-password"
					required
				/>
			</div>

			<button
				type="submit"
				disabled={loading}
				class="w-full rounded bg-primary py-2 font-medium text-white transition hover:bg-primary-light disabled:opacity-50"
			>
				{loading ? 'Signing in...' : 'Sign In'}
			</button>
		</form>

		<p class="mt-4 text-center text-sm text-primary-light/50">
			Don't have an account?
			<a href="/register" class="text-primary-lighter hover:underline">Register</a>
		</p>
	</div>
</div>
