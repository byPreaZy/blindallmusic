use crate::entities;
use async_trait::async_trait;
use blindall_core::types::{Standing, TrackAnswer, TrackHistoryEntry};
use blindall_game::store::{
    GameStore, GameTrack, PassthroughResolver, PlayerInfo, PreviewResolver, Result, RoomConfig,
    RoundRow, StoreError, TrackDeletionNotice,
};
use chrono::{DateTime, Utc};
use sqlx::Row;
use sqlx::postgres::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct DbGameStore {
    pool: PgPool,
    http: reqwest::Client,
    resolver: Arc<dyn PreviewResolver>,
    environment: String,
}

impl DbGameStore {
    pub fn new(pool: PgPool) -> Self {
        Self::with_resolver(pool, Arc::new(PassthroughResolver))
    }

    pub fn with_resolver(pool: PgPool, resolver: Arc<dyn PreviewResolver>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(3))
            .build()
            .expect("reqwest client");
        Self {
            pool,
            http,
            resolver,
            environment: "local".into(),
        }
    }

    pub fn environment(mut self, env: impl Into<String>) -> Self {
        self.environment = env.into();
        self
    }

    pub async fn post_discord_to_rooms(&self, playlist_id: i64, message: &str, color: u32) {
        let Ok(rooms) = sqlx::query(
            "SELECT r.name, r.discord_webhook_url FROM rooms r \
             JOIN playlist_room pr ON pr.room_id = r.id \
             WHERE pr.playlist_id = $1 AND r.is_public = true \
             AND r.discord_webhook_url IS NOT NULL",
        )
        .bind(playlist_id)
        .fetch_all(&self.pool)
        .await
        else {
            return;
        };
        for room in rooms {
            let name: String = room.get("name");
            let Some(url) = room.get::<Option<String>, _>("discord_webhook_url") else {
                continue;
            };
            let body = serde_json::json!({
                "embeds": [{
                    "title": name,
                    "description": message,
                    "color": color,
                }],
            });
            if let Err(e) = self.http.post(&url).json(&body).send().await {
                tracing::warn!(error = %e, url = %url, "discord webhook failed");
            }
        }
    }

    async fn notify_playlist_moderators(
        &self,
        playlist_id: i64,
        message: &str,
    ) -> Vec<TrackDeletionNotice> {
        let Ok(Some(playlist)) = sqlx::query(
            "SELECT id, name, user_id, is_public, description FROM playlists WHERE id = $1",
        )
        .bind(playlist_id)
        .fetch_optional(&self.pool)
        .await
        else {
            return vec![];
        };
        if playlist.get::<bool, _>("is_public") {
            return vec![];
        }
        let Ok(moderators) = sqlx::query_scalar::<_, i64>(
            "SELECT user_id FROM moderables WHERE moderable_id = $1 \
             AND moderable_type = 'App\\Models\\Playlist'",
        )
        .bind(playlist_id)
        .fetch_all(&self.pool)
        .await
        else {
            return vec![];
        };
        if moderators.is_empty() {
            return vec![];
        }
        let playlist_json = serde_json::json!({
            "id": playlist.get::<i64, _>("id"),
            "name": playlist.get::<String, _>("name"),
            "user_id": playlist.get::<i64, _>("user_id"),
            "is_public": playlist.get::<bool, _>("is_public"),
            "description": playlist.get::<Option<String>, _>("description"),
        });
        let data = serde_json::json!({
            "playlist": playlist_json,
            "created_at": chrono::Utc::now().format("%d/%m/%Y %H:%M").to_string(),
            "message": message,
        });
        let repo = crate::NotificationRepo::new(self.pool.clone());
        let mut notified = Vec::with_capacity(moderators.len());
        for user_id in moderators {
            if let Ok(notification_id) = repo
                .notify(user_id, "App\\Notifications\\TrackDeleted", data.clone())
                .await
            {
                notified.push(TrackDeletionNotice {
                    user_id,
                    notification_id,
                    data: data.clone(),
                });
            }
        }
        notified
    }
}

async fn track_answer_value(pool: &PgPool, track_id: i64, type_id: i64) -> String {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM track_answers WHERE track_id = $1 AND answer_type_id = $2 \
         ORDER BY id LIMIT 1",
    )
    .bind(track_id)
    .bind(type_id)
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .unwrap_or_else(|| "inconnu".into())
}

fn round_row_to_game(r: entities::Round) -> RoundRow {
    RoundRow {
        id: r.id,
        room_id: r.room_id,
        tracks: r.tracks.unwrap_or_default(),
        current: r.current,
        is_playing: r.is_playing,
        finished_at: r.finished_at,
        current_track_started_at: r.current_track_started_at,
    }
}

#[derive(Debug, Clone)]
pub struct RoomSummary {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub is_playing: bool,
    pub tracks_by_round: i64,
    pub user_count: i64,
    pub round_count: Option<i64>,
}

impl DbGameStore {
    pub async fn public_playing_rooms(&self) -> Result<Vec<RoomSummary>> {
        let rows = sqlx::query(
            "SELECT id, name, slug, is_playing, tracks_by_round, user_count, \
             (SELECT COUNT(*) FROM rounds WHERE rounds.room_id = rooms.id) AS round_count \
             FROM rooms \
             WHERE is_public = true AND is_active = true AND deleted_at IS NULL \
             ORDER BY is_playing DESC, updated_at DESC LIMIT 50",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Other(e.into()))?;
        Ok(rows
            .iter()
            .map(|r| RoomSummary {
                id: r.get("id"),
                name: r.get("name"),
                slug: r.get("slug"),
                is_playing: r.get("is_playing"),
                tracks_by_round: r.get("tracks_by_round"),
                user_count: r.get("user_count"),
                round_count: r.get("round_count"),
            })
            .collect())
    }
}

#[async_trait]
impl GameStore for DbGameStore {
    async fn room_config(&self, room_id: i64) -> Result<RoomConfig> {
        let row = sqlx::query("SELECT * FROM rooms WHERE id = $1")
            .bind(room_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StoreError::Other(e.into()))?
            .ok_or(StoreError::NotFound("room"))?;
        let row = entities::Room::from_row(&row);
        Ok(RoomConfig {
            id: row.id,
            name: row.name,
            track_duration: row.track_duration as u32,
            pause_between_tracks: row.pause_between_tracks as u32,
            pause_between_rounds: row.pause_between_rounds as u32,
            tracks_by_round: row.tracks_by_round as u32,
            is_public: row.is_public,
            is_autostart: row.is_autostart,
            is_random: row.is_random,
            discord_webhook_url: row.discord_webhook_url,
            venue_name: row.venue_name,
        })
    }

    async fn pick_tracks(&self, room: &RoomConfig) -> Result<Vec<i64>> {
        const BASE: &str = "SELECT DISTINCT t.id FROM tracks t JOIN playlist_room pr \
             ON pr.playlist_id = t.playlist_id WHERE pr.room_id = $1";
        let sql = if room.is_random {
            format!(
                "SELECT id FROM ({BASE}) d ORDER BY random() LIMIT {}",
                room.tracks_by_round
            )
        } else {
            format!("{BASE} LIMIT {}", room.tracks_by_round)
        };
        sqlx::query_scalar::<_, i64>(&sql)
            .bind(room.id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| StoreError::Other(e.into()))
    }

    async fn create_round(
        &self,
        room_id: i64,
        user_id: Option<i64>,
        tracks: &[i64],
    ) -> Result<i64> {
        let now = Utc::now();
        let res = sqlx::query_scalar::<_, i64>(
            "INSERT INTO rounds (room_id, user_id, tracks, \"current\", is_playing, \
             created_at, updated_at) VALUES ($1, $2, $3, 0, true, $4, $5) RETURNING id",
        )
        .bind(room_id)
        .bind(user_id)
        .bind(sqlx::types::Json(tracks.to_vec()))
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StoreError::Other(e.into()))?;
        Ok(res)
    }

    async fn advance_round(&self, round_id: i64, started_at: DateTime<Utc>) -> Result<()> {
        sqlx::query(
            "UPDATE rounds SET \"current\" = \"current\" + 1, \
             current_track_started_at = $1, updated_at = $2 WHERE id = $3",
        )
        .bind(started_at)
        .bind(Utc::now())
        .bind(round_id)
        .execute(&self.pool)
        .await
        .map_err(|e| StoreError::Other(e.into()))?;
        Ok(())
    }

    async fn finish_round(&self, round_id: i64, at: DateTime<Utc>) -> Result<()> {
        sqlx::query(
            "UPDATE rounds SET is_playing = false, finished_at = $1, updated_at = $2 \
             WHERE id = $3",
        )
        .bind(at)
        .bind(Utc::now())
        .bind(round_id)
        .execute(&self.pool)
        .await
        .map_err(|e| StoreError::Other(e.into()))?;
        Ok(())
    }

    async fn set_room_playing(&self, room_id: i64, playing: bool) -> Result<()> {
        sqlx::query("UPDATE rooms SET is_playing = $1, updated_at = $2 WHERE id = $3")
            .bind(playing)
            .bind(Utc::now())
            .bind(room_id)
            .execute(&self.pool)
            .await
            .map_err(|e| StoreError::Other(e.into()))?;
        Ok(())
    }

    async fn load_track(&self, track_id: i64) -> Result<Option<GameTrack>> {
        let track = sqlx::query("SELECT * FROM tracks WHERE id = $1")
            .bind(track_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StoreError::Other(e.into()))?;
        let Some(track) = track else { return Ok(None) };
        let track = entities::Track::from_row(&track);

        let rows = sqlx::query(
            "SELECT ta.id, ta.value, ta.aliases, ta.score, at.name AS type_name, \
             at.svg_icon AS svg_icon \
             FROM track_answers ta \
             LEFT JOIN answer_types at ON at.id = ta.answer_type_id \
             WHERE ta.track_id = $1",
        )
        .bind(track_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Other(e.into()))?;

        let answers = rows
            .iter()
            .map(|r| TrackAnswer {
                id: r.get::<i64, _>("id"),
                type_name: r
                    .try_get::<String, _>("type_name")
                    .unwrap_or_else(|_| "Titre".to_string()),
                value: r.get::<String, _>("value"),
                aliases: r
                    .try_get::<sqlx::types::Json<Vec<String>>, _>("aliases")
                    .map(|a| a.0)
                    .unwrap_or_default(),
                score: r
                    .try_get::<sqlx::types::BigDecimal, _>("score")
                    .map(|d| d.to_string().parse().unwrap_or(0.5))
                    .unwrap_or(0.5),
                svg_icon: r.try_get::<Option<String>, _>("svg_icon").unwrap_or(None),
            })
            .collect();

        let audio = self
            .resolver
            .resolve(&track.provider, &track.provider_id, &track.preview_url)
            .await;

        let (upvotes, downvotes): (i64, i64) = sqlx::query_as(
            "SELECT CAST(COALESCE(SUM(GREATEST(votes, 0)), 0) AS bigint), \
             CAST(COALESCE(ABS(SUM(LEAST(votes, 0))), 0) AS bigint) \
             FROM votes WHERE votable_type = 'App\\Models\\Track' AND votable_id = $1",
        )
        .bind(track.id)
        .fetch_one(&self.pool)
        .await
        .unwrap_or((0, 0));

        Ok(Some(GameTrack {
            id: track.id,
            provider: track.provider,
            preview_url: Some(track.preview_url),
            audio,
            hint: track.hint,
            artwork_url: Some(track.artwork_url).filter(|u| !u.is_empty()),
            upvotes,
            downvotes,
            answers,
        }))
    }

    async fn probe_audio(&self, audio_url: &str) -> bool {
        for _ in 0..3 {
            match self.http.get(audio_url).send().await {
                Ok(r) if r.status().is_success() => return true,
                Ok(_) => return false, // responded but not 2xx, don't retry
                Err(_) => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
        false
    }

    async fn delete_track(
        &self,
        track_id: i64,
        actor: Option<String>,
    ) -> Result<Vec<TrackDeletionNotice>> {
        let Some(track) = sqlx::query("SELECT provider, playlist_id FROM tracks WHERE id = $1")
            .bind(track_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StoreError::Other(e.into()))?
        else {
            return Ok(vec![]);
        };
        let provider: String = track.get("provider");
        let playlist_id: i64 = track.get("playlist_id");

        let title = track_answer_value(&self.pool, track_id, 2).await;
        let artist = track_answer_value(&self.pool, track_id, 1).await;
        let message = match &actor {
            Some(name) => {
                format!("Le titre {title} de {artist} a été supprimé par {name}.")
            }
            None => format!(
                "Le titre {title} de {artist} a été supprimé automatiquement \
                 car l'aperçu n'est plus disponible sur {provider}."
            ),
        };

        if self.environment == "production" {
            self.post_discord_to_rooms(playlist_id, &message, 15158332)
                .await;
        }
        let notified = self.notify_playlist_moderators(playlist_id, &message).await;

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| StoreError::Other(e.into()))?;
        sqlx::query("DELETE FROM track_answers WHERE track_id = $1")
            .bind(track_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| StoreError::Other(e.into()))?;
        sqlx::query("DELETE FROM tracks WHERE id = $1")
            .bind(track_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| StoreError::Other(e.into()))?;
        tx.commit().await.map_err(|e| StoreError::Other(e.into()))?;
        Ok(notified)
    }

    async fn load_active_rounds(&self) -> Result<Vec<RoundRow>> {
        let rows =
            sqlx::query("SELECT * FROM rounds WHERE is_playing = true OR finished_at IS NULL")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| StoreError::Other(e.into()))?;
        Ok(rows
            .iter()
            .map(|r| round_row_to_game(entities::Round::from_row(r)))
            .collect())
    }

    async fn round_row(&self, round_id: i64) -> Result<Option<RoundRow>> {
        let row = sqlx::query("SELECT * FROM rounds WHERE id = $1")
            .bind(round_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StoreError::Other(e.into()))?;
        Ok(row
            .as_ref()
            .map(|r| round_row_to_game(entities::Round::from_row(r))))
    }

    async fn players_info(&self, user_ids: &[i64]) -> Result<HashMap<i64, PlayerInfo>> {
        if user_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = sqlx::query(
            "SELECT u.id, u.name, u.photo_path, u.elo, u.is_guest, u.is_administrator, \
             u.custom_tag, u.team_id, \
             EXISTS(SELECT 1 FROM moderables m \
              JOIN rooms rm ON rm.id = m.moderable_id \
              WHERE m.user_id = u.id AND m.moderable_type = 'App\\Models\\Room' \
              AND rm.is_public = true) AS is_moderator, \
             t.name AS team_name, t.photo_path AS team_photo, \
             COALESCE(ul.level, 1) AS level, \
             COALESCE(ul.total_xp, 0) AS total_xp, \
             COALESCE(ul.current_xp, 0) AS current_xp, \
             COALESCE(ul.xp_for_next_level, 100) AS xp_for_next_level, \
             COALESCE(ul.rounds_played_count, 0) AS rounds_played_count, \
             COALESCE(ul.score_public_rooms, 0) AS score_public_rooms, \
             COALESCE(ul.months_seniority, 0) AS months_seniority, \
             COALESCE(ul.consecutive_days_streak, 0) AS consecutive_days_streak, \
             COALESCE(ul.rooms_created_count, 0) AS rooms_created_count, \
             COALESCE(ul.playlists_created_count, 0) AS playlists_created_count, \
             COALESCE(ul.tracks_liked_count, 0) AS tracks_liked_count, \
             (SELECT COUNT(*) FROM round_standings rs \
              WHERE rs.user_id = u.id AND rs.is_elo_counted) AS public_rounds_played_count \
             FROM users u \
             LEFT JOIN teams t ON t.id = u.team_id \
             LEFT JOIN user_levels ul ON ul.user_id = u.id \
             WHERE u.id = ANY($1)",
        )
        .bind(user_ids.to_vec())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Other(e.into()))?;
        Ok(rows
            .iter()
            .map(|r| {
                (
                    r.get::<i64, _>("id"),
                    PlayerInfo {
                        id: r.get("id"),
                        name: r.get("name"),
                        photo: r.get("photo_path"),
                        elo: r.get("elo"),
                        is_guest: r.get("is_guest"),
                        team_id: r.get("team_id"),
                        team_name: r.try_get("team_name").ok().flatten(),
                        team_photo: r.try_get("team_photo").ok().flatten(),
                        level: r.get("level"),
                        total_xp: r.get("total_xp"),
                        current_xp: r.get("current_xp"),
                        xp_for_next_level: r.get("xp_for_next_level"),
                        rounds_played_count: r.get("rounds_played_count"),
                        public_rounds_played_count: r.get("public_rounds_played_count"),
                        score_public_rooms: r.get("score_public_rooms"),
                        months_seniority: r.get("months_seniority"),
                        consecutive_days_streak: r.get("consecutive_days_streak"),
                        rooms_created_count: r.get("rooms_created_count"),
                        playlists_created_count: r.get("playlists_created_count"),
                        tracks_liked_count: r.get("tracks_liked_count"),
                        is_administrator: r.get("is_administrator"),
                        is_moderator: r.get("is_moderator"),
                        custom_tag: r.get("custom_tag"),
                    },
                )
            })
            .collect())
    }

    async fn write_standings(
        &self,
        standings: &[Standing],
        elo_updates: &[(i64, i64)],
    ) -> Result<()> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| StoreError::Other(e.into()))?;

        let round_id = standings.first().map(|s| s.round_id).unwrap_or(0);
        sqlx::query("SELECT id FROM rounds WHERE id = $1 FOR UPDATE")
            .bind(round_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| StoreError::Other(e.into()))?;

        let exists: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM round_standings WHERE round_id = $1")
                .bind(round_id)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| StoreError::Other(e.into()))?;
        if exists > 0 {
            tx.rollback().await.ok();
            return Ok(()); // already finalized, PHP early-returns
        }

        let now = Utc::now();
        for s in standings {
            sqlx::query(
                "INSERT INTO round_standings (round_id, room_id, user_id, team_id, \
                 position, total_score, elo_before, elo_after, elo_change, \
                 is_elo_counted, average_response_time, fast_answers_count, \
                 total_answers_count, tracks_history, win_streak, \
                 created_at, updated_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)",
            )
            .bind(s.round_id)
            .bind(s.room_id)
            .bind(s.user_id)
            .bind(s.team_id)
            .bind(s.position)
            .bind(s.total_score)
            .bind(s.elo_before)
            .bind(s.elo_after)
            .bind(s.elo_change)
            .bind(s.is_elo_counted)
            .bind(s.average_response_time)
            .bind(s.fast_answers_count)
            .bind(s.total_answers_count)
            .bind(sqlx::types::Json(&s.tracks_history))
            .bind(s.win_streak)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await
            .map_err(|e| StoreError::Other(e.into()))?;
        }

        let counted: std::collections::HashSet<i64> = standings
            .iter()
            .filter(|s| s.is_elo_counted)
            .map(|s| s.user_id)
            .collect();
        for (uid, elo_after) in elo_updates {
            if counted.contains(uid) {
                sqlx::query("UPDATE users SET elo = $1, updated_at = $2 WHERE id = $3")
                    .bind(elo_after)
                    .bind(now)
                    .bind(uid)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| StoreError::Other(e.into()))?;
            }
        }

        tx.commit().await.map_err(|e| StoreError::Other(e.into()))
    }

    async fn bump_total_score(
        &self,
        room_id: i64,
        user_id: i64,
        team_id: Option<i64>,
        score: f64,
    ) -> Result<()> {
        for (ty, id) in [("user", user_id)]
            .into_iter()
            .chain(team_id.map(|t| ("team", t)))
        {
            let morph = if ty == "user" {
                "App\\Models\\User"
            } else {
                "App\\Models\\Team"
            };
            let existing: Option<i64> = sqlx::query_scalar(
                "SELECT id FROM total_scores WHERE totalscorable_type = $1 \
                 AND totalscorable_id = $2 AND room_id = $3",
            )
            .bind(morph)
            .bind(id)
            .bind(room_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| StoreError::Other(e.into()))?;
            let now = Utc::now();
            match existing {
                Some(row_id) => {
                    sqlx::query(
                        "UPDATE total_scores SET score = score + $1, updated_at = $2 \
                         WHERE id = $3",
                    )
                    .bind(score)
                    .bind(now)
                    .bind(row_id)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| StoreError::Other(e.into()))?;
                }
                None => {
                    sqlx::query(
                        "INSERT INTO total_scores (totalscorable_type, \
                         totalscorable_id, room_id, score, created_at, updated_at) \
                         VALUES ($1, $2, $3, $4, $5, $6)",
                    )
                    .bind(morph)
                    .bind(id)
                    .bind(room_id)
                    .bind(score)
                    .bind(now)
                    .bind(now)
                    .execute(&self.pool)
                    .await
                    .map_err(|e| StoreError::Other(e.into()))?;
                }
            }
        }
        Ok(())
    }

    async fn placement_rounds_played(&self, user_id: i64) -> Result<i64> {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM round_standings WHERE user_id = $1 \
             AND is_elo_counted = true",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StoreError::Other(e.into()))
    }

    async fn win_streak(&self, room_id: i64, user_id: i64, before_round_id: i64) -> Result<i64> {
        let positions: Vec<i64> = sqlx::query_scalar(
            "SELECT s.position FROM round_standings s \
             JOIN rounds r ON r.id = s.round_id \
             WHERE s.room_id = $1 AND s.user_id = $2 AND r.id < $3 \
             AND r.finished_at IS NOT NULL \
             ORDER BY s.round_id DESC LIMIT 200",
        )
        .bind(room_id)
        .bind(user_id)
        .bind(before_round_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Other(e.into()))?;
        Ok(1 + positions.iter().take_while(|&&p| p == 1).count() as i64)
    }

    async fn round_has_standings(&self, round_id: i64) -> Result<bool> {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM round_standings WHERE round_id = $1")
            .bind(round_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StoreError::Other(e.into()))?;
        Ok(n > 0)
    }

    async fn required_answers_per_track(&self, track_ids: &[i64]) -> Result<HashMap<i64, usize>> {
        if track_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let rows = sqlx::query(
            "SELECT track_id, COUNT(*) AS c FROM track_answers \
             WHERE track_id = ANY($1) GROUP BY track_id",
        )
        .bind(track_ids.to_vec())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Other(e.into()))?;
        Ok(rows
            .iter()
            .map(|r| (r.get::<i64, _>("track_id"), r.get::<i64, _>("c") as usize))
            .collect())
    }

    async fn scores_from_db(&self, round_id: i64) -> Result<HashMap<i64, f64>> {
        let rows = sqlx::query(
            "SELECT user_id, SUM(score) AS total FROM scores \
             WHERE round_id = $1 GROUP BY user_id",
        )
        .bind(round_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Other(e.into()))?;
        Ok(rows
            .iter()
            .map(|r| {
                (
                    r.get::<i64, _>("user_id"),
                    r.try_get::<sqlx::types::BigDecimal, _>("total")
                        .map(|d| d.to_string().parse().unwrap_or(0.0))
                        .unwrap_or(0.0),
                )
            })
            .collect())
    }

    async fn tracks_history_from_db(
        &self,
        round_id: i64,
    ) -> Result<HashMap<i64, Vec<TrackHistoryEntry>>> {
        let rows = sqlx::query(
            "SELECT user_id, track_id, answer_id, time, score FROM scores \
             WHERE round_id = $1",
        )
        .bind(round_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Other(e.into()))?;
        let mut out: HashMap<i64, Vec<TrackHistoryEntry>> = HashMap::new();
        for r in &rows {
            out.entry(r.get::<i64, _>("user_id"))
                .or_default()
                .push(TrackHistoryEntry {
                    track_id: r.get("track_id"),
                    answer_id: r.try_get("answer_id").ok(),
                    response_time: r
                        .try_get::<sqlx::types::BigDecimal, _>("time")
                        .ok()
                        .and_then(|d| d.to_string().parse().ok()),
                    position: None,
                    score: r
                        .try_get::<sqlx::types::BigDecimal, _>("score")
                        .ok()
                        .and_then(|d| d.to_string().parse().ok()),
                });
        }
        Ok(out)
    }

    async fn has_legacy_scores(&self, round_id: i64) -> Result<bool> {
        let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM scores WHERE round_id = $1")
            .bind(round_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StoreError::Other(e.into()))?;
        Ok(n > 0)
    }

    async fn delete_round_scores(&self, round_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM scores WHERE round_id = $1")
            .bind(round_id)
            .execute(&self.pool)
            .await
            .map_err(|e| StoreError::Other(e.into()))?;
        Ok(())
    }

    async fn recalc_user_level(&self, user_id: i64) -> Result<Option<serde_json::Value>> {
        crate::levels::LevelRepo::new(self.pool.clone())
            .update(user_id, crate::levels::LevelTrigger::Score)
            .await
            .map_err(|e| StoreError::Other(e.into()))
    }
}
