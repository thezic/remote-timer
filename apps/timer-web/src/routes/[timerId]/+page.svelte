<script lang="ts">
	import { page } from '$app/state';
	import { formatTimeFromMs } from '$lib/time';
	import { onMount } from 'svelte';
	import type { PageProps } from './$types';

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
			// console.log('Message from server ', event.data);
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

	let newTime = $state('0');
	function setTime() {
		socket.send(JSON.stringify({ type: 'SetTime', time: parseInt(newTime) * 1000 }));
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
