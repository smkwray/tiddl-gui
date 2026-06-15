// Shared TS contract for tiddl-gui's frontend <-> Rust backend.
// Every interface mirrors a #[derive(Serialize)] struct or an emitted event
// payload. Tauri v2: JS invoke args are camelCase, Rust params snake_case.
// All download events are id-scoped (the backend owns the queue).

// ---- Command return types -------------------------------------------------

/** `check_tiddl` -> tiddl::TiddlStatus. */
export interface TiddlStatus {
  found: boolean;
  path: string;
  /** Always "" — tiddl has no --version flag. */
  version: string;
  /** "" when found; reason when not. */
  error: string;
}

/** `check_ffmpeg` -> tiddl::FfmpegStatus. Probed at startup. */
export interface FfmpegStatus {
  found: boolean;
  detail: string;
}

/** `get_auth_status` -> tiddl::AuthStatus. Parses ~/.tiddl/auth.json expires_at. */
export interface AuthStatus {
  authed: boolean;
  detail: string;
  user_id: number | null;
  country_code: string | null;
  expires_at: number | null; // unix secs
  /** File exists but token is past expiry. */
  expired: boolean;
}

/** `get_config` -> config::ConfigDto. */
export interface ConfigDto {
  // GUI-local (settings.json)
  quality: Quality;
  path_override: string; // "" = defer to tiddl config.toml
  threads: number; // default 4
  no_skip: boolean; // default false
  write_to_tiddl: boolean; // mirror quality/path into config.toml on save
  extra_path_dir: string; // extra dir prepended to PATH when spawning tiddl
  // effective (read from ~/.tiddl/config.toml)
  effective_path: string;
  // Raw values from ~/.tiddl/config.toml (for the in-app config editor).
  tiddl_download_path: string;
  tiddl_quality: string; // "" if unset
  tiddl_threads: number | null;
  tiddl_skip_existing: boolean | null;
  template_track: string;
  template_album: string;
  tiddl_config_path: string;
  tiddl_config_exists: boolean;
}

/** `album_meta` -> favorites::AlbumMeta. */
export interface AlbumMeta {
  year: number | null;
  n_tracks: number | null;
}

/**
 * One row scraped from `tiddl download search`.
 * Row is a result iff its "#" cell parsed as a positive integer.
 * IDs may duplicate across rows — key UI rows by index, not id.
 */
export interface SearchResult {
  num: number; // "#" column
  kind: SearchType; // Type column lowercased
  title: string; // verbatim incl. tags like "[STEREO]"
  id: string; // string; playlist/mix IDs are UUIDs
  resource: string; // "{kind}/{id}" — ready for `download url`
}

/**
 * One backend-owned download job (`get_queue`, `queue-item` events).
 * The backend assigns `id` and runs jobs through one serial worker.
 */
export interface QueueItem {
  id: number; // monotonic; 0 is reserved for the auth-login child
  resource: string; // submitted URL/"track/123" or "favorites: track, album"
  label: string; // resource until first Downloaded line, then last title
  status: JobStatus;
  quality: Quality;
  tracks_done: number; // count of "Downloaded ..." lines
  total: number | null; // final "Total downloads: N"
  failures: number; // parsed per-track failures (fail-loud)
  exit_code: number | null;
  error: string | null;
}

/** One read-only favorite (`cached_favorites` / `refresh_favorites`). */
export interface FavItem {
  kind: "track" | "album" | "artist" | "playlist" | "video";
  id: string;
  title: string;
  artist: string;
  resource: string; // "{kind}/{id}" — ready to enqueue
  n_tracks?: number | null; // albums: number of tracks
  number?: number | null; // album items: track number
  tag?: string | null; // "EP" / "Single" for non-album releases
  year?: number | null; // albums: release year
}

/** Minimal shape an expandable row (AlbumRow/ArtistRow) needs — satisfied by
 *  both SearchResult and FavItem. */
export interface Browsable {
  id: string;
  title: string;
  resource: string;
  artist?: string;
  n_tracks?: number | null;
  tag?: string | null;
  year?: number | null;
}

/** `cached_favorites` / `refresh_favorites` -> favorites::FavoritesCache. */
export interface FavoritesCache {
  items: FavItem[];
  checked_at: number | null; // unix secs of last refresh
  types: string[];
}

// ---- Event payloads (app.emit) --------------------------------------------

/**
 * "auth-flow" — device-flow login milestones from `tiddl auth login -n`.
 * Only open `uri` after the backend has validated its host.
 */
export interface AuthFlowEvent {
  phase: "already" | "verify" | "polling" | "success" | "expired" | "info" | "finished";
  text: string;
  uri?: string;
  code?: number;
}

/** "queue-item" — full snapshot of one item on enqueue/status change. */
export interface QueueItemEvent {
  item: QueueItem;
}

/**
 * "download-progress" — one "Downloaded <title>" line.
 * This is the ONLY progress granularity: per-track completion + final total.
 * There is NO per-byte/percent progress (Rich bars are suppressed when piped).
 */
export interface DownloadProgressEvent {
  id: number;
  tracks_done: number;
  title: string;
}

/** "download-log" — one raw output line for the activity log. */
export interface DownloadLogEvent {
  id: number;
  line: string;
}

/** "download-finished" — item process exit. */
export interface DownloadFinishedEvent {
  id: number;
  status: "done" | "error" | "cancelled";
  exit_code: number | null;
  total: number | null;
  failures: number;
  error: string | null;
  /** Note shown for cancelled items: finished tracks remain on disk. */
  leftover_note?: string | null;
}

// ---- Frontend-only types ---------------------------------------------------

export type Quality = "low" | "normal" | "high" | "max";

export const QUALITIES: { value: Quality; label: string }[] = [
  { value: "high", label: "16-bit FLAC (high)" },
  { value: "max", label: "24-bit HiRes (max)" },
  { value: "normal", label: "320k AAC (normal)" },
  { value: "low", label: "96k AAC (low)" },
];

/** Search types tiddl understands (no `mix` for search). */
export type SearchType = "track" | "album" | "artist" | "playlist" | "video";

/** Favorite types `tiddl download fav -t` accepts. */
export type FavType = "track" | "album" | "artist" | "playlist" | "video";

export type TabId = "download" | "search" | "library" | "settings";

export type JobStatus = "queued" | "running" | "done" | "error" | "cancelled";

/** Coerce a possibly-empty/NaN number input to a clamped integer. */
export function clampInt(v: unknown, min: number, max: number, dflt: number): number {
  const n = Math.floor(Number(v));
  if (!Number.isFinite(n)) return dflt;
  return Math.min(max, Math.max(min, n));
}
