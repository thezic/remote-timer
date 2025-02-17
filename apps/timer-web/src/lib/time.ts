import { zip } from 'radash';

function getTimeParts(time: number) {
	const sign = Math.sign(time);
	time = Math.abs(time);

	const hours = Math.floor(time / 3600);
	const minutes = Math.floor((time % 3600) / 60);
	const seconds = time % 60;

	return { sign, hours, minutes, seconds };
}

export function formatTimeFromSeconds(time: number, hideZeroHour = true) {
	const {
		sign: signValue,
		hours: hoursValue,
		minutes: minutesValue,
		seconds: secondsValue
	} = getTimeParts(time);
	const sign = signValue < 0 ? '-' : '';

	const hours = hoursValue > 0 || !hideZeroHour ? hoursValue.toString().padStart(2, '0') + ':' : '';
	const minutes = '' + minutesValue;
	const seconds = '' + secondsValue;
	return `${sign}${hours}${minutes.padStart(2, '0')}:${seconds.padStart(2, '0')}`;
}

export function formatHumanTimeFromSeconds(time: number) {
	const { hours, minutes, seconds } = getTimeParts(time);
	const units = ['h', 'min', 'sec'];

	return zip([hours, minutes, seconds], units)
		.filter(([value]) => value > 0)
		.map(([value, unit]) => value + unit)
		.join(' ');
}

export function seconds(ms: number) {
	return Math.floor(ms / 1000);
}
