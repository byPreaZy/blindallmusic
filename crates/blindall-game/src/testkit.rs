use crate::store::{
    GameStore, GameTrack, PlayerInfo, PresenceStore, Result, RoomConfig,
    RoundRow, ScoreStore, StoreError,
};
use async_trait::async_trait;
use blindall_core::types::Standing;
use blindall_redis::scores::TrackRecord;
use chrono::{DateTime, Utc};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

#[derive(Default)]
pub struct FakeGameStore {
    pub room: Mutex<Option<RoomConfig>>,
    pub tracks: Mutex<HashMap<i64, GameTrack>>,
    pub picked: Mutex<Vec<i64>>,
    pub rounds: Mutex<HashMap<i64, RoundRow>>,
    pub next_round_id: Mutex<i64>,
    pub room_playing: Mutex<HashMap<i64, bool>>,
    pub probe_ok: Mutex<bool>,
    pub standings_written: Mutex<Vec<Standing>>,
    pub total_scores: Mutex<Vec<(i64, i64, f64)>>,
    pub required: Mutex<HashMap<i64, usize>>,
}

impl FakeGameStore {
    pub fn insert_round(&self, round: RoundRow) {
        self.rounds.lock().unwrap().insert(round.id, round);
    }

    pub fn new(room: RoomConfig) -> Self {
        let s = Self {
            probe_ok: Mutex::new(true),
            next_round_id: Mutex::new(1),
            ..Default::default()
        };
        *s.room.lock().unwrap() = Some(room);
        s
    }
}

fn cfg(room: &Mutex<Option<RoomConfig>>, id: i64) -> Result<RoomConfig> {
    room.lock()
        .unwrap()
        .clone()
        .filter(|r| r.id == id)
        .ok_or(StoreError::NotFound("room"))
}

#[async_trait]
impl GameStore for FakeGameStore {
    async fn room_config(&self, room_id: i64) -> Result<RoomConfig> {
        cfg(&self.room, room_id)
    }
    async fn pick_tracks(&self, _room: &RoomConfig) -> Result<Vec<i64>> {
        Ok(self.picked.lock().unwrap().clone())
    }
    async fn create_round(
        &self,
        room_id: i64,
        user_id: Option<i64>,
        tracks: &[i64],
    ) -> Result<i64> {
        let _ = user_id;
        let mut next = self.next_round_id.lock().unwrap();
        let id = *next;
        *next += 1;
        self.rounds.lock().unwrap().insert(
            id,
            RoundRow {
                id,
                room_id,
                tracks: tracks.to_vec(),
                current: 0,
                is_playing: true,
                finished_at: None,
                current_track_started_at: None,
            },
        );
        Ok(id)
    }
    async fn advance_round(&self, round_id: i64, started_at: DateTime<Utc>) -> Result<()> {
        let mut rounds = self.rounds.lock().unwrap();
        let r = rounds.get_mut(&round_id).ok_or(StoreError::NotFound("round"))?;
        r.current += 1;
        r.current_track_started_at = Some(started_at);
        Ok(())
    }
    async fn finish_round(&self, round_id: i64, at: DateTime<Utc>) -> Result<()> {
        let mut rounds = self.rounds.lock().unwrap();
        if let Some(r) = rounds.get_mut(&round_id) {
            r.is_playing = false;
            r.finished_at = Some(at);
        }
        Ok(())
    }
    async fn set_room_playing(&self, room_id: i64, playing: bool) -> Result<()> {
        self.room_playing.lock().unwrap().insert(room_id, playing);
        Ok(())
    }
    async fn load_track(&self, track_id: i64) -> Result<Option<GameTrack>> {
        Ok(self.tracks.lock().unwrap().get(&track_id).cloned())
    }
    async fn probe_audio(&self, _url: &str) -> bool {
        *self.probe_ok.lock().unwrap()
    }
    async fn delete_track(
        &self,
        track_id: i64,
        _actor: Option<String>,
    ) -> Result<Vec<crate::store::TrackDeletionNotice>> {
        self.tracks.lock().unwrap().remove(&track_id);
        Ok(vec![])
    }
    async fn load_active_rounds(&self) -> Result<Vec<RoundRow>> {
        Ok(self
            .rounds
            .lock()
            .unwrap()
            .values()
            .filter(|r| r.is_playing || r.finished_at.is_none())
            .cloned()
            .collect())
    }
    async fn round_row(&self, round_id: i64) -> Result<Option<RoundRow>> {
        Ok(self.rounds.lock().unwrap().get(&round_id).cloned())
    }
    async fn players_info(&self, ids: &[i64]) -> Result<HashMap<i64, PlayerInfo>> {
        Ok(ids
            .iter()
            .map(|&id| {
                (
                    id,
                    PlayerInfo {
                        id,
                        name: format!("user{id}"),
                        photo: None,
                        elo: 1500,
                        is_guest: false,
                        team_id: None,
                        team_name: None,
                        team_photo: None,
                        level: 1,
                        total_xp: 0,
                        current_xp: 0,
                        xp_for_next_level: 100,
                        rounds_played_count: 0,
                        public_rounds_played_count: 0,
                        score_public_rooms: 0,
                        months_seniority: 0,
                        consecutive_days_streak: 0,
                        rooms_created_count: 0,
                        playlists_created_count: 0,
                        tracks_liked_count: 0,
                    },
                )
            })
            .collect())
    }
    async fn write_standings(
        &self,
        standings: &[Standing],
        _elo: &[(i64, i64)],
    ) -> Result<()> {
        self.standings_written
            .lock()
            .unwrap()
            .extend(standings.iter().cloned().collect::<Vec<_>>());
        Ok(())
    }
    async fn bump_total_score(
        &self,
        room_id: i64,
        user_id: i64,
        _team: Option<i64>,
        score: f64,
    ) -> Result<()> {
        self.total_scores
            .lock()
            .unwrap()
            .push((room_id, user_id, score));
        Ok(())
    }
    async fn placement_rounds_played(&self, _u: i64) -> Result<i64> {
        Ok(0)
    }
    async fn win_streak(&self, _r: i64, _u: i64, _b: i64) -> Result<i64> {
        Ok(0)
    }
    async fn round_has_standings(&self, round_id: i64) -> Result<bool> {
        Ok(self
            .standings_written
            .lock()
            .unwrap()
            .iter()
            .any(|s| s.round_id == round_id))
    }
    async fn required_answers_per_track(&self, _t: &[i64]) -> Result<HashMap<i64, usize>> {
        Ok(self.required.lock().unwrap().clone())
    }
    async fn scores_from_db(&self, _r: i64) -> Result<HashMap<i64, f64>> {
        Ok(HashMap::new())
    }
    async fn tracks_history_from_db(
        &self,
        _r: i64,
    ) -> Result<HashMap<i64, Vec<blindall_core::types::TrackHistoryEntry>>> {
        Ok(HashMap::new())
    }
    async fn has_legacy_scores(&self, _r: i64) -> Result<bool> {
        Ok(false)
    }
    async fn delete_round_scores(&self, _r: i64) -> Result<()> {
        Ok(())
    }
    async fn recalc_user_level(&self, _u: i64) -> Result<Option<serde_json::Value>> {
        Ok(None)
    }
}

#[derive(Default)]
pub struct FakeScoreStore {
    pub found: Mutex<HashMap<(i64, i64, i64), HashSet<i64>>>,
    pub answer_players: Mutex<HashMap<(i64, i64), HashSet<i64>>>,
    pub scores: Mutex<HashMap<(i64, i64), f64>>,
    pub history: Mutex<HashMap<(i64, i64), Vec<TrackRecord>>>,
    pub players: Mutex<HashMap<i64, HashSet<i64>>>,
    pub cleaned: Mutex<Vec<i64>>,
}

#[async_trait]
impl ScoreStore for FakeScoreStore {
    async fn found_answer_ids(&self, r: i64, u: i64, t: i64) -> Vec<i64> {
        self.found
            .lock()
            .unwrap()
            .get(&(r, u, t))
            .map(|s| s.iter().copied().collect())
            .unwrap_or_default()
    }
    async fn players_who_found(&self, r: i64, a: i64, exclude: Option<i64>) -> i64 {
        let set = self.answer_players.lock().unwrap();
        let mut n = set.get(&(r, a)).map(|s| s.len() as i64).unwrap_or(0);
        if n > 0
            && exclude
                .map(|u| set.get(&(r, a)).map(|s| s.contains(&u)).unwrap_or(false))
                .unwrap_or(false)
        {
            n -= 1;
        }
        n
    }
    async fn record_answer(&self, r: i64, u: i64, rec: &TrackRecord) -> bool {
        let Some(a) = rec.answer_id else {
            return false;
        };
        let newly = self
            .found
            .lock()
            .unwrap()
            .entry((r, u, rec.track_id))
            .or_default()
            .insert(a);
        if !newly {
            return false;
        }
        self.answer_players
            .lock()
            .unwrap()
            .entry((r, a))
            .or_default()
            .insert(u);
        self.players.lock().unwrap().entry(r).or_default().insert(u);
        self.history
            .lock()
            .unwrap()
            .entry((r, u))
            .or_default()
            .push(rec.clone());
        true
    }
    async fn record_listened(&self, r: i64, u: i64, rec: &TrackRecord) {
        self.players.lock().unwrap().entry(r).or_default().insert(u);
        self.history
            .lock()
            .unwrap()
            .entry((r, u))
            .or_default()
            .push(rec.clone());
    }
    async fn add_score(&self, r: i64, u: i64, s: f64) {
        *self.scores.lock().unwrap().entry((r, u)).or_default() += s;
    }
    async fn user_score(&self, r: i64, u: i64) -> f64 {
        self.scores.lock().unwrap().get(&(r, u)).copied().unwrap_or(0.0)
    }
    async fn count_found_for_user(&self, r: i64, u: i64, t: i64) -> usize {
        self.found
            .lock()
            .unwrap()
            .get(&(r, u, t))
            .map(|s| s.len())
            .unwrap_or(0)
    }
    async fn all_scores(&self, r: i64) -> Vec<(i64, f64)> {
        self.scores
            .lock()
            .unwrap()
            .iter()
            .filter(|((rr, _), _)| *rr == r)
            .map(|((_, u), s)| (*u, *s))
            .collect()
    }
    async fn all_tracks_history(&self, r: i64) -> Vec<(i64, Vec<TrackRecord>)> {
        self.history
            .lock()
            .unwrap()
            .iter()
            .filter(|((rr, _), _)| *rr == r)
            .map(|((_, u), h)| (*u, h.clone()))
            .collect()
    }
    async fn cleanup(&self, r: i64) {
        self.cleaned.lock().unwrap().push(r);
        self.found.lock().unwrap().retain(|(rr, _, _), _| *rr != r);
        self.answer_players.lock().unwrap().retain(|(rr, _), _| *rr != r);
        self.scores.lock().unwrap().retain(|(rr, _), _| *rr != r);
        self.history.lock().unwrap().retain(|(rr, _), _| *rr != r);
        self.players.lock().unwrap().remove(&r);
    }
}

#[derive(Default)]
pub struct FakePresenceStore {
    pub counts: Mutex<HashMap<i64, i64>>,
}

#[async_trait]
impl PresenceStore for FakePresenceStore {
    async fn member_count(&self, room_id: i64, _now: i64) -> i64 {
        self.counts.lock().unwrap().get(&room_id).copied().unwrap_or(0)
    }
}

pub fn fake_track(id: i64, provider: &str) -> GameTrack {
    GameTrack {
        id,
        provider: provider.to_string(),
        preview_url: Some(format!("https://cdn.test/{id}.mp3")),
        audio: Some(format!("https://cdn.test/{id}.mp3")),
        hint: None,
        artwork_url: Some(format!("https://cdn.test/{id}.jpg")),
        upvotes: 0,
        downvotes: 0,
        answers: vec![blindall_core::types::TrackAnswer {
            id: id * 10,
            type_name: "Titre".to_string(),
            svg_icon: None,
            value: format!("song {id}"),
            aliases: vec![],
            score: 1.0,
        }],
    }
}

pub fn fake_room(id: i64) -> RoomConfig {
    RoomConfig {
        id,
        name: format!("room-{id}"),
        track_duration: 30,
        pause_between_tracks: 3,
        pause_between_rounds: 10,
        tracks_by_round: 2,
        is_public: true,
        is_autostart: true,
        is_random: false,
        discord_webhook_url: None,
    }
}
