//! Persistence boundary for the game engine. `blindall-db` provides the
//! sqlx implementation; tests use fakes. Types mirror the DB rows.

use blindall_core::types::TrackAnswer;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct RoomConfig {
    pub id: i64,
    pub name: String,
    pub track_duration: u32,
    pub pause_between_tracks: u32,
    pub pause_between_rounds: u32,
    pub tracks_by_round: u32,
    pub is_public: bool,
    pub is_autostart: bool,
    pub is_random: bool,
    pub discord_webhook_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct GameTrack {
    pub id: i64,
    pub provider: String,
    pub preview_url: Option<String>,
    pub audio: Option<String>,
    pub hint: Option<String>,
    /// Album art for the revealed playlist card (`artwork_url` column).
    pub artwork_url: Option<String>,
    /// Vote totals (`Track::upvotes`/`downvotes` accessors) for playlist cards.
    pub upvotes: i64,
    pub downvotes: i64,
    pub answers: Vec<TrackAnswer>,
}

impl GameTrack {
    /// `provider === 'youtube' || 'local'` — those skip the audio probe.
    pub fn skips_probe(&self) -> bool {
        self.provider == "youtube" || self.provider == "local"
    }
}

/// An unfinished round as read back at boot for recovery.
#[derive(Debug, Clone)]
pub struct RoundRow {
    pub id: i64,
    pub room_id: i64,
    /// JSON array of track ids.
    pub tracks: Vec<i64>,
    /// 1-based index of the *next* slot (PHP semantics: tracks[current-1]
    /// is the playing track).
    pub current: i64,
    pub is_playing: bool,
    pub finished_at: Option<DateTime<Utc>>,
    pub current_track_started_at: Option<DateTime<Utc>>,
}

/// Player row needed by finalization/presence payloads.
#[derive(Debug, Clone)]
pub struct PlayerInfo {
    pub id: i64,
    pub name: String,
    pub photo: Option<String>,
    pub elo: i64,
    pub is_guest: bool,
    pub team_id: Option<i64>,
    /// `teams.name` — PHP serializes `user.team` as `{id, name}`.
    pub team_name: Option<String>,
    /// `teams.photo_path` — `teams_podium` serializes `team.photo` too.
    pub team_photo: Option<String>,
    /// `user_levels` row — level bubble + XP metrics on the room roster.
    pub level: i64,
    pub total_xp: i64,
    pub current_xp: i64,
    pub xp_for_next_level: i64,
    pub rounds_played_count: i64,
    /// `RoundStanding::where('is_elo_counted')->count()` — ELO "N/A" flag.
    pub public_rounds_played_count: i64,
    pub score_public_rooms: i64,
    pub months_seniority: i64,
    pub consecutive_days_streak: i64,
    pub rooms_created_count: i64,
    pub playlists_created_count: i64,
    pub tracks_liked_count: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("not found: {0}")]
    NotFound(&'static str),
    #[error("store error: {0}")]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// The score/presence operations the actor + finalizer need. The real
/// impls are `blindall_redis::{RoundScoreStore, RoomPresenceStore}`;
/// tests provide fakes — this is what makes the actor unit-testable
/// without a live Redis.
#[async_trait::async_trait]
pub trait ScoreStore: Send + Sync {
    async fn found_answer_ids(&self, round_id: i64, user_id: i64, track_id: i64)
        -> Vec<i64>;
    async fn players_who_found(&self, round_id: i64, answer_id: i64, exclude: Option<i64>)
        -> i64;
    async fn record_answer(
        &self,
        round_id: i64,
        user_id: i64,
        record: &blindall_redis::scores::TrackRecord,
    ) -> bool;
    async fn record_listened(
        &self,
        round_id: i64,
        user_id: i64,
        record: &blindall_redis::scores::TrackRecord,
    );
    async fn add_score(&self, round_id: i64, user_id: i64, score: f64);
    async fn user_score(&self, round_id: i64, user_id: i64) -> f64;
    async fn count_found_for_user(&self, round_id: i64, user_id: i64, track_id: i64) -> usize;
    async fn all_scores(&self, round_id: i64) -> Vec<(i64, f64)>;
    async fn all_tracks_history(&self, round_id: i64) -> Vec<(i64, Vec<blindall_redis::scores::TrackRecord>)>;
    async fn cleanup(&self, round_id: i64);
}

#[async_trait::async_trait]
pub trait PresenceStore: Send + Sync {
    async fn member_count(&self, room_id: i64, now_unix: i64) -> i64;
}

#[async_trait::async_trait]
impl ScoreStore for blindall_redis::RoundScoreStore {
    async fn found_answer_ids(&self, r: i64, u: i64, t: i64) -> Vec<i64> {
        Self::found_answer_ids(self, r, u, t).await.unwrap_or_default()
    }
    async fn players_who_found(&self, r: i64, a: i64, e: Option<i64>) -> i64 {
        Self::players_who_found(self, r, a, e).await.unwrap_or(0)
    }
    async fn record_answer(
        &self,
        r: i64,
        u: i64,
        rec: &blindall_redis::scores::TrackRecord,
    ) -> bool {
        Self::record_answer(self, r, u, rec).await.unwrap_or(false)
    }
    async fn record_listened(
        &self,
        r: i64,
        u: i64,
        rec: &blindall_redis::scores::TrackRecord,
    ) {
        let _ = Self::record_listened(self, r, u, rec).await;
    }
    async fn add_score(&self, r: i64, u: i64, s: f64) {
        let _ = Self::add_score(self, r, u, s).await;
    }
    async fn user_score(&self, r: i64, u: i64) -> f64 {
        Self::user_score(self, r, u).await.unwrap_or(0.0)
    }
    async fn count_found_for_user(&self, r: i64, u: i64, t: i64) -> usize {
        Self::count_found_for_user(self, r, u, t).await.unwrap_or(0)
    }
    async fn all_scores(&self, r: i64) -> Vec<(i64, f64)> {
        Self::all_scores(self, r).await.unwrap_or_default()
    }
    async fn all_tracks_history(&self, r: i64) -> Vec<(i64, Vec<blindall_redis::scores::TrackRecord>)> {
        Self::all_tracks_history(self, r).await.unwrap_or_default()
    }
    async fn cleanup(&self, r: i64) {
        let _ = Self::cleanup(self, r).await;
    }
}

#[async_trait::async_trait]
impl PresenceStore for blindall_redis::RoomPresenceStore {
    async fn member_count(&self, room_id: i64, now_unix: i64) -> i64 {
        Self::member_count(self, room_id, now_unix).await.unwrap_or(0)
    }
}

/// `Track::audio` accessor port — deezer/audius previews are expiring
/// signed URLs fetched live; other providers use the stored `preview_url`.
/// `blindall-providers` supplies the real implementation; the default
/// passthrough matches the `default => preview_url` arm.
#[async_trait::async_trait]
pub trait PreviewResolver: Send + Sync {
    async fn resolve(
        &self,
        provider: &str,
        provider_id: &str,
        preview_url: &str,
    ) -> Option<String>;
}

#[derive(Default)]
pub struct PassthroughResolver;

#[async_trait::async_trait]
impl PreviewResolver for PassthroughResolver {
    async fn resolve(
        &self,
        _provider: &str,
        _provider_id: &str,
        preview_url: &str,
    ) -> Option<String> {
        Some(preview_url.to_string())
    }
}

/// Everything the engine must persist or read — no game logic here.
/// One `TrackDeleted` notification created by `GameStore::delete_track` —
/// enough for callers to emit Laravel's `BroadcastNotificationCreated`.
#[derive(Debug, Clone)]
pub struct TrackDeletionNotice {
    pub user_id: i64,
    pub notification_id: String,
    pub data: serde_json::Value,
}

#[async_trait::async_trait]
pub trait GameStore: Send + Sync {
    async fn room_config(&self, room_id: i64) -> Result<RoomConfig>;
    /// Room's track ids: all playlists' tracks (random subset or first N,
    /// per `is_random`/`tracks_by_round` — same intent as StartRound).
    async fn pick_tracks(&self, room: &RoomConfig) -> Result<Vec<i64>>;
    async fn create_round(&self, room_id: i64, user_id: Option<i64>, tracks: &[i64])
        -> Result<i64>;
    /// Increment `rounds.current` + stamp `current_track_started_at`
    /// (mirrors `$round->increment('current')` + timestamp update).
    async fn advance_round(&self, round_id: i64, started_at: DateTime<Utc>) -> Result<()>;
    async fn finish_round(&self, round_id: i64, at: DateTime<Utc>) -> Result<()>;
    async fn set_room_playing(&self, room_id: i64, playing: bool) -> Result<()>;
    async fn load_track(&self, track_id: i64) -> Result<Option<GameTrack>>;
    /// `Http::retry(3,100)->timeout(3)->get($audioUrl)` — true if 2xx.
    async fn probe_audio(&self, audio_url: &str) -> bool;
    /// `ProcessDeletedTrack` — delete the track's answers then the track,
    /// notify private-playlist moderators, and (production only) post to
    /// the public rooms' Discord webhooks. `actor` is the deleting user's
    /// display name; `None` means automatic removal (dead preview).
    /// Returns the created notifications so callers can push
    /// `BroadcastNotificationCreated` on `private-user:{id}`.
    async fn delete_track(
        &self,
        track_id: i64,
        actor: Option<String>,
    ) -> Result<Vec<TrackDeletionNotice>>;
    /// Rounds unfinished at boot (`is_playing` or `finished_at IS NULL`).
    async fn load_active_rounds(&self) -> Result<Vec<RoundRow>>;
    /// Single round row by id.
    async fn round_row(&self, round_id: i64) -> Result<Option<RoundRow>>;
    async fn players_info(&self, user_ids: &[i64]) -> Result<HashMap<i64, PlayerInfo>>;
    /// Insert `round_standings` rows + update user elos (transactional).
    async fn write_standings(
        &self,
        standings: &[blindall_core::types::Standing],
        elo_updates: &[(i64, i64)],
    ) -> Result<()>;
    /// `total_scores` upsert+increment for a user (and team via caller).
    async fn bump_total_score(&self, room_id: i64, user_id: i64, team_id: Option<i64>, score: f64)
        -> Result<()>;
    /// is_elo_counted standings count for a user (placement rounds).
    async fn placement_rounds_played(&self, user_id: i64) -> Result<i64>;
    /// `calculateWinStreak` — consecutive position=1 in this room's past rounds.
    async fn win_streak(&self, room_id: i64, user_id: i64, before_round_id: i64) -> Result<i64>;
    /// Idempotence guard (PHP checks standings()->exists() under lock —
    /// implementations MUST lock the round row inside `write_standings`).
    async fn round_has_standings(&self, round_id: i64) -> Result<bool>;
    /// required answers per track for performance metrics.
    async fn required_answers_per_track(&self, track_ids: &[i64]) -> Result<HashMap<i64, usize>>;
    /// Legacy fallback when Redis has no scores (`scoresFromDatabase`):
    /// `scores` table grouped by user.
    async fn scores_from_db(&self, round_id: i64) -> Result<HashMap<i64, f64>>;
    /// Legacy fallback for tracks history (`tracksHistoryFromDatabase`).
    async fn tracks_history_from_db(
        &self,
        round_id: i64,
    ) -> Result<HashMap<i64, Vec<blindall_core::types::TrackHistoryEntry>>>;
    /// `round->scores()->exists()`.
    async fn has_legacy_scores(&self, round_id: i64) -> Result<bool>;
    /// `round->scores()->delete()`.
    async fn delete_round_scores(&self, round_id: i64) -> Result<()>;
    /// `UpdateUserLevel` job — recompute level and persist (db impl uses
    /// `blindall_db::levels`). Called only for public rooms. Returns the
    /// `UserLevelUpdated` payload (`None` for guests, PHP early-return).
    async fn recalc_user_level(&self, user_id: i64) -> Result<Option<serde_json::Value>>;
}
