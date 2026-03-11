<script lang="ts">
	import '../app.css';
	import { page } from '$app/state';
	import { goto } from '$app/navigation';
	import { auth, isAuthenticated, initAuth } from '$lib/stores/auth';
	import '$lib/stores/theme';
	import { onMount } from 'svelte';

	let { children } = $props();
	let initialized = $state(false);

	const publicRoutes = ['/login', '/register'];

	onMount(async () => {
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
		<div class="text-primary-light/50">Loading...</div>
	</div>
{:else}
	{@render children()}
{/if}
