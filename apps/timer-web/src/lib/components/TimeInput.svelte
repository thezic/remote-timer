<script lang="ts">
	import { formatTimeFromMs } from '$lib/time';
	import type { ChangeEventHandler } from 'svelte/elements';

	type Props = { onchange: (seconds: number) => void; value: number };
	let { onchange, value }: Props = $props();

	let isValid = $state(true);

	const timeRegex = /^\d\d?:\d\d?(:\d\d?)?$/;

	function parseTime(timeString: string): number {
		const parts = timeString.split(':').map((part) => parseInt(part));
		if (parts.length == 2) {
			parts.push(0);
		}
		const [hours, minutes, seconds] = parts;
		return hours * 3600 + minutes * 60 + seconds;
	}

	function parseUnit(unit: string): number {
		switch (unit) {
			case 'hours':
			case 'hour':
			case 'h':
				return 3600;
			case 'min':
			case 'm':
				return 60;
			case 'sec':
			case 's':
				return 1;
			default:
				return 0;
		}
	}

	function parseNaturalTime(timeString: string): number | undefined {
		const matches = timeString.matchAll(/(?<value>\d+)(?<unit>(hours|hour|h|min|m|sec|s)+)/g);
		let seconds: number | undefined = undefined;
		for (const match of matches) {
			if (!match.groups) {
				continue;
			}

			seconds = seconds ?? 0;
			const value = parseInt(match.groups.value);
			const multiplier = parseUnit(match.groups.unit);
			console.log(value, multiplier);
			seconds += value * multiplier;
		}
		return seconds;
	}

	function parseDurationString(durationString: string): number | undefined {
		durationString = durationString.replace(/\s+/g, '').toLowerCase();
		if (durationString.match(timeRegex)) {
			return parseTime(durationString);
		}

		return parseNaturalTime(durationString);
	}

	const handleChange: ChangeEventHandler<HTMLInputElement> = (event) => {
		const timeString = event.currentTarget.value;

		const time = parseDurationString(timeString);

		if (time) {
			isValid = true;
			value = time;
			onchange(time);
		} else {
			isValid = false;
		}
		console.log(isValid);
	};
</script>

<div class="inline-block">
	<input
		onfocus={(e) => e.currentTarget.select()}
		onchange={handleChange}
		type="text"
		class="block w-36 rounded border px-2 py-1"
		class:invalid={!isValid}
		value={formatTimeFromMs(value * 1000)}
	/>
	<div class="px-2 text-xs text-gray-500">00:10, 25min, 5sec, etc...</div>
</div>

<style lang="postCss">
	.invalid {
		@apply border-red-500;
	}
</style>
