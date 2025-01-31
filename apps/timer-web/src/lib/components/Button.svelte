<script lang="ts">
	let {
		onclick,
		text,
		disabled = false
	}: { onclick: () => void; text: string; disabled?: boolean } = $props();
</script>

<button
	{disabled}
	class="rounded border bg-slate-300 px-5 py-1 hover:bg-slate-200 disabled:bg-slate-200 disabled:text-gray-500"
	{onclick}
	>{text}
</button>
