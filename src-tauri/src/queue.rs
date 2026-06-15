//! Backend-owned download queue: a single serial worker runs one tiddl process
//! at a time, streams id-scoped events, and supports orphan-free cancellation
//! (each job is its own process group; cancel kills the whole group incl. ffmpeg).

use serde::Serialize;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter, Manager};

use crate::tiddl::{
    kill_group, kill_group_sync, resolve_tiddl, spawn_line_reader, strip_ansi, tiddl_command,
    SpawnInGroup, ALLOWED_QUALITIES, FAV_TYPES,
};

#[derive(Serialize, Clone)]
pub struct QueueItem {
    pub id: u64,
    pub resource: String,
    pub label: String,
    pub status: String, // queued | running | done | error | cancelled
    pub quality: String,
    pub tracks_done: u32,
    pub total: Option<u32>,
    pub failures: u32,
    pub exit_code: Option<i32>,
    pub error: Option<String>,
}

struct ChildKill {
    pid: u32,
    killed: Arc<AtomicBool>,
}

struct Job {
    id: u64,
    args: Vec<String>,
}

pub struct QueueState {
    next_id: AtomicU64,
    items: Mutex<Vec<QueueItem>>,
    children: Mutex<HashMap<u64, ChildKill>>,
    /// Cancels that arrived after an item went "running" but before its child
    /// was registered — drained by the worker right after spawn.
    pending_cancels: Mutex<HashSet<u64>>,
    tx: Mutex<Option<Sender<Job>>>,
}

impl Default for QueueState {
    fn default() -> Self {
        QueueState {
            next_id: AtomicU64::new(1), // 0 is reserved for the auth-login child
            items: Mutex::new(Vec::new()),
            children: Mutex::new(HashMap::new()),
            pending_cancels: Mutex::new(HashSet::new()),
            tx: Mutex::new(None),
        }
    }
}

impl QueueState {
    pub fn register_child(&self, id: u64, pid: u32, killed: Arc<AtomicBool>) {
        self.children.lock().unwrap().insert(id, ChildKill { pid, killed });
    }
    pub fn unregister_child(&self, id: u64) {
        self.children.lock().unwrap().remove(&id);
    }

    pub fn snapshot(&self) -> Vec<QueueItem> {
        self.items.lock().unwrap().clone()
    }

    pub fn has_running(&self) -> bool {
        self.items.lock().unwrap().iter().any(|i| i.status == "running")
    }

    fn status_of(&self, id: u64) -> Option<String> {
        self.items.lock().unwrap().iter().find(|i| i.id == id).map(|i| i.status.clone())
    }

    /// Mutate an item by id; returns the updated clone for event emission.
    fn update<F: FnOnce(&mut QueueItem)>(&self, id: u64, f: F) -> Option<QueueItem> {
        let mut items = self.items.lock().unwrap();
        if let Some(it) = items.iter_mut().find(|i| i.id == id) {
            f(it);
            Some(it.clone())
        } else {
            None
        }
    }

    /// Atomically refuse-if-cancelled / mark-running in one lock acquisition so a
    /// cancel can't slip between the check and the transition.
    fn begin_running(&self, id: u64) -> Option<QueueItem> {
        let mut items = self.items.lock().unwrap();
        let it = items.iter_mut().find(|i| i.id == id)?;
        if it.status == "cancelled" {
            return None;
        }
        it.status = "running".into();
        Some(it.clone())
    }

    fn add_pending_cancel(&self, id: u64) {
        self.pending_cancels.lock().unwrap().insert(id);
    }
    fn take_pending_cancel(&self, id: u64) -> bool {
        self.pending_cancels.lock().unwrap().remove(&id)
    }

    /// Snapshot pids under the lock, then signal outside it.
    fn child_snapshot(&self) -> Vec<(u32, Arc<AtomicBool>)> {
        self.children
            .lock()
            .unwrap()
            .values()
            .map(|ck| (ck.pid, ck.killed.clone()))
            .collect()
    }

    pub fn kill_all_children(&self) {
        for (pid, killed) in self.child_snapshot() {
            killed.store(true, Ordering::SeqCst);
            kill_group(pid);
        }
    }

    /// Shutdown variant: block until each group is dead.
    pub fn kill_all_children_blocking(&self) {
        for (pid, killed) in self.child_snapshot() {
            killed.store(true, Ordering::SeqCst);
            kill_group_sync(pid);
        }
    }
}

fn emit_item(app: &AppHandle, item: &QueueItem) {
    let _ = app.emit("queue-item", json!({ "item": item }));
}

// ---------------------------------------------------------------------------
// Validation + arg building
// ---------------------------------------------------------------------------

fn valid_quality(q: &str) -> String {
    if ALLOWED_QUALITIES.contains(&q) {
        q.to_string()
    } else {
        "high".to_string()
    }
}

/// Reject empties, leading-dash tokens (tiddl `url` has no `--` separator so a
/// leading-dash token would be parsed as a flag), and absurd lengths.
fn validate_resource(raw: &str) -> Result<String, String> {
    let r = raw.trim();
    if r.is_empty() {
        return Err("empty resource".into());
    }
    if r.starts_with('-') {
        return Err(format!("refusing resource that looks like a flag: {r}"));
    }
    if r.len() > 1024 {
        return Err("resource too long".into());
    }
    Ok(r.to_string())
}

#[allow(clippy::too_many_arguments)]
fn download_args(
    quality: &str,
    path_override: &str,
    threads: u32,
    no_skip: bool,
    output: &str,
    singles: bool,
    resource: &str,
) -> Vec<String> {
    let mut a: Vec<String> = vec!["download".into(), "-q".into(), valid_quality(quality)];
    if !path_override.is_empty() {
        a.push("-p".into());
        a.push(path_override.into());
    }
    // Optional output-template override (e.g. route album-picked tracks to Albums/).
    if !output.is_empty() {
        a.push("-o".into());
        a.push(output.into());
    }
    // Include artist EPs/singles (for "Download artist", to match the expand view).
    if singles {
        a.push("-s".into());
        a.push("include".into());
    }
    if no_skip {
        a.push("-ns".into());
    }
    a.push("-t".into());
    a.push(threads.clamp(1, 16).to_string());
    a.push("url".into());
    a.push(resource.into());
    a
}

fn fav_args(quality: &str, path_override: &str, threads: u32, types: &[String]) -> Vec<String> {
    let mut a: Vec<String> = vec!["download".into(), "-q".into(), valid_quality(quality)];
    if !path_override.is_empty() {
        a.push("-p".into());
        a.push(path_override.into());
    }
    a.push("-t".into());
    a.push(threads.clamp(1, 16).to_string());
    a.push("fav".into());
    for t in types {
        a.push("-t".into());
        a.push(t.clone());
    }
    a
}

// ---------------------------------------------------------------------------
// Public queue ops (called from tauri commands)
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn enqueue(
    app: &AppHandle,
    state: &QueueState,
    resources: Vec<String>,
    quality: String,
    path_override: String,
    threads: u32,
    no_skip: bool,
    output: String,
    singles: bool,
) -> Result<Vec<u64>, String> {
    if resources.is_empty() {
        return Err("no resources to download".into());
    }
    let mut validated = Vec::new();
    for r in &resources {
        validated.push(validate_resource(r)?); // fail loud on the first bad token
    }
    let mut ids = Vec::new();
    for r in validated {
        let args = download_args(&quality, &path_override, threads, no_skip, &output, singles, &r);
        let id = submit(app, state, r.clone(), r, valid_quality(&quality), args);
        ids.push(id);
    }
    Ok(ids)
}

pub fn enqueue_favorites(
    app: &AppHandle,
    state: &QueueState,
    types: Vec<String>,
    quality: String,
    path_override: String,
    threads: u32,
    confirm: bool,
) -> Result<u64, String> {
    if !confirm {
        return Err("Refusing: this downloads EVERY favorite of the selected types. Pass confirm=true.".into());
    }
    let mut clean: Vec<String> = Vec::new();
    for t in types {
        let t = t.to_lowercase();
        if FAV_TYPES.contains(&t.as_str()) && !clean.contains(&t) {
            clean.push(t);
        }
    }
    if clean.is_empty() {
        return Err("Select at least one favorite type".into());
    }
    let label = format!("favorites: {}", clean.join(", "));
    let args = fav_args(&quality, &path_override, threads, &clean);
    Ok(submit(app, state, label.clone(), label, valid_quality(&quality), args))
}

fn submit(
    app: &AppHandle,
    state: &QueueState,
    resource: String,
    label: String,
    quality: String,
    args: Vec<String>,
) -> u64 {
    let id = state.next_id.fetch_add(1, Ordering::SeqCst);
    let item = QueueItem {
        id,
        resource,
        label,
        status: "queued".into(),
        quality,
        tracks_done: 0,
        total: None,
        failures: 0,
        exit_code: None,
        error: None,
    };
    state.items.lock().unwrap().push(item.clone());
    emit_item(app, &item);
    ensure_worker(app, state);
    if let Some(tx) = state.tx.lock().unwrap().as_ref() {
        if tx.send(Job { id, args }).is_err() {
            if let Some(it) = state.update(id, |i| {
                i.status = "error".into();
                i.error = Some("worker unavailable".into());
            }) {
                emit_item(app, &it);
            }
        }
    }
    id
}

fn ensure_worker(app: &AppHandle, state: &QueueState) {
    let mut guard = state.tx.lock().unwrap();
    if guard.is_some() {
        return;
    }
    let (tx, rx) = channel::<Job>();
    *guard = Some(tx);
    let handle = app.clone();
    thread::spawn(move || {
        while let Ok(job) = rx.recv() {
            run_job(&handle, job);
        }
    });
}

pub fn cancel_download(app: &AppHandle, state: &QueueState, id: u64) {
    // Running child registered? kill its group; the worker finalizes it.
    let running = {
        let children = state.children.lock().unwrap();
        children.get(&id).map(|ck| {
            ck.killed.store(true, Ordering::SeqCst);
            ck.pid
        })
    };
    if let Some(pid) = running {
        kill_group(pid);
        return;
    }
    match state.status_of(id).as_deref() {
        Some("queued") => {
            // Mark cancelled so the worker's begin_running() skips it.
            if let Some(it) = state.update(id, |i| i.status = "cancelled".into()) {
                emit_item(app, &it);
                let _ = app.emit(
                    "download-finished",
                    json!({ "id": id, "status": "cancelled", "exit_code": null, "total": it.total, "failures": 0, "error": null }),
                );
            }
        }
        // Running but the child isn't registered yet (tiny spawn window): leave a
        // pending cancel the worker drains right after spawn.
        Some("running") => state.add_pending_cancel(id),
        _ => {} // done / error / cancelled / unknown — nothing to do
    }
}

pub fn cancel_all(app: &AppHandle, state: &QueueState) {
    state.kill_all_children();
    let queued_ids: Vec<u64> = state
        .items
        .lock()
        .unwrap()
        .iter()
        .filter(|i| i.status == "queued")
        .map(|i| i.id)
        .collect();
    for id in queued_ids {
        if let Some(it) = state.update(id, |i| i.status = "cancelled".into()) {
            emit_item(app, &it);
            let _ = app.emit(
                "download-finished",
                json!({ "id": id, "status": "cancelled", "exit_code": null, "total": null, "failures": 0, "error": null }),
            );
        }
    }
}

pub fn clear_finished(state: &QueueState) {
    state
        .items
        .lock()
        .unwrap()
        .retain(|i| i.status == "queued" || i.status == "running");
}

// ---------------------------------------------------------------------------
// Worker
// ---------------------------------------------------------------------------

// Match tiddl's actual failure messages, not bare substrings — a track title or
// path containing "error"/"401" must not be counted as a failure.
fn is_failure_line(l: &str) -> bool {
    l.starts_with("Can't stream")
        || l.starts_with("Error")
        || l.starts_with("Traceback")
        || l.contains("Unauthorized")
}

/// Title from a `Downloaded <title> <quality...> <path>` line. tiddl uses a
/// single space before the quality for AAC and a double space for FLAC, so cut
/// at the earliest quality marker rather than relying on the double-space.
fn extract_title(rest: &str) -> String {
    const MARKERS: [&str; 6] = [" 16-bit", " 24-bit", " 96 kbps", " 320 kbps", " Dolby", "  "];
    let cut = MARKERS.iter().filter_map(|m| rest.find(m)).min().unwrap_or(rest.len());
    rest[..cut].trim().to_string()
}

fn run_job(app: &AppHandle, job: Job) {
    let state = app.state::<QueueState>();
    let id = job.id;

    // Atomic cancelled-check + mark-running so a cancel can't slip between
    // them or resurrect a cancelled item.
    let started = match state.begin_running(id) {
        Some(it) => it,
        None => return, // cancelled while queued
    };
    emit_item(app, &started);

    let tiddl = match resolve_tiddl() {
        Some(t) => t,
        None => return finish(app, &state, id, "error", None, 0, Some("tiddl not found — was it uninstalled?".into())),
    };

    let mut child = match tiddl_command(&tiddl)
        .args(&job.args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn_in_group()
    {
        Ok(c) => c,
        Err(e) => return finish(app, &state, id, "error", None, 0, Some(format!("failed to start tiddl: {e}"))),
    };

    let pid = child.id();
    let killed = Arc::new(AtomicBool::new(false));
    state.register_child(id, pid, killed.clone());
    // Drain a cancel that raced the spawn window.
    if state.take_pending_cancel(id) {
        killed.store(true, Ordering::SeqCst);
        kill_group(pid);
    }

    let (tx, rx) = channel::<String>();
    let mut readers = Vec::new();
    if let Some(o) = child.stdout.take() {
        readers.push(spawn_line_reader(o, tx.clone()));
    }
    if let Some(e) = child.stderr.take() {
        readers.push(spawn_line_reader(e, tx.clone()));
    }
    drop(tx);

    let mut tracks_done: u32 = 0;
    let mut total: Option<u32> = None;
    let mut failures: u32 = 0;
    let mut last_err_line: Option<String> = None;

    for raw in rx {
        let line = strip_ansi(&raw);
        let t = line.trim();
        if t.is_empty() || t.contains("Auth token expires in") {
            continue;
        }
        let _ = app.emit("download-log", json!({ "id": id, "line": line }));

        if let Some(rest) = t.strip_prefix("Downloaded ") {
            tracks_done += 1;
            let title = extract_title(rest);
            state.update(id, |i| {
                i.tracks_done = tracks_done;
                if !title.is_empty() {
                    i.label = title.clone();
                }
            });
            let _ = app.emit("download-progress", json!({ "id": id, "tracks_done": tracks_done, "title": title }));
        } else if let Some(rest) = t.strip_prefix("Total downloads:") {
            if let Ok(n) = rest.trim().parse::<u32>() {
                total = Some(n);
            }
        } else if is_failure_line(t) {
            failures += 1;
            last_err_line = Some(t.to_string());
        }
    }

    for r in readers {
        let _ = r.join();
    }
    let status = child.wait();
    state.unregister_child(id);

    let code = status.as_ref().ok().and_then(|s| s.code());
    let was_killed = killed.load(Ordering::SeqCst);

    let (final_status, error) = if was_killed {
        ("cancelled", None)
    } else if code != Some(0) {
        ("error", last_err_line.clone().or_else(|| Some(format!("tiddl exited with code {}", code.unwrap_or(-1)))))
    } else if failures > 0 {
        ("error", Some(format!("Completed with {failures} failure(s) — see Activity log")))
    } else {
        ("done", None)
    };

    finish_full(app, &state, id, final_status, code, total, failures, error, was_killed);
}

#[allow(clippy::too_many_arguments)]
fn finish_full(
    app: &AppHandle,
    state: &QueueState,
    id: u64,
    status: &str,
    exit_code: Option<i32>,
    total: Option<u32>,
    failures: u32,
    error: Option<String>,
    was_killed: bool,
) {
    if let Some(it) = state.update(id, |i| {
        i.status = status.to_string();
        i.exit_code = exit_code;
        i.total = total;
        i.failures = failures;
        i.error = error.clone();
    }) {
        emit_item(app, &it);
    }
    let leftover = if was_killed {
        Some("Cancelled — already-finished tracks remain on disk; only future tracks were stopped.")
    } else {
        None
    };
    let _ = app.emit(
        "download-finished",
        json!({
            "id": id, "status": status, "exit_code": exit_code,
            "total": total, "failures": failures, "error": error,
            "leftover_note": leftover,
        }),
    );
    if !state.has_running() {
        let _ = app.emit("queue-empty", json!({}));
    }
}

fn finish(
    app: &AppHandle,
    state: &QueueState,
    id: u64,
    status: &str,
    exit_code: Option<i32>,
    failures: u32,
    error: Option<String>,
) {
    finish_full(app, state, id, status, exit_code, None, failures, error, false);
}
