<script lang="ts">
  import type { TiddlStatus } from "./types";
  let { status, onRecheck }: { status: TiddlStatus | null; onRecheck: () => void } = $props();
</script>

<div class="flex h-full items-center justify-center p-8">
  <div class="w-[min(92vw,560px)] rounded-xl border p-6" style="border-color: var(--border); background: var(--panel)">
    <h2 class="mb-1 text-lg font-semibold" style="color: var(--danger)">tiddl isn't installed (or not on PATH)</h2>
    <p class="mb-3 text-sm" style="color: var(--muted)">
      tiddl-gui is only a front-end — install the tiddl CLI yourself, then recheck.
    </p>
    <pre class="mb-3 overflow-x-auto rounded-lg p-3 text-xs" style="background: var(--bg); color: var(--text)">uv tool install tiddl    # recommended
# or:  pip install tiddl
# also needs ffmpeg + Python 3.13+</pre>
    {#if status?.error}
      <p class="mb-3 text-xs" style="color: var(--soft)">{status.error}</p>
    {/if}
    <button
      class="rounded-lg px-3 py-1.5 text-sm font-semibold"
      style="background: var(--accent-strong); color: #06231f"
      onclick={onRecheck}>Recheck</button
    >
  </div>
</div>
