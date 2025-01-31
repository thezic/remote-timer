<script lang="ts">
	import { page } from '$app/state';
	import { formatTimeFromMs } from '$lib/time';
	import TimeInput from '$lib/components/TimeInput.svelte';
	import { onMount } from 'svelte';
	import type { PageProps } from './$types';
	import Button from '$lib/components/Button.svelte';
	import QrCode from 'svelte-qrcode';

	let timerId = page.params.timerId;
	let socket: WebSocket;
	let { data }: PageProps = $props();

	let wsUrl = `${data.timerApi}/ws/${timerId}`;

	onMount(() => {
		socket = new WebSocket(wsUrl);
		socket.onopen = function () {
			console.log('WebSocket is connected');
		};

		socket.onmessage = function (event) {
			const msg = JSON.parse(event.data) as Message;
			if ('CurrentTime' in msg) {
				currentTime = msg.CurrentTime;
			}
			if ('IsRunning' in msg) {
				isRunning = msg.IsRunning;
			}
		};
	});

	let currentTime = $state(0);
	let isRunning = $state(false);

	type Message = { CurrentTime: number } | { IsRunning: boolean };

	function startTimer() {
		socket.send(JSON.stringify({ type: 'StartTimer' }));
	}

	function stopTimer() {
		socket.send(JSON.stringify({ type: 'StopTimer' }));
	}

	let newTime = $state(10 * 60);
	function setTime() {
		socket.send(JSON.stringify({ type: 'SetTime', time: newTime * 1000 }));
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
	<div><QrCode value={location.href} size="100" /></div>
</div>
