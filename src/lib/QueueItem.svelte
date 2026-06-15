<script lang="ts">
  import type { QueueItem } from "./types";
  let {
    job,
    onCancel,
    onOpen,
  }: { job: QueueItem; onCancel: (id: number) => void; onOpen: () => void } = $props();

  const icons: Record<string, string> = {
    queued: "◌",
    running: "⏳",
    done: "✓",
    error: "✖",
    cancelled: "⊘",
  };
  const colors: Record<string, string> = {
    queued: "var(--soft)",
    running: "var(--accent)",
    done: "var(--ok)",
    error: "var(--danger)",
    cancelled: "var(--soft)",
  };
</script>

<div class="flex items-center gap-3 border-b px-3 py-2 text-sm last:border-b-0" style="border-color: var(--border)">
  <span style="color: {colors[job.status]}">{icons[job.status]}</span>
  <div class="min-w-0 flex-1">
    <div class="truncate" title={job.resource}>{job.label}</div>
    <div class="text-xs" style="color: var(--soft)">
      {job.quality}
      {#if job.status === "running"}· downloading… {job.tracks_done} track{job.tracks_done === 1 ? "" : "s"}
      {:else if job.status === "done"}· done · {job.total ?? job.tracks_done} track{(job.total ?? job.tracks_done) === 1 ? "" : "s"}
      {:else if job.status === "error"}· {job.error ?? "error"}
      {:else if job.status === "cancelled"}· cancelled
      {:else}· queued{/if}
    </div>
  </div>
  {#if job.status === "running" || job.status === "queued"}
    <button class="rounded px-2 py-1 text-xs" style="background: var(--panel-soft)" onclick={() => onCancel(job.id)}>Cancel</button>
  {:else if job.status === "done"}
    <button class="rounded px-2 py-1 text-xs" style="background: var(--panel-soft)" onclick={onOpen}>Open</button>
  {/if}
</div>
