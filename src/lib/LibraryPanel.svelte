<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { store } from "./stores.svelte";
  import { clampInt, type FavType, type FavItem, type FavoritesCache } from "./types";

  let loading = $state(false);
  let error = $state("");

  const typeList: FavType[] = ["track", "album", "artist", "playlist", "video"];

  const selectedResources = $derived(
    store.favItems.filter((i) => store.favSelected[i.resource]).map((i) => i.resource),
  );

  function groupOf(kind: FavType): FavItem[] {
    return store.favItems.filter((i) => i.kind === kind);
  }
  function labelOf(i: FavItem): string {
    if (i.kind === "artist" || i.kind === "playlist") return i.title;
    return i.artist ? `${i.artist} - ${i.title}` : i.title;
  }
  function fmtDate(ts: number | null): string {
    return ts ? new Date(ts * 1000).toLocaleString() : "never";
  }

  onMount(async () => {
    if (!store.favLoaded) {
      try {
        const c = await invoke<FavoritesCache>("cached_favorites");
        store.setFavorites(c.items, c.checked_at);
      } catch (e) {
        error = String(e);
      }
      store.favLoaded = true;
    }
  });

  async function refresh() {
    error = "";
    const types = typeList.filter((t) => store.favFetchTypes[t]);
    if (types.length === 0) {
      error = "Pick at least one type to check.";
      return;
    }
    loading = true;
    try {
      const c = await invoke<FavoritesCache>("refresh_favorites", { types });
      store.setFavorites(c.items, c.checked_at);
      if (c.items.length === 0) error = "No favorites found for the selected types.";
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function selectAll(v: boolean) {
    const next: Record<string, boolean> = {};
    for (const i of store.favItems) next[i.resource] = v;
    store.favSelected = next;
  }

  async function downloadSelected() {
    if (selectedResources.length === 0) return;
    error = "";
    try {
      await invoke("enqueue", {
        resources: selectedResources,
        quality: store.optQuality,
        pathOverride: store.optPathOverride,
        threads: clampInt(store.optThreads, 1, 16, 4),
        noSkip: store.optNoSkip,
      });
      store.activeTab = "download";
    } catch (e) {
      error = String(e);
    }
  }
</script>

<div class="space-y-4 p-4">
  <div class="rounded-lg border p-3" style="border-color: var(--border); background: var(--panel)">
    <div class="mb-2 flex flex-wrap items-center gap-3 text-xs" style="color: var(--muted)">
      <span class="font-medium" style="color: var(--text)">Check favorites:</span>
      {#each typeList as t}
        <label class="flex items-center gap-1"><input type="checkbox" bind:checked={store.favFetchTypes[t]} /> {t}s</label>
      {/each}
    </div>
    <div class="flex flex-wrap items-center gap-3">
      <button
        class="rounded-lg px-3 py-1.5 text-sm font-semibold disabled:opacity-40"
        style="background: var(--accent); color: #06231f"
        disabled={loading || !store.authed}
        onclick={refresh}>{loading ? "Checking…" : store.favItems.length ? "Refresh favorites" : "Check favorites"}</button
      >
      <span class="text-xs" style="color: var(--soft)">last checked: {fmtDate(store.favCheckedAt)}</span>
    </div>
    {#if !store.authed}<p class="mt-1 text-xs" style="color: var(--danger)">Sign in first.</p>{/if}
    <p class="mt-2 text-xs" style="color: var(--soft)">
      Read-only — lists your favorites; nothing downloads until you pick items below. Downloading an artist or playlist pulls all its tracks.
    </p>
  </div>

  {#if error}<p class="text-xs" style="color: var(--danger)">{error}</p>{/if}

  {#if store.favItems.length}
    <div class="flex flex-wrap items-center gap-3">
      <button
        class="rounded-lg px-4 py-2 text-sm font-semibold disabled:opacity-40"
        style="background: var(--accent); color: #06231f"
        disabled={selectedResources.length === 0}
        onclick={downloadSelected}>Download selected ({selectedResources.length})</button
      >
      <button class="text-xs hover:underline" style="color: var(--soft)" onclick={() => selectAll(true)}>Select all</button>
      <button class="text-xs hover:underline" style="color: var(--soft)" onclick={() => selectAll(false)}>None</button>
    </div>

    {#each typeList as kind}
      {#if groupOf(kind).length}
        <div class="overflow-hidden rounded-lg border" style="border-color: var(--border); background: var(--panel)">
          <div class="px-3 py-2 text-xs font-medium uppercase" style="color: var(--soft)">{kind}s ({groupOf(kind).length})</div>
          {#each groupOf(kind) as i (i.resource)}
            <label class="flex items-center gap-3 border-t px-3 py-2 text-sm" style="border-color: var(--border)">
              <input type="checkbox" bind:checked={store.favSelected[i.resource]} />
              <span class="min-w-0 flex-1 truncate" title={labelOf(i)}>{labelOf(i)}</span>
              {#if i.year}<span class="shrink-0 text-xs tabular-nums" style="color: var(--soft)">{i.year}</span>{/if}
              <span class="hidden shrink-0 font-mono text-xs md:inline" style="color: var(--soft)">{i.id}</span>
            </label>
          {/each}
        </div>
      {/if}
    {/each}
  {:else if !loading}
    <p class="text-sm" style="color: var(--soft)">No favorites loaded yet — pick types and click “Check favorites”.</p>
  {/if}
</div>
