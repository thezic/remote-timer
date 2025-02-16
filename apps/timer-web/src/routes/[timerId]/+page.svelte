<script lang="ts">
	import { page } from '$app/state';
	import TimeInput from '$lib/components/TimeInput.svelte';
	import { getContext, onMount } from 'svelte';
	import { TimerService } from '$lib/timerService.svelte';
	import { Icon } from '@steeze-ui/svelte-icon';
	import { QrCode as QrCodeIcon, Tv, QuestionMarkCircle, Play, Stop } from '@steeze-ui/heroicons';
	import QrCode from 'svelte-qrcode';
	import ConnectionStatus from '$lib/components/ConnectionStatusBar.svelte';
	import { formatTimeFromSeconds } from '$lib/time';
	import Box from '$lib/components/Box.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import { PresetTimes } from '$lib/presets.svelte';

	let timerId = page.params.timerId;
	let currentPageUrl = $state('');
	let displayUrl = $derived(currentPageUrl + '/display');

	const service = getContext('service') as TimerService;
	const presetTimes = new PresetTimes();

	let showQrCode = $state(false);

	onMount(() => {
		currentPageUrl = window.location.href;
		presetTimes.load();
	});

	function startTimer() {
		service.startTimer();
	}

	function stopTimer() {
		service.stopTimer();
	}

	function toggleTimer() {
		if (service.isRunning) {
			stopTimer();
		} else {
			startTimer();
		}
	}

	let newTime = $state(900);
	function setTime(e?: Event) {
		e?.preventDefault();
		presetTimes.add(newTime);
		service.setTime(newTime);
	}

	function useQuickTime(seconds: number) {
		newTime = seconds;
		setTime();
	}
</script>

{#snippet quickButton(seconds: number)}
	<button class="rounded-md border border-gray-300 px-3" onclick={() => useQuickTime(seconds)}
		>{Math.round(seconds / 60)}min</button
	>
{/snippet}

<div class="mx-auto h-dvh max-w-2xl px-4 shadow-inner sm:rounded-lg">
	<Box>
		{#snippet header()}
			<hbox class="items-center justify-between pt-2">
				<div><img class="h-9 w-9" src="/remote-timer.svg" alt="icon" /></div>
				<div>
					<ConnectionStatus clientCount={service.numberOfClients} connectionState={service.state} />
				</div>
			</hbox>
			<div class="flex flex-col items-center justify-center">
				<div class="mb-2 py-2 text-6xl font-bold" class:text-red-500={service.remainingSeconds < 0}>
					{formatTimeFromSeconds(Math.abs(service.remainingSeconds))}
				</div>
				<button
					class="rounded-full border-2 border-red-600 p-5 shadow-md"
					class:rounded-full={!service.isRunning}
					class:rounded-xl={service.isRunning}
					onclick={toggleTimer}
					><Icon src={service.isRunning ? Stop : Play} class="h-16 w-16 " theme="solid" /></button
				>
			</div>
		{/snippet}

		<div class="mt-8 grid grid-cols-3 gap-4">
			{#each presetTimes.times as time}
				{@render quickButton(time)}
			{/each}
		</div>

		{#snippet footer()}
			<form class="items-top flex gap-4" onsubmit={setTime}>
				<div class="flex-grow">
					<TimeInput value={newTime} onchange={(value) => (newTime = value)} />
				</div>
				<button class="h-10 w-10 rounded-md border border-gray-300" type="submit"
					><Icon class="h-9 w-9" src={Play} theme="solid" /></button
				>
			</form>
			<hbox class="mt-8 justify-between pb-2">
				<ul class="flex space-x-4">
					<li>
						<button class="block p-2" onclick={() => (showQrCode = true)}
							><Icon class="h-9 w-9" theme="solid" src={QrCodeIcon} /></button
						>
					</li>
					<li>
						<a href={displayUrl} class="block p-2"
							><Icon class="h-9 w-9" theme="solid" src={Tv} /></a
						>
					</li>
				</ul>
				<div><button><Icon class="h-9 w-9" theme="solid" src={QuestionMarkCircle} /></button></div>
			</hbox>
		{/snippet}
	</Box>
</div>

<Modal open={showQrCode} onClose={() => (showQrCode = false)}>
	<div class="flex flex-col content-center items-center">
		<QrCode value={currentPageUrl} size="400" />
		<div><a href={displayUrl} class="underline">{timerId}</a></div>
	</div>
</Modal>

<!--
<div class="flex h-dvh flex-col">
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
				<div class="py-2 text-4xl font-bold" class:text-red-500={service.remainingSeconds < 0}>
					{formatTimeFromSeconds(Math.abs(service.remainingSeconds))}
				</div>
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
								<Button text={formatTimeFromSeconds(time)} onclick={() => (newTime = time)} />
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
-->

<style>
	vbox {
		@apply flex flex-col;
	}
	hbox {
		@apply flex flex-row;
	}
</style>
