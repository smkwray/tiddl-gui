<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { store } from "./stores.svelte";
  import { clampInt, type QueueItem as QItem } from "./types";
  import QueueItem from "./QueueItem.svelte";
  import QualitySelect from "./ui/QualitySelect.svelte";

  let formError = $state("");

  const parsed = $derived(store.urlText.split("\n").map((s) => s.trim()).filter(Boolean));
  const badDash = $derived(parsed.filter((r) => r.startsWith("-")));

  async function add() {
    formError = "";
    if (parsed.length === 0) return;
    if (badDash.length) {
      formError = `Lines can't start with "-": ${badDash[0]}`;
      return;
    }
    if (parsed.length > 100) {
      formError = `That's ${parsed.length} items — add 100 or fewer at a time.`;
      return;
    }
    try {
      await invoke("enqueue", {
        resources: parsed,
        quality: store.optQuality,
        pathOverride: store.optPathOverride,
        threads: clampInt(store.optThreads, 1, 16, 4),
        noSkip: store.optNoSkip,
      });
      store.urlText = "";
    } catch (e) {
      formError = String(e);
    }
  }
  function cancel(id: number) {
    invoke("cancel_download", { id });
  }
  function openFolder() {
    invoke("open_path", { path: store.config?.effective_path ?? "" }).catch(() => {});
  }
  function cancelAll() {
    invoke("cancel_all");
  }
  async function clearDone() {
    await invoke("clear_finished");
    store.setQueue(await invoke<QItem[]>("get_queue"));
  }
</script>

<div class="space-y-4 p-4">
  <section>
    <label for="urls" class="mb-1 block text-sm font-medium">
      URLs <span style="color: var(--soft)">(one per line — track / album / playlist / artist / mix)</span>
    </label>
    <textarea
      id="urls"
      bind:value={store.urlText}
      rows="3"
      spellcheck="false"
      placeholder={"album/12345\ntrack/67890"}
      class="w-full resize-y rounded-lg border p-3 font-mono text-sm outline-none"
      style="border-color: var(--border); background: var(--panel); color: var(--text)"
    ></textarea>
    <div class="mt-2 flex flex-wrap items-center gap-3">
      <QualitySelect bind:value={store.optQuality} onchange={() => store.persistOptions()} />
      <label class="flex items-center gap-1 text-xs" style="color: var(--muted)">
        Threads
        <input type="number" min="1" max="16" bind:value={store.optThreads} onchange={() => store.persistOptions()} class="w-14 rounded border px-2 py-1" style="border-color: var(--border); background: var(--panel); color: var(--text)" />
      </label>
      <label class="flex items-center gap-1 text-xs" style="color: var(--muted)">
        <input type="checkbox" bind:checked={store.optNoSkip} onchange={() => store.persistOptions()} /> No-skip
      </label>
      <button
        class="rounded-lg px-4 py-2 text-sm font-semibold disabled:opacity-40"
        style="background: var(--accent); color: #06231f"
        disabled={parsed.length === 0}
        onclick={add}>+ Add to queue{parsed.length ? ` (${parsed.length})` : ""}</button
      >
    </div>
    {#if formError}<p class="mt-1 text-xs" style="color: var(--danger)">{formError}</p>{/if}
    <p class="mt-1 break-all text-xs" style="color: var(--soft)">→ {store.config?.effective_path ?? ""}</p>
  </section>

  <section>
    <div class="mb-1 flex items-center justify-between">
      <span class="text-sm font-medium">Queue ({store.queue.length})</span>
      <div class="flex gap-3">
        <button class="text-xs hover:underline" style="color: var(--soft)" onclick={cancelAll}>Cancel all</button>
        <button class="text-xs hover:underline" style="color: var(--soft)" onclick={clearDone}>Clear done</button>
      </div>
    </div>
    <div class="overflow-hidden rounded-lg border" style="border-color: var(--border); background: var(--panel)">
      {#each store.queue as job (job.id)}
        <QueueItem {job} onCancel={cancel} onOpen={openFolder} />
      {:else}
        <div class="p-4 text-sm" style="color: var(--soft)">Queue is empty. Add a URL above or use Search.</div>
      {/each}
    </div>
    <p class="mt-1 text-xs" style="color: var(--soft)">Progress is per-track — tiddl reports completed tracks, not bytes.</p>
  </section>
</div>
