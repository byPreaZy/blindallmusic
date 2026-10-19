use chrono::{DateTime, Utc};
use sqlx::Row;
use sqlx::postgres::PgRow;

fn get<T>(r: &PgRow, name: &str) -> T
where
    T: for<'r> sqlx::decode::Decode<'r, sqlx::postgres::Postgres>
        + sqlx::types::Type<sqlx::postgres::Postgres>,
    for<'r> &'r str: sqlx::ColumnIndex<PgRow>,
{
    r.get(name)
}

#[derive(Debug, Clone)]
pub struct Room {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub description: Option<String>,
    pub category_id: Option<i64>,
    pub photo_path: Option<String>,
    pub slug: String,
    pub tracks_by_round: i64,
    pub track_duration: i64,
    pub pause_between_tracks: i64,
    pub pause_between_rounds: i64,
    pub is_public: bool,
    pub is_pro: bool,
    pub is_random: bool,
    pub is_active: bool,
    pub is_chat_active: bool,
    pub is_playing: bool,
    pub is_autostart: bool,
    pub is_featured: bool,
    pub discord_webhook_url: Option<String>,
    pub color: Option<String>,
    pub venue_name: Option<String>,
    pub password: Option<String>,
    pub user_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
}

impl Room {
    pub fn from_row(r: &PgRow) -> Self {
        Self {
            id: get(r, "id"),
            user_id: get(r, "user_id"),
            name: get(r, "name"),
            description: get(r, "description"),
            category_id: get(r, "category_id"),
            photo_path: get(r, "photo_path"),
            slug: get(r, "slug"),
            tracks_by_round: get(r, "tracks_by_round"),
            track_duration: get(r, "track_duration"),
            pause_between_tracks: get(r, "pause_between_tracks"),
            pause_between_rounds: get(r, "pause_between_rounds"),
            is_public: get(r, "is_public"),
            is_pro: get(r, "is_pro"),
            is_random: get(r, "is_random"),
            is_active: get(r, "is_active"),
            is_chat_active: get(r, "is_chat_active"),
            is_playing: get(r, "is_playing"),
            is_autostart: get(r, "is_autostart"),
            is_featured: get(r, "is_featured"),
            discord_webhook_url: get(r, "discord_webhook_url"),
            color: get(r, "color"),
            venue_name: get(r, "venue_name"),
            password: get(r, "password"),
            user_count: get(r, "user_count"),
            created_at: get(r, "created_at"),
            updated_at: get(r, "updated_at"),
            deleted_at: get(r, "deleted_at"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Round {
    pub id: i64,
    pub room_id: i64,
    pub user_id: Option<i64>,
    pub tracks: Option<Vec<i64>>,
    pub current: i64,
    pub is_playing: bool,
    pub finished_at: Option<DateTime<Utc>>,
    pub current_track_started_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Round {
    pub fn from_row(r: &PgRow) -> Self {
        Self {
            id: get(r, "id"),
            room_id: get(r, "room_id"),
            user_id: get(r, "user_id"),
            tracks: r
                .try_get::<sqlx::types::Json<Vec<i64>>, _>("tracks")
                .map(|j| j.0)
                .ok(),
            current: get(r, "current"),
            is_playing: get(r, "is_playing"),
            finished_at: get(r, "finished_at"),
            current_track_started_at: get(r, "current_track_started_at"),
            created_at: get(r, "created_at"),
            updated_at: get(r, "updated_at"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Track {
    pub id: i64,
    pub user_id: i64,
    pub playlist_id: i64,
    pub provider: String,
    pub provider_id: String,
    pub provider_url: Option<String>,
    pub preview_url: String,
    pub artwork_url: String,
    pub dificulty: Option<i64>,
    pub counter: Option<i64>,
    pub hint: Option<String>,
}

impl Track {
    pub fn from_row(r: &PgRow) -> Self {
        Self {
            id: get(r, "id"),
            user_id: get(r, "user_id"),
            playlist_id: get(r, "playlist_id"),
            provider: get(r, "provider"),
            provider_id: get(r, "provider_id"),
            provider_url: get(r, "provider_url"),
            preview_url: get(r, "preview_url"),
            artwork_url: get(r, "artwork_url"),
            dificulty: get(r, "dificulty"),
            counter: get(r, "counter"),
            hint: get(r, "hint"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub photo_path: Option<String>,
    pub elo: i64,
    pub is_guest: bool,
    pub is_administrator: bool,
    pub team_id: Option<i64>,
}
