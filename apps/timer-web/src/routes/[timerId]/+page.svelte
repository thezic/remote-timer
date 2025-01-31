<script lang="ts">
	import { page } from '$app/state';
	import { formatTimeFromMs } from '$lib/time';
	import { onMount } from 'svelte';
	import type { PageProps } from './$types';
	import { TimerService } from './timerService.svelte';

	let timerId = page.params.timerId;
	let { data }: PageProps = $props();

	let wsUrl = `${data.timerApi}/ws/${timerId}`;

	const service = new TimerService();

	onMount(() => {
		service.connect(wsUrl);
	});

	let currentTime = $state(0);
	let isRunning = $state(false);

	function startTimer() {
		service.startTimer();
	}

	function stopTimer() {
		service.stopTimer();
	}

	let newTime = $state(0);
	function setTime() {
		service.setTime(newTime);
	}

	const formattedTime = $derived(formatTimeFromMs(currentTime));
</script>

<h1>Timer {timerId}</h1>
<div class="px-5 py-10">
	<div class="py-2 text-4xl font-bold">{formattedTime}</div>
	<div>isRunning: {isRunning}</div>
	<div>
		<button class="rounded border bg-slate-300 px-5 py-1 hover:bg-slate-200" onclick={startTimer}
			>Start</button
		>
		<button class="rounded border bg-slate-300 px-5 py-1 hover:bg-slate-200" onclick={stopTimer}
			>Stop</button
		>
	</div>
	<div class="py-2">
		<input type="text" class="rounded border px-2 py-1" bind:value={newTime} />
		<button class="rounded border bg-slate-300 px-5 py-1 hover:bg-slate-200" onclick={setTime}
			>Set time (in seconds)</button
		>
	</div>
</div>
