//! READ-ONLY catalog search via the streaming API, ranked for relevance.
//!
//! Like `favorites.rs`, this makes read-only GET requests with the token the
//! user already authorized through `tiddl` (stored in ~/.tiddl/auth.json). It
//! never writes, never touches stream/playback endpoints, and never downloads —
//! all downloading still goes through the `tiddl` CLI. Search is already gated
//! behind sign-in in the UI.
//!
//! Why not the `tiddl` CLI search? Its output is the API's *default* per-type
//! order, which floats exact title matches (karaoke / tribute covers) above the
//! genuine track and exposes no popularity to re-rank with. Here we read the
//! same API directly and rank by `popularity` blended with a query-match boost:
//! a match in the *artist* field weighs heavily (covers carry their own artist,
//! e.g. "Party Tyme Karaoke"), a title match only lightly. So the real recording
//! wins on its popularity + artist match without the noisy title floating covers.

use crate::favorites::{api_get, read_auth};
use crate::tiddl::SearchResult;

const SEARCH_TYPES: [&str; 5] = ["track", "album", "artist", "playlist", "video"];

// A match in the artist field is the strong signal (covers/karaoke carry their
// own artist name); a title match is weak (the noisy title often contains the
// real artist's name too). Base score is the service's own 0-100 popularity.
const ARTIST_MATCH_BONUS: f64 = 40.0;
const TITLE_MATCH_BONUS: f64 = 6.0;

fn plural(kind: &str) -> (&'static str, &'static str) {
    // (UPPER for the `types` request param, lower for the response key)
    match kind {
        "track" => ("TRACKS", "tracks"),
        "album" => ("ALBUMS", "albums"),
        "artist" => ("ARTISTS", "artists"),
        "playlist" => ("PLAYLISTS", "playlists"),
        "video" => ("VIDEOS", "videos"),
        _ => ("TRACKS", "tracks"),
    }
}

/// Percent-encode a query-string value (RFC 3986 unreserved set kept as-is).
fn pct_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Lowercased, de-noised, de-duplicated query tokens used for the match boost.
fn query_tokens(q: &str) -> Vec<String> {
    const STOP: [&str; 13] = [
        "the", "a", "an", "feat", "ft", "featuring", "with", "and", "of", "to", "vs", "by", "x",
    ];
    let mut toks: Vec<String> = Vec::new();
    for t in q.to_lowercase().split(|c: char| !c.is_alphanumeric()) {
        if t.len() >= 2 && !STOP.contains(&t) && !toks.iter().any(|e| e == t) {
            toks.push(t.to_string());
        }
    }
    toks
}

/// "Artist A, Artist B" from an item's `artists[]` (fallback to `artist.name`).
fn artists_str(item: &serde_json::Value) -> String {
    if let Some(arr) = item.get("artists").and_then(|a| a.as_array()) {
        let names: Vec<&str> = arr
            .iter()
            .filter_map(|a| a.get("name").and_then(|n| n.as_str()))
            .filter(|s| !s.is_empty())
            .collect();
        if !names.is_empty() {
            return names.join(", ");
        }
    }
    item.get("artist")
        .and_then(|a| a.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("")
        .to_string()
}

/// Resolve requested kinds (empty = all) into canonical order.
fn resolve_wanted(kinds: &[String]) -> Vec<&'static str> {
    let mut wanted: Vec<&'static str> = Vec::new();
    for k in kinds {
        let k = k.to_lowercase();
        if let Some(t) = SEARCH_TYPES.iter().find(|&&t| t == k) {
            if !wanted.contains(t) {
                wanted.push(*t);
            }
        }
    }
    if wanted.is_empty() {
        wanted = SEARCH_TYPES.to_vec();
    }
    wanted
}

/// Pure ranking: turn a `/v1/search` response into ranked `SearchResult`s.
/// Each kind's items are scored (popularity + artist/title match boost) and
/// stable-sorted highest-first; ties keep the service's native order. Separated from
/// the network call so it is unit-testable on a fixture.
fn rank_results(v: &serde_json::Value, tokens: &[String], wanted: &[&str]) -> Vec<SearchResult> {
    let mut out: Vec<SearchResult> = Vec::new();

    for kind in wanted {
        let key = plural(kind).1;
        let items = match v.get(key).and_then(|b| b.get("items")).and_then(|i| i.as_array()) {
            Some(a) => a,
            None => continue,
        };

        let mut scored: Vec<(f64, SearchResult)> = Vec::new();
        for it in items {
            // Playlists are keyed by uuid; everything else by numeric id.
            let id = if *kind == "playlist" {
                it.get("uuid").and_then(|u| u.as_str()).unwrap_or("").to_string()
            } else {
                it.get("id").and_then(|x| x.as_i64()).map(|n| n.to_string()).unwrap_or_default()
            };
            if id.is_empty() {
                continue;
            }

            let raw_title = it
                .get("title")
                .and_then(|t| t.as_str())
                .or_else(|| it.get("name").and_then(|n| n.as_str()))
                .unwrap_or("")
                .to_string();
            let artists = artists_str(it);
            let display = if *kind == "artist" || artists.is_empty() {
                raw_title.clone()
            } else {
                format!("{artists} - {raw_title}")
            };

            let pop = it.get("popularity").and_then(|p| p.as_f64()).unwrap_or(0.0);
            let artist_lc = artists.to_lowercase();
            let title_lc = raw_title.to_lowercase();
            let mut score = pop;
            for t in tokens {
                if artist_lc.contains(t.as_str()) {
                    score += ARTIST_MATCH_BONUS;
                }
                if title_lc.contains(t.as_str()) {
                    score += TITLE_MATCH_BONUS;
                }
            }

            scored.push((
                score,
                SearchResult {
                    num: 0, // assigned after sorting
                    kind: (*kind).to_string(),
                    title: display,
                    id: id.clone(),
                    resource: format!("{kind}/{id}"),
                },
            ));
        }

        // Stable sort, highest score first — ties keep the service's native order.
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        out.extend(scored.into_iter().map(|(_, r)| r));
    }

    for (i, r) in out.iter_mut().enumerate() {
        r.num = (i + 1) as u32;
    }
    out
}

pub fn run_search_api(query: String, kinds: Vec<String>, limit: u32) -> Result<Vec<SearchResult>, String> {
    let q = query.trim().replace(['\r', '\n', '\0'], " ");
    if q.is_empty() {
        return Err("Enter a search query".into());
    }
    let (token, _uid, cc) = read_auth()?;

    let wanted = resolve_wanted(&kinds);
    let limit = limit.clamp(1, 50);
    let types_csv = wanted.iter().map(|k| plural(k).0).collect::<Vec<_>>().join(",");
    let url = format!(
        "https://api.tidal.com/v1/search?query={}&countryCode={}&limit={}&types={}",
        pct_encode(&q),
        cc,
        limit,
        types_csv
    );
    let v = api_get(&token, &url)?;

    Ok(rank_results(&v, &query_tokens(&q), &wanted))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real-shaped slice of `/v1/search?...&types=TRACKS` for the query
    // "the chainsmokers - closer" (popularity + artist names are the live values).
    fn closer_fixture() -> serde_json::Value {
        serde_json::json!({
            "tracks": { "items": [
                { "id": 123257012,
                  "title": "Closer (Made Popular By The Chainsmokers ft. Halsey) [Vocal Version]",
                  "popularity": 46, "artists": [{ "name": "Party Tyme Karaoke" }] },
                { "id": 63232677, "title": "Closer", "popularity": 92,
                  "artists": [{ "name": "The Chainsmokers" }, { "name": "Halsey" }] },
                { "id": 70748216, "title": "Something Just Like This", "popularity": 92,
                  "artists": [{ "name": "The Chainsmokers" }, { "name": "Coldplay" }] },
                { "id": 65024117, "title": "Closer (tribute to The Chainsmokers feat. Halsey)",
                  "popularity": 36, "artists": [{ "name": "Remixation" }] }
            ]}
        })
    }

    #[test]
    fn genuine_recording_outranks_covers() {
        let out = rank_results(
            &closer_fixture(),
            &query_tokens("the chainsmokers - closer"),
            &["track"],
        );
        let ids: Vec<&str> = out.iter().map(|r| r.id.as_str()).collect();

        // The real recording (artist match + title match) must rank first...
        assert_eq!(ids.first(), Some(&"63232677"), "real Closer should be #1, got {ids:?}");
        // ...and it must beat the equally-popular but title-mismatched Chainsmokers track,
        // which is the whole point of the title-match boost over a plain popularity sort.
        let pos = |id: &str| ids.iter().position(|x| *x == id).unwrap();
        assert!(pos("63232677") < pos("70748216"), "Closer must outrank Something Just Like This: {ids:?}");
        // The karaoke cover and the tribute (low popularity, artist mismatch) must sink.
        assert!(pos("123257012") > pos("63232677"), "karaoke cover must rank below the real track: {ids:?}");
        assert!(pos("65024117") > pos("63232677"), "tribute must rank below the real track: {ids:?}");
    }

    #[test]
    fn display_title_carries_all_artists() {
        let out = rank_results(&closer_fixture(), &query_tokens("closer"), &["track"]);
        let real = out.iter().find(|r| r.id == "63232677").unwrap();
        assert_eq!(real.title, "The Chainsmokers, Halsey - Closer");
        assert_eq!(real.resource, "track/63232677");
    }

    // End-to-end against the live API (auth + URL encode + fetch + rank).
    // Ignored by default like the other live tests; run with `--ignored`.
    #[test]
    #[ignore = "hits the live API; requires a signed-in ~/.tiddl/auth.json"]
    fn live_search_ranks_real_closer_above_karaoke() {
        let out = run_search_api("the chainsmokers - closer".into(), vec![], 15).expect("live search");
        let tracks: Vec<&str> = out.iter().filter(|r| r.kind == "track").map(|r| r.id.as_str()).collect();
        assert_eq!(tracks.first(), Some(&"63232677"), "live: real Closer should be #1, got {tracks:?}");
        let pos = |id: &str| tracks.iter().position(|x| *x == id);
        if let (Some(real), Some(kara)) = (pos("63232677"), pos("123257012")) {
            assert!(real < kara, "live: karaoke cover must rank below the real track: {tracks:?}");
        }
    }
}
