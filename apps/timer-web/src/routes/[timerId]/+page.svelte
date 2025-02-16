<script lang="ts">
	import { page } from '$app/state';
	import TimeInput from '$lib/components/TimeInput.svelte';
	import { getContext, onMount } from 'svelte';
	import { TimerService } from '$lib/timerService.svelte';
	import { Icon } from '@steeze-ui/svelte-icon';
	import { QrCode as QrCodeIcon, Tv, Play, Stop } from '@steeze-ui/heroicons';
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
		newTime = parseInt(localStorage.getItem('newTime') ?? '600');
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

	let newTime = $state(0);
	$effect(() => localStorage.setItem('newTime', newTime.toString()));

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
	<button
		class="rounded-md border border-gray-200 px-3 transition-colors hover:bg-gray-50 active:bg-gray-50"
		onclick={() => useQuickTime(seconds)}>{Math.round(seconds / 60)}min</button
	>
{/snippet}

<div class="mx-auto h-dvh max-w-2xl px-4 shadow-inner sm:rounded-lg">
	<Box>
		{#snippet header()}
			<div class="flex flex-row items-center justify-between pt-2">
				<div><img class="h-9 w-9" src="/remote-timer.svg" alt="icon" /></div>
				<div>
					<ConnectionStatus clientCount={service.numberOfClients} connectionState={service.state} />
				</div>
			</div>
			<div class="flex flex-col items-center justify-center">
				<div class="mb-2 py-2 text-6xl font-bold" class:text-red-500={service.remainingSeconds < 0}>
					{formatTimeFromSeconds(Math.abs(service.remainingSeconds))}
				</div>
				<button
					class="rounded-full border-2 border-red-400 p-5 shadow-md transition-colors hover:bg-red-50"
					class:rounded-full={!service.isRunning}
					class:rounded-xl={service.isRunning}
					onclick={toggleTimer}
				>
					{#if service.isRunning}
						<Icon src={Stop} class="h-16 w-16 " theme="solid" />
					{:else}
						<Icon src={Play} class="relative left-1 h-16 w-16" theme="solid" />
					{/if}
				</button>
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
				<button
					class="h-10 w-10 rounded-md border border-gray-300 transition-colors hover:bg-gray-50 active:bg-gray-50"
					type="submit"><Icon class="h-9 w-9" src={Play} theme="solid" /></button
				>
			</form>
			<div class="mt-8 flex flex-row justify-between pb-2">
				<ul class="flex space-x-4">
					<li>
						<button
							class="block rounded-md p-2 transition-colors hover:bg-gray-50 active:bg-gray-50"
							onclick={() => (showQrCode = true)}
							><Icon class="h-9 w-9" theme="solid" src={QrCodeIcon} /></button
						>
					</li>
					<li>
						<a
							href={displayUrl}
							class="rounde-md block p-2 transition-colors hover:bg-gray-50 active:bg-gray-50"
							><Icon class="h-9 w-9" theme="solid" src={Tv} /></a
						>
					</li>
				</ul>
				<div>
					<!-- Placeholder for right side -->
				</div>
			</div>
		{/snippet}
	</Box>
</div>

<Modal open={showQrCode} onClose={() => (showQrCode = false)}>
	<div class="flex flex-col content-center items-center">
		<QrCode value={currentPageUrl} size="400" />
		<div><a href={displayUrl} class="underline">{timerId}</a></div>
	</div>
</Modal>

<style>
</style>
