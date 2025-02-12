<script lang="ts">
	import { TimerService } from '$lib/timerService.svelte';
	import { getContext } from 'svelte';

	const service = getContext('service') as TimerService;
	let container: HTMLDivElement;
	let fullscreenState: 'windowed' | 'fullscreen' = 'windowed';

	function toggleFullscreen() {
		if (fullscreenState === 'windowed') {
			container.requestFullscreen().then(() => {
				fullscreenState = 'fullscreen';
			});
		} else {
			document.exitFullscreen();
			fullscreenState = 'windowed';
		}
	}
</script>

<div
	bind:this={container}
	class="flex h-screen items-center justify-center bg-white p-5"
	onpointerup={toggleFullscreen}
>
	<svg viewBox="0 0 70 20">
		<text x="1" y="12">
			{service.formattedTime}
		</text>
	</svg>
</div>

<style>
	svg {
		width: 100%;
	}
</style>
