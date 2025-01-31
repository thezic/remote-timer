export function formatTimeFromMs(time: number) {
	const allSeconds = Math.floor(time / 1000);
	const hours = '' + Math.floor(allSeconds / 3600);
	const minutes = '' + Math.floor((allSeconds % 3600) / 60);
	const seconds = '' + (allSeconds % 60);
	return `${hours.padStart(2, '0')}:${minutes.padStart(2, '0')}:${seconds.padStart(2, '0')}`;
}
