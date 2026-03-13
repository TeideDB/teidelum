<script lang="ts">
	import '../app.css';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { auth, isAuthenticated, initAuth, doLogout } from '$lib/stores/auth';
	import { setOnAuthExpired } from '$lib/api';
	import '$lib/stores/theme';
	import { onMount } from 'svelte';

	let { children } = $props();
	let initialized = $state(false);

	const publicRoutes = ['/login', '/register'];

	onMount(async () => {
		// When any API call gets a 401, log out and redirect to login
		setOnAuthExpired(() => {
			doLogout();
			goto('/login');
		});

		await initAuth();
		initialized = true;
	});

	$effect(() => {
		if (!initialized) return;
		const isPublic = publicRoutes.includes(page.url.pathname);

		if (!$isAuthenticated && !isPublic) {
			goto('/login');
		} else if ($isAuthenticated && isPublic) {
			goto('/');
		}
	});
</script>

{#if !initialized}
	<div class="flex min-h-screen items-center justify-center bg-navy">
		<div class="flex flex-col items-center gap-3">
			<img src="/teide-logo.svg" alt="Teidelum" class="h-10 w-10 animate-pulse" />
			<span class="text-sm text-primary-light/50">Loading...</span>
		</div>
	</div>
{:else}
	{@render children()}
{/if}
