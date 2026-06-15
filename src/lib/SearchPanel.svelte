<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { store } from "./stores.svelte";
  import { clampInt, type SearchResult, type SearchType } from "./types";
  import AlbumRow from "./AlbumRow.svelte";
  import ArtistRow from "./ArtistRow.svelte";

  let searching = $state(false);
  let inputEl = $state<HTMLInputElement | null>(null);

  // Focus the box on mount (opening the tab) and whenever Ctrl+F bumps the nonce.
  $effect(() => {
    void store.focusSearchNonce;
    inputEl?.focus();
  });

  const order: SearchType[] = ["track", "album", "artist", "playlist", "video"];
  const labels: Record<SearchType, string> = {
    track: "Tracks",
    album: "Albums",
    artist: "Artists",
    playlist: "Playlists",
    video: "Videos",
  };

  const counts = $derived(
    Object.fromEntries(order.map((k) => [k, store.searchResults.filter((r) => r.kind === k).length])) as Record<SearchType, number>,
  );
  const categories = $derived(order.filter((k) => counts[k] > 0));
  const current = $derived(store.searchResults.filter((r) => r.kind === store.searchCategory));

  async function run() {
    store.searchError = "";
    if (!store.searchQuery.trim()) return;
    searching = true;
    try {
      // empty kinds = all types; backend dedups by kind/id.
      const res = await invoke<SearchResult[]>("search", {
        query: store.searchQuery,
        kinds: [],
        limit: clampInt(store.searchLimit, 1, 25, 15),
      });
      store.setSearchResults(res);
      if (res.length === 0) store.searchError = "No results.";
    } catch (e) {
      store.searchError = String(e);
      store.setSearchResults([]);
    } finally {
      searching = false;
    }
  }
  async function queueIt(r: SearchResult) {
    try {
      await invoke("enqueue", {
        resources: [r.resource],
        quality: store.optQuality,
        pathOverride: store.optPathOverride,
        threads: clampInt(store.optThreads, 1, 16, 4),
        noSkip: store.optNoSkip,
      });
      store.appendLog(`queued ${r.resource}`);
    } catch (e) {
      store.searchError = String(e);
    }
  }
  async function now(r: SearchResult) {
    await queueIt(r);
    store.activeTab = "download";
  }
</script>

<div class="space-y-4 p-4">
  <div class="flex flex-wrap items-end gap-3">
    <div class="relative min-w-[220px] flex-1">
      <input
        bind:this={inputEl}
        bind:value={store.searchQuery}
        onkeydown={(e) => {
          if (e.key === "Enter") run();
        }}
        placeholder="Search…"
        disabled={!store.authed}
        class="w-full rounded-lg border px-3 py-2 pr-8 text-sm outline-none disabled:opacity-50"
        style="border-color: var(--border); background: var(--panel); color: var(--text)"
      />
      {#if store.searchQuery}
        <button
          class="absolute right-2 top-1/2 -translate-y-1/2 text-sm"
          style="color: var(--soft)"
          aria-label="Clear search"
          title="Clear"
          onclick={() => store.resetSearch()}>✕</button
        >
      {/if}
    </div>
    <button
      class="rounded-lg px-4 py-2 text-sm font-semibold disabled:opacity-40"
      style="background: var(--accent); color: #06231f"
      disabled={searching || !store.authed || !store.searchQuery.trim()}
      onclick={run}>{searching ? "Searching…" : "Search"}</button
    >
    <label class="flex items-center gap-1 text-xs" style="color: var(--muted)" title="results fetched per type">
      Max/type
      <input type="number" min="1" max="25" bind:value={store.searchLimit} class="w-14 rounded border px-2 py-1" style="border-color: var(--border); background: var(--panel); color: var(--text)" />
    </label>
  </div>

  {#if !store.authed}<p class="text-xs" style="color: var(--danger)">Sign in to search.</p>{/if}
  {#if store.searchError}<p class="text-xs" style="color: var(--danger)">{store.searchError}</p>{/if}

  {#if categories.length}
    <div class="flex flex-wrap gap-1 border-b pb-2" style="border-color: var(--border)">
      {#each categories as k}
        <button
          class="rounded-lg px-3 py-1 text-sm font-medium"
          style={store.searchCategory === k ? "background: var(--panel-soft); color: var(--text)" : "color: var(--muted)"}
          onclick={() => (store.searchCategory = k)}>{labels[k]} ({counts[k]})</button
        >
      {/each}
    </div>

    <div class="overflow-hidden rounded-lg border" style="border-color: var(--border); background: var(--panel)">
      {#if store.searchCategory === "album"}
        {#each current as r (r.resource)}<AlbumRow item={r} />{/each}
      {:else if store.searchCategory === "artist"}
        {#each current as r (r.resource)}<ArtistRow item={r} />{/each}
      {:else}
        {#each current as r (r.resource)}
          <div class="flex items-center gap-3 border-t px-3 py-2 text-sm first:border-t-0" style="border-color: var(--border)">
            <span class="min-w-0 flex-1 truncate" title={r.title}>{r.title}</span>
            <span class="hidden shrink-0 font-mono text-xs md:inline" style="color: var(--soft)">{r.id}</span>
            <button class="shrink-0 rounded px-2 py-1 text-xs" style="background: var(--panel-soft)" onclick={() => queueIt(r)}>Queue</button>
            <button class="shrink-0 rounded px-2 py-1 text-xs font-semibold" style="background: var(--accent); color: #06231f" onclick={() => now(r)}>↓ Now</button>
          </div>
        {/each}
      {/if}
    </div>
  {:else if store.searchRan && !searching}
    <p class="text-sm" style="color: var(--soft)">No results.</p>
  {/if}
</div>
