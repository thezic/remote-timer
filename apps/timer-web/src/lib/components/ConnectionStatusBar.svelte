<script lang="ts">
	import type { ConnectionState } from '$lib/timerService.svelte';
	import ConnectionBadge from './ConnectionBadge.svelte';

	type Props = {
		timerId: string;
		clientCount: number;
		connectionState: ConnectionState;
	};

	let { clientCount, connectionState, timerId }: Props = $props();
</script>

<div class="flex justify-between px-2 py-2">
	<div class="text-xs text-gray-500">Timer {timerId} status:{connectionState}</div>
	<ul class="flex space-x-1">
		<ConnectionBadge status={connectionState} />
		<!-- eslint-disable-next-line @typescript-eslint/no-unused-vars -->
		{#each { length: clientCount - 1 } as _}
			<ConnectionBadge status="connected" />
		{/each}
	</ul>
</div>
