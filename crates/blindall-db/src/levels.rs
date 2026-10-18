use sqlx::Row;
use sqlx::postgres::PgPool;

#[derive(Clone)]
pub struct LevelRepo {
    pool: PgPool,
}

#[derive(Debug, Clone, Copy)]
pub enum LevelTrigger {
    Login,
    Score,
    PlaylistsCount,
    RoomsCount,
    LikesCount,
}

impl LevelRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn ensure_level(&self, user_id: i64) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now();
        sqlx::query(
            "INSERT INTO user_levels \
             (user_id, level, total_xp, current_xp, xp_for_next_level, \
              score_public_rooms, rooms_created_count, months_seniority, \
              created_at, updated_at) \
             VALUES ($1, 1, 0, 0, 100, 0, 0, 0, $2, $3) \
             ON CONFLICT (user_id) DO NOTHING",
        )
        .bind(user_id)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update(
        &self,
        user_id: i64,
        trigger: LevelTrigger,
    ) -> Result<Option<serde_json::Value>, sqlx::Error> {
        let is_guest: bool = sqlx::query_scalar("SELECT is_guest FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?
            .unwrap_or(true);
        if is_guest {
            return Ok(None);
        }

        self.ensure_level(user_id).await?;

        match trigger {
            LevelTrigger::Login => self.update_login(user_id).await?,
            LevelTrigger::Score => {
                sqlx::query(
                    "UPDATE user_levels SET score_public_rooms = COALESCE(( \
                     SELECT SUM(ts.score) FROM total_scores ts \
                     JOIN rooms r ON r.id = ts.room_id AND r.is_public = true \
                     WHERE ts.totalscorable_type = 'App\\Models\\User' \
                     AND ts.totalscorable_id = $1), 0) \
                     WHERE user_id = $2",
                )
                .bind(user_id)
                .bind(user_id)
                .execute(&self.pool)
                .await?;
            }
            LevelTrigger::PlaylistsCount => {
                sqlx::query(
                    "UPDATE user_levels SET playlists_created_count = ( \
                     SELECT COUNT(*) FROM playlists WHERE user_id = $1) \
                     WHERE user_id = $2",
                )
                .bind(user_id)
                .bind(user_id)
                .execute(&self.pool)
                .await?;
            }
            LevelTrigger::RoomsCount => {
                sqlx::query(
                    "UPDATE user_levels SET rooms_created_count = ( \
                     SELECT COUNT(*) FROM rooms WHERE user_id = $1) \
                     WHERE user_id = $2",
                )
                .bind(user_id)
                .bind(user_id)
                .execute(&self.pool)
                .await?;
            }
            LevelTrigger::LikesCount => {
                sqlx::query(
                    "UPDATE user_levels SET tracks_liked_count = ( \
                     SELECT COUNT(DISTINCT v.votable_id) FROM votes v \
                     JOIN tracks t ON t.id = v.votable_id \
                     WHERE v.user_id = $1 AND v.votes = 1 \
                     AND v.votable_type = 'App\\Models\\Track') \
                     WHERE user_id = $2",
                )
                .bind(user_id)
                .bind(user_id)
                .execute(&self.pool)
                .await?;
            }
        }

        self.calculate_level_data(user_id).await.map(Some)
    }

    pub async fn snapshot(&self, user_id: i64) -> Result<Option<serde_json::Value>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT l.level, l.total_xp, l.current_xp, l.xp_for_next_level, \
             l.score_public_rooms, l.minigame_scores_total, l.months_seniority, \
             l.consecutive_days_streak, l.rooms_created_count, \
             l.playlists_created_count, l.tracks_liked_count, u.team_id \
             FROM user_levels l JOIN users u ON u.id = l.user_id \
             WHERE l.user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| {
            serde_json::json!({
                "level": r.get::<i64, _>("level"),
                "current_xp": r.get::<i64, _>("current_xp"),
                "xp_for_next_level": r.get::<i64, _>("xp_for_next_level"),
                "total_xp": r.get::<i64, _>("total_xp"),
                "level_metrics": {
                    "score_public_rooms": r.get::<i64, _>("score_public_rooms"),
                    "minigame_scores_total": r.get::<i64, _>("minigame_scores_total"),
                    "seniority_months": r.get::<i64, _>("months_seniority"),
                    "consecutive_days_streak": r.get::<i64, _>("consecutive_days_streak"),
                    "rooms_created_count": r.get::<i64, _>("rooms_created_count"),
                    "playlists_created_count": r.get::<i64, _>("playlists_created_count"),
                    "tracks_liked_count": r.get::<i64, _>("tracks_liked_count"),
                    "has_team": r.get::<Option<i64>, _>("team_id").is_some(),
                },
            })
        }))
    }

    async fn update_login(&self, user_id: i64) -> Result<(), sqlx::Error> {
        let row = sqlx::query(
            "SELECT last_login_date, consecutive_days_streak FROM user_levels \
             WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        let last: Option<chrono::NaiveDate> = row.get("last_login_date");
        let streak: i64 = row.get("consecutive_days_streak");
        let today = chrono::Utc::now().date_naive();
        let yesterday = today - chrono::Duration::days(1);
        let new_streak = match last {
            Some(d) if d == today => return Ok(()),
            Some(d) if d == yesterday => streak + 1,
            _ => 1,
        };
        sqlx::query(
            "UPDATE user_levels SET last_login_date = $1, consecutive_days_streak = $2 \
             WHERE user_id = $3",
        )
        .bind(today)
        .bind(new_streak)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn calculate_level_data(&self, user_id: i64) -> Result<serde_json::Value, sqlx::Error> {
        let row = sqlx::query(
            "SELECT l.score_public_rooms, l.rooms_created_count, \
             l.playlists_created_count, l.tracks_liked_count, \
             l.consecutive_days_streak, l.season_bonus_xp, u.created_at, u.team_id \
             FROM user_levels l JOIN users u ON u.id = l.user_id \
             WHERE l.user_id = $1",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        let seniority: i64 = {
            let created: chrono::DateTime<chrono::Utc> = row.get("created_at");
            ((chrono::Utc::now() - created).num_days() as f64 / 30.44).round() as i64
        };
        let minigame_total: i64 = sqlx::query_scalar(
            "SELECT CAST(COALESCE(SUM(score), 0) AS bigint) FROM minigame_scores WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        let data = blindall_core::levels::compute_level(&blindall_core::levels::XpInputs {
            score_public_rooms: row.get("score_public_rooms"),
            minigame_scores_total: minigame_total,
            months_seniority: seniority,
            rooms_created_count: row.get("rooms_created_count"),
            has_team: row.get::<Option<i64>, _>("team_id").is_some(),
            playlists_created_count: row.get("playlists_created_count"),
            tracks_liked_count: row.get("tracks_liked_count"),
            consecutive_days_streak: row.get("consecutive_days_streak"),
            season_bonus_xp: row.get("season_bonus_xp"),
        });
        let blindall_core::levels::LevelData {
            level,
            total_xp,
            current_xp,
            xp_for_next_level,
        } = data;

        sqlx::query(
            "UPDATE user_levels SET level = $1, total_xp = $2, current_xp = $3, \
             xp_for_next_level = $4, minigame_scores_total = $5, \
             months_seniority = $6, last_calculated_at = $7 WHERE user_id = $8",
        )
        .bind(level)
        .bind(total_xp)
        .bind(current_xp)
        .bind(xp_for_next_level)
        .bind(minigame_total)
        .bind(seniority)
        .bind(chrono::Utc::now())
        .bind(user_id)
        .execute(&self.pool)
        .await?;

        Ok(serde_json::json!({
            "level": level,
            "current_xp": current_xp,
            "xp_for_next_level": xp_for_next_level,
            "total_xp": total_xp,
            "level_metrics": {
                "score_public_rooms": row.get::<i64, _>("score_public_rooms"),
                "minigame_scores_total": minigame_total,
                "seniority_months": seniority,
                "consecutive_days_streak": row.get::<i64, _>("consecutive_days_streak"),
                "rooms_created_count": row.get::<i64, _>("rooms_created_count"),
                "playlists_created_count": row.get::<i64, _>("playlists_created_count"),
                "tracks_liked_count": row.get::<i64, _>("tracks_liked_count"),
                "season_bonus_xp": row.get::<i64, _>("season_bonus_xp"),
                "has_team": row.get::<Option<i64>, _>("team_id").is_some(),
            },
        }))
    }
}
