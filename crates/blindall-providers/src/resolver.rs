use blindall_game::store::PreviewResolver;
use fred::prelude::*;
use std::sync::Arc;
use std::time::Duration;

const DEEZER_CACHE_TTL: i64 = 300;

pub struct ProviderResolver {
    http: reqwest::Client,
    redis: Client,
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

    fn php_ser(v: &str) -> String {
        format!("s:{}:\"{}\";", v.len(), v)
    }
    fn php_unser(v: &str) -> Option<String> {
        if let Some(inner) = v.strip_prefix("s:").and_then(|s| s.strip_suffix(';')) {
            let (_, rest) = inner.split_once(':')?;
            return Some(rest.trim_matches('"').to_string());
        }
        Some(v.to_string())
    }

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
            _ => Some(preview_url.to_string()),
        }
    }
}

pub type SharedResolver = Arc<ProviderResolver>;
