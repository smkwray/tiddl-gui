<script lang="ts">
  import Modal from "./ui/Modal.svelte";
  let { onclose }: { onclose: () => void } = $props();

  const isMac = typeof navigator !== "undefined" && /mac/i.test(navigator.platform || navigator.userAgent);

  const rows: { keys: string; desc: string }[] = isMac
    ? [
        { keys: "⌘⌥← / ⌘⌥→", desc: "Previous / next tab" },
        { keys: "⌘F", desc: "Jump to Search and focus the box" },
        { keys: "⌃⇧⇥ / ⌃⇥", desc: "Prev / next result category (in Search)" },
        { keys: "Enter", desc: "Run the search (while in the box)" },
        { keys: "⌘⇧?", desc: "Show / hide this help" },
      ]
    : [
        { keys: "Ctrl+PageUp / Ctrl+PageDown", desc: "Previous / next tab" },
        { keys: "Ctrl+F", desc: "Jump to Search and focus the box" },
        { keys: "Ctrl+Shift+Tab / Ctrl+Tab", desc: "Prev / next result category (in Search)" },
        { keys: "Enter", desc: "Run the search (while in the box)" },
        { keys: "Ctrl+Shift+?", desc: "Show / hide this help" },
      ];
</script>

<Modal {onclose}>
  <div class="p-5">
    <div class="mb-3 flex items-center justify-between">
      <h2 class="text-base font-semibold">Keyboard shortcuts</h2>
      <button onclick={onclose} aria-label="Close" style="color: var(--soft)">✕</button>
    </div>
    <div class="space-y-2">
      {#each rows as r}
        <div class="flex items-baseline justify-between gap-4 text-sm">
          <kbd class="shrink-0 rounded px-2 py-0.5 font-mono text-xs" style="background: var(--panel-soft); color: var(--text)">{r.keys}</kbd>
          <span class="text-right" style="color: var(--muted)">{r.desc}</span>
        </div>
      {/each}
    </div>
  </div>
</Modal>
