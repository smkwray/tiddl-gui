//! GUI-local settings (settings.json in the OS config dir) plus format-
//! preserving read/write of tiddl's own ~/.tiddl/config.toml via toml_edit.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use toml_edit::{value, DocumentMut, Item, Table};

#[derive(Serialize, Deserialize, Clone)]
pub struct Settings {
    #[serde(default = "default_quality")]
    pub quality: String,
    #[serde(default)]
    pub path_override: String,
    #[serde(default = "default_threads")]
    pub threads: u32,
    #[serde(default)]
    pub no_skip: bool,
    #[serde(default)]
    pub write_to_tiddl: bool,
    #[serde(default)]
    pub extra_path_dir: String,
}

fn default_quality() -> String {
    "high".to_string()
}
fn default_threads() -> u32 {
    4
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            quality: default_quality(),
            path_override: String::new(),
            threads: default_threads(),
            no_skip: false,
            write_to_tiddl: false,
            extra_path_dir: String::new(),
        }
    }
}

#[derive(Serialize, Clone)]
pub struct ConfigDto {
    pub quality: String,
    pub path_override: String,
    pub threads: u32,
    pub no_skip: bool,
    pub write_to_tiddl: bool,
    pub extra_path_dir: String,
    pub effective_path: String,
    // Raw values from ~/.tiddl/config.toml (for the in-app config editor).
    pub tiddl_download_path: String,
    pub tiddl_quality: String,
    pub tiddl_threads: Option<u32>,
    pub tiddl_skip_existing: Option<bool>,
    pub template_track: String,
    pub template_album: String,
    pub tiddl_config_path: String,
    pub tiddl_config_exists: bool,
}

fn settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("tiddl-gui").join("settings.json"))
}

pub fn load_settings() -> Settings {
    if let Some(p) = settings_path() {
        if let Ok(s) = std::fs::read_to_string(&p) {
            if let Ok(cfg) = serde_json::from_str::<Settings>(&s) {
                return cfg;
            }
        }
    }
    Settings::default()
}

pub fn save_settings(s: &Settings) -> Result<(), String> {
    let p = settings_path().ok_or("could not resolve config dir")?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create config dir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    std::fs::write(&p, json).map_err(|e| format!("write settings: {e}"))
}

fn tiddl_config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".tiddl/config.toml"))
}

struct TiddlConfig {
    download_path: String,
    track_quality: String,
    threads: Option<u32>,
    skip_existing: Option<bool>,
    template_track: String,
    template_album: String,
    exists: bool,
    path: String,
}

fn read_tiddl_config() -> TiddlConfig {
    let path = tiddl_config_path();
    let path_str = path
        .as_ref()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut cfg = TiddlConfig {
        download_path: String::new(),
        track_quality: String::new(),
        threads: None,
        skip_existing: None,
        template_track: String::new(),
        template_album: String::new(),
        exists: false,
        path: path_str,
    };
    if let Some(p) = path {
        if let Ok(text) = std::fs::read_to_string(&p) {
            cfg.exists = true;
            if let Ok(doc) = text.parse::<DocumentMut>() {
                if let Some(dl) = doc.get("download").and_then(Item::as_table_like) {
                    cfg.download_path = dl.get("download_path").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    cfg.track_quality = dl.get("track_quality").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    cfg.threads = dl.get("threads_count").and_then(|v| v.as_integer()).map(|n| n as u32);
                    cfg.skip_existing = dl.get("skip_existing").and_then(|v| v.as_bool());
                }
                if let Some(t) = doc.get("templates").and_then(Item::as_table_like) {
                    cfg.template_track = t.get("track").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    cfg.template_album = t.get("album").and_then(|v| v.as_str()).unwrap_or("").to_string();
                }
            }
        }
    }
    cfg
}

/// Write quality (and path if non-empty) into ~/.tiddl/config.toml, preserving
/// comments and the [templates] section.
fn write_tiddl_config(quality: &str, path_override: &str) -> Result<(), String> {
    let p = tiddl_config_path().ok_or("could not resolve ~/.tiddl/config.toml")?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create ~/.tiddl: {e}"))?;
    }
    let mut doc = match std::fs::read_to_string(&p) {
        Ok(text) => text.parse::<DocumentMut>().map_err(|e| format!("parse config.toml: {e}"))?,
        Err(_) => DocumentMut::new(),
    };
    if doc.get("download").and_then(Item::as_table_like).is_none() {
        doc["download"] = Item::Table(Table::new());
    }
    let dl = doc["download"]
        .as_table_mut()
        .ok_or("config.toml [download] is not a table")?;
    dl["track_quality"] = value(quality);
    if !path_override.is_empty() {
        dl["download_path"] = value(path_override);
    }
    std::fs::write(&p, doc.to_string()).map_err(|e| format!("write config.toml: {e}"))
}

pub fn build_dto() -> ConfigDto {
    let s = load_settings();
    let tc = read_tiddl_config();
    let effective_path = if !s.path_override.is_empty() {
        s.path_override.clone()
    } else if !tc.download_path.is_empty() {
        tc.download_path.clone()
    } else {
        dirs::home_dir()
            .map(|h| h.join("Music/tiddl").to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    ConfigDto {
        quality: s.quality,
        path_override: s.path_override,
        threads: s.threads,
        no_skip: s.no_skip,
        write_to_tiddl: s.write_to_tiddl,
        extra_path_dir: s.extra_path_dir,
        effective_path,
        tiddl_download_path: tc.download_path,
        tiddl_quality: tc.track_quality,
        tiddl_threads: tc.threads,
        tiddl_skip_existing: tc.skip_existing,
        template_track: tc.template_track,
        template_album: tc.template_album,
        tiddl_config_path: tc.path,
        tiddl_config_exists: tc.exists,
    }
}

/// Save GUI settings; optionally mirror into config.toml. Fail-loud on partial:
/// settings.json is written first and NOT rolled back if the toml write fails.
pub fn save(
    quality: String,
    path_override: String,
    threads: u32,
    no_skip: bool,
    write_to_tiddl: bool,
    extra_path_dir: String,
) -> Result<(), String> {
    let mut s = load_settings();
    s.quality = if crate::tiddl::ALLOWED_QUALITIES.contains(&quality.as_str()) {
        quality
    } else {
        "high".to_string()
    };
    s.path_override = path_override;
    s.threads = threads.clamp(1, 16);
    s.no_skip = no_skip;
    s.write_to_tiddl = write_to_tiddl;
    s.extra_path_dir = extra_path_dir;
    save_settings(&s)?;

    if write_to_tiddl {
        if let Err(e) = write_tiddl_config(&s.quality, &s.path_override) {
            return Err(format!(
                "Saved GUI settings, but failed to write ~/.tiddl/config.toml: {e}"
            ));
        }
    }
    Ok(())
}

/// Edit tiddl's own config.toml from the GUI. Format-preserving (toml_edit keeps
/// comments and untouched sections like [metadata]/[cover]).
#[allow(clippy::too_many_arguments)]
pub fn save_tiddl_config(
    download_path: String,
    track_quality: String,
    threads: u32,
    skip_existing: bool,
    template_track: String,
    template_album: String,
) -> Result<(), String> {
    let p = tiddl_config_path().ok_or("could not resolve ~/.tiddl/config.toml")?;
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create ~/.tiddl: {e}"))?;
    }
    let mut doc = match std::fs::read_to_string(&p) {
        Ok(text) => text.parse::<DocumentMut>().map_err(|e| format!("parse config.toml: {e}"))?,
        Err(_) => DocumentMut::new(),
    };
    let q = if crate::tiddl::ALLOWED_QUALITIES.contains(&track_quality.as_str()) {
        track_quality
    } else {
        return Err(format!("invalid quality '{track_quality}'"));
    };

    if doc.get("download").and_then(Item::as_table_like).is_none() {
        doc["download"] = Item::Table(Table::new());
    }
    // Only rewrite a key when its value actually changes, so unchanged keys keep
    // their inline comments (toml_edit drops a key's trailing comment on replace).
    {
        let dl = doc["download"].as_table_mut().ok_or("config.toml [download] is not a table")?;
        let dp = download_path.trim();
        if !dp.is_empty() && dl.get("download_path").and_then(|v| v.as_str()) != Some(dp) {
            dl["download_path"] = value(dp);
        }
        if dl.get("track_quality").and_then(|v| v.as_str()) != Some(q.as_str()) {
            dl["track_quality"] = value(q.as_str());
        }
        let th = threads.clamp(1, 16) as i64;
        if dl.get("threads_count").and_then(|v| v.as_integer()) != Some(th) {
            dl["threads_count"] = value(th);
        }
        if dl.get("skip_existing").and_then(|v| v.as_bool()) != Some(skip_existing) {
            dl["skip_existing"] = value(skip_existing);
        }
    }

    if doc.get("templates").and_then(Item::as_table_like).is_none() {
        doc["templates"] = Item::Table(Table::new());
    }
    {
        let t = doc["templates"].as_table_mut().ok_or("config.toml [templates] is not a table")?;
        let tt = template_track.trim();
        if t.get("track").and_then(|v| v.as_str()) != Some(tt) {
            t["track"] = value(tt);
        }
        let ta = template_album.trim();
        if t.get("album").and_then(|v| v.as_str()) != Some(ta) {
            t["album"] = value(ta);
        }
    }

    std::fs::write(&p, doc.to_string()).map_err(|e| format!("write config.toml: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    // Idempotent round-trip on the REAL ~/.tiddl/config.toml (back it up first).
    // cargo test --lib save_config_roundtrip -- --ignored --nocapture
    #[test]
    #[ignore]
    fn save_config_roundtrip() {
        let b = read_tiddl_config();
        let q = if b.track_quality.is_empty() { "high".to_string() } else { b.track_quality.clone() };
        save_tiddl_config(
            b.download_path.clone(),
            q,
            b.threads.unwrap_or(4),
            b.skip_existing.unwrap_or(true),
            b.template_track.clone(),
            b.template_album.clone(),
        )
        .expect("save_tiddl_config");
        let a = read_tiddl_config();
        assert_eq!(b.template_track, a.template_track, "track template lost");
        assert_eq!(b.template_album, a.template_album, "album template lost");
        assert_eq!(b.download_path, a.download_path, "download_path lost");
        eprintln!("roundtrip ok: track='{}' album='{}'", a.template_track, a.template_album);
    }
}
