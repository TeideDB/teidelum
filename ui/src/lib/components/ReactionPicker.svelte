<script lang="ts">
	interface Props {
		onSelect: (emoji: string) => void;
		onClose: () => void;
	}

	let { onSelect, onClose }: Props = $props();

	const commonReactions = [
		'+1', '-1', 'heart', 'laughing', 'eyes',
		'tada', 'fire', 'rocket', '100', 'thinking'
	];

	const emojiMap: Record<string, string> = {
		'+1': '\u{1F44D}',
		'-1': '\u{1F44E}',
		'heart': '\u{2764}\u{FE0F}',
		'laughing': '\u{1F606}',
		'eyes': '\u{1F440}',
		'tada': '\u{1F389}',
		'fire': '\u{1F525}',
		'rocket': '\u{1F680}',
		'100': '\u{1F4AF}',
		'thinking': '\u{1F914}'
	};

	function handleSelect(name: string) {
		onSelect(name);
		onClose();
	}
</script>

<div class="rounded-lg border border-primary-dark/40 bg-navy-light p-2 shadow-xl">
	<div class="grid grid-cols-5 gap-1">
		{#each commonReactions as name}
			<button
				onclick={() => handleSelect(name)}
				class="rounded p-1.5 text-lg transition hover:bg-navy-mid"
				title={name}
			>
				{emojiMap[name] ?? name}
			</button>
		{/each}
	</div>
</div>
