<script lang="ts">
	import { page } from '$app/state';
	import TimeInput from '$lib/components/TimeInput.svelte';
	import { getContext, onMount } from 'svelte';
	import { TimerService } from '$lib/timerService.svelte';
	import Button from '$lib/components/Button.svelte';
	import QrCode from 'svelte-qrcode';
	import ConnectionStatusBar from '$lib/components/ConnectionStatusBar.svelte';
	import { formatTimeFromMs } from '$lib/time';

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

	const presetTimes = [30 * 60, 15 * 60, 10 * 60, 5 * 60, 3 * 60, 1 * 60];
</script>

<div class="flex h-screen flex-col">
	<header class="flex-none">
		<ConnectionStatusBar
			{timerId}
			clientCount={service.numberOfClients}
			connectionState={service.state}
		/>
	</header>
	<main class="flex-grow">
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
					<div>
						<div><TimeInput value={newTime} onchange={(value) => (newTime = value)} /></div>
						{#each presetTimes as time}
							<div>
								<Button text={formatTimeFromMs(time * 1000)} onclick={() => (newTime = time)} />
							</div>
						{/each}
					</div>
					<div><Button text="Set time" type="submit" /></div>
				</form>
			</div>
		</div>
	</main>
	<footer class="flex flex-none justify-end px-2 pb-2">
		<div class="flex flex-col items-center">
			<div><QrCode value={currentPageUrl} size="100" /></div>
			<div><a href={displayUrl} class="underline">Display</a></div>
		</div>
	</footer>
</div>
