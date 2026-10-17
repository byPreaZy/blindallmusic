use sqlx::postgres::PgPool;
use sqlx::Row;

#[derive(Clone)]
pub struct ProfileRepo {
    pool: PgPool,
}

impl ProfileRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn public_room_ids(&self) -> Result<Vec<i64>, sqlx::Error> {
        sqlx::query_scalar("SELECT id FROM rooms WHERE is_public = true AND deleted_at IS NULL")
            .fetch_all(&self.pool)
            .await
    }

    pub async fn header(&self, user_id: i64) -> Result<Option<serde_json::Value>, sqlx::Error> {
        let Some(user) = sqlx::query(
            "SELECT u.id, u.name, u.photo_path, u.elo, u.is_guest, u.team_id, \
             u.created_at, u.show_donation_history_on_profile, \
             u.is_administrator, u.custom_tag, \
             EXISTS(SELECT 1 FROM moderables m \
              JOIN rooms rm ON rm.id = m.moderable_id \
              WHERE m.user_id = u.id AND m.moderable_type = 'App\\Models\\Room' \
              AND rm.is_public = true) AS is_moderator, \
             t.name AS team_name, t.photo_path AS team_photo \
             FROM users u LEFT JOIN teams t ON t.id = u.team_id \
             WHERE u.id = $1 AND u.deleted_at IS NULL",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        if user.get::<bool, _>("is_guest") {
            return Ok(None);
        }
        let level = sqlx::query(
            "SELECT * FROM user_levels WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        let totals = sqlx::query(
            "SELECT COALESCE(SUM(ts.score), 0) AS total_score, \
             COALESCE(SUM(CASE WHEN r.is_public = true THEN ts.score ELSE 0 END), 0) AS total_public_score, \
             COALESCE(SUM(CASE WHEN r.is_public = false THEN ts.score ELSE 0 END), 0) AS total_private_score \
             FROM total_scores ts \
             LEFT JOIN rooms r ON r.id = ts.room_id \
             WHERE ts.totalscorable_type = 'App\\Models\\User' AND ts.totalscorable_id = $1",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        let rooms_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM moderables WHERE user_id = $1 \
             AND moderable_type = 'App\\Models\\Room'",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        let playlists_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM moderables WHERE user_id = $1 \
             AND moderable_type = 'App\\Models\\Playlist'",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        let bookmarks_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM bookmarks WHERE user_id = $1 \
             AND bookmarkable_type = 'App\\Models\\Room'",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        let lvl = |col: &str, default: i64| -> i64 {
            level
                .as_ref()
                .and_then(|r| r.try_get::<i64, _>(col).ok())
                .unwrap_or(default)
        };
        let has_team = user.get::<Option<i64>, _>("team_id").is_some();
        Ok(Some(serde_json::json!({
            "id": user.get::<i64, _>("id"),
            "name": user.get::<String, _>("name"),
            "photo": user.get::<Option<String>, _>("photo_path"),
            "team": user.get::<Option<String>, _>("team_name").map(|n| serde_json::json!({
                "id": user.get::<Option<i64>, _>("team_id"),
                "name": n,
                "photo": user.get::<Option<String>, _>("team_photo"),
            })),
            "level": lvl("level", 1),
            "current_xp": lvl("current_xp", 0),
            "xp_for_next_level": lvl("xp_for_next_level", 100),
            "total_xp": lvl("total_xp", 0),
            "level_metrics": level.as_ref().map(|_| serde_json::json!({
                "score_public_rooms": lvl("score_public_rooms", 0),
                "minigame_scores_total": lvl("minigame_scores_total", 0),
                "seniority_months": lvl("months_seniority", 0),
                "consecutive_days_streak": lvl("consecutive_days_streak", 0),
                "rooms_created_count": lvl("rooms_created_count", 0),
                "playlists_created_count": lvl("playlists_created_count", 0),
                "tracks_liked_count": lvl("tracks_liked_count", 0),
                "has_team": has_team,
            })),
            "created_at": user.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
            "elo": user.get::<i64, _>("elo"),
            "role": if user.get::<bool, _>("is_administrator") { "admin" }
                else if user.get::<bool, _>("is_moderator") { "moderator" } else { "" },
            "custom_tag": user.get::<Option<String>, _>("custom_tag").unwrap_or_default(),
            "total_score": totals.get::<f64, _>("total_score"),
            "total_public_score": totals.get::<f64, _>("total_public_score"),
            "total_private_score": totals.get::<f64, _>("total_private_score"),
            "stats": {
                "rooms": rooms_count,
                "playlists": playlists_count,
                "bookmarks": bookmarks_count,
            },
            "show_donation_history_on_profile": user.get::<bool, _>("show_donation_history_on_profile"),
        })))
    }

    pub async fn scores(
        &self,
        user_id: i64,
        page: i64,
        sort: &str,
        direction: &str,
    ) -> Result<serde_json::Value, sqlx::Error> {
        const PER_PAGE: i64 = 10;
        let public = self.public_room_ids().await?;
        if public.is_empty() {
            return Ok(serde_json::json!({"data": [], "total": 0}));
        }
        let total: i64 = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM total_scores ts \
             WHERE ts.totalscorable_type = 'App\\Models\\User' \
             AND ts.totalscorable_id = $1 AND ts.room_id = ANY($2)",
        )
        .bind(user_id)
        .bind(public.to_vec())
        .fetch_one(&self.pool)
        .await?;
        let order = match sort {
            "score" => "ts.score",
            "room" => "r.name",
            _ => "ts.updated_at",
        };
        let dir = if direction == "asc" { "ASC" } else { "DESC" };
        let q = format!(
            "SELECT ts.id, ts.score, ts.room_id, ts.updated_at, \
             r.name AS room_name, r.slug AS room_slug, r.photo_path AS room_photo \
             FROM total_scores ts \
             JOIN rooms r ON r.id = ts.room_id \
             WHERE ts.totalscorable_type = 'App\\Models\\User' \
             AND ts.totalscorable_id = $1 AND ts.room_id = ANY($2) \
             ORDER BY {order} {dir} LIMIT $3 OFFSET $4"
        );
        let rows = sqlx::query(&q)
            .bind(user_id)
            .bind(public.to_vec())
            .bind(PER_PAGE)
            .bind((page.max(1) - 1) * PER_PAGE)
            .fetch_all(&self.pool)
            .await?;
        Ok(serde_json::json!({
            "data": rows.iter().map(|r| serde_json::json!({
                "id": r.get::<i64, _>("id"),
                "score": r.get::<f64, _>("score"),
                "room": {
                    "id": r.get::<i64, _>("room_id"),
                    "name": r.get::<String, _>("room_name"),
                    "slug": r.get::<Option<String>, _>("room_slug"),
                    "photo": r.get::<Option<String>, _>("room_photo"),
                },
                "updated_at": r.get::<chrono::DateTime<chrono::Utc>, _>("updated_at")
                    .format("%d/%m/%Y").to_string(),
            })).collect::<Vec<_>>(),
            "total": total,
            "page": page.max(1),
            "per_page": PER_PAGE,
        }))
    }

    pub async fn likes(&self, user_id: i64, page: i64) -> Result<serde_json::Value, sqlx::Error> {
        const PER_PAGE: i64 = 10;
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM votes WHERE user_id = $1 AND votes = 1 \
             AND votable_type = 'App\\Models\\Track'",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        let rows = sqlx::query(
            "SELECT t.id, t.artwork_url, t.created_at, \
             (SELECT ta.value FROM track_answers ta JOIN answer_types at2 ON at2.id = ta.answer_type_id \
              WHERE ta.track_id = t.id AND at2.name = 'Title' LIMIT 1) AS title, \
             (SELECT ta.value FROM track_answers ta JOIN answer_types at3 ON at3.id = ta.answer_type_id \
              WHERE ta.track_id = t.id AND at3.name = 'Artist' LIMIT 1) AS artist \
             FROM tracks t \
             JOIN votes v ON v.votable_id = t.id AND v.votable_type = 'App\\Models\\Track' \
             AND v.user_id = $1 AND v.votes = 1 \
             ORDER BY t.created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(user_id)
        .bind(PER_PAGE)
        .bind((page.max(1) - 1) * PER_PAGE)
        .fetch_all(&self.pool)
        .await?;
        Ok(serde_json::json!({
            "data": rows.iter().map(|r| serde_json::json!({
                "id": r.get::<i64, _>("id"),
                "artwork_url": r.get::<Option<String>, _>("artwork_url"),
                "title": r.get::<Option<String>, _>("title"),
                "artist": r.get::<Option<String>, _>("artist"),
            })).collect::<Vec<_>>(),
            "total": total,
            "page": page.max(1),
            "per_page": PER_PAGE,
        }))
    }

    pub async fn unlike_track(&self, user_id: i64, track_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "DELETE FROM votes WHERE user_id = $1 \
             AND votable_type = 'App\\Models\\Track' AND votable_id = $2",
        )
        .bind(user_id)
        .bind(track_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn bookmarks(&self, user_id: i64, page: i64) -> Result<serde_json::Value, sqlx::Error> {
        const PER_PAGE: i64 = 10;
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM bookmarks WHERE user_id = $1 \
             AND bookmarkable_type = 'App\\Models\\Room'",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        let rows = sqlx::query(
            "SELECT r.id, r.name, r.slug, r.photo_path, \
             c.id AS cat_id, c.name AS cat_name \
             FROM bookmarks b \
             JOIN rooms r ON r.id = b.bookmarkable_id \
             LEFT JOIN categories c ON c.id = r.category_id \
             WHERE b.user_id = $1 AND b.bookmarkable_type = 'App\\Models\\Room' \
             ORDER BY b.created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(user_id)
        .bind(PER_PAGE)
        .bind((page.max(1) - 1) * PER_PAGE)
        .fetch_all(&self.pool)
        .await?;
        Ok(serde_json::json!({
            "data": rows.iter().map(|r| serde_json::json!({
                "id": r.get::<i64, _>("id"),
                "name": r.get::<String, _>("name"),
                "slug": r.get::<Option<String>, _>("slug"),
                "photo": r.get::<Option<String>, _>("photo_path"),
                "category": r.get::<Option<i64>, _>("cat_id").map(|cid| serde_json::json!({
                    "id": cid,
                    "name": r.get::<Option<String>, _>("cat_name"),
                })),
            })).collect::<Vec<_>>(),
            "total": total,
            "page": page.max(1),
            "per_page": PER_PAGE,
        }))
    }

    pub async fn score_evolution(&self, user_id: i64) -> Result<Vec<serde_json::Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT DATE(created_at) AS date, SUM(score) AS daily_score \
             FROM scores WHERE user_id = $1 \
             GROUP BY DATE(created_at) ORDER BY date LIMIT 365",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        let mut cumulative = 0.0f64;
        Ok(rows
            .iter()
            .map(|r| {
                cumulative += r.get::<f64, _>("daily_score");
                serde_json::json!({
                    "date": r.get::<chrono::NaiveDate, _>("date").to_string(),
                    "total_score": (cumulative * 10.0).round() / 10.0,
                })
            })
            .collect())
    }

    pub async fn highlights(&self, user_id: i64) -> Result<serde_json::Value, sqlx::Error> {
        let best_win: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(rs.win_streak), 0) FROM round_standings rs \
             JOIN rooms r ON r.id = rs.room_id AND r.is_public = true \
             WHERE rs.user_id = $1",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        let level = sqlx::query("SELECT * FROM user_levels WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;
        let lvl = |col: &str| -> i64 {
            level
                .as_ref()
                .and_then(|r| r.try_get::<i64, _>(col).ok())
                .unwrap_or(0)
        };
        let best_round: f64 = level
            .as_ref()
            .and_then(|r| r.try_get::<f64, _>("best_round_score").ok())
            .unwrap_or(0.0);
        let public = self.public_room_ids().await?;
        let mut top_rooms = Vec::new();
        if !public.is_empty() {
            top_rooms = sqlx::query(
                "SELECT ts.score, ts.room_id, ts.updated_at, r.name, r.slug, r.photo_path \
                 FROM total_scores ts JOIN rooms r ON r.id = ts.room_id \
                 WHERE ts.totalscorable_type = 'App\\Models\\User' \
                 AND ts.totalscorable_id = $1 AND ts.room_id = ANY($2) \
                 ORDER BY ts.score DESC LIMIT 3",
            )
            .bind(user_id)
            .bind(public.to_vec())
            .fetch_all(&self.pool)
                .await?
                .iter()
                .map(|r| {
                    let updated: Option<chrono::DateTime<chrono::Utc>> =
                        r.try_get("updated_at").ok();
                    serde_json::json!({
                        "score": r.get::<f64, _>("score"),
                        "updated_at": updated
                            .map(|d| d.format("%d/%m/%Y").to_string()),
                        "room": {
                            "id": r.get::<i64, _>("room_id"),
                            "name": r.get::<String, _>("name"),
                            "slug": r.get::<Option<String>, _>("slug"),
                            "photo": r.get::<Option<String>, _>("photo_path"),
                        },
                    })
                })
                .collect();
        }
        let week_score: f64 = sqlx::query_scalar(
            "SELECT COALESCE(ROUND(SUM(rs.total_score)::numeric, 1)::float8, 0) \
             FROM round_standings rs \
             JOIN rooms r ON r.id = rs.room_id AND r.is_public = true \
             WHERE rs.user_id = $1 AND rs.created_at >= now() - interval '7 days'",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        let position: Option<i64> = match sqlx::query_scalar::<_, i64>("SELECT elo FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?
        {
            Some(elo) => Some(
                sqlx::query_scalar::<_, i64>(
                    "SELECT COUNT(*) + 1 FROM users WHERE elo > $1 AND deleted_at IS NULL",
                )
                .bind(elo)
                .fetch_one(&self.pool)
                .await?,
            ),
            None => None,
        };
        Ok(serde_json::json!({
            "performance": {
                "rounds_played": lvl("rounds_played_count"),
                "best_round_score": best_round,
                "consecutive_days_streak": lvl("consecutive_days_streak"),
                "best_win_streak": best_win,
                "week_score": week_score,
            },
            "rank": {
                "sort": "elo",
                "position": position,
            },
            "top_rooms": top_rooms,
            "badge_metrics": {
                "rooms_created_count": lvl("rooms_created_count"),
                "unique_rooms_played_count": lvl("unique_rooms_played_count"),
                "consecutive_days_streak": lvl("consecutive_days_streak"),
                "minigame_scores_total": lvl("minigame_scores_total"),
                "rounds_played_count": lvl("rounds_played_count"),
                "months_seniority": lvl("months_seniority"),
                "tracks_liked_count": lvl("tracks_liked_count"),
            },
        }))
    }
}
