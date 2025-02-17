<script lang="ts">
	import type { ConnectionState } from '$lib/timerService.svelte';
	import ConnectionBadge from './ConnectionBadge.svelte';

	type Props = {
		clientCount: number;
		connectionState: ConnectionState;
	};

	let { clientCount, connectionState }: Props = $props();
</script>

<ul class="flex space-x-1">
	<ConnectionBadge status={connectionState} />
	<!-- eslint-disable-next-line @typescript-eslint/no-unused-vars -->
	{#each { length: clientCount - 1 } as _}
		<ConnectionBadge status="connected" />
	{/each}
</ul>
