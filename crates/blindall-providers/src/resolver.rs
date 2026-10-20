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

    fn deezer_cover_key(&self, id: &str) -> String {
        format!("{}deezer_cover_url:{}", self.cache_prefix, id)
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

    /// Deezer's `images/cover//…` URL means "no artwork" (empty hash).
    fn usable_cover(url: &str) -> Option<String> {
        if url.starts_with("http") && !url.contains("/cover//") {
            Some(url.to_string())
        } else {
            None
        }
    }

    /// One `api.deezer.com/track/{id}` call yields both the 30s preview and
    /// the album cover. A resolved-empty audio is cached as a short-lived
    /// sentinel so upstream-dead tracks stop hitting the API on every pick
    /// (previously each pick retried the same dead track, and worse, the
    /// question shipped with no audio at all).
    async fn deezer_media(
        &self,
        provider_id: &str,
        fallback_audio: &str,
        fallback_art: &str,
    ) -> blindall_game::store::ResolvedMedia {
        use blindall_game::store::ResolvedMedia;
        let audio_key = self.deezer_key(provider_id);
        let cover_key = self.deezer_cover_key(provider_id);

        let mut audio: Option<String> = None;
        let mut artwork: Option<String> = None;
        let mut audio_cached = false;
        let mut cover_cached = false;
        if let Ok(Some(v)) = self.redis.get::<Option<String>, _>(&audio_key).await {
            audio_cached = true;
            audio = Self::php_unser(&v).filter(|u| !u.is_empty());
        }
        if let Ok(Some(v)) = self.redis.get::<Option<String>, _>(&cover_key).await {
            cover_cached = true;
            artwork = Self::php_unser(&v).and_then(|u| Self::usable_cover(&u));
        }

        if !audio_cached || !cover_cached {
            let url = format!("https://api.deezer.com/track/{provider_id}");
            let mut fetched = false;
            let mut preview: Option<String> = None;
            let mut cover: Option<String> = None;
            for _ in 0..2 {
                match self.http.get(&url).send().await {
                    Ok(r) if r.status().is_success() => {
                        fetched = true;
                        let j = r.json::<serde_json::Value>().await.unwrap_or_default();
                        if j["readable"].as_bool().unwrap_or(false) {
                            preview = j["preview"].as_str().map(str::to_string);
                        }
                        cover = ["cover_xl", "cover_big", "cover_medium", "cover"]
                            .iter()
                            .find_map(|k| j["album"][k].as_str())
                            .and_then(Self::usable_cover);
                        break;
                    }
                    Ok(_) => break,
                    Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
                }
            }
            if fetched {
                if !audio_cached {
                    // Cache the outcome — including the dead sentinel — so a
                    // removed track is skipped on future picks instead of
                    // re-probed (and served audioless) every time.
                    let to_cache = preview.clone().unwrap_or_default();
                    let _ = self
                        .redis
                        .set::<(), _, _>(
                            &audio_key,
                            Self::php_ser(&to_cache),
                            Some(Expiration::EX(DEEZER_CACHE_TTL)),
                            None,
                            false,
                        )
                        .await;
                    audio = preview;
                }
                if !cover_cached {
                    let to_cache = artwork_or_empty(&cover);
                    let _ = self
                        .redis
                        .set::<(), _, _>(
                            &cover_key,
                            Self::php_ser(&to_cache),
                            Some(Expiration::EX(DEEZER_CACHE_TTL)),
                            None,
                            false,
                        )
                        .await;
                    artwork = cover;
                }
            }
        }

        if audio.is_none() && !fallback_audio.starts_with("deezer://") {
            audio = Some(fallback_audio.to_string()).filter(|u| u.starts_with("http"));
        }
        if artwork.is_none() {
            artwork = Self::usable_cover(fallback_art);
        }
        ResolvedMedia { audio, artwork }
    }
}

fn artwork_or_empty(c: &Option<String>) -> String {
    c.clone().unwrap_or_default()
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
            "deezer" => self.deezer_media(provider_id, preview_url, "").await.audio,
            "audius" => self.audius.live_track_preview(provider_id).await,
            _ => Some(preview_url.to_string()),
        }
    }

    async fn resolve_media(
        &self,
        provider: &str,
        provider_id: &str,
        preview_url: &str,
        artwork_url: &str,
    ) -> blindall_game::store::ResolvedMedia {
        match provider {
            "deezer" => {
                self.deezer_media(provider_id, preview_url, artwork_url)
                    .await
            }
            _ => {
                // audius & others: audio resolves as before, artwork passes
                // through only if it is already a usable URL.
                blindall_game::store::ResolvedMedia {
                    audio: self.resolve(provider, provider_id, preview_url).await,
                    artwork: if artwork_url.starts_with("http") {
                        Some(artwork_url.to_string())
                    } else {
                        None
                    },
                }
            }
        }
    }
}

pub type SharedResolver = Arc<ProviderResolver>;
