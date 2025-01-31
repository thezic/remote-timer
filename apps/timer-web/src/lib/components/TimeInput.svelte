<script lang="ts">
	import { formatTimeFromMs } from '$lib/time';
	import type { FormEventHandler } from 'svelte/elements';

	const { oninput, value }: { oninput: (seconds: number) => void; value: number } = $props();

	const handleInput: FormEventHandler<HTMLInputElement> = (event) => {
		const timeString = (event.target as HTMLInputElement).value;
		console.log(timeString);
		const parts = timeString.split(':').map((part) => parseInt(part)); // .map(parseInt);
		if (parts.length == 2) {
			parts.unshift(0);
		}
		const [hours, minutes, seconds] = parts;
		const time = hours * 3600 + minutes * 60 + seconds;

		console.log(time);
		oninput(time);
	};
</script>

<input
	oninput={handleInput}
	class="rounded border px-2 py-1"
	type="time"
	step="1"
	value={formatTimeFromMs(value * 1000)}
/>

<style lang="postCss">
</style>
