use serde::Serialize;
use sqlx::Row;
use sqlx::postgres::PgPool;

#[derive(Clone)]
pub struct RoomRepo {
    pool: PgPool,
}

#[derive(Debug, Serialize)]
pub struct RoomRow {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub category_id: Option<i64>,
    pub photo_path: Option<String>,
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
    pub color: Option<String>,
    pub venue_name: Option<String>,
    pub has_password: bool,
    pub user_count: i64,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug)]
pub struct RoomOptions {
    pub password: Option<String>,
    pub has_password: bool,
    pub tracks_by_round: i64,
    pub track_duration: i64,
    pub pause_between_tracks: i64,
    pub pause_between_rounds: i64,
    pub is_chat_active: bool,
    pub is_autostart: bool,
    pub is_random: bool,
    pub is_active: Option<bool>,
    pub color: Option<String>,
    pub venue_name: Option<String>,
    pub is_featured: Option<bool>,
}

impl RoomRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn map(r: sqlx::postgres::PgRow) -> RoomRow {
        RoomRow {
            id: r.get("id"),
            user_id: r.get("user_id"),
            name: r.get("name"),
            slug: r.get("slug"),
            description: r.get("description"),
            category_id: r.get("category_id"),
            photo_path: r.get("photo_path"),
            tracks_by_round: r.get("tracks_by_round"),
            track_duration: r.get("track_duration"),
            pause_between_tracks: r.get("pause_between_tracks"),
            pause_between_rounds: r.get("pause_between_rounds"),
            is_public: r.get("is_public"),
            is_pro: r.get("is_pro"),
            is_random: r.get("is_random"),
            is_active: r.get("is_active"),
            is_chat_active: r.get("is_chat_active"),
            is_playing: r.get("is_playing"),
            is_autostart: r.get("is_autostart"),
            is_featured: r.get("is_featured"),
            color: r.get("color"),
            venue_name: r.get("venue_name"),
            has_password: r.get::<Option<String>, _>("password").is_some(),
            user_count: r.get("user_count"),
            deleted_at: r.get("deleted_at"),
        }
    }

    const COLS: &'static str = "id, user_id, name, slug, description, category_id, \
        photo_path, tracks_by_round, track_duration, pause_between_tracks, \
        pause_between_rounds, is_public, is_pro, is_random, is_active, \
        is_chat_active, is_playing, is_autostart, is_featured, color, venue_name, password, \
        user_count, deleted_at";

    pub async fn password_of(&self, id: i64) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar::<_, Option<String>>("SELECT password FROM rooms WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map(|o| o.flatten())
    }

    pub async fn find(&self, id: i64) -> Result<Option<RoomRow>, sqlx::Error> {
        sqlx::query(&format!("SELECT {} FROM rooms WHERE id = $1", Self::COLS))
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map(|o| o.map(Self::map))
    }

    pub async fn find_by_slug(&self, slug: &str) -> Result<Option<RoomRow>, sqlx::Error> {
        sqlx::query(&format!("SELECT {} FROM rooms WHERE slug = $1", Self::COLS))
            .bind(slug)
            .fetch_optional(&self.pool)
            .await
            .map(|o| o.map(Self::map))
    }

    pub async fn name_taken(
        &self,
        name: &str,
        ignore_id: Option<i64>,
    ) -> Result<bool, sqlx::Error> {
        let exists: i64 = if let Some(id) = ignore_id {
            sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM rooms WHERE name = $1 AND id != $2)::int::bigint",
            )
            .bind(name)
            .bind(id)
            .fetch_one(&self.pool)
            .await?
        } else {
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM rooms WHERE name = $1)::int::bigint")
                .bind(name)
                .fetch_one(&self.pool)
                .await?
        };
        Ok(exists == 1)
    }

    pub async fn unique_slug(&self, name: &str) -> Result<String, sqlx::Error> {
        let base = blindall_core::sanitize::str_slug(name, '-');
        let mut slug = base.clone();
        if self.slug_exists(&slug).await? {
            let mut count = 0;
            while self.slug_exists(&slug).await? {
                slug = format!("{base}-{count}");
                count += 1;
            }
        }
        Ok(slug)
    }

    async fn slug_exists(&self, slug: &str) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM rooms WHERE slug = $1)::int::bigint",
        )
        .bind(slug)
        .fetch_one(&self.pool)
        .await
        .map(|v| v == 1)
    }

    pub async fn create(
        &self,
        user_id: i64,
        name: &str,
        category_id: i64,
    ) -> Result<i64, sqlx::Error> {
        let slug = self.unique_slug(name).await?;
        let now = chrono::Utc::now();
        let res = sqlx::query_scalar::<_, i64>(
            "INSERT INTO rooms (user_id, name, slug, category_id, is_random, \
             is_chat_active, is_featured, is_active, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, true, true, false, true, $5, $6) RETURNING id",
        )
        .bind(user_id)
        .bind(name)
        .bind(&slug)
        .bind(category_id)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        let id = res;
        self.attach_moderator(id, user_id).await?;
        Ok(id)
    }

    pub async fn update_fields(
        &self,
        id: i64,
        name: &str,
        description: Option<&str>,
        category_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE rooms SET name = $1, description = $2, category_id = $3 WHERE id = $4")
            .bind(name)
            .bind(description)
            .bind(category_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_options(&self, id: i64, o: &RoomOptions) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE rooms SET tracks_by_round = $1, track_duration = $2, \
             pause_between_tracks = $3, pause_between_rounds = $4, \
             is_chat_active = $5, is_autostart = $6, is_random = $7, color = $8, \
             venue_name = $10, \
             is_pro = CASE WHEN $10 IS NOT NULL THEN true ELSE is_pro END \
             WHERE id = $9",
        )
        .bind(o.tracks_by_round)
        .bind(o.track_duration)
        .bind(o.pause_between_tracks)
        .bind(o.pause_between_rounds)
        .bind(o.is_chat_active)
        .bind(o.is_autostart)
        .bind(o.is_random)
        .bind(&o.color)
        .bind(id)
        .bind(&o.venue_name)
        .execute(&self.pool)
        .await?;
        if let Some(featured) = o.is_featured {
            sqlx::query("UPDATE rooms SET is_featured = $1 WHERE id = $2")
                .bind(featured)
                .bind(id)
                .execute(&self.pool)
                .await?;
        }
        if let Some(active) = o.is_active {
            sqlx::query("UPDATE rooms SET is_active = $1 WHERE id = $2")
                .bind(active)
                .bind(id)
                .execute(&self.pool)
                .await?;
        }
        // Stocké hashé (bcrypt coût 10) — le secret partagé de room ne dort pas en clair.
        let password: Option<String> = if o.has_password {
            o.password
                .as_deref()
                .and_then(|p| bcrypt::hash(p.chars().take(72).collect::<String>(), 10).ok())
        } else {
            None
        };
        sqlx::query("UPDATE rooms SET password = $1 WHERE id = $2")
            .bind(password)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn search_track_ids(
        &self,
        room_id: i64,
        search: &str,
    ) -> Result<Vec<i64>, sqlx::Error> {
        let like = format!("%{}%", search.replace('%', "\\%").replace('_', "\\_"));
        let rows = sqlx::query(
            "SELECT DISTINCT t.id FROM tracks t \
             JOIN playlist_room pr ON pr.playlist_id = t.playlist_id \
             JOIN track_answers a ON a.track_id = t.id \
             WHERE pr.room_id = $1 AND a.value LIKE $2 ESCAPE '\\' \
             ORDER BY t.id LIMIT 10",
        )
        .bind(room_id)
        .bind(like)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|r| r.get("id")).collect())
    }

    pub async fn set_photo(&self, id: i64, path: &str) -> Result<Option<String>, sqlx::Error> {
        let prev: Option<String> = sqlx::query_scalar("SELECT photo_path FROM rooms WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .flatten();
        sqlx::query("UPDATE rooms SET photo_path = $1, updated_at = $2 WHERE id = $3")
            .bind(path)
            .bind(chrono::Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(prev)
    }

    pub async fn clear_photo(&self, id: i64) -> Result<Option<String>, sqlx::Error> {
        let prev: Option<String> = sqlx::query_scalar("SELECT photo_path FROM rooms WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .flatten();
        sqlx::query("UPDATE rooms SET photo_path = NULL, updated_at = $1 WHERE id = $2")
            .bind(chrono::Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(prev)
    }

    pub async fn update_user_count(&self, id: i64, count: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE rooms SET user_count = $1 WHERE id = $2")
            .bind(count)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn soft_delete(&self, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM moderables WHERE moderable_type = 'App\\Models\\Room' AND moderable_id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        sqlx::query("UPDATE rooms SET deleted_at = $1, photo_path = NULL WHERE id = $2")
            .bind(chrono::Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn restore(&self, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE rooms SET deleted_at = NULL WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // ---- authorization helpers -----------------------------------------

    pub async fn is_moderator(&self, room_id: i64, user_id: i64) -> bool {
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM moderables WHERE user_id = $1 \
             AND moderable_type = 'App\\Models\\Room' AND moderable_id = $2)::int::bigint",
        )
        .bind(user_id)
        .bind(room_id)
        .fetch_one(&self.pool)
        .await
        .map(|v| v == 1)
        .unwrap_or(false)
    }

    pub async fn has_control(&self, room: &RoomRow, user_id: i64, is_admin: bool) -> bool {
        is_admin || room.user_id == user_id || self.is_moderator(room.id, user_id).await
    }

    // ---- moderators ------------------------------------------------------

    pub async fn attach_moderator(&self, room_id: i64, user_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO moderables (user_id, moderable_type, moderable_id, \
             created_at, updated_at) VALUES ($1, 'App\\Models\\Room', $2, $3, $4)",
        )
        .bind(user_id)
        .bind(room_id)
        .bind(chrono::Utc::now())
        .bind(chrono::Utc::now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn detach_moderator(&self, room_id: i64, user_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "DELETE FROM moderables WHERE user_id = $1 \
             AND moderable_type = 'App\\Models\\Room' AND moderable_id = $2",
        )
        .bind(user_id)
        .bind(room_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn moderators(&self, room_id: i64) -> Result<Vec<(i64, String)>, sqlx::Error> {
        sqlx::query(
            "SELECT u.id, u.name FROM moderables m \
             JOIN users u ON u.id = m.user_id \
             WHERE m.moderable_type = 'App\\Models\\Room' AND m.moderable_id = $1 \
             ORDER BY u.name",
        )
        .bind(room_id)
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.iter().map(|r| (r.get("id"), r.get("name"))).collect())
    }

    pub async fn moderators_detail(
        &self,
        room_id: i64,
    ) -> Result<Vec<(i64, String, Option<String>)>, sqlx::Error> {
        sqlx::query(
            "SELECT u.id, u.name, u.photo_path FROM moderables m \
             JOIN users u ON u.id = m.user_id \
             WHERE m.moderable_type = 'App\\Models\\Room' AND m.moderable_id = $1 \
             ORDER BY u.name",
        )
        .bind(room_id)
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.iter()
                .map(|r| (r.get("id"), r.get("name"), r.get("photo_path")))
                .collect()
        })
    }

    /// Active rooms owned by the user — Plus perk "unlimited private rooms".
    pub async fn owned_active_count(&self, user_id: i64) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar("SELECT COUNT(*) FROM rooms WHERE user_id = $1 AND deleted_at IS NULL")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await
    }

    pub async fn tracks_count(&self, room_id: i64) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(DISTINCT t.id) FROM tracks t \
             JOIN playlist_room pr ON pr.playlist_id = t.playlist_id \
             WHERE pr.room_id = $1",
        )
        .bind(room_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn rounds_count(&self, room_id: i64) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM rounds WHERE room_id = $1")
            .bind(room_id)
            .fetch_one(&self.pool)
            .await
    }

    pub async fn is_bookmarked(&self, user_id: i64, room_id: i64) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM bookmarks WHERE user_id = $1 \
             AND bookmarkable_type = 'App\\Models\\Room' AND bookmarkable_id = $2)::int::bigint",
        )
        .bind(user_id)
        .bind(room_id)
        .fetch_one(&self.pool)
        .await?
            == 1)
    }

    pub async fn public_room_cards(
        &self,
    ) -> Result<Vec<(Option<String>, String, Option<String>)>, sqlx::Error> {
        sqlx::query(
            "SELECT slug, name, photo_path FROM rooms \
             WHERE is_public = true AND is_active = true \
             AND password IS NULL AND deleted_at IS NULL \
             ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.iter()
                .map(|r| (r.get("slug"), r.get("name"), r.get("photo_path")))
                .collect()
        })
    }

    pub async fn owner(
        &self,
        room_id: i64,
    ) -> Result<Option<(i64, String, Option<String>)>, sqlx::Error> {
        sqlx::query(
            "SELECT u.id, u.name, u.photo_path FROM rooms r \
             JOIN users u ON u.id = r.user_id WHERE r.id = $1",
        )
        .bind(room_id)
        .fetch_optional(&self.pool)
        .await
        .map(|o| o.map(|r| (r.get("id"), r.get("name"), r.get("photo_path"))))
    }

    pub async fn category_name(&self, category_id: i64) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar::<_, String>("SELECT name FROM categories WHERE id = $1")
            .bind(category_id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn category_name_slug(
        &self,
        category_id: i64,
    ) -> Result<Option<(String, String)>, sqlx::Error> {
        sqlx::query_as::<_, (String, String)>("SELECT name, slug FROM categories WHERE id = $1")
            .bind(category_id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn official_similar_rooms(
        &self,
        category_id: i64,
        exclude_id: i64,
        slugs: &[String],
        limit: i64,
    ) -> Result<Vec<(Option<String>, String, Option<String>)>, sqlx::Error> {
        sqlx::query_as::<_, (Option<String>, String, Option<String>)>(
            "SELECT slug, name, photo_path FROM rooms \
             WHERE is_public = TRUE AND is_active = TRUE \
               AND password IS NULL AND deleted_at IS NULL \
               AND category_id = $1 AND id <> $2 AND slug = ANY($3) \
             ORDER BY name LIMIT $4",
        )
        .bind(category_id)
        .bind(exclude_id)
        .bind(slugs)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn moderated_rooms(
        &self,
        user_id: i64,
        search: Option<&str>,
        trashed: Option<&str>,
    ) -> Result<Vec<(RoomRow, Option<String>, i64)>, sqlx::Error> {
        let trashed_clause = match trashed {
            Some("with") => "",
            Some("only") => "AND r.deleted_at IS NOT NULL",
            _ => "AND r.deleted_at IS NULL",
        };
        let search_clause = if search.is_some() {
            "AND (r.name ILIKE $2 OR owner.name ILIKE $2)"
        } else {
            ""
        };
        let sql = format!(
            "SELECT {}, c.name::text AS category_name, \
                    (SELECT COUNT(*) FROM rounds rd WHERE rd.room_id = r.id) AS rounds_count \
             FROM rooms r \
             JOIN moderables m ON m.moderable_id = r.id AND m.moderable_type = 'App\\Models\\Room' \
             JOIN users owner ON owner.id = r.user_id \
             LEFT JOIN categories c ON c.id = r.category_id \
             WHERE m.user_id = $1 {trashed_clause} {search_clause} \
             ORDER BY r.is_public DESC, r.name",
            Self::COLS
                .split(", ")
                .map(|c| format!("r.{c}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let mut q = sqlx::query(&sql).bind(user_id);
        if let Some(s) = search {
            q = q.bind(format!("%{s}%"));
        }
        let rows = q.fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|r| {
                let category = r
                    .try_get::<Option<String>, _>("category_name")
                    .ok()
                    .flatten();
                let rounds_count = r.try_get::<i64, _>("rounds_count").unwrap_or(0);
                (Self::map(r), category, rounds_count)
            })
            .collect())
    }

    // ---- playlists pivot ---------------------------------------------------

    pub async fn playlist_exists(&self, playlist_id: i64) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM playlists WHERE id = $1)::int::bigint",
        )
        .bind(playlist_id)
        .fetch_one(&self.pool)
        .await?
            == 1)
    }

    /// Returns true when the link was newly created (false if already attached).
    pub async fn attach_playlist(
        &self,
        room_id: i64,
        playlist_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let res = sqlx::query(
            "INSERT INTO playlist_room (playlist_id, room_id, created_at, updated_at) \
             SELECT $1, $2, $3, $4 \
             WHERE NOT EXISTS ( \
               SELECT 1 FROM playlist_room \
               WHERE playlist_id = $1 AND room_id = $2)",
        )
        .bind(playlist_id)
        .bind(room_id)
        .bind(chrono::Utc::now())
        .bind(chrono::Utc::now())
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn detach_playlist(&self, room_id: i64, playlist_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM playlist_room WHERE playlist_id = $1 AND room_id = $2")
            .bind(playlist_id)
            .bind(room_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Track belongs to a playlist attached to this room (in-room moderation scope).
    pub async fn track_in_room(&self, room_id: i64, track_id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM tracks t \
             JOIN playlist_room pr ON pr.playlist_id = t.playlist_id \
             WHERE pr.room_id = $1 AND t.id = $2)",
        )
        .bind(room_id)
        .bind(track_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn room_playlists(&self, room_id: i64) -> Result<Vec<(i64, String)>, sqlx::Error> {
        sqlx::query(
            "SELECT p.id, p.name FROM playlist_room pr \
             JOIN playlists p ON p.id = pr.playlist_id \
             WHERE pr.room_id = $1",
        )
        .bind(room_id)
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.iter().map(|r| (r.get("id"), r.get("name"))).collect())
    }

    // ---- bookmarks ---------------------------------------------------------

    /// Returns true when the bookmark was newly inserted (false if already present).
    pub async fn bookmark(&self, user_id: i64, room_id: i64) -> Result<bool, sqlx::Error> {
        let res = sqlx::query(
            "INSERT INTO bookmarks (user_id, bookmarkable_type, bookmarkable_id, \
             created_at, updated_at) \
             SELECT $1, 'App\\Models\\Room', $2, $3, $4 \
             WHERE NOT EXISTS ( \
               SELECT 1 FROM bookmarks \
               WHERE user_id = $1 AND bookmarkable_type = 'App\\Models\\Room' \
               AND bookmarkable_id = $2)",
        )
        .bind(user_id)
        .bind(room_id)
        .bind(chrono::Utc::now())
        .bind(chrono::Utc::now())
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn unbookmark(&self, user_id: i64, room_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "DELETE FROM bookmarks WHERE user_id = $1 \
             AND bookmarkable_type = 'App\\Models\\Room' AND bookmarkable_id = $2",
        )
        .bind(user_id)
        .bind(room_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ---- misc ----------------------------------------------------------------

    pub async fn category_exists(&self, id: i64) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM categories WHERE id = $1)::int::bigint",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?
            == 1)
    }

    pub async fn categories(&self) -> Result<Vec<(i64, String)>, sqlx::Error> {
        sqlx::query("SELECT id, name FROM categories ORDER BY name")
            .fetch_all(&self.pool)
            .await
            .map(|rows| rows.iter().map(|r| (r.get("id"), r.get("name"))).collect())
    }

    pub async fn category_by_slug(
        &self,
        slug: &str,
    ) -> Result<Option<(i64, String, String)>, sqlx::Error> {
        sqlx::query("SELECT id, name, slug FROM categories WHERE slug = $1 LIMIT 1")
            .bind(slug)
            .fetch_optional(&self.pool)
            .await
            .map(|o| o.map(|r| (r.get("id"), r.get("name"), r.get("slug"))))
    }

    pub async fn public_for_category(&self, category_id: i64) -> Result<Vec<RoomRow>, sqlx::Error> {
        sqlx::query(&format!(
            "SELECT {} FROM rooms WHERE category_id = $1 AND is_public = true \
             AND is_active = true \
             AND password IS NULL AND deleted_at IS NULL ORDER BY is_playing DESC",
            Self::COLS
        ))
        .bind(category_id)
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.into_iter().map(Self::map).collect())
    }

    pub async fn user_exists(&self, id: i64) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM users WHERE id = $1 AND deleted_at IS NULL)::int::bigint",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?
            == 1)
    }

    // ---- leaderboard (`RoomLeaderboardService`) -----------------------------

    pub async fn leaderboards(&self, room_id: i64) -> Result<serde_json::Value, sqlx::Error> {
        Ok(serde_json::json!({
            "lifetime": self.user_leaderboard(room_id, None).await?,
            "week": self.user_leaderboard(room_id, Some(7)).await?,
            "teams": self.team_leaderboard(room_id).await?,
        }))
    }

    async fn user_leaderboard(
        &self,
        room_id: i64,
        since_days: Option<i64>,
    ) -> Result<Vec<serde_json::Value>, sqlx::Error> {
        let totals: Vec<(i64, f64)> = match since_days {
            None => sqlx::query(
                "SELECT ts.totalscorable_id AS user_id, ROUND(SUM(ts.score)::numeric, 1)::float8 AS total \
                 FROM total_scores ts \
                 JOIN users u ON u.id = ts.totalscorable_id AND u.is_guest = false \
                 WHERE ts.room_id = $1 AND ts.totalscorable_type = 'App\\Models\\User' \
                 GROUP BY ts.totalscorable_id ORDER BY total DESC LIMIT 50",
            )
            .bind(room_id)
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|r| (r.get("user_id"), r.get("total")))
            .collect(),
            Some(days) => sqlx::query(
                "SELECT rs.user_id, ROUND(SUM(rs.total_score)::numeric, 1)::float8 AS total \
                 FROM round_standings rs \
                 JOIN users u ON u.id = rs.user_id AND u.is_guest = false \
                 WHERE rs.room_id = $1 AND rs.created_at >= $2 \
                 GROUP BY rs.user_id ORDER BY total DESC LIMIT 50",
            )
            .bind(room_id)
            .bind(chrono::Utc::now() - chrono::Duration::days(days))
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|r| (r.get("user_id"), r.get("total")))
            .collect(),
        };
        if totals.is_empty() {
            return Ok(vec![]);
        }
        let ids: Vec<i64> = totals.iter().map(|t| t.0).collect();

        let users: std::collections::HashMap<i64, serde_json::Value> = sqlx::query(
            "SELECT u.id, u.name, u.photo_path, u.elo, u.is_guest, \
             u.is_administrator, \
             CASE WHEN EXISTS(SELECT 1 FROM donations dn WHERE dn.user_id = u.id AND dn.month_key = $2) \
                OR EXISTS(SELECT 1 FROM user_subscriptions s WHERE s.user_id = u.id \
                  AND s.status IN ('active','trialing','past_due') \
                  AND (s.current_period_end IS NULL OR s.current_period_end > now())) \
             THEN u.custom_tag ELSE '' END AS custom_tag, \
             EXISTS(SELECT 1 FROM moderables m \
              JOIN rooms rm ON rm.id = m.moderable_id \
              WHERE m.user_id = u.id AND m.moderable_type = 'App\\Models\\Room' \
              AND rm.is_public = true) AS is_moderator, \
             l.level, l.total_xp, l.current_xp, l.xp_for_next_level \
             FROM users u LEFT JOIN user_levels l ON l.user_id = u.id \
             WHERE u.id = ANY($1)",
        )
        .bind(&ids)
        .bind(crate::donations::current_month_key())
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|r| {
            let level = r.try_get::<i64, _>("level").ok();
            let photo_path: Option<String> = r.get("photo_path");
            (
                r.get("id"),
                serde_json::json!({
                    "id": r.get::<i64, _>("id"),
                    "name": r.get::<String, _>("name"),
                    "photo": photo_path,
                    "elo": r.try_get::<i64, _>("elo").unwrap_or(1500),
                    "is_guest": r.get::<bool, _>("is_guest"),
                    "role": if r.get::<bool, _>("is_administrator") { "admin" }
                        else if r.get::<bool, _>("is_moderator") { "moderator" } else { "" },
                    "custom_tag": r.get::<Option<String>, _>("custom_tag").unwrap_or_default(),
                    "user_level": level.map(|lv| serde_json::json!({
                        "level": lv,
                        "total_xp": r.get::<i64, _>("total_xp"),
                        "current_xp": r.get::<i64, _>("current_xp"),
                        "xp_for_next_level": r.get::<i64, _>("xp_for_next_level"),
                    })),
                }),
            )
        })
        .collect();

        let stats_q = format!(
            "SELECT user_id, COUNT(*) AS rounds_played, \
             ROUND(AVG(total_score)::numeric, 1)::float8 AS avg_score_per_round, \
             ROUND(AVG(average_response_time)::numeric, 2)::float8 AS avg_response_time, \
             ROUND(MAX(total_score)::numeric, 1)::float8 AS best_round_score, \
             MAX(win_streak) AS best_win_streak \
             FROM round_standings WHERE room_id = $1 AND user_id = ANY($2) {} \
             GROUP BY user_id",
            if since_days.is_some() {
                "AND created_at >= $3"
            } else {
                ""
            }
        );
        let mut stats_q = sqlx::query(&stats_q).bind(room_id).bind(&ids);
        if let Some(days) = since_days {
            stats_q = stats_q.bind(chrono::Utc::now() - chrono::Duration::days(days));
        }
        let stats: std::collections::HashMap<i64, sqlx::postgres::PgRow> = stats_q
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|r| (r.get("user_id"), r))
            .collect();

        Ok(totals
            .into_iter()
            .enumerate()
            .map(|(i, (uid, total))| {
                let s = stats.get(&uid);
                let level = users
                    .get(&uid)
                    .and_then(|u| u.get("user_level"))
                    .and_then(|l| l.get("level"))
                    .and_then(|l| l.as_i64())
                    .unwrap_or(1);
                serde_json::json!({
                    "rank": i + 1,
                    "user_id": uid,
                    "total": total,
                    "user": users.get(&uid),
                    "stats": {
                        "level": level,
                        "elo": users.get(&uid).and_then(|u| u.get("elo")).and_then(|e| e.as_i64()).unwrap_or(1500),
                        "score": total,
                        "rounds_played": s.map(|r| r.get::<i64, _>("rounds_played")).unwrap_or(0),
                        "avg_score_per_round": s.and_then(|r| r.try_get::<f64, _>("avg_score_per_round").ok()),
                        "avg_response_time": s.and_then(|r| r.try_get::<f64, _>("avg_response_time").ok()),
                        "best_round_score": s.and_then(|r| r.try_get::<f64, _>("best_round_score").ok()),
                        "best_win_streak": s.and_then(|r| r.try_get::<i64, _>("best_win_streak").ok()),
                    },
                })
            })
            .collect())
    }

    async fn team_leaderboard(&self, room_id: i64) -> Result<Vec<serde_json::Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT t.id AS team_id, t.name, t.photo_path, \
             ROUND(SUM(ts.score)::numeric, 1)::float8 AS total \
             FROM total_scores ts \
             JOIN teams t ON t.id = ts.totalscorable_id \
             WHERE ts.room_id = $1 AND ts.totalscorable_type = 'App\\Models\\Team' \
             GROUP BY t.id, t.name, t.photo_path ORDER BY total DESC LIMIT 50",
        )
        .bind(room_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                serde_json::json!({
                    "rank": i + 1,
                    "team_id": r.get::<i64, _>("team_id"),
                    "total": r.get::<f64, _>("total"),
                    "team": {
                        "id": r.get::<i64, _>("team_id"),
                        "name": r.get::<String, _>("name"),
                        "photo": r.get::<Option<String>, _>("photo_path"),
                    },
                })
            })
            .collect())
    }

    pub async fn user_snapshot(
        &self,
        room_id: i64,
        user_id: i64,
        boards: &serde_json::Value,
    ) -> Result<serde_json::Value, sqlx::Error> {
        let find_rank = |key: &str| -> Option<(f64, i64)> {
            boards.get(key)?.as_array()?.iter().find_map(|e| {
                if e.get("user_id")?.as_i64()? == user_id {
                    Some((e.get("total")?.as_f64()?, e.get("rank")?.as_i64()?))
                } else {
                    None
                }
            })
        };
        let week = match find_rank("week") {
            Some((total, rank)) => {
                serde_json::json!({"user_id": user_id, "total": total, "rank": rank})
            }
            None => {
                let total: f64 = sqlx::query_scalar(
                    "SELECT COALESCE(SUM(total_score), 0) FROM round_standings \
                     WHERE room_id = $1 AND user_id = $2 AND created_at >= $3",
                )
                .bind(room_id)
                .bind(user_id)
                .bind(chrono::Utc::now() - chrono::Duration::days(7))
                .fetch_one(&self.pool)
                .await?;
                if total <= 0.0 {
                    serde_json::Value::Null
                } else {
                    serde_json::json!({"user_id": user_id, "total": (total * 10.0).round() / 10.0})
                }
            }
        };
        let lifetime = match find_rank("lifetime") {
            Some((total, rank)) => {
                serde_json::json!({"user_id": user_id, "total": total, "rank": rank})
            }
            None => {
                let total: f64 = sqlx::query_scalar(
                    "SELECT COALESCE(SUM(score), 0) FROM total_scores \
                     WHERE room_id = $1 AND totalscorable_id = $2 \
                     AND totalscorable_type = 'App\\Models\\User'",
                )
                .bind(room_id)
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?;
                if total <= 0.0 {
                    serde_json::Value::Null
                } else {
                    serde_json::json!({"user_id": user_id, "total": (total * 10.0).round() / 10.0})
                }
            }
        };
        Ok(serde_json::json!({ "week": week, "lifetime": lifetime }))
    }
}
