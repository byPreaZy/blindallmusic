//! blindall-redis — Redis layer for Blind'All Music.
//!
//! Same key schema as `app/Services/RoundScoreService.php` and
//! `app/Services/RoomPresenceService.php` so a PHP/Rust dual-run can share
//! the store. Multi-command sequences are packed into Lua scripts —
//! strictly more atomic than the PHP call sequence (order counting is now
//! race-free instead of best-effort).

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
