//! Registry of live room actors + boot recovery.
//!
//! One entry per active room (PHP equivalent: the set of rooms with an
//! unfinished round on the Horizon queue). Actors are spawned lazily on
//! first command and dropped via `Shutdown`.

use crate::actor::{RoomActor, RoomCommand, RoomSnapshot};
use crate::store::{GameStore, RoomConfig};
use crate::store::{PresenceStore, ScoreStore};
use blindall_realtime::Hub;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

#[derive(Clone)]
pub struct GameManager {
    store: Arc<dyn GameStore>,
    scores: Arc<dyn ScoreStore>,
    presence: Arc<dyn PresenceStore>,
    hub: Hub,
    /// `storage.url("")` trimmed — handed to each actor so broadcast
    /// payloads can carry accessor-resolved photo URLs.
    photo_prefix: String,
    actors: Arc<RwLock<HashMap<i64, mpsc::Sender<RoomCommand>>>>,
}

impl GameManager {
    pub fn new(
        store: Arc<dyn GameStore>,
        scores: Arc<dyn ScoreStore>,
        presence: Arc<dyn PresenceStore>,
        hub: Hub,
        photo_prefix: String,
    ) -> Self {
        Self {
            store,
            scores,
            presence,
            hub,
            photo_prefix,
            actors: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Boot recovery: every unfinished round gets its actor back, resumed
    /// at the next track boundary (blueprint §3.2 — never mid-track).
    pub async fn recover(&self) -> anyhow::Result<()> {
        let rounds = self.store.load_active_rounds().await?;
        for round in rounds {
            let room = self.store.room_config(round.room_id).await?;
            let tx = self.actor_for(room).await;
            let _ = tx.send(RoomCommand::Recover { round }).await;
        }
        tracing::info!("game recovery done");
        Ok(())
    }

    /// Get (or lazily spawn) the actor for a room.
    pub async fn actor_for(&self, room: RoomConfig) -> mpsc::Sender<RoomCommand> {
        {
            let actors = self.actors.read().await;
            if let Some(tx) = actors.get(&room.id) {
                if !tx.is_closed() {
                    return tx.clone();
                }
            }
        }
        let mut actors = self.actors.write().await;
        if let Some(tx) = actors.get(&room.id) {
            if !tx.is_closed() {
                return tx.clone();
            }
        }
        let tx = RoomActor::spawn(
            room.clone(),
            self.store.clone(),
            self.scores.clone(),
            self.presence.clone(),
            self.hub.clone(),
            self.photo_prefix.clone(),
        );
        actors.insert(room.id, tx.clone());
        tx
    }

    /// Convenience for the API layer: send a command to room's actor.
    pub async fn send(&self, room_id: i64, cmd: RoomCommand) -> anyhow::Result<()> {
        let room = self.store.room_config(room_id).await?;
        let tx = self.actor_for(room).await;
        tx.send(cmd)
            .await
            .map_err(|_| anyhow::anyhow!("room actor gone"))
    }

    /// Room ids whose actor is alive — the sweeper skips their rounds so it
    /// never finishes a round an actor still owns.
    pub async fn live_room_ids(&self) -> Vec<i64> {
        let actors = self.actors.read().await;
        actors
            .iter()
            .filter(|(_, tx)| !tx.is_closed())
            .map(|(id, _)| *id)
            .collect()
    }

    /// `rounds:force-clear` admin op — send `Stop` to every live actor.
    /// Returns the room ids stopped.
    pub async fn stop_all(&self) -> Vec<i64> {
        let txs: Vec<(i64, mpsc::Sender<RoomCommand>)> = {
            let actors = self.actors.read().await;
            actors.iter().map(|(id, tx)| (*id, tx.clone())).collect()
        };
        let mut stopped = Vec::new();
        for (id, tx) in txs {
            if tx.send(RoomCommand::Stop).await.is_ok() {
                stopped.push(id);
            }
        }
        stopped
    }

    /// Snapshot for `GET /rooms/{id}`-style reads.
    pub async fn snapshot(&self, room_id: i64) -> anyhow::Result<Option<RoomSnapshot>> {
        let tx = {
            let actors = self.actors.read().await;
            actors.get(&room_id).cloned()
        };
        let Some(tx) = tx else { return Ok(None) };
        let (reply, rx) = tokio::sync::oneshot::channel();
        tx.send(RoomCommand::Snapshot { reply }).await?;
        Ok(Some(rx.await?))
    }
}
