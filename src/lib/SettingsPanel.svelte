<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { store } from "./stores.svelte";
  import { clampInt, type ConfigDto, type Quality } from "./types";
  import QualitySelect from "./ui/QualitySelect.svelte";

  let { onRecheck }: { onRecheck: () => void | Promise<void> } = $props();

  // Platform-appropriate example path for the Extra PATH dir hint.
  const isMac = typeof navigator !== "undefined" && /mac/i.test(navigator.platform || navigator.userAgent);

  const c = store.config;

  // Download options (quality/threads/no-skip) now live on the Download tab.
  // Settings only handles folder override + extra PATH (persisted via the store).
  let saveMsg = $state("");
  let saveOk = $state(false);

  async function saveOptions() {
    saveMsg = "";
    try {
      await store.persistOptions();
      saveOk = true;
      saveMsg = "Saved.";
    } catch (e) {
      saveOk = false;
      saveMsg = String(e);
    }
  }
  async function chooseOverride() {
    const p = await open({ directory: true, multiple: false, title: "Override download folder" });
    if (typeof p === "string") {
      store.optPathOverride = p;
      await saveOptions();
    }
  }
  async function useDefault() {
    store.optPathOverride = "";
    await saveOptions();
  }
  function openFolder() {
    invoke("open_path", { path: store.config?.effective_path ?? "" }).catch(() => {});
  }

  // --- tiddl config.toml editor ---
  let tcPath = $state(c?.tiddl_download_path ?? "");
  let tcQuality = $state<Quality>((c?.tiddl_quality as Quality) || "high");
  let tcThreads = $state(c?.tiddl_threads ?? 4);
  let tcSkip = $state(c?.tiddl_skip_existing ?? true);
  let tcTrack = $state(c?.template_track ?? "");
  let tcAlbum = $state(c?.template_album ?? "");
  let tcMsg = $state("");
  let tcOk = $state(false);

  async function pickTcPath() {
    const p = await open({ directory: true, multiple: false, title: "tiddl download folder" });
    if (typeof p === "string") tcPath = p;
  }
  function seedTc() {
    const cc = store.config;
    tcPath = cc?.tiddl_download_path ?? "";
    tcQuality = (cc?.tiddl_quality as Quality) || "high";
    tcThreads = cc?.tiddl_threads ?? 4;
    tcSkip = cc?.tiddl_skip_existing ?? true;
    tcTrack = cc?.template_track ?? "";
    tcAlbum = cc?.template_album ?? "";
  }
  async function recheck() {
    await onRecheck();
    seedTc(); // re-sync the editor to the (possibly changed) config on disk
  }
  async function saveTiddl() {
    tcMsg = "";
    try {
      await invoke("save_tiddl_config", {
        downloadPath: tcPath,
        trackQuality: tcQuality,
        threads: clampInt(tcThreads, 1, 16, 4),
        skipExisting: tcSkip,
        templateTrack: tcTrack,
        templateAlbum: tcAlbum,
      });
      store.config = await invoke<ConfigDto>("get_config");
      seedTc(); // reflect normalized saved values
      tcOk = true;
      tcMsg = "Saved to config.toml.";
    } catch (e) {
      tcOk = false;
      tcMsg = String(e);
    }
  }

  const inputCls = "rounded border px-2 py-1 text-xs";
  const inputStyle = "border-color: var(--border); background: var(--panel); color: var(--text)";
</script>

<div class="space-y-5 p-4">
  <section class="space-y-3">
    <h2 class="text-sm font-semibold">
      Folders &amp; PATH <span class="text-xs font-normal" style="color: var(--soft)">(quality / threads / no-skip are on the Download tab)</span>
    </h2>

    <div class="rounded-lg border p-3" style="border-color: var(--border); background: var(--panel)">
      <div class="mb-1 text-xs font-medium">
        Download folder <span style="color: var(--soft)">({store.optPathOverride ? "tiddl-gui override" : "using tiddl config"})</span>
      </div>
      <div class="mb-2 break-all font-mono text-xs" style="color: var(--muted)">{store.config?.effective_path ?? ""}</div>
      <div class="flex flex-wrap gap-2">
        <button class="rounded px-2.5 py-1 text-xs" style="background: var(--panel-soft)" onclick={chooseOverride}>Override…</button>
        {#if store.optPathOverride}
          <button class="rounded px-2.5 py-1 text-xs" style="background: var(--panel-soft)" onclick={useDefault}>Use tiddl default</button>
        {/if}
        <button class="rounded px-2.5 py-1 text-xs" style="background: var(--panel-soft)" onclick={openFolder}>Open</button>
      </div>
    </div>

    <label class="block text-xs" style="color: var(--muted)">
      Extra PATH dir <span style="color: var(--soft)">(if tiddl or ffmpeg live somewhere unusual)</span>
      <input type="text" bind:value={store.optExtraPath} placeholder={isMac ? "/opt/ffmpeg/bin" : "C:\\ffmpeg\\bin"} spellcheck="false" class="mt-1 w-full font-mono {inputCls}" style={inputStyle} />
    </label>

    <div class="flex items-center gap-3">
      <button class="rounded-lg px-4 py-2 text-sm font-semibold" style="background: var(--accent); color: #06231f" onclick={saveOptions}>Save paths</button>
      {#if saveMsg}<span class="text-xs" style="color: {saveOk ? 'var(--ok)' : 'var(--danger)'}">{saveMsg}</span>{/if}
    </div>
  </section>

  <section class="space-y-3">
    <h2 class="text-sm font-semibold">
      tiddl config <span class="text-xs font-normal" style="color: var(--soft)">(~/.tiddl/config.toml — used by tiddl everywhere, incl. the terminal; comments preserved)</span>
    </h2>
    <div class="grid gap-3 rounded-lg border p-3" style="border-color: var(--border); background: var(--panel)">
      <label class="block text-xs" style="color: var(--muted)">
        Download path
        <div class="mt-1 flex gap-2">
          <input type="text" bind:value={tcPath} spellcheck="false" class="flex-1 font-mono {inputCls}" style={inputStyle} />
          <button class="rounded px-2 py-1 text-xs" style="background: var(--panel-soft)" onclick={pickTcPath}>Choose…</button>
        </div>
      </label>
      <div class="flex flex-wrap items-end gap-4">
        <label class="text-xs" style="color: var(--muted)">Track quality<div class="mt-1"><QualitySelect bind:value={tcQuality} /></div></label>
        <label class="text-xs" style="color: var(--muted)">Threads <input type="number" min="1" max="16" bind:value={tcThreads} class="ml-1 w-14 {inputCls}" style={inputStyle} /></label>
        <label class="flex items-center gap-1 text-xs" style="color: var(--muted)"><input type="checkbox" bind:checked={tcSkip} /> skip existing</label>
      </div>
      <label class="block text-xs" style="color: var(--muted)">
        Track template
        <input type="text" bind:value={tcTrack} spellcheck="false" placeholder="Tracks/{'{item.artist}'} - {'{item.title}'}" class="mt-1 w-full font-mono {inputCls}" style={inputStyle} />
      </label>
      <label class="block text-xs" style="color: var(--muted)">
        Album template
        <input type="text" bind:value={tcAlbum} spellcheck="false" placeholder="Albums/{'{album.artist}'} - {'{album.title}'}/..." class="mt-1 w-full font-mono {inputCls}" style={inputStyle} />
      </label>
      <div class="flex items-center gap-3">
        <button class="rounded-lg px-3 py-1.5 text-sm font-semibold" style="background: var(--accent); color: #06231f" onclick={saveTiddl}>Save tiddl config</button>
        {#if tcMsg}<span class="text-xs" style="color: {tcOk ? 'var(--ok)' : 'var(--danger)'}">{tcMsg}</span>{/if}
      </div>
      <p class="break-all text-xs" style="color: var(--soft)">
        binary: {store.tiddl?.path ?? "—"} · file: {store.config?.tiddl_config_path ?? ""}{store.config?.tiddl_config_exists ? "" : " (will be created)"}
      </p>
      {#if store.ffmpeg && !store.ffmpeg.found}<p class="text-xs" style="color: var(--danger)">ffmpeg: {store.ffmpeg.detail}</p>{/if}
    </div>
    <button class="rounded-lg px-3 py-1.5 text-sm" style="background: var(--panel-soft)" onclick={recheck}>Recheck tiddl / ffmpeg</button>
  </section>
</div>
