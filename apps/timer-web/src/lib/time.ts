export function formatTimeFromSeconds(time: number) {
	const sign = time < 0 ? '-' : '';
	time = Math.abs(time);
	const hours = '' + Math.floor(time / 3600);
	const minutes = '' + Math.floor((time % 3600) / 60);
	const seconds = '' + (time % 60);
	return `${sign}${hours.padStart(2, '0')}:${minutes.padStart(2, '0')}:${seconds.padStart(2, '0')}`;
}

export function seconds(ms: number) {
	return Math.floor(ms / 1000);
}
