<script lang="ts">
  import type { Snippet } from "svelte";
  let { label, children }: { label: string; children?: Snippet } = $props();
</script>

<label class="block">
  <span class="mb-1 block text-xs font-medium" style="color: var(--muted)">{label}</span>
  {@render children?.()}
</label>
