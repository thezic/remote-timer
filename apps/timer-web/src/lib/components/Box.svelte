<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = { header?: Snippet; footer?: Snippet; children: Snippet; scroll?: boolean };

	let { header, children, footer, scroll = false }: Props = $props();
</script>

<div class="flex h-full flex-col">
	{#if header}<div>{@render header()}</div>{/if}
	<div class="flex-grow" class:overflow-auto={scroll} class:overflow-hidden={!scroll}>
		{@render children()}
	</div>
	{#if footer}<div>{@render footer()}</div>{/if}
</div>
