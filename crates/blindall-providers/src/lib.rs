//! blindall-providers — music provider HTTP clients.
//!
//! Two layers: `track.audio` live-preview resolution (Deezer expiring
//! URLs, Audius stream endpoints) and provider search (Spotify, Deezer,
//! iTunes, Audius, YouTube API + scrape) with PHP-compatible merging,
//! error objects, quota/rate-limit handling and search caching.

pub mod audius;
pub mod deezer;
pub mod itunes;
pub mod resolver;
pub mod search;
pub mod spotify;
pub mod types;
pub mod youtube;

pub use resolver::{ProviderResolver, SharedResolver};
pub use search::{MusicProviders, SharedProviders};
pub use types::{ProviderError, ProviderResult, ProviderTrack, ProvidersConfig};
