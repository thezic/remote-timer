<script lang="ts">
	import { page } from '$app/state';
	import TimeInput from '$lib/components/TimeInput.svelte';
	import { getContext, onMount } from 'svelte';
	import { TimerService } from '$lib/timerService.svelte';
	import Button from '$lib/components/Button.svelte';
	import QrCode from 'svelte-qrcode';

	let timerId = page.params.timerId;
	let currentPageUrl = $state('');
	let displayUrl = $derived(currentPageUrl + '/display');

	const service = getContext('service') as TimerService;

	onMount(() => {
		currentPageUrl = window.location.href;
	});

	function startTimer() {
		service.startTimer();
	}

	function stopTimer() {
		service.stopTimer();
	}

	let newTime = $state(900);
	function setTime() {
		service.setTime(newTime);
	}
</script>

<div class="flex justify-between px-2 py-2">
	<h1>Timer {timerId} status:{service.state}</h1>
	<ul class="flex space-x-1">
		<li
			class="h-4 w-4 rounded bg-yellow-200"
			class:bg-red-200={service.state == 'disconnected'}
			class:bg-yellow-200={service.state == 'connecting'}
			class:bg-teal-300={service.state == 'connected'}
		>
			&nbsp;
		</li>
		<!-- eslint-disable-next-line @typescript-eslint/no-unused-vars -->
		{#each { length: service.numberOfClients - 1 } as _}
			<li class="h-4 w-4 rounded bg-teal-300">&nbsp;</li>
		{/each}
	</ul>
</div>
<div class="flex flex-col items-center justify-center px-5 py-10">
	<div>
		<div class="py-2 text-4xl font-bold">{service.formattedTime}</div>
	</div>
	<div class="flex grow justify-between">
		<Button text="Start" onclick={startTimer} disabled={service.isRunning} />
		<Button text="Stop" onclick={stopTimer} disabled={!service.isRunning} />
	</div>
	<div class="py-2">
		<form class="flex flex-row" onsubmit={setTime}>
			<TimeInput value={newTime} onchange={(value) => (newTime = value)} />
			<div><Button text="Set time" type="submit" /></div>
		</form>
	</div>
	<div><QrCode value={currentPageUrl} size="100" /></div>
	<div><a href={displayUrl} class="underline">Display</a></div>
</div>
