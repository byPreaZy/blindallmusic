//! Shared domain types. Field names mirror the PHP payloads (camelCase at
//! the serialization boundary is handled by the api/realtime layers).

use serde::{Deserialize, Serialize};

/// Mirrors `track_answers` + its `type.name` and `aliases` JSON column.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackAnswer {
    pub id: i64,
    /// `answer_types.name` (e.g. "Titre", "Artiste") — used in NewScore payload.
    pub type_name: String,
    /// `answer_types.svg_icon` — rendered in the found-answer chip
    /// (`type.svg_icon` on PHP's serialized `good_answers`).
    #[serde(default)]
    pub svg_icon: Option<String>,
    /// Raw value as stored (may contain parentheses stripped at match time).
    pub value: String,
    /// Additional accepted spellings (`track_answers.aliases`).
    #[serde(default)]
    pub aliases: Vec<String>,
    /// Base points (decimal(3,1) in DB).
    pub score: f64,
}

/// One entry of the per-user Redis tracks history
/// (`round:{id}:tracks:{userId}` JSON).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrackHistoryEntry {
    pub track_id: i64,
    #[serde(default)]
    pub answer_id: Option<i64>,
    #[serde(default)]
    pub response_time: Option<f64>,
    #[serde(default)]
    pub position: Option<i64>,
    #[serde(default)]
    pub score: Option<f64>,
}

/// A podium entry as produced by `Round::usersPodium` /
/// `RoundFinalizationService::buildPodium`.
#[derive(Debug, Clone)]
pub struct PodiumEntry {
    pub user_id: i64,
    pub total: f64,
    pub team_id: Option<i64>,
}

/// One row of `round_standings` (JSON `tracks_history` stays structured).
#[derive(Debug, Clone, Serialize)]
pub struct Standing {
    pub round_id: i64,
    pub room_id: i64,
    pub user_id: i64,
    pub team_id: Option<i64>,
    pub position: i64,
    pub total_score: f64,
    pub elo_before: i64,
    pub elo_after: i64,
    pub elo_change: i64,
    pub is_elo_counted: bool,
    pub average_response_time: Option<f64>,
    pub fast_answers_count: i64,
    pub total_answers_count: i64,
    pub tracks_history: Vec<TrackHistoryEntry>,
    pub win_streak: i64,
}

#[derive(Debug, Clone, Default)]
pub struct PerformanceMetrics {
    pub average_response_time: Option<f64>,
    pub fast_answers_count: i64,
    pub total_answers_count: i64,
}
