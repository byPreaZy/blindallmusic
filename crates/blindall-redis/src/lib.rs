pub mod keys;
pub mod presence;
pub mod scores;

pub use scores::RoundScoreStore;
pub use presence::RoomPresenceStore;

#[derive(Debug, thiserror::Error)]
pub enum RedisError {
    #[error("redis error: {0}")]
    Client(#[from] fred::error::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, RedisError>;
