<script lang="ts">
  import { QUALITIES, type Quality } from "../types";
  let {
    value = $bindable("high"),
    onchange,
  }: { value?: Quality; onchange?: () => void } = $props();
</script>

<select
  bind:value
  {onchange}
  class="rounded-lg border px-3 py-2 text-sm outline-none"
  style="border-color: var(--border); background: var(--panel); color: var(--text)"
>
  {#each QUALITIES as q}
    <option value={q.value}>{q.label}</option>
  {/each}
</select>
