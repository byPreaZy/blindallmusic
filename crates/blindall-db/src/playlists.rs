use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::postgres::PgPool;
use sqlx::Row;

#[derive(Clone)]
pub struct PlaylistRepo {
    pool: PgPool,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaylistRow {
    pub id: i64,
    pub name: String,
    pub user_id: i64,
    pub is_public: bool,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaylistListItem {
    #[serde(flatten)]
    pub playlist: PlaylistRow,
    pub owner_name: String,
    pub total_tracks: i64,
    pub moderators: Vec<ModeratorEntry>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModeratorEntry {
    pub id: i64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AnswerRow {
    pub id: i64,
    pub answer_type_id: String,
    pub value: String,
    pub score: f64,
    pub aliases: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlaylistTrack {
    pub id: i64,
    pub provider: String,
    pub provider_id: String,
    pub provider_url: Option<String>,
    pub preview_url: String,
    pub artwork_url: String,
    pub dificulty: Option<i64>,
    pub hint: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub answers: Vec<AnswerRow>,
    pub up_votes: i64,
    pub down_votes: i64,
}

impl PlaylistRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn find(&self, id: i64) -> Result<Option<PlaylistRow>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id, name, user_id, is_public, description FROM playlists WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| PlaylistRow {
            id: r.get("id"),
            name: r.get("name"),
            user_id: r.get("user_id"),
            is_public: r.get::<bool, _>("is_public"),
            description: r.get("description"),
        }))
    }

    pub async fn is_moderator(&self, playlist_id: i64, user_id: i64) -> bool {
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM moderables WHERE user_id = $1 \
             AND moderable_id = $2 AND moderable_type = 'App\\Models\\Playlist')::int::bigint",
        )
        .bind(user_id)
        .bind(playlist_id)
        .fetch_one(&self.pool)
        .await
        .map(|v| v == 1)
        .unwrap_or(false)
    }

    pub async fn can_edit(&self, p: &PlaylistRow, user_id: i64, is_admin: bool) -> bool {
        p.user_id == user_id || is_admin || self.is_moderator(p.id, user_id).await
    }

    pub async fn list_for_user(
        &self,
        user_id: i64,
        is_admin: bool,
    ) -> Result<Vec<PlaylistListItem>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT p.id, p.name, p.user_id, p.is_public, p.description, \
             p.updated_at, u.name AS owner_name, \
             (SELECT COUNT(*) FROM tracks t WHERE t.playlist_id = p.id) AS total_tracks \
             FROM playlists p JOIN users u ON u.id = p.user_id \
             WHERE $2 OR p.id IN (SELECT moderable_id FROM moderables \
               WHERE user_id = $1 AND moderable_type = 'App\\Models\\Playlist') \
             ORDER BY p.updated_at DESC",
        )
        .bind(user_id)
        .bind(is_admin)
        .fetch_all(&self.pool)
        .await?;

        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let id: i64 = r.get("id");
            out.push(PlaylistListItem {
                playlist: PlaylistRow {
                    id,
                    name: r.get("name"),
                    user_id: r.get("user_id"),
                    is_public: r.get::<bool, _>("is_public"),
                    description: r.get("description"),
                },
                owner_name: r.get("owner_name"),
                total_tracks: r.get("total_tracks"),
                moderators: self.moderators(id).await.unwrap_or_default(),
                updated_at: r.get("updated_at"),
            });
        }
        Ok(out)
    }

    pub async fn moderators(&self, playlist_id: i64) -> Result<Vec<ModeratorEntry>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT u.id, u.name FROM moderables m JOIN users u ON u.id = m.user_id \
             WHERE m.moderable_id = $1 AND m.moderable_type = 'App\\Models\\Playlist'",
        )
        .bind(playlist_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| ModeratorEntry {
                id: r.get("id"),
                name: r.get("name"),
            })
            .collect())
    }

    pub async fn create(&self, user_id: i64, name: &str) -> Result<i64, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let now = Utc::now();
        let res = sqlx::query_scalar::<_, i64>(
            "INSERT INTO playlists (name, user_id, is_public, created_at, updated_at) \
             VALUES ($1, $2, false, $3, $4) RETURNING id",
        )
        .bind(name)
        .bind(user_id)
        .bind(now)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        let id = res;
        sqlx::query(
            "INSERT INTO moderables (user_id, moderable_type, moderable_id, \
             created_at, updated_at) VALUES ($1, 'App\\Models\\Playlist', $2, $3, $4)",
        )
        .bind(user_id)
        .bind(id)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(id)
    }

    pub async fn attach_moderator(&self, playlist_id: i64, user_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO moderables (user_id, moderable_type, moderable_id, \
             created_at, updated_at) VALUES ($1, 'App\\Models\\Playlist', $2, $3, $4)",
        )
        .bind(user_id)
        .bind(playlist_id)
        .bind(chrono::Utc::now())
        .bind(chrono::Utc::now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn detach_moderator(&self, playlist_id: i64, user_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "DELETE FROM moderables WHERE user_id = $1 AND moderable_id = $2 \
             AND moderable_type = 'App\\Models\\Playlist'",
        )
        .bind(user_id)
        .bind(playlist_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn export_tracks(
        &self,
        playlist_id: i64,
    ) -> Result<Vec<PlaylistTrack>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT t.id, t.provider, t.provider_id, t.provider_url, t.preview_url, \
             t.artwork_url, t.dificulty, t.hint, t.created_at, \
             COALESCE((SELECT CAST(SUM(v.votes) AS bigint) FROM votes v \
               WHERE v.votable_id = t.id AND v.votable_type = 'App\\Models\\Track' \
               AND v.votes > 0), 0) AS up_votes, \
             COALESCE(ABS((SELECT CAST(SUM(v.votes) AS bigint) FROM votes v \
               WHERE v.votable_id = t.id AND v.votable_type = 'App\\Models\\Track' \
               AND v.votes < 0)), 0) AS down_votes \
             FROM tracks t WHERE t.playlist_id = $1 ORDER BY t.id",
        )
        .bind(playlist_id)
        .fetch_all(&self.pool)
        .await?;

        let mut tracks = Vec::with_capacity(rows.len());
        for r in rows {
            let id: i64 = r.get("id");
            tracks.push(PlaylistTrack {
                id,
                provider: r.get("provider"),
                provider_id: r.get("provider_id"),
                provider_url: r.get("provider_url"),
                preview_url: r.get("preview_url"),
                artwork_url: r.get("artwork_url"),
                dificulty: r.get::<Option<i64>, _>("dificulty"),
                hint: r.get("hint"),
                created_at: r.get("created_at"),
                answers: self.track_answers(id).await.unwrap_or_default(),
                up_votes: r.get("up_votes"),
                down_votes: r.get("down_votes"),
            });
        }
        Ok(tracks)
    }

    pub async fn update(
        &self,
        id: i64,
        name: &str,
        description: Option<&str>,
        user_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE playlists SET name = $1, description = $2, user_id = $3, \
             updated_at = $4 WHERE id = $5",
        )
        .bind(name)
        .bind(description)
        .bind(user_id)
        .bind(Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn delete(&self, id: i64) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query(
            "DELETE FROM moderables WHERE moderable_id = $1 \
             AND moderable_type = 'App\\Models\\Playlist'",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("DELETE FROM tracks WHERE playlist_id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM playlists WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn has_rooms(&self, playlist_id: i64) -> bool {
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM playlist_room WHERE playlist_id = $1)::int::bigint",
        )
        .bind(playlist_id)
        .fetch_one(&self.pool)
        .await
        .map(|v| v == 1)
        .unwrap_or(false)
    }

    pub async fn provider_track_id(
        &self,
        playlist_id: i64,
        provider_id: &str,
    ) -> Result<Option<i64>, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT id FROM tracks WHERE playlist_id = $1 AND provider_id = $2 LIMIT 1",
        )
        .bind(playlist_id)
        .bind(provider_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn tracks_page(
        &self,
        playlist_id: i64,
        page: i64,
        per_page: i64,
    ) -> Result<(Vec<PlaylistTrack>, i64), sqlx::Error> {
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM tracks WHERE playlist_id = $1",
        )
        .bind(playlist_id)
        .fetch_one(&self.pool)
        .await?;

        let rows = sqlx::query(
            "SELECT t.id, t.provider, t.provider_id, t.provider_url, t.preview_url, \
             t.artwork_url, t.dificulty, t.hint, t.created_at, \
             COALESCE((SELECT CAST(SUM(v.votes) AS bigint) FROM votes v \
               WHERE v.votable_id = t.id AND v.votable_type = 'App\\Models\\Track' \
               AND v.votes > 0), 0) AS up_votes, \
             COALESCE(ABS((SELECT CAST(SUM(v.votes) AS bigint) FROM votes v \
               WHERE v.votable_id = t.id AND v.votable_type = 'App\\Models\\Track' \
               AND v.votes < 0)), 0) AS down_votes \
             FROM tracks t WHERE t.playlist_id = $1 \
             ORDER BY t.created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(playlist_id)
        .bind(per_page)
        .bind((page.max(1) - 1) * per_page)
        .fetch_all(&self.pool)
        .await?;

        let mut tracks = Vec::with_capacity(rows.len());
        for r in rows {
            let id: i64 = r.get("id");
            tracks.push(PlaylistTrack {
                id,
                provider: r.get("provider"),
                provider_id: r.get("provider_id"),
                provider_url: r.get("provider_url"),
                preview_url: r.get("preview_url"),
                artwork_url: r.get("artwork_url"),
                dificulty: r.get::<Option<i64>, _>("dificulty"),
                hint: r.get("hint"),
                created_at: r.get("created_at"),
                answers: self.track_answers(id).await.unwrap_or_default(),
                up_votes: r.get("up_votes"),
                down_votes: r.get("down_votes"),
            });
        }
        Ok((tracks, total))
    }

    pub async fn track_answers_labeled(
        &self,
        track_id: i64,
    ) -> Result<Vec<serde_json::Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT at.name AS type, ta.value FROM track_answers ta \
             JOIN answer_types at ON at.id = ta.answer_type_id \
             WHERE ta.track_id = $1 ORDER BY at.id, ta.id",
        )
        .bind(track_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "type": r.get::<String, _>("type"),
                    "value": r.get::<String, _>("value"),
                })
            })
            .collect())
    }

    pub async fn track_answers(&self, track_id: i64) -> Result<Vec<AnswerRow>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, answer_type_id, value, score, aliases \
             FROM track_answers WHERE track_id = $1",
        )
        .bind(track_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| AnswerRow {
                id: r.get("id"),
                answer_type_id: r.get::<i64, _>("answer_type_id").to_string(),
                value: r.get("value"),
                score: r.get::<f64, _>("score"),
                aliases: r.get("aliases"),
            })
            .collect())
    }

    /// Point a track at a different provider resource (moderation rematch):
    /// replaces provider/provider_id/preview/artwork and, when supplied,
    /// syncs the Title/Artist answers so players answer what they hear.
    pub async fn rematch_track(
        &self,
        track_id: i64,
        provider: &str,
        provider_id: &str,
        provider_url: Option<&str>,
        preview_url: &str,
        artwork_url: Option<&str>,
        track_name: Option<&str>,
        artist_name: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let r = sqlx::query(
            "UPDATE tracks SET provider = $2, provider_id = $3, provider_url = $4, \
             preview_url = $5, artwork_url = COALESCE($6, artwork_url), updated_at = now() \
             WHERE id = $1",
        )
        .bind(track_id)
        .bind(provider)
        .bind(provider_id)
        .bind(provider_url)
        .bind(preview_url)
        .bind(artwork_url)
        .execute(&mut *tx)
        .await?;
        if r.rows_affected() == 0 {
            return Ok(false);
        }
        for (type_name, new_value) in [("Title", track_name), ("Artist", artist_name)] {
            if let Some(v) = new_value.map(str::trim).filter(|s| !s.is_empty()) {
                sqlx::query(
                    "UPDATE track_answers ta SET value = $2, updated_at = now() \
                     FROM answer_types at \
                     WHERE at.id = ta.answer_type_id AND ta.track_id = $1 AND at.name = $3",
                )
                .bind(track_id)
                .bind(v)
                .bind(type_name)
                .execute(&mut *tx)
                .await?;
            }
        }
        tx.commit().await?;
        Ok(true)
    }

    pub async fn upsert_track(
        &self,
        playlist_id: i64,
        user_id: i64,
        provider: &str,
        provider_id: &str,
        provider_url: &str,
        preview_url: &str,
        artwork_url: &str,
        artist_name: &str,
        track_name: &str,
    ) -> Result<i64, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let existing = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM tracks WHERE playlist_id = $1 AND provider = $2 AND provider_id = $3",
        )
        .bind(playlist_id)
        .bind(provider)
        .bind(provider_id)
        .fetch_optional(&mut *tx)
        .await?;

        let now = Utc::now();
        let track_id = match existing {
            Some(id) => {
                sqlx::query(
                    "UPDATE tracks SET provider_url = $1, preview_url = $2, \
                     artwork_url = $3, updated_at = $4 WHERE id = $5",
                )
                .bind(provider_url)
                .bind(preview_url)
                .bind(artwork_url)
                .bind(now)
                .bind(id)
                .execute(&mut *tx)
                .await?;
                id
            }
            None => {
                let res = sqlx::query_scalar::<_, i64>(
                    "INSERT INTO tracks (user_id, playlist_id, provider, provider_id, \
                     provider_url, preview_url, artwork_url, created_at, updated_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING id",
                )
                .bind(user_id)
                .bind(playlist_id)
                .bind(provider)
                .bind(provider_id)
                .bind(provider_url)
                .bind(preview_url)
                .bind(artwork_url)
                .bind(now)
                .bind(now)
                .fetch_one(&mut *tx)
                .await?;
                res
            }
        };

        let (clean_title, feats, medley) =
            blindall_core::sanitize::clean_import_title(track_name);
        let title = if clean_title.is_empty() {
            track_name
        } else {
            clean_title.as_str()
        };
        let aliases = if medley.len() > 1 {
            serde_json::to_value(&medley).unwrap_or_default()
        } else {
            serde_json::json!([])
        };
        let empty_aliases = serde_json::json!([]);

        for (type_id, value) in [(1i64, artist_name), (2i64, title)] {
            let found = sqlx::query_scalar::<_, i64>(
                "SELECT id FROM track_answers WHERE track_id = $1 AND answer_type_id = $2",
            )
            .bind(track_id)
            .bind(type_id)
            .fetch_optional(&mut *tx)
            .await?;
            match found {
                Some(aid) => {
                    sqlx::query(
                        "UPDATE track_answers SET value = $1, updated_at = $2 WHERE id = $3",
                    )
                    .bind(value)
                    .bind(now)
                    .bind(aid)
                    .execute(&mut *tx)
                    .await?;
                }
                None => {
                    sqlx::query(
                        "INSERT INTO track_answers (track_id, answer_type_id, value, \
                         aliases, score, created_at, updated_at) \
                         VALUES ($1, $2, $3, $4, 0.5, $5, $6)",
                    )
                    .bind(track_id)
                    .bind(type_id)
                    .bind(value)
                    .bind(if type_id == 2 { &aliases } else { &empty_aliases })
                    .bind(now)
                    .bind(now)
                    .execute(&mut *tx)
                    .await?;
                }
            }
        }

        for feat in &feats {
            sqlx::query(
                "INSERT INTO track_answers (track_id, answer_type_id, value, aliases, \
                 score, created_at, updated_at) \
                 SELECT $1, 5, $2, '[]'::jsonb, 0.5, $3, $4 \
                 WHERE NOT EXISTS (SELECT 1 FROM track_answers \
                 WHERE track_id = $1 AND value ILIKE $2)",
            )
            .bind(track_id)
            .bind(feat)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(track_id)
    }

    pub async fn update_track_meta(
        &self,
        track_id: i64,
        dificulty: Option<i64>,
        hint: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE tracks SET dificulty = $1, hint = $2, updated_at = $3 WHERE id = $4")
            .bind(dificulty)
            .bind(hint)
            .bind(Utc::now())
            .bind(track_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn delete_track(&self, track_id: i64) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM track_answers WHERE track_id = $1")
            .bind(track_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM tracks WHERE id = $1")
            .bind(track_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn track_ids(&self, playlist_id: i64) -> Result<Vec<i64>, sqlx::Error> {
        sqlx::query_scalar("SELECT id FROM tracks WHERE playlist_id = $1 ORDER BY id")
            .bind(playlist_id)
            .fetch_all(&self.pool)
            .await
    }

    pub async fn track_belongs_to(&self, playlist_id: i64, track_id: i64) -> bool {
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM tracks WHERE id = $1 AND playlist_id = $2)::int::bigint",
        )
        .bind(track_id)
        .bind(playlist_id)
        .fetch_one(&self.pool)
        .await
        .map(|v| v == 1)
        .unwrap_or(false)
    }

    pub async fn answer_types(&self) -> Result<Vec<(i64, String)>, sqlx::Error> {
        let rows = sqlx::query("SELECT id, name FROM answer_types ORDER BY id")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .map(|r| (r.get::<i64, _>("id"), r.get::<String, _>("name")))
            .collect())
    }

    pub async fn list_public(&self) -> Result<Vec<(i64, String, i64)>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT p.id, p.name, COUNT(t.id) AS tracks_count FROM playlists p \
             LEFT JOIN tracks t ON t.playlist_id = p.id \
             WHERE p.is_public = true \
             GROUP BY p.id, p.name ORDER BY p.name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| (r.get("id"), r.get("name"), r.get("tracks_count")))
            .collect())
    }

    pub async fn rooms_for_playlist(
        &self,
        playlist_id: i64,
    ) -> Result<Vec<(i64, String, String)>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT r.id, r.slug, r.name FROM playlist_room rp \
             JOIN rooms r ON r.id = rp.room_id \
             WHERE rp.playlist_id = $1 AND r.deleted_at IS NULL ORDER BY r.name",
        )
        .bind(playlist_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| (r.get("id"), r.get("slug"), r.get("name")))
            .collect())
    }

    pub async fn difficulty_counts(
        &self,
        playlist_id: i64,
    ) -> Result<Vec<(i64, i64)>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT dificulty, COUNT(*) AS c FROM tracks \
             WHERE playlist_id = $1 GROUP BY dificulty",
        )
        .bind(playlist_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                (
                    r.get::<Option<i64>, _>("dificulty").unwrap_or(0),
                    r.get::<i64, _>("c"),
                )
            })
            .collect())
    }

    pub async fn track_count(&self, playlist_id: i64) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM tracks WHERE playlist_id = $1",
        )
        .bind(playlist_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn track_playlist_id(&self, track_id: i64) -> Result<Option<i64>, sqlx::Error> {
        sqlx::query_scalar::<_, i64>("SELECT playlist_id FROM tracks WHERE id = $1")
            .bind(track_id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn create_answer(
        &self,
        track_id: i64,
        answer_type_id: i64,
        value: &str,
        score: f64,
        aliases: Option<Vec<String>>,
    ) -> Result<i64, sqlx::Error> {
        let aliases_json = aliases
            .filter(|a| !a.is_empty())
            .map(|a| sqlx::types::Json(serde_json::Value::from(a)));
        let res = sqlx::query_scalar::<_, i64>(
            "INSERT INTO track_answers (track_id, answer_type_id, value, score, \
             aliases, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
        )
        .bind(track_id)
        .bind(answer_type_id)
        .bind(value)
        .bind(score)
        .bind(aliases_json)
        .bind(Utc::now())
        .bind(Utc::now())
        .fetch_one(&self.pool)
        .await?;
        Ok(res)
    }

    pub async fn update_answer(
        &self,
        answer_id: i64,
        answer_type_id: i64,
        value: &str,
        score: f64,
        aliases: Option<Vec<String>>,
    ) -> Result<(), sqlx::Error> {
        let aliases_json = aliases
            .filter(|a| !a.is_empty())
            .map(|a| sqlx::types::Json(serde_json::Value::from(a)));
        sqlx::query(
            "UPDATE track_answers SET answer_type_id = $1, value = $2, score = $3, \
             aliases = $4, updated_at = $5 WHERE id = $6",
        )
        .bind(answer_type_id)
        .bind(value)
        .bind(score)
        .bind(aliases_json)
        .bind(Utc::now())
        .bind(answer_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn answer_belongs_to(&self, track_id: i64, answer_id: i64) -> bool {
        sqlx::query_scalar::<_, i64>(
            "SELECT id FROM track_answers WHERE id = $1 AND track_id = $2 LIMIT 1",
        )
        .bind(answer_id)
        .bind(track_id)
        .fetch_optional(&self.pool)
        .await
        .map(|r| r.is_some())
        .unwrap_or(false)
    }

    pub async fn delete_answer(&self, answer_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM track_answers WHERE id = $1")
            .bind(answer_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn upvoted_track_ids(&self, user_id: i64) -> Result<Vec<i64>, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT votable_id FROM votes \
             WHERE votable_type = 'App\\Models\\Track' AND votes > 0 AND user_id = $1",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn duplicate_track(
        &self,
        playlist_id: i64,
        user_id: i64,
        src_track_id: i64,
    ) -> Result<Option<i64>, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT dst.id FROM tracks dst JOIN tracks src ON src.id = $1 \
             WHERE dst.playlist_id = $2 AND dst.provider = src.provider \
             AND dst.provider_id = src.provider_id LIMIT 1",
        )
        .bind(src_track_id)
        .bind(playlist_id)
        .fetch_optional(&mut *tx)
        .await?;
        if exists.is_some() {
            tx.rollback().await?;
            return Ok(None);
        }
        let now = Utc::now();
        let new_id: Option<i64> = sqlx::query_scalar::<_, i64>(
            "INSERT INTO tracks (user_id, playlist_id, provider, provider_id, \
             provider_url, preview_url, artwork_url, dificulty, hint, \
             created_at, updated_at) \
             SELECT $1, $2, provider, provider_id, provider_url, preview_url, \
             artwork_url, dificulty, hint, $3, $4 FROM tracks WHERE id = $5 RETURNING id",
        )
        .bind(user_id)
        .bind(playlist_id)
        .bind(now)
        .bind(now)
        .bind(src_track_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(new_id) = new_id else {
            tx.rollback().await?;
            return Ok(None);
        };
        sqlx::query(
            "INSERT INTO track_answers (track_id, answer_type_id, value, aliases, \
             score, created_at, updated_at) \
             SELECT $1, answer_type_id, value, aliases, score, $2, $3 \
             FROM track_answers WHERE track_id = $4",
        )
        .bind(new_id)
        .bind(now)
        .bind(now)
        .bind(src_track_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Some(new_id))
    }

    pub async fn room_ids_for_playlist(&self, playlist_id: i64) -> Vec<i64> {
        sqlx::query_scalar::<_, i64>(
            "SELECT room_id FROM playlist_room WHERE playlist_id = $1",
        )
        .bind(playlist_id)
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default()
    }
}
