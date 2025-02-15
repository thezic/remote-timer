<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = { open: boolean; onClose: () => void; children: Snippet };
	let { open, onClose, children }: Props = $props();
	function close() {
		onClose();
	}
</script>

{#if open}
	<div
		role="button"
		tabindex="-1"
		class="fixed left-0 top-0 h-dvh w-dvw bg-black opacity-25"
		onclick={close}
		onkeydown={(e) => e.key === 'Escape' && close()}
	></div>

	<div
		class="pointer-events-none fixed left-0 top-0 h-dvh w-dvw flex-col content-center items-center justify-center p-4"
	>
		<div class="pointer-events-auto w-full overflow-hidden rounded-lg bg-white p-4">
			<div class="flex justify-end">X</div>
			<div>{@render children()}</div>
		</div>
	</div>
{/if}
