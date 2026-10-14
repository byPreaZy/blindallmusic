use crate::types::{ProviderError, ProviderResult, ProviderTrack};
use serde_json::Value;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const TOKEN_URL: &str = "https://accounts.spotify.com/api/token";
const API: &str = "https://api.spotify.com/v1";

pub struct SpotifyClient {
    http: reqwest::Client,
    client_id: String,
    client_secret: String,
    market: String,
    token: Mutex<Option<(String, Instant)>>,
}

impl SpotifyClient {
    pub fn new(client_id: &str, client_secret: &str, market: &str) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
            client_id: client_id.to_string(),
            client_secret: client_secret.to_string(),
            market: if market.is_empty() { "FR".into() } else { market.into() },
            token: Mutex::new(None),
        }
    }

    async fn access_token(&self) -> Result<String, ProviderError> {
        if let Some((tok, at)) = self.token.lock().unwrap().as_ref() {
            if at.elapsed() < Duration::from_secs(3000) {
                return Ok(tok.clone());
            }
        }
        let resp = self
            .http
            .post(TOKEN_URL)
            .basic_auth(&self.client_id, Some(&self.client_secret))
            .form(&[("grant_type", "client_credentials")])
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "spotify token request failed");
                ProviderError::new("spotify", format!("Spotify : {e}"), 500)
            })?;
        let j = resp.json::<Value>().await.unwrap_or_default();
        let token = j["access_token"]
            .as_str()
            .ok_or_else(|| ProviderError::new("spotify", "Spotify : no access_token", 500))?
            .to_string();
        *self.token.lock().unwrap() = Some((token.clone(), Instant::now()));
        Ok(token)
    }

    pub async fn search_track(&self, term: &str) -> ProviderResult {
        if term.trim().is_empty() {
            return Ok(vec![]);
        }
        let token = self.access_token().await?;
        let resp = self
            .http
            .get(format!("{API}/search"))
            .bearer_auth(token)
            .query(&[
                ("q", term),
                ("type", "track,artist"),
                ("market", self.market.as_str()),
                ("limit", "50"),
            ])
            .send()
            .await
            .map_err(|e| {
                tracing::error!(term, error = %e, "spotify search failed");
                ProviderError::new("spotify", format!("Spotify : {e}"), 500)
            })?;
        let status = resp.status().as_u16();
        let j = resp.json::<Value>().await.unwrap_or_default();
        if status != 200 {
            let msg = j["error"]["message"].as_str().unwrap_or("Spotify error");
            return Err(ProviderError::new("spotify", format!("Spotify : {msg}"), status));
        }

        let items = j["tracks"]["items"].as_array();
        let total = j["tracks"]["total"].as_i64().unwrap_or(0);
        let tracks: Vec<ProviderTrack> = items
            .map(|a| {
                a.iter()
                    .filter(|t| {
                        t["is_playable"].as_bool().unwrap_or(false)
                            && t["preview_url"]
                                .as_str()
                                .map(|u| u.starts_with("http"))
                                .unwrap_or(false)
                    })
                    .map(format_track)
                    .collect()
            })
            .unwrap_or_default();

        if total > 0 && tracks.is_empty() {
            return Err(ProviderError::new(
                "spotify",
                "Spotify : Les pistes trouvées ne sont pas disponibles en streaming",
                404,
            ));
        }
        Ok(tracks)
    }

    pub async fn find_playlist(&self, playlist_id: &str) -> Result<Value, ProviderError> {
        let token = self.access_token().await?;
        let resp = self
            .http
            .get(format!("{API}/playlists/{}", playlist_id.trim()))
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "spotify playlist lookup failed");
                ProviderError::new("spotify", format!("Spotify : {e}"), 500)
            })?;
        let status = resp.status().as_u16();
        let j = resp.json::<Value>().await.unwrap_or_default();
        if status != 200 {
            let msg = j["error"]["message"].as_str().unwrap_or("Spotify error");
            return Err(ProviderError::new("spotify", format!("Spotify : {msg}"), status));
        }
        Ok(serde_json::json!({
            "id": j["id"].as_str().unwrap_or_default(),
            "name": j["name"].as_str().unwrap_or_default(),
            "description": j["description"].as_str().unwrap_or_default(),
            "tracks_count": j["tracks"]["total"].as_i64().unwrap_or(0),
            "image": j["images"][0]["url"].as_str().unwrap_or_default(),
        }))
    }

    pub async fn playlist_tracks(
        &self,
        playlist_id: &str,
    ) -> Result<Vec<ProviderTrack>, ProviderError> {
        let token = self.access_token().await?;
        let mut url = format!(
            "{API}/playlists/{}/tracks?market={}&limit=100",
            playlist_id.trim(),
            self.market
        );
        let mut out = Vec::new();
        let mut hops = 0u32;
        loop {
            let resp = self
                .http
                .get(&url)
                .bearer_auth(&token)
                .send()
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "spotify playlist tracks failed");
                    ProviderError::new("spotify", format!("Spotify : {e}"), 500)
                })?;
            let status = resp.status().as_u16();
            let j = resp.json::<Value>().await.unwrap_or_default();
            if status != 200 {
                let msg = j["error"]["message"].as_str().unwrap_or("Spotify error");
                return Err(ProviderError::new("spotify", format!("Spotify : {msg}"), status));
            }
            if let Some(items) = j["items"].as_array() {
                out.extend(
                    items
                        .iter()
                        .map(|i| &i["track"])
                        .filter(|t| {
                            t["is_playable"].as_bool().unwrap_or(false)
                                && t["preview_url"]
                                    .as_str()
                                    .map(|u| u.starts_with("http"))
                                    .unwrap_or(false)
                        })
                        .map(format_track),
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

pub fn format_track(t: &Value) -> ProviderTrack {
    ProviderTrack {
        provider: "spotify".to_string(),
        provider_id: t["id"].as_str().unwrap_or_default().to_string(),
        provider_url: t["external_urls"]["spotify"].as_str().map(str::to_string),
        provider_popularity: t["popularity"].as_i64().unwrap_or(0),
        artist_name: t["artists"][0]["name"].as_str().map(str::to_string),
        track_name: t["name"].as_str().unwrap_or_default().to_string(),
        album_name: t["album"]["name"].as_str().map(str::to_string),
        preview_url: t["preview_url"].as_str().map(str::to_string),
        release_date: t["album"]["release_date"]
            .as_str()
            .map(|d| d.chars().take(10).collect()),
        artwork_url: t["album"]["images"][2]["url"]
            .as_str()
            .or_else(|| t["album"]["images"][0]["url"].as_str())
            .map(str::to_string),
    }
}
