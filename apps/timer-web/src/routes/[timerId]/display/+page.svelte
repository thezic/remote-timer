<script lang="ts">
	import { TimerService } from '$lib/timerService.svelte';
	import { getContext } from 'svelte';

	const service = getContext('service') as TimerService;
</script>

<div class="flex h-screen items-center justify-center p-5">
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
