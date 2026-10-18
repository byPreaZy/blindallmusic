use crate::types::{ProviderError, ProviderTrack, ProvidersConfig};
use crate::{
    audius::AudiusService,
    deezer::DeezerClient,
    itunes::ItunesClient,
    spotify::SpotifyClient,
    youtube::{YouTubeApiClient, YouTubeScrapeClient},
};
use fred::prelude::*;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const CACHE_TTL: i64 = 300;

#[derive(Serialize, Deserialize)]
struct CachedSearch {
    results: Vec<ProviderTrack>,
    errors: Vec<ProviderError>,
}

pub struct MusicProviders {
    cfg: ProvidersConfig,
    deezer: DeezerClient,
    spotify: SpotifyClient,
    itunes: ItunesClient,
    audius: AudiusService,
    youtube_api: YouTubeApiClient,
    youtube_scrape: YouTubeScrapeClient,
    redis: Client,
}

impl MusicProviders {
    pub fn new(cfg: ProvidersConfig, redis: Client) -> Self {
        Self {
            spotify: SpotifyClient::new(
                &cfg.spotify_client_id,
                &cfg.spotify_client_secret,
                &cfg.spotify_market,
            ),
            itunes: ItunesClient::new(&cfg.app_locale),
            audius: AudiusService::new(&cfg.audius_app_name),
            youtube_api: YouTubeApiClient::new(&cfg.youtube_api_key, &cfg.app_url),
            youtube_scrape: YouTubeScrapeClient::new(),
            deezer: DeezerClient::new(),
            cfg,
            redis,
        }
    }

    fn cache_key(term: &str, providers: &[&str]) -> String {
        format!("rs:music_search:{term}:{}", providers.join(","))
    }

    pub async fn search(
        &self,
        term: &str,
        providers: &[&str],
        is_public_moderator: bool,
    ) -> (Vec<ProviderTrack>, Vec<ProviderError>) {
        if term.trim().is_empty() || providers.is_empty() {
            return (vec![], vec![]);
        }

        let key = Self::cache_key(term, providers);
        if let Ok(Some(v)) = self.redis.get::<Option<String>, _>(&key).await {
            if let Ok(cached) = serde_json::from_str::<CachedSearch>(&v) {
                return (cached.results, cached.errors);
            }
        }

        let mut futs: Vec<
            std::pin::Pin<
                Box<dyn std::future::Future<Output = crate::types::ProviderResult> + Send>,
            >,
        > = Vec::new();
        for p in providers {
            match *p {
                "itunes" => futs.push(Box::pin(self.itunes.search_track(term))),
                "audius" => futs.push(Box::pin(self.audius.search_track(term))),
                "youtube" if is_public_moderator => {
                    futs.push(Box::pin(self.youtube_scrape.search_track(term)))
                }
                "youtube" => futs.push(Box::pin(self.youtube_api.search_track(term))),
                "spotify" => futs.push(Box::pin(self.spotify.search_track(term))),
                "deezer" => futs.push(Box::pin(self.deezer.search_track(term))),
                _ => {}
            }
        }

        let mut results: Vec<ProviderTrack> = vec![];
        let mut errors: Vec<ProviderError> = vec![];
        for res in futures::future::join_all(futs).await {
            match res {
                Ok(tracks) => results.extend(
                    tracks
                        .into_iter()
                        .filter(|t| !t.provider_id.is_empty() && t.preview_url.is_some()),
                ),
                Err(e) => errors.push(e),
            }
        }

        let cached = CachedSearch {
            results: results.clone(),
            errors: errors.clone(),
        };
        if let Ok(v) = serde_json::to_string(&cached) {
            let _ = self
                .redis
                .set::<(), _, _>(&key, v, Some(Expiration::EX(CACHE_TTL)), None, false)
                .await;
        }
        (results, errors)
    }

    pub fn config(&self) -> &ProvidersConfig {
        &self.cfg
    }

    pub async fn find_playlist(
        &self,
        provider: &str,
        playlist_id: &str,
    ) -> Result<serde_json::Value, ProviderError> {
        match provider {
            "deezer" => self.deezer.find_playlist(playlist_id).await,
            "spotify" => self.spotify.find_playlist(playlist_id).await,
            _ => Err(ProviderError::new("providers", "Unsupported provider", 422)),
        }
    }

    pub async fn playlist_tracks(
        &self,
        provider: &str,
        playlist_id: &str,
    ) -> Result<Vec<ProviderTrack>, ProviderError> {
        match provider {
            "deezer" => self.deezer.playlist_tracks(playlist_id).await,
            "spotify" => self.spotify.playlist_tracks(playlist_id).await,
            _ => Err(ProviderError::new("providers", "Unsupported provider", 422)),
        }
    }
}

pub type SharedProviders = Arc<MusicProviders>;
