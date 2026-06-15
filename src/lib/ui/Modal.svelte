<script lang="ts">
  import type { Snippet } from "svelte";
  let { onclose, children }: { onclose: () => void; children?: Snippet } = $props();

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
  }
</script>

<svelte:window {onkeydown} />

<div
  class="fixed inset-0 z-50 flex items-center justify-center p-4"
  style="background: rgba(0,0,0,.5)"
  role="presentation"
  onclick={(e) => {
    if (e.target === e.currentTarget) onclose();
  }}
>
  <div
    class="w-[min(92vw,460px)] rounded-xl border shadow-2xl"
    style="background: var(--panel); border-color: var(--border)"
  >
    {@render children?.()}
  </div>
</div>
