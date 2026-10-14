use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderTrack {
    pub provider: String,
    pub provider_id: String,
    pub provider_url: Option<String>,
    pub provider_popularity: i64,
    pub artist_name: Option<String>,
    pub track_name: String,
    pub album_name: Option<String>,
    pub preview_url: Option<String>,
    pub release_date: Option<String>,
    pub artwork_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderError {
    pub error: bool,
    pub provider: String,
    pub message: String,
    pub status_code: u16,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub quota_exceeded: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub rate_limited: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reset_time: Option<String>,
}

impl ProviderError {
    pub fn new(provider: &'static str, message: impl Into<String>, status_code: u16) -> Self {
        Self {
            error: true,
            provider: provider.to_string(),
            message: message.into(),
            status_code,
            quota_exceeded: false,
            rate_limited: false,
            reset_time: None,
        }
    }
}

pub type ProviderResult = Result<Vec<ProviderTrack>, ProviderError>;

#[derive(Debug, Clone, Default)]
pub struct ProvidersConfig {
    pub spotify_client_id: String,
    pub spotify_client_secret: String,
    pub spotify_market: String,
    pub audius_app_name: String,
    pub youtube_api_key: String,
    pub app_url: String,
    pub app_locale: String,
}
