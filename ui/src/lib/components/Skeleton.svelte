<script lang="ts">
	interface Props {
		variant?: 'message' | 'channel';
		count?: number;
	}

	let { variant = 'message', count = 4 }: Props = $props();
</script>

{#if variant === 'message'}
	{#each Array(count) as _, i}
		<div class="flex gap-3 px-1 py-2 {i > 0 ? '' : 'mt-3'}">
			<div class="skeleton h-9 w-9 flex-shrink-0 rounded-full"></div>
			<div class="flex-1 space-y-2 pt-0.5">
				<div class="flex items-center gap-2">
					<div class="skeleton h-3.5 w-24 rounded"></div>
					<div class="skeleton h-3 w-12 rounded"></div>
				</div>
				<div class="skeleton h-3 rounded" style="width: {60 + (i % 3) * 15}%"></div>
				{#if i % 2 === 0}
					<div class="skeleton h-3 rounded" style="width: {30 + (i % 4) * 10}%"></div>
				{/if}
			</div>
		</div>
	{/each}
{:else if variant === 'channel'}
	{#each Array(count) as _}
		<div class="flex items-center gap-2 px-2 py-1.5">
			<div class="skeleton h-3 w-3 rounded"></div>
			<div class="skeleton h-3.5 rounded" style="width: {40 + Math.random() * 30}%"></div>
		</div>
	{/each}
{/if}

<style>
	.skeleton {
		background: linear-gradient(
			90deg,
			var(--color-navy-light) 25%,
			var(--color-primary-darker) 50%,
			var(--color-navy-light) 75%
		);
		background-size: 200% 100%;
		animation: shimmer 1.5s infinite;
	}

	@keyframes shimmer {
		0% {
			background-position: 200% 0;
		}
		100% {
			background-position: -200% 0;
		}
	}
</style>
