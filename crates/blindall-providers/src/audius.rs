use std::sync::RwLock;
use std::time::Duration;

use crate::types::{ProviderResult, ProviderTrack};
use serde_json::Value;

pub struct AudiusService {
    http: reqwest::Client,
    app_name: String,
    host: RwLock<Option<String>>,
}

impl Default for AudiusService {
    fn default() -> Self {
        Self::new("")
    }
}

impl AudiusService {
    pub fn new(app_name: &str) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap_or_default(),
            app_name: app_name.to_string(),
            host: RwLock::new(None),
        }
    }

    pub async fn search_track(&self, term: &str) -> ProviderResult {
        if term.trim().is_empty() {
            return Ok(vec![]);
        }
        let Some(host) = self.resolve_host().await else {
            return Ok(vec![]);
        };
        let query = crate::deezer::urlencoded(term.trim());
        let resp = self
            .http
            .get(format!("{host}/v1/tracks/search"))
            .query(&[
                ("query", query.as_str()),
                ("limit", "10"),
                ("app_name", self.app_name.as_str()),
                ("search_type", "all"),
            ])
            .send()
            .await;
        let tracks = match resp {
            Ok(r) if r.status().is_success() => r.json::<Value>().await.unwrap_or_default()["data"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter(|t| !t["id"].as_str().unwrap_or_default().is_empty())
                        .map(|t| self.format_track(&host, t))
                        .collect()
                })
                .unwrap_or_default(),
            _ => vec![],
        };
        Ok(tracks)
    }

    fn format_track(&self, host: &str, t: &Value) -> ProviderTrack {
        let id = t["id"].as_str().unwrap_or_default();
        let permalink = t["permalink"].as_str();
        ProviderTrack {
            provider: "audius".to_string(),
            provider_id: id.to_string(),
            provider_url: Some(match permalink {
                Some(p) => format!("https://audius.co{p}"),
                None => format!("https://audius.co/tracks/{id}"),
            }),
            provider_popularity: t["play_count"].as_i64().unwrap_or(0),
            artist_name: t["user"]["name"].as_str().map(str::to_string),
            track_name: t["title"].as_str().unwrap_or_default().to_string(),
            album_name: t["playlist_name"].as_str().map(str::to_string),
            preview_url: if id.is_empty() {
                None
            } else {
                Some(format!("{host}/v1/tracks/{id}/stream"))
            },
            release_date: t["release_date"]
                .as_str()
                .map(|d| d.chars().take(10).collect()),
            artwork_url: t["artwork"]["480x480"].as_str().map(str::to_string),
        }
    }

    async fn resolve_host(&self) -> Option<String> {
        if let Some(h) = self.host.read().ok()?.clone() {
            return Some(h);
        }
        let resp = self.http.get("https://api.audius.co").send().await.ok()?;
        let data = resp.json::<serde_json::Value>().await.ok()?;
        let mut hosts: Vec<String> = data["data"]
            .as_array()?
            .iter()
            .filter_map(|h| h.as_str().map(str::to_string))
            .collect();
        if hosts.is_empty() {
            return None;
        }

        use rand::seq::SliceRandom;
        hosts.shuffle(&mut rand::rng());

        let mut picked = hosts.first().cloned();
        for h in &hosts {
            if let Ok(r) = self.http.get(format!("{h}/v1/health_check")).send().await {
                if r.status().is_success() {
                    picked = Some(h.clone());
                    break;
                }
            }
        }
        if let (Some(p), Ok(mut guard)) = (picked.clone(), self.host.write()) {
            *guard = Some(p);
        }
        picked
    }

    pub async fn live_track_preview(&self, provider_id: &str) -> Option<String> {
        let host = self.resolve_host().await?;
        Some(format!("{host}/v1/tracks/{provider_id}/stream"))
    }
}
