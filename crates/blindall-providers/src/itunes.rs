//! `AppleMusicService::searchTrack` port (provider key: `itunes`).

use crate::types::{ProviderError, ProviderResult, ProviderTrack};
use serde_json::Value;
use std::time::Duration;

pub struct ItunesClient {
    http: reqwest::Client,
    /// `config('app.locale')` → `country` param.
    country: String,
}

impl ItunesClient {
    pub fn new(country: &str) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
            country: country.to_string(),
        }
    }

    /// GET itunes.apple.com/search — query is transliterated to ASCII then
    /// spaces→+ (PHP: htmlspecialchars + iconv TRANSLIT//IGNORE + str_replace).
    pub async fn search_track(&self, term: &str) -> ProviderResult {
        if term.trim().is_empty() {
            return Ok(vec![]);
        }
        // iconv('UTF-8','ASCII//TRANSLIT//IGNORE') ≈ deunicode.
        let query = deunicode::deunicode(term.trim()).replace(' ', "+");
        let url = format!(
            "https://itunes.apple.com/search?term={}&country={}&media=music&limit=10&output=json",
            query, self.country
        );
        let resp = self.http.get(&url).send().await.map_err(|e| {
            tracing::error!(term, error = %e, "itunes search failed");
            ProviderError::new("itunes", format!("iTunes : {e}"), 500)
        })?;
        if !resp.status().is_success() {
            return Err(ProviderError::new(
                "itunes",
                "iTunes service is temporarily unavailable",
                resp.status().as_u16(),
            ));
        }
        let j = resp.json::<Value>().await.unwrap_or_default();
        let tracks = j["results"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter(|t| t["isStreamable"].as_bool().unwrap_or(false))
                    .map(format_track)
                    .collect()
            })
            .unwrap_or_default();
        Ok(tracks)
    }
}

fn format_track(t: &Value) -> ProviderTrack {
    ProviderTrack {
        provider: "itunes".to_string(),
        provider_id: t["trackId"].as_i64().map(|i| i.to_string()).unwrap_or_default(),
        provider_url: t["trackViewUrl"].as_str().map(str::to_string),
        provider_popularity: 0,
        artist_name: t["artistName"].as_str().map(str::to_string),
        track_name: t["trackName"].as_str().unwrap_or_default().to_string(),
        album_name: t["collectionName"].as_str().map(str::to_string),
        preview_url: t["previewUrl"].as_str().map(str::to_string),
        release_date: t["releaseDate"]
            .as_str()
            .map(|d| d.chars().take(10).collect()),
        artwork_url: t["artworkUrl100"].as_str().map(str::to_string),
    }
}
