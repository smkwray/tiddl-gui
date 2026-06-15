//! READ-ONLY listing of the user's own favorites.
//!
//! Makes read-only GET requests to the favorites endpoints using the token the
//! user already authorized through `tiddl` (stored in ~/.tiddl/auth.json). It
//! never writes, never touches stream/playback endpoints, and never downloads —
//! all downloading still goes through the `tiddl` CLI.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize, Deserialize, Clone)]
pub struct FavItem {
    pub kind: String, // track | album | artist | playlist | video
    pub id: String,
    pub title: String,
    pub artist: String,
    pub resource: String, // "{kind}/{id}" — ready for `tiddl download url`
    #[serde(default)]
    pub n_tracks: Option<u32>, // albums: number of tracks
    #[serde(default)]
    pub number: Option<u32>, // album items: track number
    #[serde(default)]
    pub tag: Option<String>, // release type for non-albums: "EP" / "Single"
    #[serde(default)]
    pub year: Option<u32>, // albums: release year
}

/// Year from an album object's "releaseDate" ("YYYY-MM-DD").
fn parse_year(v: &serde_json::Value) -> Option<u32> {
    v.get("releaseDate")
        .and_then(|d| d.as_str())
        .and_then(|s| s.get(0..4))
        .and_then(|y| y.parse().ok())
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct FavoritesCache {
    pub items: Vec<FavItem>,
    pub checked_at: Option<i64>,
    pub types: Vec<String>,
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn cache_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("tiddl-gui").join("favorites.json"))
}

pub fn cached() -> FavoritesCache {
    if let Some(p) = cache_path() {
        if let Ok(t) = std::fs::read_to_string(&p) {
            if let Ok(c) = serde_json::from_str::<FavoritesCache>(&t) {
                return c;
            }
        }
    }
    FavoritesCache::default()
}

fn read_auth() -> Result<(String, String, String), String> {
    let p = dirs::home_dir()
        .ok_or("no home dir")?
        .join(".tiddl/auth.json");
    let text = std::fs::read_to_string(&p).map_err(|_| "Not signed in".to_string())?;
    let v: serde_json::Value = serde_json::from_str(&text).map_err(|_| "auth.json unreadable".to_string())?;
    let token = v
        .get("token")
        .and_then(|t| t.as_str())
        .filter(|s| !s.is_empty())
        .ok_or("Not signed in")?
        .to_string();
    let uid = v
        .get("user_id")
        .map(|u| u.as_str().map(|s| s.to_string()).unwrap_or_else(|| u.to_string()))
        .filter(|s| !s.is_empty() && s != "null")
        .ok_or("missing user_id")?;
    let cc = v
        .get("country_code")
        .and_then(|c| c.as_str())
        .unwrap_or("US")
        .to_string();
    Ok((token, uid, cc))
}

fn endpoint_plural(t: &str) -> Option<&'static str> {
    match t.to_lowercase().as_str() {
        "track" | "tracks" => Some("tracks"),
        "album" | "albums" => Some("albums"),
        "artist" | "artists" => Some("artists"),
        "playlist" | "playlists" => Some("playlists"),
        "video" | "videos" => Some("videos"),
        _ => None,
    }
}

fn singular(plural: &str) -> &'static str {
    match plural {
        "tracks" => "track",
        "albums" => "album",
        "artists" => "artist",
        "playlists" => "playlist",
        "videos" => "video",
        _ => "track",
    }
}

/// One read-only GET to the streaming API with the user's token.
fn api_get(token: &str, url: &str) -> Result<serde_json::Value, String> {
    let body = match ureq::get(url)
        .set("Authorization", &format!("Bearer {token}"))
        .set("Accept", "application/json")
        .timeout(std::time::Duration::from_secs(20))
        .call()
    {
        Ok(r) => r.into_string().map_err(|e| format!("read body: {e}"))?,
        Err(ureq::Error::Status(401, _)) => {
            return Err("The service rejected the token (401) — refresh or sign in again".into())
        }
        Err(ureq::Error::Status(code, r)) => {
            let detail = r.into_string().unwrap_or_default();
            return Err(format!("API error {code}: {}", detail.chars().take(160).collect::<String>()));
        }
        Err(e) => return Err(format!("network error: {e}")),
    };
    serde_json::from_str(&body).map_err(|e| format!("parse API response: {e}"))
}

fn fetch_type(token: &str, uid: &str, cc: &str, plural: &str) -> Result<Vec<FavItem>, String> {
    let kind = singular(plural);
    let mut out = Vec::new();
    let mut offset: u32 = 0;
    loop {
        let url = format!(
            "https://api.tidal.com/v1/users/{uid}/favorites/{plural}?countryCode={cc}&limit=50&offset={offset}&order=DATE&orderDirection=DESC"
        );
        let v = api_get(token, &url)?;
        let items = v.get("items").and_then(|i| i.as_array()).cloned().unwrap_or_default();
        let total = v.get("totalNumberOfItems").and_then(|t| t.as_u64()).unwrap_or(0);
        if items.is_empty() {
            break;
        }
        for w in &items {
            let it = w.get("item").unwrap_or(w);
            let id = if plural == "playlists" {
                it.get("uuid").and_then(|u| u.as_str()).unwrap_or("").to_string()
            } else {
                it.get("id").and_then(|x| x.as_i64()).map(|n| n.to_string()).unwrap_or_default()
            };
            if id.is_empty() {
                continue;
            }
            let title = it
                .get("title")
                .and_then(|t| t.as_str())
                .or_else(|| it.get("name").and_then(|n| n.as_str()))
                .unwrap_or("")
                .to_string();
            let artist = it
                .get("artist")
                .and_then(|a| a.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("")
                .to_string();
            out.push(FavItem {
                kind: kind.to_string(),
                id: id.clone(),
                title,
                artist,
                resource: format!("{kind}/{id}"),
                n_tracks: None,
                number: None,
                tag: None,
                year: parse_year(it),
            });
        }
        offset += items.len() as u32;
        if (offset as u64) >= total || offset > 10_000 {
            break;
        }
    }
    Ok(out)
}

fn numeric_id(id: &str) -> Result<&str, String> {
    let id = id.trim();
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return Err("invalid id".into());
    }
    Ok(id)
}

fn fetch_artist_releases(token: &str, cc: &str, id: &str, filter: Option<&str>) -> Result<Vec<FavItem>, String> {
    let mut out = Vec::new();
    let mut offset: u32 = 0;
    loop {
        let mut url = format!("https://api.tidal.com/v1/artists/{id}/albums?countryCode={cc}&limit=50&offset={offset}");
        if let Some(f) = filter {
            url.push_str(&format!("&filter={f}"));
        }
        let v = api_get(token, &url)?;
        let items = v.get("items").and_then(|i| i.as_array()).cloned().unwrap_or_default();
        let total = v.get("totalNumberOfItems").and_then(|t| t.as_u64()).unwrap_or(0);
        if items.is_empty() {
            break;
        }
        for a in &items {
            let aid = match a.get("id").and_then(|x| x.as_i64()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let title = a.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string();
            let artist = a.get("artist").and_then(|x| x.get("name")).and_then(|n| n.as_str()).unwrap_or("").to_string();
            let n_tracks = a.get("numberOfTracks").and_then(|n| n.as_u64()).map(|n| n as u32);
            let tag = match a.get("type").and_then(|t| t.as_str()) {
                Some("EP") => Some("EP".to_string()),
                Some("SINGLE") => Some("Single".to_string()),
                _ => None,
            };
            out.push(FavItem {
                kind: "album".into(),
                id: aid.clone(),
                title,
                artist,
                resource: format!("album/{aid}"),
                n_tracks,
                number: None,
                tag,
                year: parse_year(a),
            });
        }
        offset += items.len() as u32;
        if (offset as u64) >= total || offset > 2000 {
            break;
        }
    }
    Ok(out)
}

/// READ-ONLY: an artist's full albums plus EPs/singles (merged) for the expand view.
pub fn artist_albums(artist_id: String) -> Result<Vec<FavItem>, String> {
    let (token, _uid, cc) = read_auth()?;
    let id = numeric_id(&artist_id)?;
    let mut out = fetch_artist_releases(&token, &cc, id, None)?; // full albums
    let singles = fetch_artist_releases(&token, &cc, id, Some("EPSANDSINGLES"))?;
    let seen: HashSet<String> = out.iter().map(|a| a.id.clone()).collect();
    for s in singles {
        if !seen.contains(&s.id) {
            out.push(s);
        }
    }
    Ok(out)
}

/// READ-ONLY: list an album's tracks (for the expand-album view).
pub fn album_tracks(album_id: String) -> Result<Vec<FavItem>, String> {
    let (token, _uid, cc) = read_auth()?;
    let id = numeric_id(&album_id)?;
    let mut out = Vec::new();
    let mut offset: u32 = 0;
    loop {
        let url = format!("https://api.tidal.com/v1/albums/{id}/items?countryCode={cc}&limit=100&offset={offset}");
        let v = api_get(&token, &url)?;
        let items = v.get("items").and_then(|i| i.as_array()).cloned().unwrap_or_default();
        let total = v.get("totalNumberOfItems").and_then(|t| t.as_u64()).unwrap_or(0);
        if items.is_empty() {
            break;
        }
        for w in &items {
            let typ = w.get("type").and_then(|t| t.as_str()).unwrap_or("track");
            let it = w.get("item").unwrap_or(w);
            let tid = match it.get("id").and_then(|x| x.as_i64()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let title = it.get("title").and_then(|t| t.as_str()).unwrap_or("").to_string();
            let artist = it.get("artist").and_then(|x| x.get("name")).and_then(|n| n.as_str()).unwrap_or("").to_string();
            let number = it.get("trackNumber").and_then(|n| n.as_u64()).map(|n| n as u32);
            let kind = if typ == "video" { "video" } else { "track" };
            out.push(FavItem {
                kind: kind.to_string(),
                id: tid.clone(),
                title,
                artist,
                resource: format!("{kind}/{tid}"),
                n_tracks: None,
                number,
                tag: None,
                year: parse_year(it),
            });
        }
        offset += items.len() as u32;
        if (offset as u64) >= total || offset > 2000 {
            break;
        }
    }
    Ok(out)
}

/// Fetch favorites for the requested types, write the cache, and return it.
pub fn refresh(types: Vec<String>) -> Result<FavoritesCache, String> {
    let (token, uid, cc) = read_auth()?;
    let mut plurals: Vec<String> = Vec::new();
    for t in &types {
        if let Some(p) = endpoint_plural(t) {
            if !plurals.iter().any(|x| x == p) {
                plurals.push(p.to_string());
            }
        }
    }
    if plurals.is_empty() {
        return Err("Select at least one favorite type to check".into());
    }
    let mut items = Vec::new();
    for p in &plurals {
        items.extend(fetch_type(&token, &uid, &cc, p)?);
    }
    let cache = FavoritesCache {
        items,
        checked_at: Some(now_unix()),
        types: plurals,
    };
    if let Some(p) = cache_path() {
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let json = serde_json::to_string_pretty(&cache).map_err(|e| e.to_string())?;
        std::fs::write(&p, json).map_err(|e| format!("write favorites cache: {e}"))?;
    }
    Ok(cache)
}

#[derive(Serialize, Clone)]
pub struct AlbumMeta {
    pub year: Option<u32>,
    pub n_tracks: Option<u32>,
}

/// READ-ONLY: fetch year + track count for one album (to enrich search rows).
pub fn album_meta(album_id: String) -> Result<AlbumMeta, String> {
    let (token, _uid, cc) = read_auth()?;
    let id = numeric_id(&album_id)?;
    let v = api_get(&token, &format!("https://api.tidal.com/v1/albums/{id}?countryCode={cc}"))?;
    Ok(AlbumMeta {
        year: parse_year(&v),
        n_tracks: v.get("numberOfTracks").and_then(|n| n.as_u64()).map(|n| n as u32),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    // Hits the live streaming API using ~/.tiddl/auth.json. Run explicitly:
    //   cargo test --lib favorites -- --ignored --nocapture
    #[test]
    #[ignore]
    fn fetch_live_favorites() {
        let c = refresh(vec!["tracks".into(), "albums".into(), "artists".into()]).expect("refresh");
        eprintln!("fetched {} favorites; checked_at={:?}", c.items.len(), c.checked_at);
        for i in c.items.iter().take(5) {
            eprintln!("  [{}] {} - {}  -> {}", i.kind, i.artist, i.title, i.resource);
        }
        assert!(c.checked_at.is_some());
        assert!(!c.items.is_empty(), "expected at least one favorite");
    }

    #[test]
    #[ignore]
    fn browse_artist_and_album() {
        let albums = artist_albums("8958".into()).expect("artist_albums"); // Hilary Duff
        eprintln!("artist 8958 -> {} albums", albums.len());
        for a in albums.iter().take(3) {
            eprintln!("  album {} ({} tr) -> {}", a.title, a.n_tracks.unwrap_or(0), a.resource);
        }
        assert!(!albums.is_empty());

        let tracks = album_tracks("514401".into()).expect("album_tracks"); // Alicia Keys
        eprintln!("album 514401 -> {} tracks", tracks.len());
        for t in tracks.iter().take(3) {
            eprintln!("  #{} {} -> {}", t.number.unwrap_or(0), t.title, t.resource);
        }
        assert!(!tracks.is_empty());
    }
}
