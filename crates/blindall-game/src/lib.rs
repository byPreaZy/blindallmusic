//! blindall-game — the round state machine.
//!
//! Replaces Laravel's delayed-job chain (`StartRound`,
//! `ProcessTrackPlayed`, `ProcessTrackEnded`, `ProcessRoundFinished`,
//! `ProcessRoundFinalization`) with one tokio actor per room; timers are
//! `tokio::time::Instant` deadlines instead of Redis queue delays.

pub mod actor;
pub mod events;
pub mod finalize;
pub mod manager;
pub mod store;
/// In-memory `GameStore`/`ScoreStore`/`PresenceStore` doubles used by the
/// actor tests; `pub` so integration tests (and other crates' tests)
/// can reuse them.
#[doc(hidden)]
pub mod testkit;

pub use actor::{RoomActor, RoomCommand, RoomSnapshot};
pub use manager::GameManager;
pub use store::{
    GameStore, GameTrack, PlayerInfo, Result, RoomConfig, RoundRow, StoreError,
};
