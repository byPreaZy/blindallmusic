//! `ProviderResolver` — implements the game `PreviewResolver` trait.
//! Ports `DeezerService::getLiveTrackPreview` (live fetch + 300s cache)
//! and `AudiusService::getLiveTrackPreview` (discovery host + stream URL).
//!
//! Cache: the PHP side uses Laravel Cache in Redis under
//! `{prefix}deezer_preview_url:{id}` with PHP-serialized values
//! (`s:N:"..."`). Same key + format → both stacks share the cache.

use blindall_game::store::PreviewResolver;
use fred::prelude::*;
use std::sync::Arc;
use std::time::Duration;

const DEEZER_CACHE_TTL: i64 = 300;

pub struct ProviderResolver {
    http: reqwest::Client,
    redis: Client,
    /// Laravel `cache.prefix` — `{slug(APP_NAME)}_cache_`.
    cache_prefix: String,
    audius: crate::audius::AudiusService,
}

impl ProviderResolver {
    pub fn new(redis: Client, cache_prefix: &str, audius_app_name: &str) -> Self {
        Self {
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap_or_default(),
            redis,
            cache_prefix: cache_prefix.to_string(),
            audius: crate::audius::AudiusService::new(audius_app_name),
        }
    }

    fn deezer_key(&self, id: &str) -> String {
        format!("{}deezer_preview_url:{}", self.cache_prefix, id)
    }

    /// PHP-serialize a string (`s:N:"…"`) for cache coexistence.
    fn php_ser(v: &str) -> String {
        format!("s:{}:\"{}\";", v.len(), v)
    }
    /// Read `s:N:"…"` back (also tolerates a raw string).
    fn php_unser(v: &str) -> Option<String> {
        if let Some(inner) = v.strip_prefix("s:").and_then(|s| s.strip_suffix(';')) {
            let (_, rest) = inner.split_once(':')?;
            return Some(rest.trim_matches('"').to_string());
        }
        Some(v.to_string())
    }

    /// `getLiveTrackPreview` — cached live Deezer preview URL.
    async fn deezer_preview(&self, provider_id: &str, fallback: &str) -> Option<String> {
        let key = self.deezer_key(provider_id);
        if let Ok(Some(v)) = self.redis.get::<Option<String>, _>(&key).await {
            if let Some(url) = Self::php_unser(&v) {
                if !url.is_empty() {
                    return Some(url);
                }
            }
        }

        let url = format!("https://api.deezer.com/track/{provider_id}");
        let mut preview: Option<String> = None;
        for _ in 0..2 {
            match self.http.get(&url).send().await {
                Ok(r) if r.status().is_success() => {
                    let j = r.json::<serde_json::Value>().await.unwrap_or_default();
                    // `readable` flag + `preview` field (PHP checks both).
                    if j["readable"].as_bool().unwrap_or(false) {
                        preview = j["preview"].as_str().map(str::to_string);
                    }
                    break;
                }
                Ok(_) => break,
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }

        let result = match preview {
            Some(p) => Some(p),
            // Fallback to stored preview_url unless it's a deezer://
            // placeholder (PHP's audio accessor does the same).
            None => {
                if fallback.starts_with("deezer://") {
                    None
                } else {
                    Some(fallback.to_string())
                }
            }
        };

        if let Some(url) = &result {
            let _ = self
                .redis
                .set::<(), _, _>(&key, Self::php_ser(url), Some(Expiration::EX(DEEZER_CACHE_TTL)), None, false)
                .await;
        }
        result
    }
}

#[async_trait::async_trait]
impl PreviewResolver for ProviderResolver {
    async fn resolve(
        &self,
        provider: &str,
        provider_id: &str,
        preview_url: &str,
    ) -> Option<String> {
        match provider {
            "deezer" => self.deezer_preview(provider_id, preview_url).await,
            "audius" => self.audius.live_track_preview(provider_id).await,
            // spotify / itunes / local / youtube / default → preview_url
            _ => Some(preview_url.to_string()),
        }
    }
}

/// Convenience alias used by the server bootstrap.
pub type SharedResolver = Arc<ProviderResolver>;
