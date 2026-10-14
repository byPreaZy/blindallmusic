//! blindall-realtime — native WebSocket hub for Blind'All Music.
//!
//! Replaces Reverb/Pusher/Echo: a single JSON-multiplexed connection per
//! client, channels `room:{id}` (presence+events), `room.public:{id}`
//! (anon), `user:{id}` (private), `chat.message:{id}`.
//!
//! Presence is owned by the hub (real TCP connections) — not by a Redis
//! heartbeat, which is mirrored only for the HTTP-facing member count.

pub mod hub;
pub mod protocol;
pub mod ws;

pub use hub::{Connection, Hub};
pub use protocol::{ClientOp, PresenceKind, ServerOp};
pub use ws::{serve, AuthzError, ChannelAuthorizer};
