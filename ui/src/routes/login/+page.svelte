<script lang="ts">
	import { goto } from '$app/navigation';
	import { doLogin } from '$lib/stores/auth';

	let username = $state('');
	let password = $state('');
	let error = $state<string | null>(null);
	let loading = $state(false);
	let showPassword = $state(false);

	const errorMessages: Record<string, string> = {
		invalid_credentials: 'Invalid username or password',
		server_misconfigured: 'Server is not configured properly',
		internal_error: 'Something went wrong. Please try again.',
		fetch_error: 'Cannot reach the server. Check your connection.'
	};

	function friendlyError(err: string): string {
		return errorMessages[err] || err;
	}

	async function handleSubmit(e: SubmitEvent) {
		e.preventDefault();
		if (!username.trim() || !password) return;

		loading = true;
		error = null;

		const err = await doLogin(username.trim(), password);
		loading = false;

		if (err) {
			error = friendlyError(err);
		} else {
			goto('/');
		}
	}

	function handleKeydown(e: KeyboardEvent) {
		if (e.key === 'Enter' && !loading) {
			const form = (e.target as HTMLElement).closest('form');
			if (form) form.requestSubmit();
		}
	}
</script>

<svelte:head>
	<title>Sign in - Teidelum</title>
</svelte:head>

<div class="flex min-h-screen items-center justify-center bg-navy px-4">
	<div class="w-full max-w-sm">
		<!-- Logo & heading -->
		<div class="mb-8 flex flex-col items-center gap-3">
			<img src="/teide-logo.svg" alt="Teidelum" class="h-12 w-12" />
			<h1 class="font-[Oswald] text-2xl font-semibold tracking-wide text-heading">Sign in to Teidelum</h1>
			<p class="text-sm text-primary-light/50">Enter your credentials to continue</p>
		</div>

		<!-- Form card -->
		<div class="rounded-lg border border-primary-dark/40 bg-navy-light p-6 shadow-xl">
			<form onsubmit={handleSubmit} class="space-y-4">
				{#if error}
					<div class="flex items-center gap-2 rounded bg-red-900/40 border border-red-800/50 px-3 py-2.5 text-sm text-red-300">
						<svg class="h-4 w-4 flex-shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24">
							<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-2.5L13.732 4c-.77-.833-1.964-.833-2.732 0L4.082 16.5c-.77.833.192 2.5 1.732 2.5z" />
						</svg>
						{error}
					</div>
				{/if}

				<div>
					<label for="username" class="mb-1.5 block text-sm font-medium text-primary-lighter/80">Username</label>
					<input
						id="username"
						type="text"
						bind:value={username}
						onkeydown={handleKeydown}
						class="w-full rounded border border-primary-dark/40 bg-navy px-3 py-2.5 text-body placeholder-primary-light/30 transition focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
						placeholder="your-username"
						autocomplete="username"
						autocapitalize="none"
						spellcheck="false"
						required
					/>
				</div>

				<div>
					<label for="password" class="mb-1.5 block text-sm font-medium text-primary-lighter/80">Password</label>
					<div class="relative">
						<input
							id="password"
							type={showPassword ? 'text' : 'password'}
							bind:value={password}
							onkeydown={handleKeydown}
							class="w-full rounded border border-primary-dark/40 bg-navy px-3 py-2.5 pr-10 text-body placeholder-primary-light/30 transition focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
							placeholder="Enter password"
							autocomplete="current-password"
							required
						/>
						<button
							type="button"
							onclick={() => (showPassword = !showPassword)}
							class="absolute right-2 top-1/2 -translate-y-1/2 p-1 text-primary-light/40 hover:text-primary-lighter transition"
							tabindex={-1}
							aria-label={showPassword ? 'Hide password' : 'Show password'}
						>
							{#if showPassword}
								<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13.875 18.825A10.05 10.05 0 0112 19c-4.478 0-8.268-2.943-9.543-7a9.97 9.97 0 011.563-3.029m5.858.908a3 3 0 114.243 4.243M9.878 9.878l4.242 4.242M9.88 9.88l-3.29-3.29m7.532 7.532l3.29 3.29M3 3l3.59 3.59m0 0A9.953 9.953 0 0112 5c4.478 0 8.268 2.943 9.543 7a10.025 10.025 0 01-4.132 5.411m0 0L21 21" /></svg>
							{:else}
								<svg class="h-4 w-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" /><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" /></svg>
							{/if}
						</button>
					</div>
				</div>

				<button
					type="submit"
					disabled={loading || !username.trim() || !password}
					class="w-full rounded bg-primary py-2.5 font-medium text-white transition hover:bg-primary-light disabled:cursor-not-allowed disabled:opacity-40"
				>
					{#if loading}
						<span class="inline-flex items-center gap-2">
							<svg class="h-4 w-4 animate-spin" fill="none" viewBox="0 0 24 24"><circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" /><path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" /></svg>
							Signing in...
						</span>
					{:else}
						Sign In
					{/if}
				</button>
			</form>
		</div>

		<p class="mt-5 text-center text-sm text-primary-light/40">
			Don't have an account?
			<a href="/register" class="text-primary-lighter hover:text-heading hover:underline transition">Create one</a>
		</p>
	</div>
</div>
