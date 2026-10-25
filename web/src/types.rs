#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use serde_json::Value;

fn default_level() -> i64 {
    1
}

fn default_xp_next() -> i64 {
    100
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct User {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub photo_path: Option<String>,
    #[serde(default)]
    pub elo: i64,
    #[serde(default)]
    pub is_guest: bool,
    #[serde(default)]
    pub is_administrator: bool,
    #[serde(default)]
    pub is_public_moderator: bool,
    #[serde(default)]
    pub team_id: Option<i64>,
    #[serde(default)]
    pub photo: Option<String>,
    #[serde(default)]
    pub team: Option<Value>,
    #[serde(default = "default_level")]
    pub level: i64,
    #[serde(default)]
    pub current_xp: i64,
    #[serde(default = "default_xp_next")]
    pub xp_for_next_level: i64,
    #[serde(default)]
    pub total_xp: i64,
    #[serde(default)]
    pub level_metrics: Option<Value>,
    #[serde(default)]
    pub donor_perks: Vec<String>,
    #[serde(default)]
    pub is_supporter: bool,
    #[serde(default)]
    pub plan: Option<String>,
    #[serde(default)]
    pub custom_tag: String,
    #[serde(default)]
    pub portal_url: String,
    #[serde(default)]
    pub can: UserCan,
}

/// Mirrors the Laravel `Gate::abilities()` map exposed by /auth/me so the
/// client can hide actions the server would reject.
#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
pub struct UserCan {
    #[serde(default, rename = "sendSuggestion")]
    pub send_suggestion: bool,
    #[serde(default, rename = "changeRoomPicture")]
    pub change_room_picture: bool,
    #[serde(default, rename = "changeTeamPicture")]
    pub change_team_picture: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: User,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct RoomUserTeam {
    pub id: i64,
    #[serde(default)]
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct RoomUserLevel {
    #[serde(default)]
    pub level: i64,
    #[serde(default)]
    pub rounds_played_count: i64,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct RoomUser {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub photo: Option<String>,
    #[serde(default)]
    pub elo: i64,
    #[serde(default)]
    pub is_guest: bool,
    #[serde(default)]
    pub team: Option<RoomUserTeam>,
    #[serde(default, rename = "userLevel")]
    pub user_level: Option<RoomUserLevel>,
    #[serde(default)]
    pub public_rounds_played_count: i64,
    #[serde(default)]
    pub donor_perks: Vec<String>,
    #[serde(default)]
    pub is_supporter: bool,
    #[serde(default)]
    pub role: String,
    #[serde(default)]
    pub custom_tag: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct RoomSummary {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub slug: String,
    #[serde(default)]
    pub is_playing: bool,
    #[serde(default)]
    pub member_count: i64,
    #[serde(default)]
    pub tracks_by_round: i64,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct RoomConfig {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub track_duration: f64,
    #[serde(default)]
    pub pause_between_tracks: f64,
    #[serde(default)]
    pub pause_between_rounds: f64,
    #[serde(default)]
    pub tracks_by_round: i64,
    #[serde(default)]
    pub is_public: bool,
    #[serde(default)]
    pub is_autostart: bool,
    #[serde(default)]
    pub is_playing: bool,
    #[serde(default)]
    pub member_count: i64,
    #[serde(default)]
    pub can_edit: bool,
    #[serde(default)]
    pub slug: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub photo: Option<String>,
    #[serde(default)]
    pub is_chat_active: bool,
    #[serde(default)]
    pub is_random: bool,
    #[serde(default)]
    pub tracks_count: Option<i64>,
    #[serde(default)]
    pub rounds_count: Option<i64>,
    #[serde(default)]
    pub is_bookmarked: bool,
    #[serde(default)]
    pub is_active: bool,
    #[serde(default)]
    pub moderators: Vec<Value>,
    #[serde(default)]
    pub owner: Option<Value>,
    #[serde(default)]
    pub category: Option<Value>,
    #[serde(default)]
    pub has_password: bool,
    #[serde(default)]
    pub is_featured: bool,
    #[serde(default)]
    pub color: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TrackAnswer {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub order: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Track {
    pub id: i64,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub preview_url: Option<String>,
    #[serde(default)]
    pub audio: Option<String>,
    #[serde(default)]
    pub hint: Option<String>,
    #[serde(default)]
    pub answers: Vec<TrackAnswer>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Round {
    pub id: i64,
    #[serde(default)]
    pub room_id: i64,
    #[serde(default)]
    pub current: i64,
    #[serde(default)]
    pub is_playing: bool,
    #[serde(default)]
    pub tracks: Vec<i64>,
    #[serde(default)]
    pub current_track_started_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct JoinedPayload {
    #[serde(default)]
    pub round: Option<Round>,
    #[serde(default)]
    pub track: Option<Track>,
    #[serde(default, rename = "startTime")]
    pub start_time: f64,
    #[serde(default)]
    pub scores: Value,
    #[serde(default)]
    pub users: Vec<Value>,
    #[serde(default, rename = "roundId")]
    pub round_id: Option<i64>,
    #[serde(default, rename = "playedTracks")]
    pub played_tracks: Vec<Value>,
    #[serde(default)]
    pub room: Option<Value>,
    #[serde(default)]
    pub party: bool,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GoodAnswer {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub order: Option<i64>,
    #[serde(default, rename = "speedBonus")]
    pub speed_bonus: Option<bool>,
    #[serde(default, rename = "type")]
    pub answer_type: Option<GoodAnswerType>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GoodAnswerType {
    #[serde(default)]
    pub svg_icon: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CheckMessage {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub body: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct CheckResponse {
    #[serde(default)]
    pub words: Vec<String>,
    #[serde(default)]
    pub good_answers: Vec<GoodAnswer>,
    #[serde(default)]
    pub total_score: Option<f64>,
    #[serde(default)]
    pub message: Option<CheckMessage>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChatMessage {
    pub id: i64,
    #[serde(default)]
    pub room_id: i64,
    #[serde(default)]
    pub user_id: i64,
    #[serde(default)]
    pub user_name: Option<String>,
    #[serde(default)]
    pub user_photo: Option<String>,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub user: Option<Value>,
}

// ---- playlists -------------------------------------------------------------

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PlaylistSummary {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub user_id: i64,
    #[serde(default)]
    pub is_public: bool,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub owner_name: String,
    #[serde(default)]
    pub total_tracks: i64,
    #[serde(default)]
    pub moderators: Vec<IdName>,
    #[serde(default)]
    pub updated_at: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct IdName {
    pub id: i64,
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PlaylistDetail {
    pub playlist: PlaylistFull,
    #[serde(default)]
    pub can_edit: bool,
    #[serde(default)]
    pub owned: bool,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PlaylistFull {
    pub id: i64,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub user_id: i64,
    #[serde(default)]
    pub is_public: bool,
    #[serde(default)]
    pub moderators: Vec<IdName>,
    #[serde(default)]
    pub rooms: Vec<PlaylistRoom>,
    #[serde(default)]
    pub difficulties: Vec<DifficultyCount>,
    #[serde(default)]
    pub total_tracks: i64,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PlaylistRoom {
    pub id: i64,
    pub slug: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct DifficultyCount {
    pub dificulty: i64,
    pub count: i64,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct PlaylistTrack {
    pub id: i64,
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub provider_id: String,
    #[serde(default)]
    pub provider_url: Option<String>,
    #[serde(default)]
    pub preview_url: String,
    #[serde(default)]
    pub artwork_url: String,
    #[serde(default)]
    pub dificulty: Option<i64>,
    #[serde(default)]
    pub hint: Option<String>,
    #[serde(default)]
    pub answers: Vec<TrackAnswerRow>,
    #[serde(default)]
    pub up_votes: i64,
    #[serde(default)]
    pub down_votes: i64,
    /// Reason => downvote count for this track (Laravel downvote_breakdown).
    #[serde(default)]
    pub downvote_breakdown: std::collections::BTreeMap<String, i64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct TrackAnswerRow {
    pub id: i64,
    #[serde(default)]
    pub answer_type_id: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub score: f64,
    #[serde(default)]
    pub aliases: Option<Value>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ProviderResult {
    pub provider: String,
    #[serde(default)]
    pub provider_id: String,
    #[serde(default)]
    pub provider_url: Option<String>,
    #[serde(default)]
    pub provider_popularity: i64,
    #[serde(default)]
    pub artist_name: Option<String>,
    #[serde(default)]
    pub track_name: String,
    #[serde(default)]
    pub album_name: Option<String>,
    #[serde(default)]
    pub preview_url: Option<String>,
    #[serde(default)]
    pub artwork_url: Option<String>,
    #[serde(default)]
    pub added: Option<i64>,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct Category {
    pub id: i64,
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct RoomEdit {
    pub room: Value,
    #[serde(default)]
    pub owned: bool,
}
