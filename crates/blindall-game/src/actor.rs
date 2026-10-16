use crate::events;
use crate::finalize;
use crate::store::{GameStore, GameTrack, PresenceStore, RoomConfig, RoundRow, ScoreStore};
use blindall_core::scoring::{self, CheckOutcome, CheckRequest};
use blindall_redis::scores::TrackRecord;
use blindall_realtime::Hub;
use chrono::Utc;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep_until, Instant, Sleep};
use std::pin::Pin;

pub enum RoomCommand {
    Start { user_id: Option<i64> },
    Pause,
    Resume,
    Stop,
    CheckAnswer {
        user_id: i64,
        track_id: i64,
        text: String,
        client_words: Vec<String>,
        current_time: f64,
        reply: oneshot::Sender<CheckOutcome>,
    },
    TrackListened { user_id: i64, track_id: i64 },
    UpdateConfig { cfg: RoomConfig },
    Snapshot { reply: oneshot::Sender<RoomSnapshot> },
    Recover { round: RoundRow },
    Shutdown,
}

#[derive(Debug, Clone)]
pub struct RoomSnapshot {
    pub room_id: i64,
    pub is_playing: bool,
    pub round: Option<RoundRow>,
    pub current_track: Option<GameTrack>,
    pub played_track_ids: Vec<i64>,
}

enum Phase {
    Idle,
    Playing { ends_at: Instant },
    InterTrack { next_at: Instant },
    Paused { resume_in: ResumeKind },
    Finalizing { restart_at: Instant },
}

#[derive(Clone, Copy)]
enum ResumeKind {
    Playing { remaining: Duration },
    InterTrack { remaining: Duration },
}

pub struct RoomActor {
    room: RoomConfig,
    round: Option<RoundRow>,
    phase: Phase,
    current_track: Option<GameTrack>,
    store: Arc<dyn GameStore>,
    scores: Arc<dyn ScoreStore>,
    presence: Arc<dyn PresenceStore>,
    hub: Hub,
    photo_prefix: String,
    inbox: mpsc::Receiver<RoomCommand>,
    timer: Pin<Box<Sleep>>,
    presence_grace_until: Option<Instant>,
}

impl RoomActor {
    pub fn spawn(
        room: RoomConfig,
        store: Arc<dyn GameStore>,
        scores: Arc<dyn ScoreStore>,
        presence: Arc<dyn PresenceStore>,
        hub: Hub,
        photo_prefix: String,
    ) -> mpsc::Sender<RoomCommand> {
        let (tx, rx) = mpsc::channel(256);
        let mut actor = Self {
            room,
            round: None,
            phase: Phase::Idle,
            current_track: None,
            store,
            scores,
            presence,
            hub,
            photo_prefix,
            inbox: rx,
            timer: Box::pin(sleep_until(Instant::now() + Duration::from_secs(86400))),
            presence_grace_until: None,
        };
        tokio::spawn(async move { actor.run().await });
        tx
    }

    pub fn recover(&mut self, round: RoundRow) {
        self.round = Some(round);
        self.phase = Phase::InterTrack {
            next_at: Instant::now() + Duration::from_secs(2),
        };
        // After a restart, presence scores may all be stale (>90s) while the
        // room is in fact full of players about to reconnect. Give them a
        // window to heartbeat before the empty-audience check can kill the
        // recovered round.
        self.presence_grace_until = Some(Instant::now() + Duration::from_secs(90));
        self.arm_timer();
    }

    async fn run(&mut self) {
        loop {
            tokio::select! {
                cmd = self.inbox.recv() => {
                    match cmd {
                        Some(c) => if self.handle(c).await { return; },
                        None => return,
                    }
                }
                _ = &mut self.timer => {
                    self.on_timer().await;
                }
            }
        }
    }

    fn arm_timer(&mut self) {
        let deadline = match &self.phase {
            Phase::Playing { ends_at } => *ends_at,
            Phase::InterTrack { next_at } => *next_at,
            Phase::Finalizing { restart_at } => *restart_at,
            _ => Instant::now() + Duration::from_secs(86400),
        };
        self.timer.as_mut().reset(deadline);
    }

    fn park_timer(&mut self) {
        self.timer
            .as_mut()
            .reset(Instant::now() + Duration::from_secs(86400));
    }

    async fn handle(&mut self, cmd: RoomCommand) -> bool {
        match cmd {
            RoomCommand::Start { user_id } => self.start_round(user_id).await,
            RoomCommand::Pause => self.pause().await,
            RoomCommand::Resume => self.resume().await,
            RoomCommand::Stop => self.stop().await,
            RoomCommand::CheckAnswer {
                user_id,
                track_id,
                text,
                client_words,
                current_time,
                reply,
            } => {
                let out = self.check_answer(user_id, track_id, text, client_words, current_time).await;
                let _ = reply.send(out);
            }
            RoomCommand::TrackListened { user_id, track_id } => {
                self.track_listened(user_id, track_id).await;
            }
            RoomCommand::UpdateConfig { cfg } => self.room = cfg,
            RoomCommand::Snapshot { reply } => {
                let _ = reply.send(RoomSnapshot {
                    room_id: self.room.id,
                    is_playing: self.round.as_ref().map(|r| r.is_playing).unwrap_or(false),
                    round: self.round.clone(),
                    current_track: self.current_track.clone(),
                    played_track_ids: self.played_track_ids(),
                });
            }
            RoomCommand::Recover { round } => self.recover(round),
            RoomCommand::Shutdown => return true,
        }
        false
    }

    fn played_track_ids(&self) -> Vec<i64> {
        self.round
            .as_ref()
            .map(|r| {
                let end = (r.current - 1).clamp(0, r.tracks.len() as i64) as usize;
                r.tracks[..end].to_vec()
            })
            .unwrap_or_default()
    }

    // ---- round lifecycle -------------------------------------------------

    async fn start_round(&mut self, user_id: Option<i64>) {
        if let Some(r) = &self.round {
            if r.is_playing && !r.tracks.is_empty() && r.finished_at.is_none() {
                return;
            }
        }

        let tracks = match self.store.pick_tracks(&self.room).await {
            Ok(t) => t,
            Err(e) => {
                tracing::error!(room = self.room.id, error = %e, "pick_tracks failed");
                return;
            }
        };

        let round_id = if let Some(r) = &self.round {
            if r.tracks.is_empty() && r.finished_at.is_none() {
                r.id // PHP reuses an empty round row if present
            } else {
                match self.store.create_round(self.room.id, user_id, &tracks).await {
                    Ok(id) => id,
                    Err(e) => {
                        tracing::error!(room = self.room.id, error = %e, "create_round failed");
                        return;
                    }
                }
            }
        } else {
            match self.store.create_round(self.room.id, user_id, &tracks).await {
                Ok(id) => id,
                Err(e) => {
                    tracing::error!(room = self.room.id, error = %e, "create_round failed");
                    return;
                }
            }
        };

        if tracks.is_empty() {
            let _ = self.store.finish_round(round_id, Utc::now()).await;
            let _ = self.store.set_room_playing(self.room.id, false).await;
            self.round = Some(RoundRow {
                id: round_id,
                room_id: self.room.id,
                tracks: vec![],
                current: 0,
                is_playing: false,
                finished_at: Some(Utc::now()),
                current_track_started_at: None,
            });
            let room_v = self.room_payload().await;
            self.broadcast("RoundFinished", events::round_finished(
                self.round.as_ref().unwrap(), &room_v, json!([]), json!([]),
            )).await;
            self.broadcast_public_state().await;
            tracing::warn!(room = self.room.id, "no tracks : round aborted");
            return;
        }

        let _ = self.store.set_room_playing(self.room.id, true).await;
        self.round = Some(RoundRow {
            id: round_id,
            room_id: self.room.id,
            tracks,
            current: 0,
            is_playing: true,
            finished_at: None,
            current_track_started_at: None,
        });

        let room_v = self.room_payload().await;
        self.broadcast("RoundStarted", events::round_started(self.round.as_ref().unwrap(), &room_v)).await;
        self.broadcast_public_state().await;

        self.play_next_track().await;
    }

    async fn play_next_track(&mut self) {
        loop {
            let Some(round) = self.round.as_mut() else {
                self.park_timer();
                return;
            };
            if round.finished_at.is_some() {
                self.park_timer();
                return;
            }
            let idx = round.current as usize;
            if idx >= round.tracks.len() {
                self.stop().await;
                return;
            }
            let track_id = round.tracks[idx];

            if let Err(e) = self.store.advance_round(round.id, Utc::now()).await {
                tracing::error!(round = round.id, error = %e, "advance_round failed");
                self.park_timer();
                return;
            }
            let round = self.round.as_mut().unwrap();
            round.current += 1;
            round.current_track_started_at = Some(Utc::now());

            let track = match self.store.load_track(track_id).await {
                Ok(Some(t)) => t,
                Ok(None) => {
                    tracing::error!(track_id, "track not found : skipping");
                    continue; // PHP: recursive playNextTrack()
                }
                Err(e) => {
                    tracing::error!(track_id, error = %e, "load_track failed");
                    self.park_timer();
                    return;
                }
            };

            if !track.skips_probe() {
                let dead = match &track.audio {
                    None => {
                        tracing::info!(track_id, "missing audio url : skipping");
                        true
                    }
                    Some(url) => {
                        if self.store.probe_audio(url).await {
                            false
                        } else {
                            tracing::info!(track_id, "audio probe failed : skipping");
                            true
                        }
                    }
                };
                if dead {
                    let store = self.store.clone();
                    let hub = self.hub.clone();
                    tokio::spawn(async move {
                        let Ok(notified) = store.delete_track(track_id, None).await else {
                            return;
                        };
                        for n in notified {
                            hub.broadcast(
                                &format!("user:{}", n.user_id),
                                "Illuminate\\Notifications\\Events\\BroadcastNotificationCreated",
                                serde_json::json!({
                                    "id": n.notification_id,
                                    "type": "App\\Notifications\\TrackDeleted",
                                    "notifiable_type": "App\\Models\\User",
                                    "notifiable_id": n.user_id,
                                    "data": n.data,
                                }),
                                None,
                            )
                            .await;
                        }
                    });
                    continue;
                }
            }

            self.current_track = Some(track.clone());
            let room_v = self.room_payload().await;
            let payload = events::track_played(self.round.as_ref().unwrap(), &room_v, &track);
            self.broadcast("TrackPlayed", payload).await;
            self.broadcast_public_state().await;

            self.phase = Phase::Playing {
                ends_at: Instant::now() + Duration::from_secs(self.room.track_duration as u64),
            };
            self.arm_timer();
            return;
        }
    }

    async fn pause(&mut self) {
        let now = Instant::now();
        let resume_in = match self.phase {
            Phase::Playing { ends_at } => ResumeKind::Playing {
                remaining: ends_at.saturating_duration_since(now),
            },
            Phase::InterTrack { next_at } => ResumeKind::InterTrack {
                remaining: next_at.saturating_duration_since(now),
            },
            _ => return,
        };
        if let Some(r) = self.round.as_mut() {
            r.is_playing = false;
        }
        self.phase = Phase::Paused { resume_in };
        self.arm_timer();
        self.broadcast("TrackPaused", json!({"room_id": self.room.id})).await;
    }

    async fn resume(&mut self) {
        let now = Instant::now();
        self.phase = match self.phase {
            Phase::Paused {
                resume_in: ResumeKind::Playing { remaining },
            } => Phase::Playing { ends_at: now + remaining },
            Phase::Paused {
                resume_in: ResumeKind::InterTrack { remaining },
            } => Phase::InterTrack { next_at: now + remaining },
            _ => return,
        };
        if let Some(r) = self.round.as_mut() {
            r.is_playing = true;
        }
        self.arm_timer();
        self.broadcast("TrackResumed", json!({"room_id": self.room.id})).await;
    }

    async fn stop(&mut self) {
        let Some(round) = self.round.as_mut() else {
            return;
        };
        if round.finished_at.is_some() {
            return; // PHP: stop is idempotent via finished_at
        }
        let now = Utc::now();
        round.is_playing = false;
        round.finished_at = Some(now);
        let round_id = round.id;
        let payload = events::round_payload(round);
        let _ = self.store.finish_round(round_id, now).await;
        let _ = self.store.set_room_playing(self.room.id, false).await;

        self.broadcast("RoundFinalizing", payload).await;
        self.broadcast_public_state().await;

        finalize::spawn(
            round_id,
            self.room.clone(),
            self.store.clone(),
            self.scores.clone(),
            self.hub.clone(),
            self.presence.clone(),
            self.photo_prefix.clone(),
        );

        self.phase = Phase::Finalizing {
            restart_at: Instant::now() + Duration::from_secs(self.room.pause_between_rounds as u64),
        };
        self.arm_timer();
    }

    async fn on_timer(&mut self) {
        match self.phase {
            Phase::Playing { .. } => {
                if self.round.as_ref().map(|r| r.is_playing).unwrap_or(false) {
                    self.on_track_end().await;
                } else {
                    self.park_timer();
                }
            }
            Phase::InterTrack { .. } => {
                if self.audience_empty().await {
                    self.stop().await;
                } else {
                    self.play_next_track().await;
                }
            }
            Phase::Finalizing { .. } => {
                if self.room.is_autostart
                    && self.hub.channel_conn_count(&events::room_channel(self.room.id)).await > 0
                {
                    self.start_round(None).await;
                } else {
                    self.phase = Phase::Idle;
                    self.round = None;
                    self.arm_timer();
                }
            }
            _ => {}
        }
    }

    // Presence-based audience check : a heartbeating player counts for ~90s,
    // so a momentary WS drop (metro) doesn't kill the round. Everyone truly
    // gone -> stop the chain instead of playing to an empty room; the
    // finalize broadcast resets the public track counter to 0.
    async fn audience_empty(&self) -> bool {
        if self
            .presence_grace_until
            .is_some_and(|until| Instant::now() < until)
        {
            return false;
        }
        self.presence
            .member_count(self.room.id, Utc::now().timestamp())
            .await
            == 0
    }

    async fn on_track_end(&mut self) {
        if self.audience_empty().await {
            self.stop().await;
            return;
        }
        let Some(round) = self.round.as_ref() else {
            return;
        };
        let is_last = round.current as usize >= round.tracks.len();

        let next_id = if !is_last {
            Some(round.tracks[round.current as usize])
        } else {
            None
        };
        let next_track = match next_id {
            Some(id) => self.store.load_track(id).await.ok().flatten(),
            None => None,
        };
        let finished_payload = self
            .current_track
            .as_ref()
            .map(events::track_playlist_payload);

        let payload = events::track_ended(
            round,
            self.room.pause_between_tracks,
            finished_payload,
            next_track.as_ref(),
            Utc::now(),
        );
        self.broadcast("TrackEnded", payload).await;

        if is_last {
            self.play_next_track().await;
        } else {
            self.phase = Phase::InterTrack {
                next_at: Instant::now()
                    + Duration::from_secs(self.room.pause_between_tracks as u64),
            };
            self.arm_timer();
        }
    }

    // ---- scoring ----------------------------------------------------------

    async fn check_answer(
        &mut self,
        user_id: i64,
        track_id: i64,
        text: String,
        client_words: Vec<String>,
        current_time: f64,
    ) -> CheckOutcome {
        let Some(round) = self.round.as_ref() else {
            return CheckOutcome::Bad;
        };
        if round.finished_at.is_some() || !self.is_current_track(track_id) {
            return CheckOutcome::Bad; // PHP: 409 round_ended/track_mismatch
        }

        let answers = self
            .current_track
            .as_ref()
            .map(|t| t.answers.clone())
            .unwrap_or_default();
        let hint = self.current_track.as_ref().and_then(|t| t.hint.clone());
        let total_answers = answers.len();
        let round_id = round.id;
        let room_id = self.room.id;
        let track_duration = self.room.track_duration;

        let req = CheckRequest {
            user_id,
            track_id,
            text,
            client_words,
            current_time,
            track_duration,
            hint,
            answers,
            total_track_answers: total_answers,
        };

        let backend = RedisBackend {
            round_id,
            store: self.scores.clone(),
            handle: tokio::runtime::Handle::current(),
        };
        let outcome = tokio::task::spawn_blocking(move || {
            let mut backend = backend;
            scoring::check(&req, &mut backend)
        })
        .await
        .unwrap_or(CheckOutcome::Bad);

        if let CheckOutcome::Good { answers, total_score, found_all } = &outcome {
            self.broadcast(
                "NewScore",
                events::new_score(room_id, user_id, track_id, answers, *total_score, current_time),
            )
            .await;
            if *found_all {
                let (name, photo) = self
                    .store
                    .players_info(&[user_id])
                    .await
                    .ok()
                    .and_then(|m| m.get(&user_id).map(|p| (p.name.clone(), p.photo.clone())))
                    .unwrap_or_else(|| (String::new(), None));
                let room_v = self.room_payload().await;
                let photo_url =
                    events::resolve_photo_url(&self.photo_prefix, photo.as_deref(), &name);
                self.broadcast(
                    "UserHasFoundAllTheAnswers",
                    events::user_found_all(&room_v, &name, user_id, &photo_url, current_time),
                )
                .await;
            }
        }
        outcome
    }

    fn is_current_track(&self, track_id: i64) -> bool {
        self.round
            .as_ref()
            .and_then(|r| r.tracks.get((r.current - 1).max(0) as usize))
            .map(|&id| id == track_id)
            .unwrap_or(false)
    }

    async fn track_listened(&mut self, user_id: i64, track_id: i64) {
        let Some(round) = self.round.as_ref() else {
            return;
        };
        if round.finished_at.is_some() || !round.tracks.contains(&track_id) {
            return;
        }
        let _ = self
            .scores
            .record_listened(
                round.id,
                user_id,
                &TrackRecord {
                    track_id,
                    answer_id: None,
                    response_time: None,
                    position: None,
                    score: None,
                },
            )
            .await;
    }

    // ---- broadcasts ---------------------------------------------------------

    async fn broadcast(&self, event: &str, data: Value) {
        self.hub
            .broadcast(&events::room_channel(self.room.id), event, data, None)
            .await;
    }

    async fn broadcast_public_state(&self) {
        let count = self
            .presence
            .member_count(self.room.id, Utc::now().timestamp())
            .await;
        let current = self.round.as_ref().map(|r| r.current).unwrap_or(0);
        let is_playing = self.round.as_ref().map(|r| r.is_playing).unwrap_or(false);
        self.hub
            .broadcast(
                &events::public_channel(self.room.id),
                "RoomPublicState",
                events::room_public_state(&self.room, count, current, is_playing),
                None,
            )
            .await;
    }

    async fn room_payload(&self) -> Value {
        json!({
            "id": self.room.id,
            "track_duration": self.room.track_duration,
            "pause_between_tracks": self.room.pause_between_tracks,
            "pause_between_rounds": self.room.pause_between_rounds,
            "tracks_by_round": self.room.tracks_by_round,
            "is_public": self.room.is_public,
            "is_playing": self.round.as_ref().map(|r| r.is_playing).unwrap_or(false),
        })
    }
}

struct RedisBackend {
    round_id: i64,
    store: Arc<dyn ScoreStore>,
    handle: tokio::runtime::Handle,
}

impl RedisBackend {
    fn run<F, T>(&self, fut: F) -> T
    where
        F: std::future::Future<Output = T>,
    {
        self.handle.block_on(fut)
    }
}

impl scoring::ScoreBackend for RedisBackend {
    fn found_answer_ids(&mut self, user_id: i64, track_id: i64) -> std::collections::HashSet<i64> {
        let store = self.store.clone();
        let round_id = self.round_id;
        self.run(async move { store.found_answer_ids(round_id, user_id, track_id).await })
            .into_iter()
            .collect()
    }

    fn record_track_details(
        &mut self,
        user_id: i64,
        track_id: i64,
        response_time: Option<f64>,
        score: Option<f64>,
        answer_id: Option<i64>,
    ) -> bool {
        let record = TrackRecord {
            track_id,
            answer_id,
            response_time,
            position: None,
            score,
        };
        let store = self.store.clone();
        let round_id = self.round_id;
        match answer_id {
            Some(_) => self.run(async move {
                store.record_answer(round_id, user_id, &record).await
            }),
            None => {
                self.run(async move {
                    store.record_listened(round_id, user_id, &record).await
                });
                true
            }
        }
    }

    fn players_who_found(&mut self, _track_id: i64, answer_id: i64, exclude_user: i64) -> usize {
        let store = self.store.clone();
        let round_id = self.round_id;
        self.run(async move {
            store
                .players_who_found(round_id, answer_id, Some(exclude_user))
                .await
        }) as usize
    }

    fn add_score(&mut self, user_id: i64, score: f64) {
        let store = self.store.clone();
        let round_id = self.round_id;
        self.run(async move { store.add_score(round_id, user_id, score).await })
    }

    fn get_user_score(&mut self, user_id: i64) -> f64 {
        let store = self.store.clone();
        let round_id = self.round_id;
        self.run(async move { store.user_score(round_id, user_id).await })
    }

    fn count_found_for_user(&mut self, user_id: i64, track_id: i64) -> usize {
        let store = self.store.clone();
        let round_id = self.round_id;
        self.run(async move {
            store
                .count_found_for_user(round_id, user_id, track_id)
                .await
        })
    }
}
