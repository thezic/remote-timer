<script lang="ts">
	import type { Snippet } from 'svelte';
	import { Icon } from '@steeze-ui/svelte-icon';
	import { XMark } from '@steeze-ui/heroicons';

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
		<div class="pointer-events-auto m-auto w-fit overflow-hidden rounded-lg bg-white">
			<div class="flex justify-end">
				<button class="p-1 transition-colors hover:bg-gray-50 active:bg-gray-50" onclick={close}
					><Icon src={XMark} class="h-7 w-7" /></button
				>
			</div>
			<div class="w-fit px-7 pb-2 pt-0">{@render children()}</div>
		</div>
	</div>
{/if}
