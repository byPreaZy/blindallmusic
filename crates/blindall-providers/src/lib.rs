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
