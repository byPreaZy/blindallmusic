//! Teams — `TeamController`/`TeamRequestController` persistence.
//! Team membership is `users.team_id`; requests live in `team_requests`
//! (unique team+user); scores come from `total_scores`/`round_standings`
//! scoped to public rooms.

use sqlx::postgres::PgPool;
use sqlx::Row;

#[derive(Clone)]
pub struct TeamRepo {
    pool: PgPool,
}

pub const TEAM_SEATS: i64 = 8;

pub struct TeamRow {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub photo_path: Option<String>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl TeamRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn map(r: sqlx::postgres::PgRow) -> TeamRow {
        TeamRow {
            id: r.get("id"),
            user_id: r.get("user_id"),
            name: r.get("name"),
            photo_path: r.get("photo_path"),
            deleted_at: r.get("deleted_at"),
        }
    }

    pub async fn find(&self, id: i64) -> Result<Option<TeamRow>, sqlx::Error> {
        sqlx::query("SELECT id, user_id, name, photo_path, deleted_at FROM teams WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map(|o| o.map(Self::map))
    }

    pub async fn name_taken(&self, name: &str, ignore_id: Option<i64>) -> Result<bool, sqlx::Error> {
        let exists: i64 = match ignore_id {
            Some(id) => sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM teams WHERE name = $1 AND id != $2)::int::bigint")
                .bind(name)
                .bind(id)
                .fetch_one(&self.pool)
                .await?,
            None => sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM teams WHERE name = $1)::int::bigint")
                .bind(name)
                .fetch_one(&self.pool)
                .await?,
        };
        Ok(exists == 1)
    }

    /// `TeamController::index` — paginated teams ranked by public-room score,
    /// owner not banned, optional name search.
    pub async fn list(
        &self,
        search: Option<&str>,
        page: i64,
    ) -> Result<(Vec<serde_json::Value>, i64), sqlx::Error> {
        const PER_PAGE: i64 = 12;
        let like = search.map(|s| format!("%{s}%"));
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM teams t \
             JOIN users o ON o.id = t.user_id \
             WHERE t.deleted_at IS NULL AND ($1 IS NULL OR t.name LIKE $2) \
             AND NOT EXISTS(SELECT 1 FROM bans b \
              WHERE b.bannable_type = 'App\\Models\\User' AND b.bannable_id = o.id \
              AND b.deleted_at IS NULL AND (b.expired_at IS NULL OR b.expired_at > NOW()))",
        )
        .bind(&like)
        .bind(&like)
        .fetch_one(&self.pool)
        .await?;
        let rows = sqlx::query(
            "SELECT t.id, t.name, t.photo_path, t.user_id, o.name AS owner_name, \
             (SELECT COUNT(*) FROM users m WHERE m.team_id = t.id) AS members_count, \
             COALESCE((SELECT ROUND(SUM(ts.score)::numeric, 1)::float8 FROM total_scores ts \
              JOIN rooms r ON r.id = ts.room_id AND r.is_public = true \
              WHERE ts.totalscorable_type = 'App\\Models\\Team' \
              AND ts.totalscorable_id = t.id), 0) AS team_points, \
             (SELECT COUNT(DISTINCT rs.round_id) FROM round_standings rs \
              WHERE rs.team_id = t.id) AS rounds_played \
             FROM teams t \
             JOIN users o ON o.id = t.user_id \
             WHERE t.deleted_at IS NULL AND ($1 IS NULL OR t.name LIKE $2) \
             AND NOT EXISTS(SELECT 1 FROM bans b \
              WHERE b.bannable_type = 'App\\Models\\User' AND b.bannable_id = o.id \
              AND b.deleted_at IS NULL AND (b.expired_at IS NULL OR b.expired_at > NOW())) \
             ORDER BY team_points DESC, t.name \
             LIMIT $3 OFFSET $4",
        )
        .bind(&like)
        .bind(&like)
        .bind(PER_PAGE)
        .bind((page.max(1) - 1) * PER_PAGE)
        .fetch_all(&self.pool)
        .await?;
        let teams = rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.get::<i64, _>("id"),
                    "name": r.get::<String, _>("name"),
                    "photo_path": r.get::<Option<String>, _>("photo_path"),
                    "user_id": r.get::<i64, _>("user_id"),
                    "owner_name": r.get::<String, _>("owner_name"),
                    "members_count": r.get::<i64, _>("members_count"),
                    "team_points": r.get::<f64, _>("team_points"),
                    "rounds_played": r.get::<i64, _>("rounds_played"),
                })
            })
            .collect();
        Ok((teams, count))
    }

    /// `TeamController::store` — create + assign owner membership.
    pub async fn create(&self, owner_id: i64, name: &str) -> Result<i64, sqlx::Error> {
        let now = chrono::Utc::now();
        let res = sqlx::query_scalar::<_, i64>("INSERT INTO teams (user_id, name, created_at, updated_at) VALUES ($1, $2, $3, $4) RETURNING id")
            .bind(owner_id)
            .bind(name)
            .bind(now)
            .bind(now)
            .fetch_one(&self.pool)
            .await?;
        let id = res;
        sqlx::query("UPDATE users SET team_id = $1 WHERE id = $2")
            .bind(id)
            .bind(owner_id)
            .execute(&self.pool)
            .await?;
        Ok(id)
    }

    /// `HasPicture::updatePhoto` — sets `photo_path`, returns the previous
    /// path so the caller can delete the old file.
    pub async fn set_photo(
        &self,
        id: i64,
        path: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        let prev: Option<String> =
            sqlx::query_scalar("SELECT photo_path FROM teams WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .flatten();
        sqlx::query("UPDATE teams SET photo_path = $1 WHERE id = $2")
            .bind(path)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(prev)
    }

    /// `HasPicture::deletePhoto`.
    pub async fn clear_photo(&self, id: i64) -> Result<Option<String>, sqlx::Error> {
        let prev: Option<String> =
            sqlx::query_scalar("SELECT photo_path FROM teams WHERE id = $1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?
                .flatten();
        sqlx::query("UPDATE teams SET photo_path = NULL WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(prev)
    }

    pub async fn rename(&self, id: i64, name: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE teams SET name = $1 WHERE id = $2")
            .bind(name)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn member_ids(&self, team_id: i64) -> Result<Vec<i64>, sqlx::Error> {
        sqlx::query_scalar("SELECT id FROM users WHERE team_id = $1 AND deleted_at IS NULL")
            .bind(team_id)
            .fetch_all(&self.pool)
            .await
    }

    /// `switchOwner` — team.user_id = new owner.
    pub async fn switch_owner(&self, team_id: i64, new_owner_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE teams SET user_id = $1 WHERE id = $2")
            .bind(new_owner_id)
            .bind(team_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `leave`/`removeMember` — users.team_id = NULL.
    pub async fn remove_member(&self, user_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET team_id = NULL WHERE id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `destroy` — clear all members then delete the team.
    pub async fn delete(&self, team_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET team_id = NULL WHERE team_id = $1")
            .bind(team_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM teams WHERE id = $1")
            .bind(team_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `TeamController::show` — score + stats + member contributions on
    /// public rooms only.
    pub async fn team_show(&self, team_id: i64) -> Result<serde_json::Value, sqlx::Error> {
        let score: f64 = sqlx::query_scalar(
            "SELECT COALESCE(ROUND(SUM(ts.score)::numeric, 1)::float8, 0) FROM total_scores ts \
             JOIN rooms r ON r.id = ts.room_id AND r.is_public = true \
             WHERE ts.totalscorable_type = 'App\\Models\\Team' \
             AND ts.totalscorable_id = $1",
        )
        .bind(team_id)
        .fetch_one(&self.pool)
        .await?;
        let agg = sqlx::query(
            "SELECT COUNT(DISTINCT rs.round_id) AS rounds_played, \
             MAX(rs.created_at) AS last_played_at \
             FROM round_standings rs \
             JOIN rooms r ON r.id = rs.room_id AND r.is_public = true \
             WHERE rs.team_id = $1",
        )
        .bind(team_id)
        .fetch_one(&self.pool)
        .await?;
        let rounds_played: i64 = agg.get("rounds_played");
        let last_played: Option<chrono::DateTime<chrono::Utc>> = agg.get("last_played_at");
        let members = sqlx::query(
            "SELECT u.id, u.name, u.photo_path, \
             COALESCE((SELECT SUM(rs2.total_score) FROM round_standings rs2 \
              JOIN rooms r2 ON r2.id = rs2.room_id AND r2.is_public = true \
              WHERE rs2.team_id = $1 AND rs2.user_id = u.id), 0) AS score, \
             (SELECT COUNT(DISTINCT rs3.round_id) FROM round_standings rs3 \
              JOIN rooms r3 ON r3.id = rs3.room_id AND r3.is_public = true \
              WHERE rs3.team_id = $2 AND rs3.user_id = u.id) AS rounds_played \
             FROM users u WHERE u.team_id = $3 AND u.deleted_at IS NULL \
             ORDER BY score DESC",
        )
        .bind(team_id)
        .bind(team_id)
        .bind(team_id)
        .fetch_all(&self.pool)
        .await?;
        let members_sum: f64 = members.iter().map(|r| r.get::<f64, _>("score")).sum();
        Ok(serde_json::json!({
            "score": score,
            "stats": {
                "rounds_played": rounds_played,
                "last_played_at": last_played,
                "avg_points_per_round": if rounds_played > 0 {
                    (score / rounds_played as f64 * 10.0).round() / 10.0
                } else { 0.0 },
            },
            "members": members.iter().map(|r| {
                let score: f64 = r.get("score");
                serde_json::json!({
                    "id": r.get::<i64, _>("id"),
                    "name": r.get::<String, _>("name"),
                    "photo": r.get::<Option<String>, _>("photo_path"),
                    "score": score,
                    "rounds_played": r.get::<i64, _>("rounds_played"),
                    "contribution_percent": if members_sum > 0.0 {
                        (score / members_sum * 100.0 * 10.0).round() / 10.0
                    } else { 0.0 },
                })
            }).collect::<Vec<_>>(),
        }))
    }

    // ---- team_requests -------------------------------------------------------

    /// Pending (non-declined) requests of a user — `teamRequests()->count()`.
    /// `teamRequest` pending for (team, user) — used by the SPA to show the
    /// join/cancel button state like `TeamController::show`'s `userRequest`.
    pub async fn has_pending_request(
        &self,
        team_id: i64,
        user_id: i64,
    ) -> Result<bool, sqlx::Error> {
        let n: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM team_requests WHERE team_id = $1 AND user_id = $2 \
             AND declined_at IS NULL",
        )
        .bind(team_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(n > 0)
    }

    /// `User::cachedTeamRequestIds` — `{pending, declined}` team_id lists
    /// used by the SPA to render join/cancel/declined button states.
    pub async fn team_request_ids(
        &self,
        user_id: i64,
    ) -> Result<(Vec<i64>, Vec<i64>), sqlx::Error> {
        let rows = sqlx::query(
            "SELECT team_id, declined_at IS NOT NULL AS declined \
             FROM team_requests WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        let pending = rows
            .iter()
            .filter(|r| !r.get::<bool, _>("declined"))
            .map(|r| r.get::<i64, _>("team_id"))
            .collect();
        let declined = rows
            .iter()
            .filter(|r| r.get::<bool, _>("declined"))
            .map(|r| r.get::<i64, _>("team_id"))
            .collect();
        Ok((pending, declined))
    }

    pub async fn pending_requests_count(&self, user_id: i64) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM team_requests WHERE user_id = $1 AND declined_at IS NULL",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
    }

    /// `TeamRequestController::store` — create the join request.
    pub async fn create_request(&self, team_id: i64, user_id: i64) -> Result<i64, sqlx::Error> {
        let now = chrono::Utc::now();
        let res = sqlx::query_scalar::<_, i64>(
            "INSERT INTO team_requests (team_id, user_id, created_at, updated_at) \
             VALUES ($1, $2, $3, $4) RETURNING id",
        )
        .bind(team_id)
        .bind(user_id)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        Ok(res)
    }

    pub async fn find_request(&self, id: i64) -> Result<Option<(i64, i64)>, sqlx::Error> {
        sqlx::query("SELECT team_id, user_id FROM team_requests WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map(|o| o.map(|r| (r.get("team_id"), r.get("user_id"))))
    }

    /// `approve()` — set member + delete ALL the user's pending requests.
    pub async fn approve_request(&self, request_id: i64) -> Result<bool, sqlx::Error> {
        let Some((team_id, user_id)) = self.find_request(request_id).await? else {
            return Ok(false);
        };
        sqlx::query("UPDATE users SET team_id = $1 WHERE id = $2")
            .bind(team_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        sqlx::query("DELETE FROM team_requests WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(true)
    }

    /// `reject()` — mark declined_at.
    pub async fn decline_request(&self, request_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE team_requests SET declined_at = $1 WHERE id = $2")
            .bind(chrono::Utc::now())
            .bind(request_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `cancel()` — delete the user's request on a team.
    pub async fn cancel_request(&self, team_id: i64, user_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM team_requests WHERE team_id = $1 AND user_id = $2")
            .bind(team_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
