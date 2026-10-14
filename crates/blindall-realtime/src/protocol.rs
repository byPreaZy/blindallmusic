//! Wire protocol — JSON text frames. Replaces the Pusher/Echo protocol
//! with a single multiplexed connection per client (blueprint §6).

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Client → server.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ClientOp {
    Subscribe {
        channel: String,
        /// Correlates the `ack`/`error` response.
        req_id: Option<String>,
    },
    Unsubscribe {
        channel: String,
        req_id: Option<String>,
    },
    Heartbeat {
        ts: Option<i64>,
    },
}

/// Server → client.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ServerOp {
    /// A broadcast event on a subscribed channel. `event` keeps the PHP
    /// broadcast names verbatim (`TrackPlayed`, `NewScore`, …).
    Event {
        channel: String,
        event: String,
        data: Value,
        /// Per-channel monotonically increasing sequence.
        seq: u64,
    },
    /// Presence lifecycle on presence channels (`room:{id}`).
    Presence {
        channel: String,
        kind: PresenceKind,
        /// `sync` carries the full member list; join/leave carry one member.
        users: Vec<Value>,
    },
    Ack {
        req_id: Option<String>,
    },
    Pong {
        ts: Option<i64>,
    },
    Error {
        code: &'static str,
        req_id: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresenceKind {
    Sync,
    Join,
    Leave,
}
