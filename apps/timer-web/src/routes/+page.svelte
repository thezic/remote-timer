<script lang="ts">
	import { goto } from '$app/navigation';
	import Button from '$lib/components/Button.svelte';
	import { onMount } from 'svelte';

	type HistoryItem = { id: string; created: number };
	class TimerHistory {
		history: HistoryItem[] = $state([]);
		constructor() {}

		load() {
			try {
				this.history = JSON.parse(localStorage.getItem('history') || '[]');
			} catch (e) {
				console.error(e);
				this.history = [];
			}
		}

		newTimer(): string {
			const id = crypto.randomUUID();
			this.history = [{ id, created: Date.now() }, ...this.history];
			localStorage.setItem('history', JSON.stringify(this.history));

			return id;
		}

		clear() {
			localStorage.removeItem('history');
			this.history = [];
		}
	}

	let history = new TimerHistory();

	onMount(() => {
		history.load();
	});

	function createTimer() {
		const id = history.newTimer();
		goto(id);
	}
</script>

<div class="flex h-screen items-center justify-center">
	<div class="overflow-hidden rounded-md border border-gray-300">
		<h1 class="flex justify-center bg-slate-600 px-2 py-2 text-xl text-white">Remote timer</h1>
		<div class="px-9 py-2 pt-4"><Button onclick={createTimer} text="Create new timer" /></div>
		{#if history.history.length > 0}
			<div class="px-9 py-2">
				<div>History</div>
				<ul>
					{#each history.history as item (item.id)}
						<li class="py-2">
							<a href="/{item.id}" class="text-gray-600 underline"
								>{new Date(item.created).toLocaleString()}</a
							>
						</li>
					{/each}
				</ul>
				<button
					class="mt-2 text-gray-500 underline hover:text-gray-900"
					onclick={() => history.clear()}>clear</button
				>
			</div>
		{/if}
	</div>
</div>
