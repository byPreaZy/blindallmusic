//! The hub: channels → connections, fan-out, presence bookkeeping.
//!
//! Presence channels (`room:{id}`) keep an in-memory member map — the real
//! "who is here" source of truth (WS disconnects are detected by TCP
//! close, so the PHP heartbeat trick is only mirrored to Redis for the
//! HTTP-facing member count, in blindall-game).

use crate::protocol::{PresenceKind, ServerOp};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

pub type ConnId = u64;
/// Capacity of a connection's outbound queue before it's considered dead.
const OUTBOUND_CAP: usize = 512;

#[derive(Clone)]
pub struct Outbound {
    tx: mpsc::Sender<ServerOp>,
}

impl Outbound {
    /// Non-blocking send; returns false if the client is too far behind.
    fn send(&self, op: ServerOp) -> bool {
        self.tx.try_send(op).is_ok()
    }
}

/// Handle given to a live connection — used to remove it on disconnect.
pub struct Connection {
    pub id: ConnId,
    pub user_id: i64,
    outbound: Outbound,
    hub: Hub,
    subscriptions: Vec<String>,
}

impl Connection {
    /// Deliver one frame to this socket.
    pub fn send(&self, op: ServerOp) -> bool {
        self.outbound.send(op)
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        let hub = self.hub.clone();
        let id = self.id;
        let user_id = self.user_id;
        let subs = std::mem::take(&mut self.subscriptions);
        // Spawn so Drop stays sync; ordering with respect to in-flight
        // broadcasts is not required (leave is best-effort).
        tokio::spawn(async move {
            for ch in subs {
                hub.detach(id, user_id, &ch).await;
            }
        });
    }
}

struct ChannelState {
    /// conn_id → outbound socket
    conns: HashMap<ConnId, (i64, Outbound)>,
    /// user_id → (member payload, open-conn count on this channel)
    members: HashMap<i64, (Value, usize)>,
    seq: u64,
}

impl ChannelState {
    fn next_seq(&mut self) -> u64 {
        self.seq += 1;
        self.seq
    }
}

#[derive(Clone, Default)]
pub struct Hub {
    channels: Arc<RwLock<HashMap<String, ChannelState>>>,
    next_conn: Arc<AtomicU64>,
}

impl Hub {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            next_conn: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Register a socket (post-auth). Returns its connection handle and the
    /// receiver end the ws layer must drain into the TCP socket.
    pub fn connect(&self, user_id: i64) -> (Connection, mpsc::Receiver<ServerOp>) {
        let id = self.next_conn.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::channel(OUTBOUND_CAP);
        (
            Connection {
                id,
                user_id,
                outbound: Outbound { tx },
                hub: self.clone(),
                subscriptions: Vec::new(),
            },
            rx,
        )
    }

    /// `subscribe` — registers the socket and, for presence channels,
    /// returns the member snapshot taken AFTER the joiner is added
    /// (Pusher `.here` includes the joining member itself).
    /// `presence:join` is announced to the *other* sockets only, and only
    /// on the user's first socket (multi-tab connections do not
    /// re-announce).
    pub async fn subscribe(
        &self,
        conn: &mut Connection,
        channel: &str,
        member: Option<Value>,
    ) -> Option<Vec<Value>> {
        let mut first_join = false;
        let snapshot;

        {
            let mut channels = self.channels.write().await;
            let state = channels
                .entry(channel.to_string())
                .or_insert(ChannelState {
                    conns: HashMap::new(),
                    members: HashMap::new(),
                    seq: 0,
                });

            state
                .conns
                .insert(conn.id, (conn.user_id, conn.outbound.clone()));

            if let Some(m) = &member {
                match state.members.get_mut(&conn.user_id) {
                    Some((_, n)) => *n += 1,
                    None => {
                        state.members.insert(conn.user_id, (m.clone(), 1));
                        first_join = true;
                    }
                }
            }

            // `.here` semantics: the sync snapshot includes self.
            snapshot = member.as_ref().map(|_| {
                state
                    .members
                    .values()
                    .map(|(v, _)| v.clone())
                    .collect::<Vec<_>>()
            });

            conn.subscriptions.push(channel.to_string());
        }

        if first_join {
            self.broadcast_presence_except(
                channel,
                PresenceKind::Join,
                vec![member.unwrap()],
                Some(conn.id),
            )
            .await;
        }

        snapshot
    }

    /// `unsubscribe` — detach this socket only.
    pub async fn unsubscribe(&self, conn: &mut Connection, channel: &str) {
        conn.subscriptions.retain(|c| c != channel);
        self.detach(conn.id, conn.user_id, channel).await;
    }

    /// Detach a conn from a channel; emits `presence:leave` when the user
    /// has no remaining socket on it.
    pub async fn detach(&self, id: ConnId, user_id: i64, channel: &str) {
        let mut channels = self.channels.write().await;
        let Some(state) = channels.get_mut(channel) else {
            return;
        };

        if state.conns.remove(&id).is_none() {
            return;
        }

        let mut leave_payload: Option<Value> = None;
        if let Some((payload, n)) = state.members.get_mut(&user_id) {
            *n -= 1;
            if *n == 0 {
                leave_payload = Some(payload.clone());
                state.members.remove(&user_id);
            }
        }

        if state.conns.is_empty() {
            channels.remove(channel);
            return;
        }
        drop(channels);

        if let Some(p) = leave_payload {
            self.broadcast_presence(channel, PresenceKind::Leave, vec![p])
                .await;
        }
    }

    /// Broadcast a named event to every socket on `channel`.
    /// `to_subscriber` events (Pusher `toOthers`) skip `except` when given.
    pub async fn broadcast(
        &self,
        channel: &str,
        event: &str,
        data: Value,
        except: Option<ConnId>,
    ) -> usize {
        let (seq, targets) = {
            let mut channels = self.channels.write().await;
            let Some(state) = channels.get_mut(channel) else {
                return 0;
            };
            let seq = state.next_seq();
            (
                seq,
                state
                    .conns
                    .iter()
                    .map(|(id, (_, out))| (*id, out.clone()))
                    .collect::<Vec<_>>(),
            )
        };

        let mut delivered = 0;
        for (id, out) in targets {
            if except == Some(id) {
                continue;
            }
            if out.send(ServerOp::Event {
                channel: channel.to_string(),
                event: event.to_string(),
                data: data.clone(),
                seq,
            }) {
                delivered += 1;
            }
        }
        delivered
    }

    async fn broadcast_presence(&self, channel: &str, kind: PresenceKind, users: Vec<Value>) {
        self.broadcast_presence_except(channel, kind, users, None).await;
    }

    /// Presence announcements travel as structured `presence` frames, not
    /// regular events, so clients can separate roster updates from game
    /// events.
    async fn broadcast_presence_except(
        &self,
        channel: &str,
        kind: PresenceKind,
        users: Vec<Value>,
        except: Option<ConnId>,
    ) {
        let targets: Vec<(ConnId, Outbound)> = {
            let channels = self.channels.read().await;
            channels
                .get(channel)
                .map(|s| {
                    s.conns
                        .iter()
                        .map(|(id, (_, out))| (*id, out.clone()))
                        .collect()
                })
                .unwrap_or_default()
        };

        for (id, out) in targets {
            if except == Some(id) {
                continue;
            }
            let _ = out.send(ServerOp::Presence {
                channel: channel.to_string(),
                kind,
                users: users.clone(),
            });
        }
    }

    /// Current presence members for a channel (for `joined` hydration or
    /// diagnostics — the hub, not Redis, is the source of truth).
    pub async fn presence_members(&self, channel: &str) -> Vec<Value> {
        self.channels
            .read()
            .await
            .get(channel)
            .map(|s| s.members.values().map(|(v, _)| v.clone()).collect())
            .unwrap_or_default()
    }

    /// Number of open sockets on a channel.
    pub async fn channel_conn_count(&self, channel: &str) -> usize {
        self.channels
            .read()
            .await
            .get(channel)
            .map(|s| s.conns.len())
            .unwrap_or(0)
    }

    /// Distinct member count on a presence channel.
    pub async fn presence_count(&self, channel: &str) -> usize {
        self.channels
            .read()
            .await
            .get(channel)
            .map(|s| s.members.len())
            .unwrap_or(0)
    }
}
