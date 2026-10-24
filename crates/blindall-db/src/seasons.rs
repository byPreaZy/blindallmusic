use chrono::{DateTime, Datelike, Duration, TimeZone, Utc};
use serde_json::{Value, json};
use sqlx::{PgPool, Row};

const PODIUM_SIZE: i64 = 50;
const BOARD_PER_PAGE: i64 = 50;

fn reward_xp(rank: i64) -> i64 {
    match rank {
        1 => 500,
        2 => 300,
        3 => 200,
        4..=10 => 100,
        _ => 25,
    }
}

/// Monday 00:00 UTC → next Monday 00:00 UTC.
fn week_window(t: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
    let days_since_monday = t.weekday().num_days_from_monday() as i64;
    let start = Utc
        .with_ymd_and_hms(t.year(), t.month(), t.day(), 0, 0, 0)
        .single()
        .unwrap_or(t)
        - Duration::days(days_since_monday);
    (start, start + Duration::days(7))
}

fn season_name(start: DateTime<Utc>) -> String {
    let iso = start.iso_week();
    format!("S{}-W{:02}", iso.year(), iso.week())
}

#[derive(Clone)]
pub struct SeasonRepo {
    pool: PgPool,
}

impl SeasonRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn row_to_json(r: &sqlx::postgres::PgRow) -> Value {
        let ends_at: DateTime<Utc> = r.get("ends_at");
        json!({
            "id": r.get::<i64, _>("id"),
            "name": r.get::<String, _>("name"),
            "starts_at": r.get::<DateTime<Utc>, _>("starts_at").to_rfc3339(),
            "ends_at": ends_at.to_rfc3339(),
            "ends_in_seconds": (ends_at - Utc::now()).num_seconds().max(0),
            "finalized": r.get::<Option<Value>, _>("podium").is_some(),
        })
    }

    /// Returns the active season, finalizing expired ones and creating the next
    /// weekly window when needed (lazy lifecycle — no scheduler required).
    /// The second tuple item lists users who just received season rewards so
    /// the caller can refresh their computed level.
    pub async fn ensure_current(
        &self,
        notifications: &crate::NotificationRepo,
    ) -> Result<(Value, Vec<i64>), sqlx::Error> {
        let rewarded = self.finalize_expired(notifications).await?;

        let now = Utc::now();
        if let Some(r) = sqlx::query(
            "SELECT id, name, starts_at, ends_at, podium FROM seasons \
             WHERE starts_at <= $1 AND ends_at > $1 ORDER BY ends_at LIMIT 1",
        )
        .bind(now)
        .fetch_optional(&self.pool)
        .await?
        {
            return Ok((Self::row_to_json(&r), rewarded));
        }

        // No active season: resume right after the latest known window so seasons
        // stay contiguous, otherwise start a fresh ISO week.
        let last_end: Option<DateTime<Utc>> =
            sqlx::query_scalar("SELECT MAX(ends_at) FROM seasons")
                .fetch_one(&self.pool)
                .await?;
        let (mut start, _) = week_window(now);
        if let Some(e) = last_end {
            if e > start {
                start = e;
            }
        }
        let end = start + Duration::days(7);
        let name = season_name(start);
        let r = sqlx::query(
            "INSERT INTO seasons (name, starts_at, ends_at) VALUES ($1, $2, $3) \
             ON CONFLICT DO NOTHING \
             RETURNING id, name, starts_at, ends_at, podium",
        )
        .bind(&name)
        .bind(start)
        .bind(end)
        .fetch_optional(&self.pool)
        .await?;
        match r {
            Some(r) => Ok((Self::row_to_json(&r), rewarded)),
            None => {
                let r = sqlx::query(
                    "SELECT id, name, starts_at, ends_at, podium FROM seasons \
                     WHERE starts_at <= $1 AND ends_at > $1 ORDER BY ends_at LIMIT 1",
                )
                .bind(now)
                .fetch_one(&self.pool)
                .await?;
                Ok((Self::row_to_json(&r), rewarded))
            }
        }
    }

    /// Snapshot the podium and grant XP + notifications for every season that
    /// ended without being finalized yet. Returns the rewarded user ids.
    async fn finalize_expired(
        &self,
        notifications: &crate::NotificationRepo,
    ) -> Result<Vec<i64>, sqlx::Error> {
        let mut rewarded = Vec::new();
        let expired = sqlx::query(
            "SELECT id, name, starts_at, ends_at FROM seasons \
             WHERE ends_at <= NOW() AND podium IS NULL",
        )
        .fetch_all(&self.pool)
        .await?;
        for s in expired {
            let sid: i64 = s.get("id");
            let name: String = s.get("name");
            let start: DateTime<Utc> = s.get("starts_at");
            let end: DateTime<Utc> = s.get("ends_at");

            let rows = sqlx::query(
                "SELECT rs.user_id, ROUND(SUM(rs.total_score)::numeric, 1)::float8 AS total \
                 FROM round_standings rs \
                 JOIN rooms r ON r.id = rs.room_id AND r.is_public \
                 JOIN users u ON u.id = rs.user_id AND u.is_guest = false \
                 WHERE rs.created_at >= $1 AND rs.created_at < $2 \
                 GROUP BY rs.user_id ORDER BY total DESC LIMIT $3",
            )
            .bind(start)
            .bind(end)
            .bind(PODIUM_SIZE)
            .fetch_all(&self.pool)
            .await?;

            let ids: Vec<i64> = rows.iter().map(|r| r.get("user_id")).collect();
            let names: std::collections::HashMap<i64, (String, Option<String>)> = if ids.is_empty()
            {
                Default::default()
            } else {
                sqlx::query("SELECT id, name, photo_path FROM users WHERE id = ANY($1)")
                    .bind(&ids)
                    .fetch_all(&self.pool)
                    .await?
                    .iter()
                    .map(|r| {
                        (
                            r.get::<i64, _>("id"),
                            (
                                r.get::<String, _>("name"),
                                r.get::<Option<String>, _>("photo_path"),
                            ),
                        )
                    })
                    .collect()
            };

            let podium: Vec<Value> = rows
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let uid: i64 = r.get("user_id");
                    let rank = (i + 1) as i64;
                    let (uname, photo) = names.get(&uid).cloned().unwrap_or_default();
                    json!({
                        "rank": rank,
                        "user_id": uid,
                        "name": uname,
                        "photo_path": photo,
                        "total": r.get::<f64, _>("total"),
                        "reward_xp": reward_xp(rank),
                    })
                })
                .collect();

            sqlx::query("UPDATE seasons SET podium = $1::jsonb, updated_at = NOW() WHERE id = $2")
                .bind(json!(podium).to_string())
                .bind(sid)
                .execute(&self.pool)
                .await?;

            // XP rewards + a notification per ranked player.
            for p in &podium {
                let uid = p["user_id"].as_i64().unwrap_or(0);
                let xp = p["reward_xp"].as_i64().unwrap_or(0);
                let rank = p["rank"].as_i64().unwrap_or(0);
                sqlx::query(
                    "INSERT INTO user_levels (user_id, season_bonus_xp, created_at, updated_at) \
                     VALUES ($2, $1, NOW(), NOW()) \
                     ON CONFLICT (user_id) DO UPDATE \
                     SET season_bonus_xp = user_levels.season_bonus_xp + $1, updated_at = NOW()",
                )
                .bind(xp)
                .bind(uid)
                .execute(&self.pool)
                .await?;
                rewarded.push(uid);
                let _ = notifications
                    .notify(
                        uid,
                        "App\\Notifications\\SeasonResult",
                        json!({
                            "season": name,
                            "rank": rank,
                            "xp": xp,
                            "message": format!("{name} : tu termines #{rank} et gagnes {xp} XP bonus !"),
                        }),
                    )
                    .await;
            }

            sqlx::query(
                "UPDATE seasons SET rewards_granted_at = NOW(), updated_at = NOW() WHERE id = $1",
            )
            .bind(sid)
            .execute(&self.pool)
            .await?;
        }
        Ok(rewarded)
    }

    /// Live standings inside the season window (public rooms only).
    pub async fn leaderboard(&self, season_id: i64, page: i64) -> Result<Value, sqlx::Error> {
        let s =
            sqlx::query("SELECT id, name, starts_at, ends_at, podium FROM seasons WHERE id = $1")
                .bind(season_id)
                .fetch_optional(&self.pool)
                .await?;
        let Some(s) = s else {
            return Ok(Value::Null);
        };
        let start: DateTime<Utc> = s.get("starts_at");
        let end: DateTime<Utc> = s.get("ends_at");
        let page = page.max(1);
        let offset = (page - 1) * BOARD_PER_PAGE;

        let totals: Vec<sqlx::postgres::PgRow> = sqlx::query(
            "SELECT rs.user_id, ROUND(SUM(rs.total_score)::numeric, 1)::float8 AS total, \
             COUNT(*) AS rounds_played, \
             ROUND(MAX(rs.total_score)::numeric, 1)::float8 AS best_round \
             FROM round_standings rs \
             JOIN rooms r ON r.id = rs.room_id AND r.is_public \
                 JOIN users u ON u.id = rs.user_id AND u.is_guest = false \
             WHERE rs.created_at >= $1 AND rs.created_at < $2 \
             GROUP BY rs.user_id ORDER BY total DESC \
             LIMIT $3 OFFSET $4",
        )
        .bind(start)
        .bind(end)
        .bind(BOARD_PER_PAGE)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;

        let total_players: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT rs.user_id) FROM round_standings rs \
             JOIN rooms r ON r.id = rs.room_id AND r.is_public \
                 JOIN users u ON u.id = rs.user_id AND u.is_guest = false \
             WHERE rs.created_at >= $1 AND rs.created_at < $2",
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.pool)
        .await?;

        let ids: Vec<i64> = totals.iter().map(|r| r.get("user_id")).collect();
        let users: std::collections::HashMap<i64, Value> = if ids.is_empty() {
            Default::default()
        } else {
            sqlx::query(
                "SELECT u.id, u.name, u.photo_path, u.elo, u.is_guest, l.level, \
                 u.is_administrator, \
                 CASE WHEN EXISTS(SELECT 1 FROM donations dn WHERE dn.user_id = u.id AND dn.month_key = $2) \
                    OR EXISTS(SELECT 1 FROM user_subscriptions s WHERE s.user_id = u.id \
                      AND s.status IN ('active','trialing','past_due') \
                      AND (s.current_period_end IS NULL OR s.current_period_end > now())) \
                 THEN u.custom_tag ELSE '' END AS custom_tag, \
                 EXISTS(SELECT 1 FROM moderables m \
                  JOIN rooms rm ON rm.id = m.moderable_id \
                  WHERE m.user_id = u.id AND m.moderable_type = 'App\\Models\\Room' \
                  AND rm.is_public = true) AS is_moderator \
                 FROM users u LEFT JOIN user_levels l ON l.user_id = u.id \
                 WHERE u.id = ANY($1)",
            )
            .bind(&ids)
            .bind(crate::donations::current_month_key())
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|r| {
                (
                    r.get::<i64, _>("id"),
                    json!({
                        "id": r.get::<i64, _>("id"),
                        "name": r.get::<String, _>("name"),
                        "photo_path": r.get::<Option<String>, _>("photo_path"),
                        "elo": r.try_get::<i64, _>("elo").unwrap_or(1500),
                        "is_guest": r.get::<bool, _>("is_guest"),
                        "level": r.try_get::<i64, _>("level").unwrap_or(1),
                        "role": if r.get::<bool, _>("is_administrator") { "admin" }
                            else if r.get::<bool, _>("is_moderator") { "moderator" } else { "" },
                        "custom_tag": r.get::<Option<String>, _>("custom_tag").unwrap_or_default(),
                    }),
                )
            })
            .collect()
        };

        let data: Vec<Value> = totals
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let uid: i64 = r.get("user_id");
                json!({
                    "rank": offset + (i as i64) + 1,
                    "user_id": uid,
                    "total": r.get::<f64, _>("total"),
                    "rounds_played": r.get::<i64, _>("rounds_played"),
                    "best_round": r.try_get::<f64, _>("best_round").ok(),
                    "user": users.get(&uid).cloned().unwrap_or(Value::Null),
                })
            })
            .collect();

        Ok(json!({
            "season": Self::row_to_json(&s),
            "podium": s.get::<Option<Value>, _>("podium"),
            "data": data,
            "total": total_players,
            "page": page,
            "last_page": (total_players + BOARD_PER_PAGE - 1) / BOARD_PER_PAGE,
        }))
    }

    /// Current-season rank for one user.
    pub async fn user_rank(&self, season_id: i64, user_id: i64) -> Result<Value, sqlx::Error> {
        let s = sqlx::query("SELECT starts_at, ends_at FROM seasons WHERE id = $1")
            .bind(season_id)
            .fetch_optional(&self.pool)
            .await?;
        let Some(s) = s else {
            return Ok(Value::Null);
        };
        let start: DateTime<Utc> = s.get("starts_at");
        let end: DateTime<Utc> = s.get("ends_at");
        let row = sqlx::query(
            "SELECT rank, total FROM ( \
                 SELECT rs.user_id, ROUND(SUM(rs.total_score)::numeric, 1)::float8 AS total, \
                 RANK() OVER (ORDER BY SUM(rs.total_score) DESC) AS rank \
                 FROM round_standings rs \
                 JOIN rooms r ON r.id = rs.room_id AND r.is_public \
                 JOIN users u ON u.id = rs.user_id AND u.is_guest = false \
                 WHERE rs.created_at >= $1 AND rs.created_at < $2 \
                 GROUP BY rs.user_id) t WHERE t.user_id = $3",
        )
        .bind(start)
        .bind(end)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(match row {
            Some(r) => json!({
                "rank": r.get::<i64, _>("rank"),
                "total": r.get::<f64, _>("total"),
            }),
            None => Value::Null,
        })
    }

    /// Recently finalized seasons with their podium snapshot.
    pub async fn past(&self, limit: i64) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, name, ends_at, podium FROM seasons \
             WHERE podium IS NOT NULL ORDER BY ends_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                json!({
                    "id": r.get::<i64, _>("id"),
                    "name": r.get::<String, _>("name"),
                    "ends_at": r.get::<DateTime<Utc>, _>("ends_at").to_rfc3339(),
                    "podium": r.get::<Option<Value>, _>("podium"),
                })
            })
            .collect())
    }
}
