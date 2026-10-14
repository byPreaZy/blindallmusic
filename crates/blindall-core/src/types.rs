use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackAnswer {
    pub id: i64,
    pub type_name: String,
    #[serde(default)]
    pub svg_icon: Option<String>,
    pub value: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    pub score: f64,
}

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

#[derive(Debug, Clone)]
pub struct PodiumEntry {
    pub user_id: i64,
    pub total: f64,
    pub team_id: Option<i64>,
}

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
