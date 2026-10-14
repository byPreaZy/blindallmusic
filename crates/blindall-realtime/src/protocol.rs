use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ClientOp {
    Subscribe {
        channel: String,
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

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ServerOp {
    Event {
        channel: String,
        event: String,
        data: Value,
        seq: u64,
    },
    Presence {
        channel: String,
        kind: PresenceKind,
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
