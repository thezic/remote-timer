<script lang="ts">
	import { formatTimeFromSeconds } from '$lib/time';
	import { TimerService } from '$lib/timerService.svelte';
	import { getContext } from 'svelte';

	const service = getContext('service') as TimerService;
	let container: HTMLDivElement;
	let fullscreenState: 'windowed' | 'fullscreen' = 'windowed';

	const isOvertime = $derived(service.remainingSeconds < 0);
	const displayTime = $derived(formatTimeFromSeconds(Math.abs(service.remainingSeconds), false));

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
	<svg viewBox="0 0 70 20" class:overtime={isOvertime}>
		<text x="1" y="12">
			{displayTime}
		</text>
	</svg>
</div>

<style>
	svg {
		width: 100%;
	}

	.overtime {
		@apply fill-red-500;
	}
</style>
