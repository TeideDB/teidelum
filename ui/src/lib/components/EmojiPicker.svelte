<script lang="ts">
	import { onMount } from 'svelte';

	interface Props {
		onSelect: (emoji: string) => void;
	}

	let { onSelect }: Props = $props();

	let container: HTMLDivElement | undefined = $state();

	// Map native emoji to short name for reactions API
	const nativeToName: Record<string, string> = {
		'\u{1F44D}': '+1',
		'\u{1F44E}': '-1',
		'\u{2764}\u{FE0F}': 'heart',
		'\u{1F606}': 'laughing',
		'\u{1F440}': 'eyes',
		'\u{1F389}': 'tada',
		'\u{1F525}': 'fire',
		'\u{1F680}': 'rocket',
		'\u{1F4AF}': '100',
		'\u{1F914}': 'thinking'
	};

	onMount(async () => {
		const { Picker } = await import('emoji-mart');
		const data = (await import('@emoji-mart/data')).default;

		if (!container) return;

		const picker = new Picker({
			data,
			onEmojiSelect: (emoji: { native: string; id: string }) => {
				// Use known short name if available, otherwise use the emoji id
				const name = nativeToName[emoji.native] || emoji.id;
				onSelect(name);
			},
			theme: 'dark',
			previewPosition: 'none',
			skinTonePosition: 'search',
			maxFrequentRows: 2
		});

		container.appendChild(picker as unknown as Node);
	});
</script>

<div bind:this={container} class="emoji-picker-container"></div>

<style>
	.emoji-picker-container {
		line-height: normal;
	}

	.emoji-picker-container :global(em-emoji-picker) {
		--em-rgb-background: 30, 41, 59;
		--em-rgb-input: 51, 65, 85;
		--em-rgb-color: 226, 232, 240;
	}
</style>
