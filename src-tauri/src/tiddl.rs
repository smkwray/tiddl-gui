//! Thin wrapper around the user-installed `tiddl` CLI.
//! This crate ships NO tiddl code — it only locates and runs the binary the
//! user installed, streams its output, and parses it. All subprocessing is
//! Rust-side; user input is passed as separate args (never a shell string).

use serde::Serialize;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::config;
use crate::queue::QueueState;

pub const ALLOWED_QUALITIES: [&str; 4] = ["low", "normal", "high", "max"];
pub const FAV_TYPES: [&str; 5] = ["track", "album", "playlist", "artist", "video"];

// ---------------------------------------------------------------------------
// Status DTOs
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
pub struct TiddlStatus {
    pub found: bool,
    pub path: String,
    pub version: String, // always "" — tiddl has no --version flag
    pub error: String,
}

#[derive(Serialize, Clone)]
pub struct FfmpegStatus {
    pub found: bool,
    pub detail: String,
}

#[derive(Serialize, Clone)]
pub struct AuthStatus {
    pub authed: bool,
    pub detail: String,
    pub user_id: Option<u64>,
    pub country_code: Option<String>,
    pub expires_at: Option<i64>,
    pub expired: bool,
}

#[derive(Serialize, Clone)]
pub struct SearchResult {
    pub num: u32,
    pub kind: String,
    pub title: String,
    pub id: String,
    pub resource: String,
}

#[derive(Serialize, Clone)]
struct AuthFlow {
    phase: String,
    text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<i32>,
}

// ---------------------------------------------------------------------------
// Process helpers (PATH, spawn, ANSI, line reading)
// ---------------------------------------------------------------------------

/// GUI apps launched from a desktop launcher (Start menu, Finder, Raycast, etc.)
/// inherit a minimal PATH. Build an augmented PATH so `tiddl` AND the `ffmpeg` it
/// shells out to resolve. Portable via `join_paths` (never hardcode the separator).
pub fn augmented_path() -> OsString {
    let mut dirs_list: Vec<PathBuf> = Vec::new();

    // User-configured extra dir wins (prepended).
    let extra = config::load_settings().extra_path_dir;
    if !extra.is_empty() {
        dirs_list.push(PathBuf::from(shellexpand_tilde(&extra)));
    }
    // Explicitly wire in ffmpeg's directory so the tiddl child always finds it,
    // even under a minimal GUI PATH.
    if let Some(fdir) = ffmpeg_dir() {
        dirs_list.push(PathBuf::from(fdir));
    }

    if let Some(home) = dirs::home_dir() {
        dirs_list.push(home.join(".local/bin")); // pipx / uv tool default bin
        dirs_list.push(home.join("bin"));
    }

    #[cfg(target_os = "macos")]
    {
        dirs_list.push(PathBuf::from("/opt/homebrew/bin"));
        dirs_list.push(PathBuf::from("/usr/local/bin"));
        dirs_list.push(PathBuf::from("/opt/local/bin"));
    }
    #[cfg(target_os = "linux")]
    {
        dirs_list.push(PathBuf::from("/usr/local/bin"));
        dirs_list.push(PathBuf::from("/usr/bin"));
        dirs_list.push(PathBuf::from("/bin"));
    }
    #[cfg(windows)]
    {
        for d in windows_bin_dirs() {
            dirs_list.push(d);
        }
    }
    #[cfg(unix)]
    {
        dirs_list.push(PathBuf::from("/usr/bin"));
        dirs_list.push(PathBuf::from("/bin"));
    }

    if let Some(existing) = std::env::var_os("PATH") {
        for p in std::env::split_paths(&existing) {
            dirs_list.push(p);
        }
    }

    std::env::join_paths(dirs_list).unwrap_or_else(|_| std::env::var_os("PATH").unwrap_or_default())
}

/// Directory containing the ffmpeg binary, by checking common install
/// locations (pure file checks — no process spawn, so augmented_path() can call
/// it without recursion).
pub fn ffmpeg_dir() -> Option<String> {
    let exe = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let mut cands: Vec<PathBuf> = Vec::new();
    // Honor a user-set extra dir first (mirrors augmented_path), so a custom
    // ffmpeg location is both used and reported by check_ffmpeg.
    let extra = config::load_settings().extra_path_dir;
    if !extra.is_empty() {
        cands.push(PathBuf::from(shellexpand_tilde(&extra)).join(exe));
    }
    if let Some(home) = dirs::home_dir() {
        cands.push(home.join(".local/bin").join(exe));
    }
    #[cfg(target_os = "macos")]
    {
        cands.push(PathBuf::from("/opt/homebrew/bin/ffmpeg"));
        cands.push(PathBuf::from("/usr/local/bin/ffmpeg"));
        cands.push(PathBuf::from("/opt/local/bin/ffmpeg"));
    }
    #[cfg(target_os = "linux")]
    {
        cands.push(PathBuf::from("/usr/local/bin/ffmpeg"));
        cands.push(PathBuf::from("/usr/bin/ffmpeg"));
        cands.push(PathBuf::from("/bin/ffmpeg"));
    }
    #[cfg(windows)]
    {
        for d in windows_bin_dirs() {
            cands.push(d.join(exe));
        }
    }
    for c in &cands {
        if c.exists() {
            return c.parent().map(|p| p.to_string_lossy().into_owned());
        }
    }
    // Last resort: scan the inherited PATH so an ffmpeg installed anywhere on PATH
    // (choco/scoop/winget/apt/manual) is found and REPORTED — otherwise the status
    // pill shows a false "not found" even though the tiddl child (which inherits
    // augmented_path) runs ffmpeg fine. Reads env PATH directly, so no recursion.
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            if !dir.as_os_str().is_empty() && dir.join(exe).exists() {
                return Some(dir.to_string_lossy().into_owned());
            }
        }
    }
    None
}

fn shellexpand_tilde(p: &str) -> String {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest).to_string_lossy().into_owned();
        }
    }
    p.to_string()
}

/// Windows: directories where a CLI tool's launcher (tiddl.exe / ffmpeg.exe)
/// commonly lands across the popular install routes. Used both to augment PATH
/// (so the tiddl child finds ffmpeg) and to resolve binaries to absolute paths.
#[cfg(windows)]
fn windows_bin_dirs() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();

    // Python console-scripts: `pip install [--user] tiddl` drops tiddl.exe into a
    // <PythonRoot>\Scripts dir. Enumerate the standard Python roots and pull each
    // PythonNN\Scripts (covers system, per-user, and pip --user installs).
    let mut py_roots: Vec<PathBuf> = Vec::new();
    if let Ok(appdata) = std::env::var("APPDATA") {
        py_roots.push(PathBuf::from(appdata).join("Python")); // pip --user
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        py_roots.push(PathBuf::from(local).join("Programs").join("Python")); // python.org per-user
    }
    py_roots.push(PathBuf::from("C:\\")); // python.org system-wide -> C:\Python3XX
    for root in &py_roots {
        if let Ok(entries) = std::fs::read_dir(root) {
            for e in entries.flatten() {
                if e.file_name().to_string_lossy().starts_with("Python") {
                    out.push(e.path().join("Scripts"));
                }
            }
        }
    }

    // scoop shims, chocolatey bin, winget links, misc per-user programs.
    if let Some(home) = dirs::home_dir() {
        out.push(home.join("scoop").join("shims"));
    }
    if let Ok(pd) = std::env::var("ProgramData") {
        out.push(PathBuf::from(pd).join("chocolatey").join("bin"));
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        out.push(PathBuf::from(&local).join("Microsoft").join("WinGet").join("Links"));
        out.push(PathBuf::from(local).join("Programs"));
    }
    out
}

/// Suppress the console window a child console process would otherwise flash
/// under a windowed GUI build (`windows_subsystem = "windows"`). No-op off Windows.
pub fn no_window(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

/// Locate the tiddl binary (explicit install dirs first — robust for bundled
/// GUI apps — then PATH).
pub fn resolve_tiddl() -> Option<String> {
    let exe = if cfg!(windows) { "tiddl.exe" } else { "tiddl" };
    let mut candidates: Vec<PathBuf> = Vec::new();

    // 1. User-configured extra dir wins, so a tiddl in a venv / unusual location
    //    resolves to an absolute path deterministically (mirrors ffmpeg_dir).
    let extra = config::load_settings().extra_path_dir;
    if !extra.is_empty() {
        candidates.push(PathBuf::from(shellexpand_tilde(&extra)).join(exe));
    }
    // 2. pipx / uv tool default bin (cross-platform).
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".local").join("bin").join(exe));
    }
    #[cfg(target_os = "macos")]
    {
        candidates.push(PathBuf::from("/opt/homebrew/bin/tiddl"));
        candidates.push(PathBuf::from("/usr/local/bin/tiddl"));
    }
    #[cfg(target_os = "linux")]
    {
        candidates.push(PathBuf::from("/usr/local/bin/tiddl"));
        candidates.push(PathBuf::from("/usr/bin/tiddl"));
    }
    #[cfg(windows)]
    {
        for d in windows_bin_dirs() {
            candidates.push(d.join(exe)); // pip[/--user] Scripts, scoop, choco, winget
        }
    }
    for c in &candidates {
        if c.exists() {
            return Some(c.to_string_lossy().into_owned());
        }
    }

    // 3. which-style scan of the augmented PATH — returns an ABSOLUTE path so the
    //    status display is meaningful instead of a bare "tiddl".
    for dir in std::env::split_paths(&augmented_path()) {
        if !dir.as_os_str().is_empty() && dir.join(exe).exists() {
            return Some(dir.join(exe).to_string_lossy().into_owned());
        }
    }

    // 4. Last-resort probe: let the OS resolve `tiddl` via PATH/PATHEXT.
    if tiddl_command(exe)
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        return Some(exe.to_string());
    }
    None
}

/// A Command for `program` with the GUI-augmented PATH and (on Windows) no
/// console flash.
pub fn tiddl_command(program: &str) -> Command {
    let mut cmd = Command::new(program);
    cmd.env("PATH", augmented_path());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Strip ANSI/CSI escape sequences while preserving UTF-8 (box-drawing chars).
pub fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            if chars.peek() == Some(&'[') {
                chars.next();
                while let Some(&n) = chars.peek() {
                    chars.next();
                    if ('@'..='~').contains(&n) {
                        break;
                    }
                }
            } else {
                chars.next();
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Read one line (without trailing CR/LF) using lossy UTF-8 — never panics on
/// non-UTF-8 bytes the way `BufRead::lines()` would.
fn read_line_lossy<R: BufRead>(r: &mut R, buf: &mut Vec<u8>) -> std::io::Result<Option<String>> {
    buf.clear();
    let n = r.read_until(b'\n', buf)?;
    if n == 0 {
        return Ok(None);
    }
    while matches!(buf.last(), Some(b'\n') | Some(b'\r')) {
        buf.pop();
    }
    let mut line = String::from_utf8_lossy(buf).into_owned();
    if line.len() > 4000 {
        line.truncate(4000);
        line.push('…');
    }
    Ok(Some(line))
}

/// Spawn a thread that reads `reader` line-by-line into `sender`.
pub fn spawn_line_reader<R: Read + Send + 'static>(
    reader: R,
    sender: Sender<String>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut br = BufReader::new(reader);
        let mut buf = Vec::new();
        loop {
            match read_line_lossy(&mut br, &mut buf) {
                Ok(Some(line)) => {
                    if sender.send(line).is_err() {
                        break;
                    }
                }
                _ => break,
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Status checks
// ---------------------------------------------------------------------------

pub fn check() -> TiddlStatus {
    match resolve_tiddl() {
        Some(path) => TiddlStatus {
            found: true,
            path,
            version: String::new(),
            error: String::new(),
        },
        None => TiddlStatus {
            found: false,
            path: String::new(),
            version: String::new(),
            error: tiddl_not_found_msg(),
        },
    }
}

fn tiddl_not_found_msg() -> String {
    #[cfg(windows)]
    {
        "tiddl not found on PATH, in a venv's Scripts dir, %APPDATA%\\Python\\Python3XX\\Scripts, scoop, chocolatey, or winget. Install it (pip install tiddl / pipx install tiddl / uv tool install tiddl) or point Settings → Extra PATH dir at the folder holding tiddl.exe.".to_string()
    }
    #[cfg(target_os = "macos")]
    {
        "tiddl not found in ~/.local/bin, Homebrew, /usr/local/bin, or PATH. Install it (uv tool install tiddl / pip install tiddl) or set Settings → Extra PATH dir.".to_string()
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        "tiddl not found in ~/.local/bin, /usr/local/bin, /usr/bin, or PATH. Install it (uv tool install tiddl / pip install tiddl) or set Settings → Extra PATH dir.".to_string()
    }
}

fn ffmpeg_not_found_msg() -> String {
    #[cfg(windows)]
    {
        "ffmpeg not found — install it (winget install Gyan.FFmpeg / choco install ffmpeg / scoop install ffmpeg) or set an extra PATH dir in Settings".to_string()
    }
    #[cfg(target_os = "macos")]
    {
        "ffmpeg not found — install it (e.g. `brew install ffmpeg`) or set an extra PATH dir in Settings".to_string()
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        "ffmpeg not found — install it with your package manager (e.g. `apt install ffmpeg`) or set an extra PATH dir in Settings".to_string()
    }
}

pub fn check_ffmpeg() -> FfmpegStatus {
    // Resolve to an ABSOLUTE binary — a GUI app's minimal PATH makes a bare
    // `ffmpeg` lookup fail even though the binary exists in a known dir.
    let exe = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    let dir = match ffmpeg_dir() {
        Some(d) => d,
        None => {
            return FfmpegStatus {
                found: false,
                detail: ffmpeg_not_found_msg(),
            }
        }
    };
    let bin = std::path::Path::new(&dir).join(exe);
    // The binary exists (ffmpeg_dir verified it); read its version for display.
    let mut vcmd = Command::new(&bin);
    vcmd.arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    no_window(&mut vcmd);
    let version = vcmd
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).lines().next().unwrap_or("ffmpeg").to_string())
        .unwrap_or_else(|| "ffmpeg".to_string());
    FfmpegStatus { found: true, detail: format!("{version}  @ {dir}") }
}

pub fn auth_status() -> AuthStatus {
    let path = match dirs::home_dir() {
        Some(h) => h.join(".tiddl/auth.json"),
        None => {
            return AuthStatus {
                authed: false,
                detail: "could not resolve home directory".into(),
                user_id: None,
                country_code: None,
                expires_at: None,
                expired: false,
            }
        }
    };
    if !path.exists() {
        return AuthStatus {
            authed: false,
            detail: "Not signed in — run sign-in to authorize".into(),
            user_id: None,
            country_code: None,
            expires_at: None,
            expired: false,
        };
    }
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => {
            return AuthStatus {
                authed: false,
                detail: "auth.json unreadable".into(),
                user_id: None,
                country_code: None,
                expires_at: None,
                expired: false,
            }
        }
    };
    let v: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => {
            return AuthStatus {
                authed: false,
                detail: "auth.json unreadable".into(),
                user_id: None,
                country_code: None,
                expires_at: None,
                expired: false,
            }
        }
    };
    let token_present = v.get("token").and_then(|t| t.as_str()).map(|s| !s.is_empty()).unwrap_or(false);
    let expires_at = v.get("expires_at").and_then(|e| e.as_i64());
    let user_id = v.get("user_id").and_then(|u| u.as_str().and_then(|s| s.parse().ok()).or_else(|| u.as_u64()));
    let country_code = v.get("country_code").and_then(|c| c.as_str()).map(|s| s.to_string());
    let now = now_unix();
    let expired = expires_at.map(|e| e <= now).unwrap_or(false);
    let authed = token_present && expires_at.map(|e| e > now + 60).unwrap_or(false);

    let detail = if !token_present {
        "Not signed in".to_string()
    } else if expired {
        "Token expired — refresh or sign in again".to_string()
    } else if let Some(e) = expires_at {
        let mins = ((e - now).max(0)) / 60;
        let cc = country_code.clone().unwrap_or_default();
        format!("Signed in{} — token valid {}h {}m", if cc.is_empty() { String::new() } else { format!(" ({cc})") }, mins / 60, mins % 60)
    } else {
        "Signed in".to_string()
    };

    AuthStatus { authed, detail, user_id, country_code, expires_at, expired }
}

fn now_unix() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Auth device flow (streamed)
// ---------------------------------------------------------------------------

fn extract_tidal_url(line: &str) -> Option<String> {
    let idx = line.find("https://link.tidal.com/")?;
    let rest = &line[idx..];
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '\'' || c == '"' || c == ')')
        .unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

fn emit_auth(app: &AppHandle, phase: &str, text: &str, uri: Option<String>, code: Option<i32>) {
    let _ = app.emit(
        "auth-flow",
        AuthFlow { phase: phase.to_string(), text: text.to_string(), uri, code },
    );
}

/// Run `tiddl auth login -n`, streaming device-flow milestones as `auth-flow`
/// events. Registered under reserved queue id 0 so the UI can cancel it.
pub fn auth_login(app: &AppHandle, state: &QueueState) -> Result<(), String> {
    let tiddl = resolve_tiddl().ok_or("tiddl not found — was it uninstalled?")?;
    let mut child = tiddl_command(&tiddl)
        .args(["auth", "login", "-n"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn_in_group()
        .map_err(|e| format!("Failed to start tiddl auth login: {e}"))?;

    let pid = child.id();
    let killed = Arc::new(AtomicBool::new(false));
    state.register_child(0, pid, killed.clone());

    let (tx, rx) = std::sync::mpsc::channel::<String>();
    let mut readers = Vec::new();
    if let Some(o) = child.stdout.take() {
        readers.push(spawn_line_reader(o, tx.clone()));
    }
    if let Some(e) = child.stderr.take() {
        readers.push(spawn_line_reader(e, tx.clone()));
    }
    drop(tx);

    for raw in rx {
        let line = strip_ansi(&raw);
        let t = line.trim();
        if t.is_empty() || t.contains("Auth token expires in") {
            continue;
        }
        if t.contains("Already logged in") {
            emit_auth(app, "already", "Already signed in.", None, None);
        } else if let Some(uri) = extract_tidal_url(t) {
            emit_auth(app, "verify", "Open this link to authorize:", Some(uri), None);
        } else if t.contains("Authenticating") || t.contains("time left") {
            emit_auth(app, "polling", t, None, None);
        } else if t.contains("Logged in") {
            emit_auth(app, "success", "Signed in!", None, None);
        } else if t.to_lowercase().contains("expired") {
            emit_auth(app, "expired", "Authentication time expired.", None, None);
        } else {
            emit_auth(app, "info", t, None, None);
        }
    }
    for r in readers {
        let _ = r.join();
    }
    let status = child.wait().map_err(|e| format!("auth login wait failed: {e}"))?;
    state.unregister_child(0);
    let code = status.code().unwrap_or(-1);
    emit_auth(app, "finished", "", None, Some(code));
    if killed.load(Ordering::SeqCst) {
        return Err("cancelled".into());
    }
    Ok(())
}

pub fn auth_logout() -> Result<String, String> {
    run_simple(&["auth", "logout"])
}

pub fn auth_refresh() -> Result<String, String> {
    run_simple(&["auth", "refresh"])
}

fn run_simple(args: &[&str]) -> Result<String, String> {
    let tiddl = resolve_tiddl().ok_or("tiddl not found")?;
    let out = tiddl_command(&tiddl)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Failed to start tiddl: {e}"))?;
    let mut combined = String::new();
    for l in String::from_utf8_lossy(&out.stdout)
        .lines()
        .chain(String::from_utf8_lossy(&out.stderr).lines())
    {
        let l = strip_ansi(l);
        if l.trim().is_empty() || l.contains("Auth token expires in") {
            continue;
        }
        combined.push_str(l.trim());
        combined.push('\n');
    }
    if out.status.success() {
        Ok(combined.trim().to_string())
    } else {
        Err(if combined.is_empty() {
            format!("tiddl exited with code {}", out.status.code().unwrap_or(-1))
        } else {
            combined.trim().to_string()
        })
    }
}

// ---------------------------------------------------------------------------
// Process-group spawn + kill (portable, no libc)
// ---------------------------------------------------------------------------

/// Extension to spawn a child as its own process-group leader so we can later
/// kill the whole tree (tiddl + the ffmpeg it spawns).
pub trait SpawnInGroup {
    fn spawn_in_group(&mut self) -> std::io::Result<std::process::Child>;
}

impl SpawnInGroup for Command {
    fn spawn_in_group(&mut self) -> std::io::Result<std::process::Child> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            self.process_group(0);
        }
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW
            self.creation_flags(0x00000200 | 0x08000000);
        }
        self.spawn()
    }
}

#[cfg(unix)]
fn group_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(format!("-{pid}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Kill a process group (Unix) / tree (Windows), returning immediately. The
/// SIGKILL escalation runs from a detached thread and is skipped if the group
/// already exited, so we never signal a reused pid.
pub fn kill_group(pid: u32) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill").arg("-TERM").arg(format!("-{pid}")).status();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(2000));
            if group_alive(pid) {
                let _ = Command::new("kill").arg("-KILL").arg(format!("-{pid}")).status();
            }
        });
    }
    #[cfg(windows)]
    {
        let mut cmd = Command::new("taskkill");
        cmd.args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        no_window(&mut cmd);
        let _ = cmd.status();
    }
}

/// Synchronous group kill for the SHUTDOWN path: TERM, poll up to
/// ~1.5s for the group to die, then SIGKILL inline. Never relies on a detached
/// thread, which would die with the process on quit and orphan tiddl/ffmpeg.
pub fn kill_group_sync(pid: u32) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill").arg("-TERM").arg(format!("-{pid}")).status();
        for _ in 0..30 {
            if !group_alive(pid) {
                return;
            }
            thread::sleep(Duration::from_millis(50));
        }
        let _ = Command::new("kill").arg("-KILL").arg(format!("-{pid}")).status();
    }
    #[cfg(windows)]
    {
        let mut cmd = Command::new("taskkill");
        cmd.args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        no_window(&mut cmd);
        let _ = cmd.status();
    }
}
