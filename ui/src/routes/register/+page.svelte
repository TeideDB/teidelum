<script lang="ts">
	import { goto } from '$app/navigation';
	import { doRegister } from '$lib/stores/auth';

	let username = $state('');
	let displayName = $state('');
	let email = $state('');
	let password = $state('');
	let confirmPassword = $state('');
	let error = $state<string | null>(null);
	let loading = $state(false);
	let showPassword = $state(false);

	// Validation
	const usernameValid = $derived(
		username.length === 0 || /^[a-zA-Z0-9_-]{2,30}$/.test(username)
	);
	const usernameHint = $derived(
		username.length > 0 && !usernameValid
			? '2-30 characters, letters, numbers, hyphens, underscores'
			: ''
	);
	const emailValid = $derived(
		email.length === 0 || /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)
	);

	// Password strength
	const passwordStrength = $derived.by(() => {
		if (!password) return { score: 0, label: '', color: '' };
		let score = 0;
		if (password.length >= 8) score++;
		if (password.length >= 12) score++;
		if (/[a-z]/.test(password) && /[A-Z]/.test(password)) score++;
		if (/\d/.test(password)) score++;
		if (/[^a-zA-Z0-9]/.test(password)) score++;

		if (score <= 1) return { score: 1, label: 'Weak', color: 'bg-red-500' };
		if (score <= 2) return { score: 2, label: 'Fair', color: 'bg-yellow-500' };
		if (score <= 3) return { score: 3, label: 'Good', color: 'bg-blue-400' };
		return { score: 4, label: 'Strong', color: 'bg-green-500' };
	});

	const passwordsMatch = $derived(
		confirmPassword.length === 0 || password === confirmPassword
	);

	const canSubmit = $derived(
		username.trim().length >= 2 &&
		usernameValid &&
		email.trim().length > 0 &&
		emailValid &&
		password.length >= 8 &&
		password === confirmPassword &&
		!loading
	);

	const errorMessages: Record<string, string> = {
		username_taken: 'This username is already taken',
		email_taken: 'An account with this email already exists',
		password_too_short: 'Password must be at least 8 characters',
		invalid_arguments: 'Please fill in all required fields',
		server_misconfigured: 'Server is not configured properly',
		internal_error: 'Something went wrong. Please try again.',
		fetch_error: 'Cannot reach the server. Check your connection.'
	};

	function friendlyError(err: string): string {
		return errorMessages[err] || err;
	}

	async function handleSubmit(e: SubmitEvent) {
		e.preventDefault();
		if (!canSubmit) return;

		if (password !== confirmPassword) {
			error = 'Passwords do not match';
			return;
		}

		loading = true;
		error = null;

		const err = await doRegister(username.trim(), password, email.trim(), displayName.trim() || undefined);
		loading = false;

		if (err) {
			error = friendlyError(err);
		} else {
			goto('/');
		}
	}
</script>

<svelte:head>
	<title>Create Account - Teidelum</title>
</svelte:head>

<div class="flex min-h-screen items-center justify-center bg-navy px-4">
	<div class="w-full max-w-sm">
		<!-- Logo & heading -->
		<div class="mb-8 flex flex-col items-center gap-3">
			<img src="/teide-logo.svg" alt="Teidelum" class="h-12 w-12" />
			<h1 class="font-[Oswald] text-2xl font-semibold tracking-wide text-heading">Create Account</h1>
			<p class="text-sm text-primary-light/50">Set up your Teidelum account</p>
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

				<!-- Username -->
				<div>
					<label for="username" class="mb-1.5 block text-sm font-medium text-primary-lighter/80">
						Username <span class="text-red-400">*</span>
					</label>
					<input
						id="username"
						type="text"
						bind:value={username}
						class="w-full rounded border bg-navy px-3 py-2.5 text-body placeholder-primary-light/30 transition focus:outline-none focus:ring-1 {usernameValid ? 'border-primary-dark/40 focus:border-primary focus:ring-primary' : 'border-red-500/60 focus:border-red-500 focus:ring-red-500'}"
						placeholder="your-username"
						autocomplete="username"
						autocapitalize="none"
						spellcheck="false"
						required
					/>
					{#if usernameHint}
						<p class="mt-1 text-xs text-red-400/80">{usernameHint}</p>
					{/if}
				</div>

				<!-- Display name -->
				<div>
					<label for="displayName" class="mb-1.5 block text-sm font-medium text-primary-lighter/80">
						Display Name
						<span class="text-primary-light/30 font-normal">(optional)</span>
					</label>
					<input
						id="displayName"
						type="text"
						bind:value={displayName}
						class="w-full rounded border border-primary-dark/40 bg-navy px-3 py-2.5 text-body placeholder-primary-light/30 transition focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
						placeholder="How you'll appear to others"
						autocomplete="name"
					/>
				</div>

				<!-- Email -->
				<div>
					<label for="email" class="mb-1.5 block text-sm font-medium text-primary-lighter/80">
						Email <span class="text-red-400">*</span>
					</label>
					<input
						id="email"
						type="email"
						bind:value={email}
						class="w-full rounded border bg-navy px-3 py-2.5 text-body placeholder-primary-light/30 transition focus:outline-none focus:ring-1 {emailValid ? 'border-primary-dark/40 focus:border-primary focus:ring-primary' : 'border-red-500/60 focus:border-red-500 focus:ring-red-500'}"
						placeholder="you@example.com"
						autocomplete="email"
						required
					/>
					{#if email && !emailValid}
						<p class="mt-1 text-xs text-red-400/80">Please enter a valid email address</p>
					{/if}
				</div>

				<!-- Password -->
				<div>
					<label for="password" class="mb-1.5 block text-sm font-medium text-primary-lighter/80">
						Password <span class="text-red-400">*</span>
					</label>
					<div class="relative">
						<input
							id="password"
							type={showPassword ? 'text' : 'password'}
							bind:value={password}
							class="w-full rounded border border-primary-dark/40 bg-navy px-3 py-2.5 pr-10 text-body placeholder-primary-light/30 transition focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
							placeholder="Minimum 8 characters"
							autocomplete="new-password"
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
					<!-- Password strength meter -->
					{#if password}
						<div class="mt-2 flex items-center gap-2">
							<div class="flex flex-1 gap-1">
								{#each [1, 2, 3, 4] as level}
									<div class="h-1 flex-1 rounded-full {passwordStrength.score >= level ? passwordStrength.color : 'bg-primary-dark/40'}"></div>
								{/each}
							</div>
							<span class="text-xs {passwordStrength.score <= 1 ? 'text-red-400' : passwordStrength.score <= 2 ? 'text-yellow-400' : 'text-primary-lighter/60'}">{passwordStrength.label}</span>
						</div>
						{#if password.length > 0 && password.length < 8}
							<p class="mt-1 text-xs text-red-400/80">At least 8 characters required</p>
						{/if}
					{/if}
				</div>

				<!-- Confirm password -->
				<div>
					<label for="confirmPassword" class="mb-1.5 block text-sm font-medium text-primary-lighter/80">
						Confirm Password <span class="text-red-400">*</span>
					</label>
					<input
						id="confirmPassword"
						type={showPassword ? 'text' : 'password'}
						bind:value={confirmPassword}
						class="w-full rounded border bg-navy px-3 py-2.5 text-body placeholder-primary-light/30 transition focus:outline-none focus:ring-1 {passwordsMatch ? 'border-primary-dark/40 focus:border-primary focus:ring-primary' : 'border-red-500/60 focus:border-red-500 focus:ring-red-500'}"
						placeholder="Re-enter password"
						autocomplete="new-password"
						required
					/>
					{#if !passwordsMatch}
						<p class="mt-1 text-xs text-red-400/80">Passwords do not match</p>
					{/if}
				</div>

				<button
					type="submit"
					disabled={!canSubmit}
					class="w-full rounded bg-primary py-2.5 font-medium text-white transition hover:bg-primary-light disabled:cursor-not-allowed disabled:opacity-40"
				>
					{#if loading}
						<span class="inline-flex items-center gap-2">
							<svg class="h-4 w-4 animate-spin" fill="none" viewBox="0 0 24 24"><circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" /><path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" /></svg>
							Creating account...
						</span>
					{:else}
						Create Account
					{/if}
				</button>
			</form>
		</div>

		<p class="mt-5 text-center text-sm text-primary-light/40">
			Already have an account?
			<a href="/login" class="text-primary-lighter hover:text-heading hover:underline transition">Sign in</a>
		</p>
	</div>
</div>
