//! `DeezerService::searchTrack` port.

use crate::types::{ProviderError, ProviderResult, ProviderTrack};
use serde_json::Value;
use std::time::Duration;

pub struct DeezerClient {
    http: reqwest::Client,
}

impl Default for DeezerClient {
    fn default() -> Self {
        Self::new()
    }
}

impl DeezerClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
        }
    }

    /// GET api.deezer.com/search/track?q=…&strict=on&limit=25 → readable only.
    pub async fn search_track(&self, term: &str) -> ProviderResult {
        if term.trim().is_empty() {
            return Ok(vec![]);
        }
        let url = format!(
            "https://api.deezer.com/search/track?q={}&strict=on&limit=25",
            urlencoded(term.trim())
        );
        let resp = self.http.get(&url).send().await.map_err(|e| {
            tracing::error!(error = %e, "deezer search failed");
            ProviderError::new("deezer", "Deezer service is temporarily unavailable", 500)
        })?;
        let status = resp.status().as_u16();
        if !resp.status().is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(ProviderError::new("deezer", format!("Deezer : {body}"), status));
        }
        let j = resp.json::<Value>().await.unwrap_or_default();
        let tracks = j["data"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|t| t["readable"].as_bool().unwrap_or(false))
                    .map(format_track)
                    .collect()
            })
            .unwrap_or_default();
        Ok(tracks)
    }

    /// `DeezerService::findPlaylist` — GET api.deezer.com/playlist/{id}
    /// → {id, name, description, tracks_count, image}.
    pub async fn find_playlist(&self, playlist_id: &str) -> Result<Value, ProviderError> {
        let url = format!(
            "https://api.deezer.com/playlist/{}",
            urlencoded(playlist_id.trim())
        );
        let resp = self.http.get(&url).send().await.map_err(|e| {
            tracing::error!(error = %e, "deezer playlist lookup failed");
            ProviderError::new("deezer", "Deezer service is temporarily unavailable", 500)
        })?;
        let status = resp.status().as_u16();
        let j = resp.json::<Value>().await.unwrap_or_default();
        if status >= 400 || j["error"].is_object() {
            return Err(ProviderError::new(
                "deezer",
                format!("Deezer : {}", j["error"]["message"].as_str().unwrap_or("unknown error")),
                if status == 0 { 404 } else { status },
            ));
        }
        Ok(serde_json::json!({
            "id": j["id"].as_i64().unwrap_or(0),
            "name": j["title"].as_str().unwrap_or_default(),
            "description": j["description"].as_str().unwrap_or_default(),
            "tracks_count": j["nb_tracks"].as_i64().unwrap_or(0),
            "image": j["picture_medium"].as_str().unwrap_or_default(),
        }))
    }

    /// `DeezerService::importPlaylist` — paginated GET of
    /// /playlist/{id}/tracks following the `next` links (100/page).
    /// Tracks are formatted `forStorage` → preview_url is the
    /// `deezer://{id}` placeholder resolved live at playback.
    pub async fn playlist_tracks(&self, playlist_id: &str) -> Result<Vec<ProviderTrack>, ProviderError> {
        let mut url = format!(
            "https://api.deezer.com/playlist/{}/tracks?limit=100",
            urlencoded(playlist_id.trim())
        );
        let mut out = Vec::new();
        let mut hops = 0u32;
        loop {
            let resp = self.http.get(&url).send().await.map_err(|e| {
                tracing::error!(error = %e, "deezer playlist tracks failed");
                ProviderError::new("deezer", "Deezer service is temporarily unavailable", 500)
            })?;
            if !resp.status().is_success() {
                let body = resp.text().await.unwrap_or_default();
                return Err(ProviderError::new("deezer", format!("Deezer : {body}"), 502));
            }
            let j = resp.json::<Value>().await.unwrap_or_default();
            if let Some(err) = j["error"].as_object() {
                return Err(ProviderError::new(
                    "deezer",
                    format!("Deezer : {}", err["message"].as_str().unwrap_or("unknown error")),
                    404,
                ));
            }
            if let Some(data) = j["data"].as_array() {
                out.extend(
                    data.iter()
                        .filter(|t| t["readable"].as_bool().unwrap_or(false))
                        .map(format_track_for_storage),
                );
            }
            match j["next"].as_str() {
                Some(next) if hops < 25 => {
                    url = next.to_string();
                    hops += 1;
                }
                _ => break,
            }
        }
        Ok(out)
    }
}

/// `formatTrack($track, forStorage: false)` — real preview URL.
pub fn format_track(t: &Value) -> ProviderTrack {
    ProviderTrack {
        provider: "deezer".to_string(),
        provider_id: t["id"].as_i64().map(|i| i.to_string()).unwrap_or_default(),
        provider_url: t["link"].as_str().map(str::to_string),
        provider_popularity: t["rank"].as_i64().unwrap_or(0),
        artist_name: t["artist"]["name"].as_str().map(str::to_string),
        track_name: t["title"].as_str().unwrap_or_default().to_string(),
        album_name: t["album"]["title"].as_str().map(str::to_string),
        preview_url: t["preview"].as_str().map(str::to_string),
        // PHP: release_date => null (getReleaseDate too slow).
        release_date: None,
        artwork_url: t["album"]["cover_medium"].as_str().map(str::to_string),
    }
}

/// `formatTrack($track, forStorage: true)` — preview_url becomes the
/// `deezer://{id}` placeholder, resolved via live API call at playback
/// (`Track::audio()` in PHP, `resolver.rs` here).
pub fn format_track_for_storage(t: &Value) -> ProviderTrack {
    let mut track = format_track(t);
    track.preview_url = Some(format!("deezer://{}", track.provider_id));
    track
}

/// `urlencode` (RFC 3986, spaces → %20 not +).
pub fn urlencoded(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencoded_escapes_like_php() {
        assert_eq!(urlencoded("a b"), "a%20b");
        assert_eq!(urlencoded("AC/DC"), "AC%2FDC");
        assert_eq!(urlencoded("l'équipe"), "l%27%C3%A9quipe");
    }
}
