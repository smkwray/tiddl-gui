// Singleton reactive store (Svelte 5 runes). Components import `store` and read
// fields directly; the global event listeners in App.svelte are the only writers
// of queue/log/authFlow via the helper methods.

import { invoke } from "@tauri-apps/api/core";
import { clampInt } from "./types";
import type {
  TiddlStatus,
  FfmpegStatus,
  AuthStatus,
  ConfigDto,
  QueueItem,
  AuthFlowEvent,
  TabId,
  Quality,
  DownloadProgressEvent,
  DownloadFinishedEvent,
  SearchType,
  SearchResult,
  FavType,
  FavItem,
  AlbumMeta,
} from "./types";

const LOG_CAP = 2000;

class AppStore {
  tiddl = $state<TiddlStatus | null>(null);
  ffmpeg = $state<FfmpegStatus | null>(null);
  auth = $state<AuthStatus | null>(null);
  config = $state<ConfigDto | null>(null);
  queue = $state<QueueItem[]>([]);
  log = $state<string[]>([]);
  logCollapsed = $state(true);
  authFlow = $state<AuthFlowEvent | null>(null);
  showLogin = $state(false);
  activeTab = $state<TabId>("download");
  theme = $state<"dark" | "light">("dark");
  showHelp = $state(false);
  focusSearchNonce = $state(0);

  // GUI download options — single source of truth (edited in the Download tab,
  // used by Search/Library/Album too). Persisted to settings.json.
  optQuality = $state<Quality>("high");
  optThreads = $state(4);
  optNoSkip = $state(false);
  optPathOverride = $state("");
  optExtraPath = $state("");
  urlText = $state(""); // Download tab textarea — in the store so tab switches don't lose it

  // Search tab state — lives in the store so it survives tab switches.
  // (Not persisted to disk: a fresh app launch starts blank.)
  searchQuery = $state("");
  searchLimit = $state(15);
  searchKinds = $state<Record<SearchType, boolean>>({
    track: true,
    album: false,
    artist: false,
    playlist: false,
    video: false,
  });
  searchResults = $state<SearchResult[]>([]);
  searchError = $state("");
  searchRan = $state(false);
  searchCategory = $state<SearchType>("track");

  // Browse tree (Search expand): caches keyed by id survive tab switches.
  artistAlbumsCache = $state<Record<string, FavItem[]>>({});
  albumTracksCache = $state<Record<string, FavItem[]>>({});
  albumMetaCache = $state<Record<string, AlbumMeta>>({}); // year/n_tracks for search-album rows
  expanded = $state<Record<string, boolean>>({});
  trackSel = $state<Record<string, boolean>>({}); // keyed by track resource

  setSearchResults(results: SearchResult[]) {
    this.searchResults = results;
    this.searchRan = true;
    const order: SearchType[] = ["track", "album", "artist", "playlist", "video"];
    const first = order.find((k) => results.some((r) => r.kind === k));
    if (first) this.searchCategory = first;
  }

  // ---- GUI options persistence + keyboard nav ----
  initOptions(c: ConfigDto) {
    this.optQuality = c.quality;
    this.optThreads = c.threads;
    this.optNoSkip = c.no_skip;
    this.optPathOverride = c.path_override;
    this.optExtraPath = c.extra_path_dir;
  }
  async persistOptions() {
    // Normalize the bound value so a cleared number field snaps back to a valid
    // integer (input + store + disk stay consistent).
    this.optThreads = clampInt(this.optThreads, 1, 16, 4);
    await invoke("save_config", {
      quality: this.optQuality,
      pathOverride: this.optPathOverride,
      threads: this.optThreads,
      noSkip: this.optNoSkip,
      writeToTiddl: false,
      extraPathDir: this.optExtraPath,
    });
    this.config = await invoke<ConfigDto>("get_config");
  }

  cycleTab(dir: number) {
    const tabs: TabId[] = ["download", "search", "library", "settings"];
    const i = tabs.indexOf(this.activeTab);
    this.activeTab = tabs[(i + dir + tabs.length) % tabs.length];
  }
  cycleSearchCategory(dir: number) {
    const order: SearchType[] = ["track", "album", "artist", "playlist", "video"];
    const cats = order.filter((k) => this.searchResults.some((r) => r.kind === k));
    if (cats.length === 0) return;
    const i = Math.max(0, cats.indexOf(this.searchCategory));
    this.searchCategory = cats[(i + dir + cats.length) % cats.length];
  }

  resetSearch() {
    this.searchQuery = "";
    this.searchResults = [];
    this.searchError = "";
    this.searchRan = false;
  }

  // Favorites tab — list is cached to disk by the backend; mirrored here so it
  // survives tab switches. Selection defaults to all-checked when (re)loaded.
  favItems = $state<FavItem[]>([]);
  favCheckedAt = $state<number | null>(null);
  favSelected = $state<Record<string, boolean>>({});
  favFetchTypes = $state<Record<FavType, boolean>>({
    track: true,
    album: true,
    artist: false,
    playlist: false,
    video: false,
  });
  favLoaded = $state(false);

  setFavorites(items: FavItem[], checkedAt: number | null) {
    this.favItems = items;
    this.favCheckedAt = checkedAt;
    this.favSelected = Object.fromEntries(items.map((i) => [i.resource, true]));
  }

  get tiddlReady(): boolean {
    return this.tiddl?.found ?? false;
  }
  get authed(): boolean {
    return this.auth?.authed ?? false;
  }
  get busy(): boolean {
    return this.queue.some((j) => j.status === "running" || j.status === "queued");
  }
  get activeCount(): number {
    return this.queue.filter((j) => j.status === "running" || j.status === "queued").length;
  }

  appendLog(line: string) {
    this.log.push(line);
    if (this.log.length > LOG_CAP) this.log = this.log.slice(-LOG_CAP);
  }
  clearLog() {
    this.log = [];
  }

  setQueue(items: QueueItem[]) {
    this.queue = items;
  }
  upsertItem(item: QueueItem) {
    const i = this.queue.findIndex((q) => q.id === item.id);
    if (i >= 0) this.queue[i] = item;
    else this.queue.push(item);
  }
  patchProgress(e: DownloadProgressEvent) {
    const it = this.queue.find((q) => q.id === e.id);
    if (it) {
      it.tracks_done = e.tracks_done;
      if (e.title) it.label = e.title;
    }
  }
  finishItem(e: DownloadFinishedEvent) {
    const it = this.queue.find((q) => q.id === e.id);
    if (it) {
      it.status = e.status;
      it.exit_code = e.exit_code;
      it.total = e.total;
      it.failures = e.failures;
      it.error = e.error;
    }
  }

  setAuthFlow(e: AuthFlowEvent) {
    this.authFlow = e;
  }

  applyTheme() {
    document.documentElement.setAttribute("data-theme", this.theme);
  }
  initTheme() {
    const saved = localStorage.getItem("tiddl-gui-theme");
    if (saved === "light" || saved === "dark") this.theme = saved;
    this.applyTheme();
  }
  toggleTheme() {
    this.theme = this.theme === "dark" ? "light" : "dark";
    localStorage.setItem("tiddl-gui-theme", this.theme);
    this.applyTheme();
  }
}

export const store = new AppStore();
