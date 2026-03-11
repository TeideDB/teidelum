<script lang="ts">
	import { presence } from '$lib/stores/users';
	import type { Id } from '$lib/types';

	interface Props {
		userId: Id;
		size?: 'sm' | 'md';
	}

	let { userId, size = 'sm' }: Props = $props();

	const userPresence = $derived($presence.get(userId) ?? 'away');
	const isActive = $derived(userPresence === 'active');

	const sizeClasses = $derived(
		size === 'sm' ? 'h-2.5 w-2.5' : 'h-3 w-3'
	);
</script>

<span
	class="inline-block rounded-full {sizeClasses} {isActive ? 'bg-green-500' : 'border-2 border-gray-500 bg-transparent'}"
	title={isActive ? 'Active' : 'Away'}
></span>
