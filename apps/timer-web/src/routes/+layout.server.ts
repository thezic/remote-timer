import type { LayoutServerLoad } from './$types';
import { env } from '$env/dynamic/private';

export const load: LayoutServerLoad = () => {
	return {
		timerApi: env.TIMER_API
	};
};
