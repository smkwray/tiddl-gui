<script lang="ts">
  import type { TabId } from "./types";
  let {
    active,
    disabled = false,
    onselect,
  }: { active: TabId; disabled?: boolean; onselect: (t: TabId) => void } = $props();

  const tabs: { id: TabId; label: string }[] = [
    { id: "download", label: "Download" },
    { id: "search", label: "Search" },
    { id: "library", label: "Library" },
    { id: "settings", label: "Settings" },
  ];
</script>

<nav class="flex gap-1 border-b px-4 py-2" style="border-color: var(--border); background: var(--panel)">
  {#each tabs as t}
    <button
      {disabled}
      onclick={() => onselect(t.id)}
      class="rounded-lg px-3 py-1.5 text-sm font-medium transition disabled:opacity-40"
      style={active === t.id
        ? "background: var(--panel-soft); color: var(--text)"
        : "color: var(--muted)"}>{t.label}</button
    >
  {/each}
</nav>
