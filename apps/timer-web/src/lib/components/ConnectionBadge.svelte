<script lang="ts">
	import type { ConnectionState } from '$lib/timerService.svelte';

	type Props = {
		status: ConnectionState;
	};

	let { status }: Props = $props();
</script>

<li
	class="h-4 w-4 rounded bg-yellow-200"
	class:bg-red-200={status == 'disconnected'}
	class:bg-yellow-200={status == 'connecting'}
	class:bg-teal-300={status == 'connected'}
>
	&nbsp;
</li>
