<script lang="ts">
	import { page } from '$app/state';
	import { onMount, setContext } from 'svelte';
	import { TimerService } from '$lib/timerService.svelte';
	import type { LayoutProps } from './$types';

	let timerId = page.params.timerId;
	let { children, data }: LayoutProps = $props();

	const service = new TimerService();
	setContext('service', service);

	let wsUrl = `${data.timerApi}/ws/${timerId}`;
	onMount(() => {
		service.connect(wsUrl);
	});
</script>

{@render children()}
