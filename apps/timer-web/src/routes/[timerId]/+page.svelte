<script lang="ts">
	import { page } from '$app/state';
	import { onMount } from 'svelte';

	let timerId = page.params.timerId;
	let socket: WebSocket;

	onMount(() => {
		socket = new WebSocket(`ws://localhost:8080/ws/${timerId}`);
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
		socket.send(JSON.stringify({ type: 'SetTime', time: parseInt(newTime) }));
	}

	function formatTime(time: number) {
		const allSeconds = Math.floor(time / 1000);
		const hours = Math.floor(allSeconds / 3600);
		const minutes = Math.floor((allSeconds % 3600) / 60);
		const seconds = allSeconds % 60;
		return `${hours}:${minutes}:${seconds}`;
	}

	const formattedTime = $derived(formatTime(currentTime));
</script>

<h1>Timer {timerId}</h1>
<div>isRunning: {isRunning}</div>
<div>time: {formattedTime}</div>
<div>
	<button class="rounded border px-5" onclick={startTimer}>Start</button>
	<button class="rounded border px-5" onclick={stopTimer}>Stop</button>
</div>
<div>
	<input type="text" class="rounded border" bind:value={newTime} />
	<button class="rounded border px-5" onclick={setTime}>Set time</button>
</div>
