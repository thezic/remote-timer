<script lang="ts">
	import { goto } from '$app/navigation';

	function createTimer() {
		const id = crypto.randomUUID();
		goto(id);
	}
</script>

<h1>Timer</h1>
<button onclick={createTimer}>Create timer</button>
