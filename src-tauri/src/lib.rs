mod config;
mod favorites;
mod queue;
mod search;
mod tiddl;

use tauri::{AppHandle, Emitter, Manager, State};

use config::ConfigDto;
use queue::{QueueItem, QueueState};
use tiddl::{AuthStatus, FfmpegStatus, SearchResult, TiddlStatus};

// ---- status -------------------------------------------------------------

#[tauri::command]
fn check_tiddl() -> TiddlStatus {
    tiddl::check()
}

#[tauri::command]
fn check_ffmpeg() -> FfmpegStatus {
    tiddl::check_ffmpeg()
}

#[tauri::command]
fn get_auth_status() -> AuthStatus {
    tiddl::auth_status()
}

// ---- auth (device flow + token mgmt) ------------------------------------

#[tauri::command]
async fn auth_login(app: AppHandle) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<QueueState>();
        tiddl::auth_login(&app, &state)
    })
    .await
    .map_err(|e| format!("auth task failed: {e}"))?
}

#[tauri::command]
async fn auth_logout() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(tiddl::auth_logout)
        .await
        .map_err(|e| format!("logout task failed: {e}"))?
}

#[tauri::command]
async fn auth_refresh() -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(tiddl::auth_refresh)
        .await
        .map_err(|e| format!("refresh task failed: {e}"))?
}

// ---- search -------------------------------------------------------------

#[tauri::command]
async fn search(query: String, kinds: Vec<String>, limit: u32) -> Result<Vec<SearchResult>, String> {
    tauri::async_runtime::spawn_blocking(move || search::run_search_api(query, kinds, limit))
        .await
        .map_err(|e| format!("search task failed: {e}"))?
}

// ---- queue / downloads --------------------------------------------------

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn enqueue(
    app: AppHandle,
    state: State<QueueState>,
    resources: Vec<String>,
    quality: String,
    path_override: String,
    threads: u32,
    no_skip: bool,
    output: Option<String>,
    singles: Option<bool>,
) -> Result<Vec<u64>, String> {
    queue::enqueue(
        &app,
        &state,
        resources,
        quality,
        path_override,
        threads,
        no_skip,
        output.unwrap_or_default(),
        singles.unwrap_or(false),
    )
}

#[tauri::command]
fn download_favorites(
    app: AppHandle,
    state: State<QueueState>,
    types: Vec<String>,
    quality: String,
    path_override: String,
    threads: u32,
    confirm: bool,
) -> Result<u64, String> {
    queue::enqueue_favorites(&app, &state, types, quality, path_override, threads, confirm)
}

#[tauri::command]
fn cancel_download(app: AppHandle, state: State<QueueState>, id: u64) {
    queue::cancel_download(&app, &state, id);
}

#[tauri::command]
fn cancel_all(app: AppHandle, state: State<QueueState>) {
    queue::cancel_all(&app, &state);
}

#[tauri::command]
fn get_queue(state: State<QueueState>) -> Vec<QueueItem> {
    state.snapshot()
}

#[tauri::command]
fn clear_finished(state: State<QueueState>) {
    queue::clear_finished(&state);
}

// ---- favorites (read-only; see favorites.rs) ----------

#[tauri::command]
fn cached_favorites() -> favorites::FavoritesCache {
    favorites::cached()
}

#[tauri::command]
async fn refresh_favorites(types: Vec<String>) -> Result<favorites::FavoritesCache, String> {
    tauri::async_runtime::spawn_blocking(move || favorites::refresh(types))
        .await
        .map_err(|e| format!("favorites task failed: {e}"))?
}

#[tauri::command]
async fn artist_albums(artist_id: String) -> Result<Vec<favorites::FavItem>, String> {
    tauri::async_runtime::spawn_blocking(move || favorites::artist_albums(artist_id))
        .await
        .map_err(|e| format!("artist-albums task failed: {e}"))?
}

#[tauri::command]
async fn album_tracks(album_id: String) -> Result<Vec<favorites::FavItem>, String> {
    tauri::async_runtime::spawn_blocking(move || favorites::album_tracks(album_id))
        .await
        .map_err(|e| format!("album-tracks task failed: {e}"))?
}

#[tauri::command]
async fn album_meta(album_id: String) -> Result<favorites::AlbumMeta, String> {
    tauri::async_runtime::spawn_blocking(move || favorites::album_meta(album_id))
        .await
        .map_err(|e| format!("album-meta task failed: {e}"))?
}

// ---- config -------------------------------------------------------------

#[tauri::command]
fn get_config() -> ConfigDto {
    config::build_dto()
}

#[tauri::command]
fn save_config(
    quality: String,
    path_override: String,
    threads: u32,
    no_skip: bool,
    write_to_tiddl: bool,
    extra_path_dir: String,
) -> Result<(), String> {
    config::save(quality, path_override, threads, no_skip, write_to_tiddl, extra_path_dir)
}

#[tauri::command]
fn save_tiddl_config(
    download_path: String,
    track_quality: String,
    threads: u32,
    skip_existing: bool,
    template_track: String,
    template_album: String,
) -> Result<(), String> {
    config::save_tiddl_config(download_path, track_quality, threads, skip_existing, template_track, template_album)
}

// ---- shell helpers ------------------------------------------------------

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    if path.is_empty() || !std::path::Path::new(&path).exists() {
        return Err(format!("Path does not exist: {path}"));
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(&path).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        // explorer is a GUI app (no console flash). Reveal-and-select a file, but
        // open a directory directly so we never launch a media player on a track.
        let mut cmd = std::process::Command::new("explorer");
        if std::path::Path::new(&path).is_dir() {
            cmd.arg(&path);
        } else {
            cmd.arg(format!("/select,{path}"));
        }
        let _ = cmd.spawn();
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(&path).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    let u = url.trim();
    if !u.starts_with("https://link.tidal.com/") {
        return Err("refusing to open an untrusted URL".into());
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(u).spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "windows")]
    {
        // `cmd /C start "" <url>` opens the default browser; CREATE_NO_WINDOW keeps
        // the cmd console from flashing on the windowed GUI. URL is pre-validated.
        let mut cmd = std::process::Command::new("cmd");
        cmd.args(["/C", "start", "", u]);
        tiddl::no_window(&mut cmd);
        cmd.spawn().map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(u).spawn().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn quit(app: AppHandle, state: State<QueueState>) {
    queue::cancel_all(&app, &state); // mark queued cancelled + emit
    state.kill_all_children_blocking(); // synchronously ensure tiddl/ffmpeg die
    app.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(QueueState::default())
        .invoke_handler(tauri::generate_handler![
            check_tiddl,
            check_ffmpeg,
            get_auth_status,
            auth_login,
            auth_logout,
            auth_refresh,
            search,
            enqueue,
            download_favorites,
            cached_favorites,
            refresh_favorites,
            artist_albums,
            album_tracks,
            album_meta,
            save_tiddl_config,
            cancel_download,
            cancel_all,
            get_queue,
            clear_finished,
            get_config,
            save_config,
            open_path,
            open_url,
            quit,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let app = window.app_handle();
                let state = app.state::<QueueState>();
                if state.has_running() {
                    api.prevent_close();
                    let _ = app.emit("close-requested", serde_json::json!({}));
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tiddl-gui")
        .run(|handle, event| {
            if let tauri::RunEvent::Exit = event {
                // Never orphan tiddl/ffmpeg on quit — block until groups die.
                handle.state::<QueueState>().kill_all_children_blocking();
            }
        });
}
