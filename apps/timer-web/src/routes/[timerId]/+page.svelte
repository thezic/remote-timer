<script lang="ts">
	import { page } from '$app/state';
	import { formatTimeFromMs } from '$lib/time';
	import TimeInput from '$lib/components/TimeInput.svelte';
	import { onMount } from 'svelte';
	import type { PageProps } from './$types';
	import { TimerService } from './timerService.svelte';
	import Button from '$lib/components/Button.svelte';
	import QrCode from 'svelte-qrcode';

	let timerId = page.params.timerId;
	let { data }: PageProps = $props();

	let wsUrl = `${data.timerApi}/ws/${timerId}`;
	let currentPageUrl = $state('');

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
<div class="flex flex-col items-center justify-center px-5 py-10">
	<div>
		<div class="py-2 text-4xl font-bold">{formattedTime}</div>
	</div>
	<div class="flex grow justify-between">
		<Button text="Start" onclick={startTimer} disabled={isRunning} />
		<Button text="Stop" onclick={stopTimer} disabled={!isRunning} />
	</div>
	<div class="py-2">
		<div>
			<TimeInput value={newTime} oninput={(value) => (newTime = value)} />
			<Button text="Set time" onclick={setTime} />
		</div>
	</div>
	<div><QrCode value={currentPageUrl} size="100" /></div>
</div>
