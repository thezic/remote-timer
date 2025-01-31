<script lang="ts">
	import { page } from '$app/state';
	import { TimerService } from '$lib/timerService.svelte';
	import { onMount } from 'svelte';
	import type { PageProps } from './$types';

	const service = new TimerService();
	let timerId = page.params.timerId;

	let { data }: PageProps = $props();
	const wsUrl = `${data.timerApi}/ws/${timerId}`;

	onMount(() => {
		service.connect(wsUrl);
	});
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
