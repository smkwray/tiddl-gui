<script lang="ts">
  import type { Snippet } from "svelte";
  let {
    variant = "neutral",
    title = "",
    children,
  }: { variant?: "ok" | "danger" | "neutral"; title?: string; children?: Snippet } = $props();

  const styles: Record<string, string> = {
    ok: "background: color-mix(in srgb, var(--ok) 14%, transparent); color: var(--ok)",
    danger: "background: color-mix(in srgb, var(--danger) 14%, transparent); color: var(--danger)",
    neutral: "background: var(--panel-soft); color: var(--muted)",
  };
</script>

<span class="rounded-full px-2.5 py-1 text-xs whitespace-nowrap" style={styles[variant]} {title}>
  {@render children?.()}
</span>
