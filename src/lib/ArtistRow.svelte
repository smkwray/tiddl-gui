<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { store } from "./stores.svelte";
  import { clampInt, type Browsable, type FavItem } from "./types";
  import AlbumRow from "./AlbumRow.svelte";

  let { item }: { item: Browsable } = $props();

  const expanded = $derived(!!store.expanded[item.resource]);
  const albums = $derived(store.artistAlbumsCache[item.id] ?? []);
  let loading = $state(false);
  let err = $state("");
  // Once expanded and loaded, we know whether downloading would get anything.
  const known = $derived(store.artistAlbumsCache[item.id] !== undefined);
  const empty = $derived(known && albums.length === 0);

  const label = $derived(item.artist ? `${item.artist} - ${item.title}` : item.title);

  async function toggle() {
    const cur = !!store.expanded[item.resource];
    store.expanded[item.resource] = !cur;
    if (!cur && !store.artistAlbumsCache[item.id]) {
      loading = true;
      err = "";
      try {
        store.artistAlbumsCache[item.id] = await invoke<FavItem[]>("artist_albums", { artistId: item.id });
      } catch (e) {
        err = String(e);
        store.expanded[item.resource] = false;
      } finally {
        loading = false;
      }
    }
  }
  // Same enqueue call for both actions (as for track rows in Search): "↓ Now"
  // additionally jumps to the Download tab, "Queue" leaves you where you are.
  async function downloadArtist(now: boolean) {
    err = "";
    try {
      await invoke("enqueue", {
        resources: [item.resource],
        quality: store.optQuality,
        pathOverride: store.optPathOverride,
        threads: clampInt(store.optThreads, 1, 16, 4),
        noSkip: store.optNoSkip,
        singles: true, // grab EPs/singles too, matching the expanded view
      });
      store.appendLog(`queued ${item.resource}`);
      if (now) store.activeTab = "download";
    } catch (e) {
      err = String(e);
    }
  }
</script>

<div class="border-t" style="border-color: var(--border)">
  <div class="flex items-center gap-2 px-3 py-2 text-sm">
    <button class="w-4 shrink-0 text-xs" style="color: var(--soft)" aria-label="Expand artist" onclick={toggle}>{expanded ? "▾" : "▸"}</button>
    <span class="min-w-0 flex-1 truncate" title={label}>{label}</span>
    <button
      class="shrink-0 rounded px-2 py-1 text-xs disabled:opacity-40"
      style="background: var(--panel-soft)"
      disabled={empty}
      title={empty ? "Nothing to download — this match has no releases" : "Queue all albums, EPs & singles"}
      onclick={() => downloadArtist(false)}>Queue</button
    >
    <button
      class="shrink-0 rounded px-2 py-1 text-xs font-semibold disabled:opacity-40"
      style="background: var(--accent); color: #06231f"
      disabled={empty}
      title={empty ? "Nothing to download — this match has no releases" : "Download all albums, EPs & singles now"}
      onclick={() => downloadArtist(true)}>↓ Now</button
    >
  </div>
  <!-- Row-level so a failed queue/download is visible while collapsed too (an album-load
       failure also collapses the row, which used to hide its own message). -->
  {#if err}<p class="px-3 pb-2 text-xs" style="color: var(--danger)">{err}</p>{/if}
  {#if expanded}
    <div class="ml-5 pl-1" style="border-left: 2px solid var(--border)">
      {#if loading}
        <p class="px-3 py-1 text-xs" style="color: var(--soft)">Loading albums…</p>
      {:else if albums.length === 0}
        <p class="px-3 py-1 text-xs" style="color: var(--soft)">No albums, EPs or singles — this match only appears via features/compilations, so downloading it gets nothing.</p>
      {:else}
        {#each albums as a (a.resource)}<AlbumRow item={a} />{/each}
      {/if}
    </div>
  {/if}
</div>
