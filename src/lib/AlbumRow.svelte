<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { store } from "./stores.svelte";
  import { clampInt, type AlbumMeta, type Browsable, type FavItem } from "./types";

  let { item }: { item: Browsable } = $props();

  const expanded = $derived(!!store.expanded[item.resource]);
  const tracks = $derived(store.albumTracksCache[item.id] ?? []);
  const selResources = $derived(tracks.filter((t) => store.trackSel[t.resource]).map((t) => t.resource));
  let loading = $state(false);
  let err = $state("");

  const label = $derived(item.artist ? `${item.artist} - ${item.title}` : item.title);

  // Search-album rows arrive without a date/track-count (text-scrape) — fetch
  // them lazily (read-only) and cache. Artist-expanded albums already have them.
  const meta = $derived(store.albumMetaCache[item.id]);
  const year = $derived(item.year ?? meta?.year ?? null);
  const nTracks = $derived(item.n_tracks ?? meta?.n_tracks ?? null);
  onMount(async () => {
    if (item.year == null && store.albumMetaCache[item.id] === undefined) {
      try {
        store.albumMetaCache[item.id] = await invoke<AlbumMeta>("album_meta", { albumId: item.id });
      } catch {
        /* leave year/track-count unknown */
      }
    }
  });

  function albumTemplate(): string {
    return (
      store.config?.template_album ||
      "Albums/{album.artist} - {album.title}/{item.number:02}. {item.artist} - {item.title}"
    );
  }

  async function toggle() {
    const cur = !!store.expanded[item.resource];
    store.expanded[item.resource] = !cur;
    if (!cur && !store.albumTracksCache[item.id]) {
      loading = true;
      err = "";
      try {
        const t = await invoke<FavItem[]>("album_tracks", { albumId: item.id });
        store.albumTracksCache[item.id] = t;
        for (const x of t) if (store.trackSel[x.resource] === undefined) store.trackSel[x.resource] = true;
      } catch (e) {
        err = String(e);
        store.expanded[item.resource] = false;
      } finally {
        loading = false;
      }
    }
  }
  function setAll(v: boolean) {
    for (const t of tracks) store.trackSel[t.resource] = v;
  }

  // Same enqueue call for both actions (as for track rows in Search): "↓ Now"
  // additionally jumps to the Download tab, "Queue" leaves you where you are.
  async function enqueue(resources: string[], output: string, now: boolean) {
    if (resources.length === 0) return;
    try {
      await invoke("enqueue", {
        resources,
        quality: store.optQuality,
        pathOverride: store.optPathOverride,
        threads: clampInt(store.optThreads, 1, 16, 4),
        noSkip: store.optNoSkip,
        output,
      });
      store.appendLog(`queued ${resources.join(", ")}`);
      if (now) store.activeTab = "download";
    } catch (e) {
      err = String(e);
    }
  }
  // Whole album -> Albums/ via the album template. Picked tracks -> Albums/ via -o override.
  const queueAlbum = () => enqueue([item.resource], "", false);
  const downloadAlbumNow = () => enqueue([item.resource], "", true);
  const downloadSelected = () => enqueue(selResources, albumTemplate(), true);
</script>

<div class="border-t" style="border-color: var(--border)">
  <div class="flex items-center gap-2 px-3 py-2 text-sm">
    <button class="w-4 shrink-0 text-xs" style="color: var(--soft)" aria-label="Expand album" onclick={toggle}>{expanded ? "▾" : "▸"}</button>
    <span class="min-w-0 flex-1 truncate" title={label}>{label}</span>
    {#if year}<span class="shrink-0 text-xs tabular-nums" style="color: var(--soft)">{year}</span>{/if}
    {#if item.tag}<span class="shrink-0 rounded px-1.5 py-0.5 text-[10px] uppercase" style="background: var(--panel-soft); color: var(--soft)">{item.tag}</span>{/if}
    {#if nTracks}<span class="shrink-0 text-xs" style="color: var(--soft)">{nTracks} tr</span>{/if}
    <button class="shrink-0 rounded px-2 py-1 text-xs" style="background: var(--panel-soft)" title="Add this album to the queue" onclick={queueAlbum}>Queue</button>
    <button class="shrink-0 rounded px-2 py-1 text-xs font-semibold" style="background: var(--accent); color: #06231f" title="Download this album now" onclick={downloadAlbumNow}>↓ Now</button>
  </div>
  {#if expanded}
    <div class="pb-2 pl-8 pr-3">
      {#if loading}
        <p class="py-1 text-xs" style="color: var(--soft)">Loading tracks…</p>
      {:else if err}
        <p class="py-1 text-xs" style="color: var(--danger)">{err}</p>
      {:else}
        <div class="mb-1 flex flex-wrap items-center gap-3 text-xs" style="color: var(--soft)">
          <button class="hover:underline" onclick={() => setAll(true)}>all</button>
          <button class="hover:underline" onclick={() => setAll(false)}>none</button>
          <button
            class="rounded px-2 py-0.5 font-semibold disabled:opacity-40"
            style="background: var(--accent); color: #06231f"
            disabled={selResources.length === 0}
            onclick={downloadSelected}>Download {selResources.length} selected → Albums/</button
          >
        </div>
        {#each tracks as t (t.resource)}
          <label class="flex items-center gap-2 py-0.5 text-sm">
            <input type="checkbox" bind:checked={store.trackSel[t.resource]} />
            <span class="w-6 shrink-0 text-right text-xs" style="color: var(--soft)">{t.number ?? ""}</span>
            <span class="min-w-0 flex-1 truncate" title={t.title}>{t.title}</span>
          </label>
        {/each}
      {/if}
    </div>
  {/if}
</div>
