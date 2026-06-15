<script lang="ts">
  let {
    lines,
    collapsed,
    ontoggle,
    onclear,
  }: { lines: string[]; collapsed: boolean; ontoggle: () => void; onclear: () => void } = $props();

  let el = $state<HTMLDivElement | null>(null);

  // Auto-scroll to bottom as lines grow (writes only to the DOM, no state).
  $effect(() => {
    void lines.length;
    if (el && !collapsed) el.scrollTop = el.scrollHeight;
  });
</script>

<div class="border-t" style="border-color: var(--border); background: var(--panel)">
  <div class="flex items-center justify-between px-4 py-1.5">
    <button class="text-xs font-medium" style="color: var(--muted)" onclick={ontoggle}>
      {collapsed ? "▸" : "▾"} Activity ({lines.length})
    </button>
    <button class="text-xs hover:underline" style="color: var(--soft)" onclick={onclear}>clear</button>
  </div>
  {#if !collapsed}
    <div
      bind:this={el}
      class="h-40 overflow-y-auto px-4 pb-2 font-mono text-xs leading-relaxed"
      style="color: var(--muted)"
    >
      {#each lines as line}
        <div class="whitespace-pre-wrap break-all">{line}</div>
      {:else}
        <div style="color: var(--soft)">No activity yet.</div>
      {/each}
    </div>
  {/if}
</div>
