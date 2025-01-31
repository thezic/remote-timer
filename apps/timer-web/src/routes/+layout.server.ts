import type { LayoutServerLoad } from './$types';
import { TIMER_API } from '$env/static/private';

export const load: LayoutServerLoad = () => {
	return {
		timerApi: TIMER_API
	};
};
