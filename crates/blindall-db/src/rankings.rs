use sqlx::{PgPool, Row};
use serde_json::{json, Value};

const PER_PAGE: i64 = 10;
const TEAMS_PER_PAGE: i64 = 50;
const MINIGAMES_PER_PAGE: i64 = 50;

pub const SORTS: [&str; 6] = ["level", "score", "elo", "week", "avg_time", "best_round"];

#[derive(Clone)]
pub struct RankingsRepo {
    pool: PgPool,
}

struct RankRow {
    user_id: i64,
    #[allow(dead_code)]
    sort_value: f64,
}

struct Page {
    rows: Vec<RankRow>,
    total: i64,
    page: i64,
    per_page: i64,
}

impl Page {
    fn last_page(&self) -> i64 {
        (self.total + self.per_page - 1) / self.per_page.max(1)
    }
}

impl RankingsRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ---- scope helpers --------------------------------------------------------

    pub async fn official_rooms(&self) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, name FROM rooms WHERE is_public AND deleted_at IS NULL \
             ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| json!({"id": r.get::<i64, _>("id"), "name": r.get::<String, _>("name")}))
            .collect())
    }

    pub async fn resolve_official_room_id(
        &self,
        room_id: Option<i64>,
    ) -> Result<Option<i64>, sqlx::Error> {
        match room_id {
            Some(id) if id > 0 => {
                sqlx::query_scalar(
                    "SELECT id FROM rooms WHERE id = $1 AND is_public AND deleted_at IS NULL",
                )
                .bind(id)
                .fetch_optional(&self.pool)
                .await
            }
            _ => Ok(None),
        }
    }

    fn scoped_users_subquery(room_id: Option<i64>, n: usize) -> String {
        match room_id {
            Some(_) => {
                format!("SELECT DISTINCT user_id FROM round_standings WHERE room_id = ${n}")
            }
            None => {
                "SELECT DISTINCT rs.user_id FROM round_standings rs \
                 JOIN rooms r ON r.id = rs.room_id AND r.is_public"
                    .to_string()
            }
        }
    }

    fn standings_scope(room_id: Option<i64>, n: usize) -> String {
        match room_id {
            Some(_) => format!("FROM round_standings rs JOIN users u ON u.id = rs.user_id AND u.is_guest = false WHERE rs.room_id = ${n}"),
            None => {
                "FROM round_standings rs JOIN rooms r ON r.id = rs.room_id AND r.is_public \
                 WHERE 1=1"
                    .to_string()
            }
        }
    }

    fn user_scores_scope(room_id: Option<i64>, n: usize) -> String {
        match room_id {
            Some(_) => format!(
                "FROM total_scores ts WHERE ts.totalscorable_type = 'App\\Models\\User' \
                 AND ts.room_id = ${n}"
            ),
            None => {
                "FROM total_scores ts JOIN rooms r ON r.id = ts.room_id AND r.is_public \
                 WHERE ts.totalscorable_type = 'App\\Models\\User'"
                    .to_string()
            }
        }
    }

    // ---- paginators -----------------------------------------------------------

    async fn paginate(
        &self,
        sort: &str,
        page: i64,
        room_id: Option<i64>,
    ) -> Result<Page, sqlx::Error> {
        let page = page.max(1);
        let offset = (page - 1) * PER_PAGE;
        let sub = Self::scoped_users_subquery(room_id, 1);
        let (lim, off): (usize, usize) = if room_id.is_some() { (2, 3) } else { (1, 2) };

        let (rows_sql, count_sql) = match sort {
            "level" => (
                format!(
                    "SELECT ul.user_id, ul.level AS sort_value \
                     FROM user_levels ul JOIN users u ON u.id = ul.user_id \
                     WHERE u.deleted_at IS NULL AND u.is_guest = false {} \
                     ORDER BY ul.level DESC, ul.total_xp DESC LIMIT ${lim} OFFSET ${off}",
                    if room_id.is_some() {
                        format!("AND ul.user_id IN ({sub})")
                    } else {
                        String::new()
                    }
                ),
                format!(
                    "SELECT COUNT(*) FROM user_levels ul JOIN users u ON u.id = ul.user_id \
                     WHERE u.deleted_at IS NULL AND u.is_guest = false {}",
                    if room_id.is_some() {
                        format!("AND ul.user_id IN ({sub})")
                    } else {
                        String::new()
                    }
                ),
            ),
            "score" => {
                if room_id.is_some() {
                    (
                        format!(
                            "SELECT ts.totalscorable_id AS user_id, ROUND(SUM(ts.score)::numeric, 1)::float8 AS sort_value \
                             {} GROUP BY ts.totalscorable_id ORDER BY sort_value DESC LIMIT ${lim} OFFSET ${off}",
                            Self::user_scores_scope(room_id, 1)
                        ),
                        format!(
                            "SELECT COUNT(*) FROM (SELECT ts.totalscorable_id {} GROUP BY ts.totalscorable_id) x",
                            Self::user_scores_scope(room_id, 1)
                        ),
                    )
                } else {
                    (
                        "SELECT ul.user_id, ul.score_public_rooms AS sort_value \
                         FROM user_levels ul JOIN users u ON u.id = ul.user_id \
                         WHERE u.deleted_at IS NULL AND u.is_guest = false \
                         ORDER BY ul.score_public_rooms DESC, ul.total_xp DESC LIMIT $1 OFFSET $2"
                            .to_string(),
                        "SELECT COUNT(*) FROM user_levels ul JOIN users u ON u.id = ul.user_id \
                         WHERE u.deleted_at IS NULL AND u.is_guest = false"
                            .to_string(),
                    )
                }
            }
            "elo" => (
                if room_id.is_some() {
                    format!(
                        "SELECT id AS user_id, elo AS sort_value FROM users \
                         WHERE elo IS NOT NULL AND deleted_at IS NULL AND is_guest = false AND id IN ({sub}) \
                         ORDER BY elo DESC, id ASC LIMIT ${lim} OFFSET ${off}"
                    )
                } else {
                    "SELECT id AS user_id, elo AS sort_value FROM users \
                     WHERE elo IS NOT NULL AND deleted_at IS NULL AND is_guest = false \
                     ORDER BY elo DESC, id ASC LIMIT $1 OFFSET $2"
                        .to_string()
                },
                if room_id.is_some() {
                    format!(
                        "SELECT COUNT(*) FROM users \
                         WHERE elo IS NOT NULL AND deleted_at IS NULL AND is_guest = false AND id IN ({sub})"
                    )
                } else {
                    "SELECT COUNT(*) FROM users WHERE elo IS NOT NULL AND deleted_at IS NULL AND is_guest = false"
                        .to_string()
                },
            ),
            "week" => (
                format!(
                    "SELECT rs.user_id, ROUND(SUM(rs.total_score)::numeric, 1)::float8 AS sort_value \
                     {} AND rs.created_at >= now() - interval '7 days' \
                     GROUP BY rs.user_id ORDER BY sort_value DESC LIMIT ${lim} OFFSET ${off}",
                    Self::standings_scope(room_id, 1)
                ),
                format!(
                    "SELECT COUNT(*) FROM (SELECT rs.user_id {} \
                     AND rs.created_at >= now() - interval '7 days' \
                     GROUP BY rs.user_id) x",
                    Self::standings_scope(room_id, 1)
                ),
            ),
            "avg_time" => (
                format!(
                    "SELECT rs.user_id, ROUND(AVG(rs.average_response_time)::numeric, 3)::float8 AS sort_value \
                     {} AND rs.average_response_time IS NOT NULL \
                     GROUP BY rs.user_id ORDER BY sort_value ASC LIMIT ${lim} OFFSET ${off}",
                    Self::standings_scope(room_id, 1)
                ),
                format!(
                    "SELECT COUNT(*) FROM (SELECT rs.user_id {} \
                     AND rs.average_response_time IS NOT NULL GROUP BY rs.user_id) x",
                    Self::standings_scope(room_id, 1)
                ),
            ),
            "best_round" => {
                if room_id.is_some() {
                    (
                        format!(
                            "SELECT rs.user_id, ROUND(MAX(rs.total_score)::numeric, 1)::float8 AS sort_value \
                             {} GROUP BY rs.user_id ORDER BY sort_value DESC LIMIT ${lim} OFFSET ${off}",
                            Self::standings_scope(room_id, 1)
                        ),
                        format!(
                            "SELECT COUNT(*) FROM (SELECT rs.user_id {} GROUP BY rs.user_id) x",
                            Self::standings_scope(room_id, 1)
                        ),
                    )
                } else {
                    (
                        "SELECT ul.user_id, ul.best_round_score AS sort_value \
                         FROM user_levels ul JOIN users u ON u.id = ul.user_id \
                         WHERE u.deleted_at IS NULL AND u.is_guest = false \
                         ORDER BY ul.best_round_score DESC, ul.score_public_rooms DESC \
                         LIMIT $1 OFFSET $2"
                            .to_string(),
                        "SELECT COUNT(*) FROM user_levels ul JOIN users u ON u.id = ul.user_id \
                         WHERE u.deleted_at IS NULL AND u.is_guest = false"
                            .to_string(),
                    )
                }
            }
            _ => unreachable!("normalizeSort guards"),
        };

        let room_bind = room_id.is_some();
        let total: i64 = {
            let q = sqlx::query_scalar::<_, i64>(&count_sql);
            let q = if room_bind { q.bind(room_id.unwrap()) } else { q };
            q.fetch_one(&self.pool).await?
        };
        let rows = {
            let q = sqlx::query(&rows_sql);
            let q = if room_bind { q.bind(room_id.unwrap()) } else { q };
            q.bind(PER_PAGE)
                .bind(offset)
                .fetch_all(&self.pool)
                .await?
                .iter()
                .map(|r| RankRow {
                    user_id: r.get::<i64, _>("user_id"),
                    sort_value: r
                        .try_get::<f64, _>("sort_value")
                        .or_else(|_| r.try_get::<i64, _>("sort_value").map(|v| v as f64))
                        .unwrap_or(0.0),
                })
                .collect()
        };
        Ok(Page {
            rows,
            total,
            page,
            per_page: PER_PAGE,
        })
    }

    // ---- entry formatting -----------------------------------------------------

    async fn format_entries(
        &self,
        user_ids: &[i64],
        room_id: Option<i64>,
    ) -> Result<Vec<Value>, sqlx::Error> {
        if user_ids.is_empty() {
            return Ok(vec![]);
        }
        let user_rows = sqlx::query(
            "SELECT u.id, u.name, u.photo_path, u.elo, u.is_guest, \
             ul.level, ul.total_xp, ul.current_xp, ul.xp_for_next_level, \
             ul.score_public_rooms, ul.best_round_score AS lvl_best, ul.rounds_played_count \
             FROM users u LEFT JOIN user_levels ul ON ul.user_id = u.id \
             WHERE u.id = ANY($1)",
        )
        .bind(user_ids.to_vec())
        .fetch_all(&self.pool)
        .await?;
        let any_n = if room_id.is_some() { 2 } else { 1 };
        let stats_rows = {
            let scope = Self::standings_scope(room_id, 1);
            let q = format!(
                "SELECT rs.user_id, \
                 ROUND(SUM(CASE WHEN rs.created_at >= now() - interval '7 days' \
                     THEN rs.total_score ELSE 0 END)::numeric, 1)::float8 AS week_score, \
                 COUNT(*) AS rounds_played, \
                 ROUND(AVG(rs.average_response_time)::numeric, 3)::float8 AS avg_response_time, \
                 ROUND(MAX(rs.total_score)::numeric, 1)::float8 AS best_round_score, \
                 MAX(rs.win_streak) AS best_win_streak \
                 {scope} AND rs.user_id = ANY(${any_n}) GROUP BY rs.user_id"
            );
            let mut q = sqlx::query(&q);
            if let Some(rid) = room_id {
                q = q.bind(rid);
            }
            q.bind(user_ids.to_vec()).fetch_all(&self.pool).await?
        };
        let room_score_rows = if let Some(rid) = room_id {
            let scope = Self::user_scores_scope(room_id, 1);
            sqlx::query(&format!(
                "SELECT ts.totalscorable_id AS user_id, \
                 ROUND(SUM(ts.score)::numeric, 1)::float8 AS total_score \
                 {scope} AND ts.totalscorable_id = ANY($2) GROUP BY ts.totalscorable_id"
            ))
            .bind(rid)
            .bind(user_ids.to_vec())
            .fetch_all(&self.pool)
            .await?
        } else {
            vec![]
        };

        let stats_of = |uid: i64| stats_rows.iter().find(|r| r.get::<i64, _>("user_id") == uid);
        let room_score_of = |uid: i64| {
            room_score_rows
                .iter()
                .find(|r| r.get::<i64, _>("user_id") == uid)
                .map(|r| r.get::<f64, _>("total_score"))
        };
        let mut entries = vec![];
        for uid in user_ids {
            let Some(u) = user_rows.iter().find(|r| r.get::<i64, _>("id") == *uid) else {
                continue;
            };
            let st = stats_of(*uid);
            let level: Option<i64> = u.get("level");
            let elo: i64 = u.get::<Option<i64>, _>("elo").unwrap_or(1500);
            let score = match room_id {
                Some(_) => room_score_of(*uid).unwrap_or(0.0),
                None => u
                    .get::<Option<i64>, _>("score_public_rooms")
                    .unwrap_or(0) as f64,
            };
            let standing_best: f64 = st
                .and_then(|r| r.try_get::<f64, _>("best_round_score").ok())
                .unwrap_or(0.0);
            let best_round = match room_id {
                Some(_) => standing_best,
                None => standing_best.max(
                    u.get::<Option<f64>, _>("lvl_best").unwrap_or(0.0),
                ),
            };
            let rounds_played: i64 = st
                .map(|r| r.get::<i64, _>("rounds_played"))
                .or_else(|| u.get::<Option<i64>, _>("rounds_played_count"))
                .unwrap_or(0);
            entries.push(json!({
                "user": {
                    "id": u.get::<i64, _>("id"),
                    "name": u.get::<String, _>("name"),
                    "photo_path": u.get::<Option<String>, _>("photo_path"),
                    "elo": elo,
                    "is_guest": u.get::<bool, _>("is_guest"),
                    "userLevel": level.map(|l| json!({
                        "level": l,
                        "total_xp": u.get::<Option<i64>, _>("total_xp").unwrap_or(0),
                        "current_xp": u.get::<Option<i64>, _>("current_xp").unwrap_or(0),
                        "xp_for_next_level": u.get::<Option<i64>, _>("xp_for_next_level").unwrap_or(100),
                    })),
                },
                "stats": {
                    "level": level.unwrap_or(1),
                    "total_xp": u.get::<Option<i64>, _>("total_xp").unwrap_or(0),
                    "elo": elo,
                    "score": score,
                    "week_score": st.and_then(|r| r.try_get::<f64, _>("week_score").ok()).unwrap_or(0.0),
                    "rounds_played": rounds_played,
                    "avg_response_time": st.and_then(|r| r.try_get::<f64, _>("avg_response_time").ok()),
                    "best_round_score": best_round,
                    "best_win_streak": st.and_then(|r| r.try_get::<i64, _>("best_win_streak").ok()),
                },
            }));
        }
        Ok(entries)
    }

    pub async fn leaderboard(
        &self,
        sort: &str,
        page: i64,
        room_id: Option<i64>,
    ) -> Result<Value, sqlx::Error> {
        let sort = if SORTS.contains(&sort) { sort } else { "elo" };
        let page = self.paginate(sort, page, room_id).await?;
        let ids: Vec<i64> = page.rows.iter().map(|r| r.user_id).collect();
        let data = self.format_entries(&ids, room_id).await?;
        Ok(json!({
            "data": data,
            "total": page.total,
            "per_page": page.per_page,
            "current_page": page.page,
            "last_page": page.last_page().max(1),
        }))
    }

    // ---- user context -----------------------------------------------------------

    pub async fn user_context(
        &self,
        user_id: i64,
        sort: &str,
        room_id: Option<i64>,
    ) -> Result<Value, sqlx::Error> {
        let sort = if SORTS.contains(&sort) { sort } else { "elo" };
        let position = self.user_position(user_id, sort, room_id).await?;
        let entry = self
            .format_entries(&[user_id], room_id)
            .await?
            .into_iter()
            .next();
        Ok(json!({"position": position, "entry": entry}))
    }

    async fn user_position(
        &self,
        user_id: i64,
        sort: &str,
        room_id: Option<i64>,
    ) -> Result<Option<i64>, sqlx::Error> {
        let pos: Option<i64> = match sort {
            "level" => {
                let lvl = sqlx::query(
                    "SELECT level, total_xp FROM user_levels WHERE user_id = $1",
                )
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?;
                let Some(lvl) = lvl else { return Ok(None) };
                let (l, x): (i64, i64) = (lvl.get("level"), lvl.get("total_xp"));
                let (a, b, c): (usize, usize, usize) =
                    if room_id.is_some() { (2, 3, 4) } else { (1, 2, 3) };
                let q = format!(
                    "SELECT COUNT(*) + 1 FROM user_levels ul JOIN users u ON u.id = ul.user_id \
                     WHERE u.deleted_at IS NULL AND u.is_guest = false {} \
                     AND (ul.level > ${a} OR (ul.level = ${b} AND ul.total_xp > ${c}))",
                    if room_id.is_some() {
                        format!("AND ul.user_id IN ({})", Self::scoped_users_subquery(room_id, 1))
                    } else {
                        String::new()
                    }
                );
                let q = sqlx::query_scalar::<_, i64>(&q);
                let q = if room_id.is_some() { q.bind(room_id.unwrap()) } else { q };
                Some(q.bind(l).bind(l).bind(x).fetch_one(&self.pool).await?)
            }
            "score" => {
                if room_id.is_some() {
                    let my: f64 = sqlx::query_scalar(
                        "SELECT COALESCE(SUM(ts.score), 0) FROM total_scores ts \
                         WHERE ts.totalscorable_type = 'App\\Models\\User' \
                         AND ts.totalscorable_id = $1 AND ts.room_id = $2",
                    )
                    .bind(user_id)
                    .bind(room_id.unwrap())
                    .fetch_one(&self.pool)
                    .await?;
                    if my <= 0.0 {
                        return Ok(None);
                    }
                    Some(
                        sqlx::query_scalar::<_, i64>(
                            "SELECT COUNT(*) + 1 FROM (SELECT ts.totalscorable_id \
                             FROM total_scores ts WHERE ts.totalscorable_type = 'App\\Models\\User' \
                             AND ts.room_id = $1 GROUP BY ts.totalscorable_id \
                             HAVING ROUND(SUM(ts.score)::numeric, 1) > $2) x",
                        )
                        .bind(room_id.unwrap())
                        .bind(my)
                        .fetch_one(&self.pool)
                        .await?,
                    )
                } else {
                    let my: Option<i64> = sqlx::query_scalar(
                        "SELECT score_public_rooms FROM user_levels WHERE user_id = $1",
                    )
                    .bind(user_id)
                    .fetch_optional(&self.pool)
                    .await?;
                    match my {
                        Some(s) if s > 0 => Some(
                            sqlx::query_scalar::<_, i64>(
                                "SELECT COUNT(*) + 1 FROM user_levels ul JOIN users u \
                                 ON u.id = ul.user_id WHERE u.deleted_at IS NULL AND u.is_guest = false \
                                 AND ul.score_public_rooms > $1",
                            )
                            .bind(s)
                            .fetch_one(&self.pool)
                            .await?,
                        ),
                        _ => return Ok(None),
                    }
                }
            }
            "elo" => {
                let elo: i64 = sqlx::query_scalar(
                    "SELECT COALESCE(elo, 1500) FROM users WHERE id = $1",
                )
                .bind(user_id)
                .fetch_one(&self.pool)
                .await?;
                let q = format!(
                    "SELECT COUNT(*) + 1 FROM users WHERE elo IS NOT NULL AND is_guest = false AND deleted_at IS NULL AND elo > $1 {}",
                    if room_id.is_some() {
                        format!("AND id IN ({})", Self::scoped_users_subquery(room_id, 2))
                    } else {
                        String::new()
                    }
                );
                let q = sqlx::query_scalar::<_, i64>(&q).bind(elo);
                let q = if room_id.is_some() { q.bind(room_id.unwrap()) } else { q };
                Some(q.fetch_one(&self.pool).await?)
            }
            "week" => {
                let scope = Self::standings_scope(room_id, 1);
                let u_n = if room_id.is_some() { 2 } else { 1 };
                let q = format!(
                    "SELECT COALESCE(SUM(rs.total_score), 0) {scope} \
                     AND rs.user_id = ${u_n} AND rs.created_at >= now() - interval '7 days'"
                );
                let mut q = sqlx::query_scalar::<_, f64>(&q);
                if let Some(rid) = room_id {
                    q = q.bind(rid);
                }
                let my = q.bind(user_id).fetch_one(&self.pool).await?;
                if my <= 0.0 {
                    return Ok(None);
                }
                let scope = Self::standings_scope(room_id, 1);
                let my_n = if room_id.is_some() { 2 } else { 1 };
                let q = format!(
                    "SELECT COUNT(*) + 1 FROM (SELECT rs.user_id {scope} \
                     AND rs.created_at >= now() - interval '7 days' \
                     GROUP BY rs.user_id \
                     HAVING ROUND(SUM(rs.total_score)::numeric, 1) > ${my_n}) x"
                );
                let mut q = sqlx::query_scalar::<_, i64>(&q);
                if let Some(rid) = room_id {
                    q = q.bind(rid);
                }
                Some(q.bind(my).fetch_one(&self.pool).await?)
            }
            "avg_time" => {
                let scope = Self::standings_scope(room_id, 1);
                let u_n = if room_id.is_some() { 2 } else { 1 };
                let q = format!(
                    "SELECT AVG(rs.average_response_time) {scope} \
                     AND rs.user_id = ${u_n} AND rs.average_response_time IS NOT NULL"
                );
                let mut q = sqlx::query_scalar::<_, Option<f64>>(&q);
                if let Some(rid) = room_id {
                    q = q.bind(rid);
                }
                let Some(my) = q.bind(user_id).fetch_one(&self.pool).await? else {
                    return Ok(None);
                };
                let scope = Self::standings_scope(room_id, 1);
                let my_n = if room_id.is_some() { 2 } else { 1 };
                let q = format!(
                    "SELECT COUNT(*) + 1 FROM (SELECT rs.user_id {scope} \
                     AND rs.average_response_time IS NOT NULL GROUP BY rs.user_id \
                     HAVING AVG(rs.average_response_time) < ${my_n}) x"
                );
                let mut q = sqlx::query_scalar::<_, i64>(&q);
                if let Some(rid) = room_id {
                    q = q.bind(rid);
                }
                Some(q.bind(my).fetch_one(&self.pool).await?)
            }
            "best_round" => {
                if room_id.is_some() {
                    let my: f64 = sqlx::query_scalar(
                        "SELECT COALESCE(MAX(rs.total_score), 0) FROM round_standings rs \
                         WHERE rs.user_id = $1 AND rs.room_id = $2",
                    )
                    .bind(user_id)
                    .bind(room_id.unwrap())
                    .fetch_one(&self.pool)
                    .await?;
                    if my <= 0.0 {
                        return Ok(None);
                    }
                    Some(
                        sqlx::query_scalar::<_, i64>(
                            "SELECT COUNT(*) + 1 FROM (SELECT rs.user_id FROM round_standings rs \
                             WHERE rs.room_id = $1 GROUP BY rs.user_id \
                             HAVING MAX(rs.total_score) > $2) x",
                        )
                        .bind(room_id.unwrap())
                        .bind(my)
                        .fetch_one(&self.pool)
                        .await?,
                    )
                } else {
                    let lvl_best: f64 = sqlx::query_scalar(
                        "SELECT COALESCE(best_round_score, 0) FROM user_levels WHERE user_id = $1",
                    )
                    .bind(user_id)
                    .fetch_optional(&self.pool)
                    .await?
                    .map(|v: f64| v)
                    .unwrap_or(0.0);
                    let st_best: f64 = sqlx::query_scalar(
                        "SELECT COALESCE(MAX(rs.total_score), 0) FROM round_standings rs \
                         JOIN rooms r ON r.id = rs.room_id AND r.is_public \
                         WHERE rs.user_id = $1",
                    )
                    .bind(user_id)
                    .fetch_one(&self.pool)
                    .await?;
                    let my = lvl_best.max(st_best);
                    if my <= 0.0 {
                        return Ok(None);
                    }
                    Some(
                        sqlx::query_scalar::<_, i64>(
                            "SELECT COUNT(*) + 1 FROM user_levels ul JOIN users u \
                             ON u.id = ul.user_id WHERE u.deleted_at IS NULL AND u.is_guest = false \
                             AND ul.best_round_score > $1",
                        )
                        .bind(my)
                        .fetch_one(&self.pool)
                        .await?,
                    )
                }
            }
            _ => None,
        };
        Ok(pos)
    }

    // ---- teams ----------------------------------------------------------------

    pub async fn teams_leaderboard(&self, page: i64) -> Result<Value, sqlx::Error> {
        let page = page.max(1);
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM (SELECT ts.totalscorable_id FROM total_scores ts \
             JOIN teams t ON t.id = ts.totalscorable_id AND t.deleted_at IS NULL \
             JOIN rooms r ON r.id = ts.room_id AND r.is_public \
             WHERE ts.totalscorable_type = 'App\\Models\\Team' \
             GROUP BY ts.totalscorable_id) x",
        )
        .fetch_one(&self.pool)
        .await?;
        let rows = sqlx::query(
            "SELECT ts.totalscorable_id AS team_id, t.name, t.photo_path, \
             ROUND(SUM(ts.score)::numeric, 1)::float8 AS total_score \
             FROM total_scores ts \
             JOIN teams t ON t.id = ts.totalscorable_id AND t.deleted_at IS NULL \
             JOIN rooms r ON r.id = ts.room_id AND r.is_public \
             WHERE ts.totalscorable_type = 'App\\Models\\Team' \
             GROUP BY ts.totalscorable_id, t.name, t.photo_path \
             ORDER BY total_score DESC LIMIT $1 OFFSET $2",
        )
        .bind(TEAMS_PER_PAGE)
        .bind((page - 1) * TEAMS_PER_PAGE)
        .fetch_all(&self.pool)
        .await?;
        let data: Vec<Value> = rows
            .iter()
            .map(|r| {
                json!({
                    "team": {
                        "id": r.get::<i64, _>("team_id"),
                        "name": r.get::<String, _>("name"),
                        "photo_path": r.get::<Option<String>, _>("photo_path"),
                    },
                    "total_score": r.get::<f64, _>("total_score"),
                })
            })
            .collect();
        Ok(json!({
            "data": data,
            "total": total,
            "per_page": TEAMS_PER_PAGE,
            "current_page": page,
            "last_page": ((total + TEAMS_PER_PAGE - 1) / TEAMS_PER_PAGE).max(1),
        }))
    }

    pub async fn user_team_position(&self, user_id: i64) -> Result<Value, sqlx::Error> {
        let team_id: Option<i64> =
            sqlx::query_scalar("SELECT team_id FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_optional(&self.pool)
                .await?
                .flatten();
        let Some(team_id) = team_id else {
            return Ok(json!({"position": null, "team_score": 0}));
        };
        let my: f64 = sqlx::query_scalar(
            "SELECT COALESCE(ROUND(SUM(ts.score)::numeric, 1)::float8, 0) FROM total_scores ts \
             JOIN rooms r ON r.id = ts.room_id AND r.is_public \
             WHERE ts.totalscorable_type = 'App\\Models\\Team' AND ts.totalscorable_id = $1",
        )
        .bind(team_id)
        .fetch_one(&self.pool)
        .await?;
        if my <= 0.0 {
            return Ok(json!({"position": null, "team_score": 0}));
        }
        let position: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) + 1 FROM (SELECT ts.totalscorable_id FROM total_scores ts \
             JOIN teams t ON t.id = ts.totalscorable_id AND t.deleted_at IS NULL \
             JOIN rooms r ON r.id = ts.room_id AND r.is_public \
             WHERE ts.totalscorable_type = 'App\\Models\\Team' \
             GROUP BY ts.totalscorable_id HAVING ROUND(SUM(ts.score)::numeric, 1) > $1) x",
        )
        .bind(my)
        .fetch_one(&self.pool)
        .await?;
        Ok(json!({"position": position, "team_score": my, "team_id": team_id}))
    }

    // ---- minigames ------------------------------------------------------------

    pub async fn minigames_leaderboard(&self, page: i64) -> Result<Value, sqlx::Error> {
        let page = page.max(1);
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM (SELECT user_id FROM minigame_scores GROUP BY user_id) x",
        )
        .fetch_one(&self.pool)
        .await?;
        let rows = sqlx::query(
            "SELECT ms.user_id, CAST(SUM(ms.score) AS bigint) AS total_score, \
             u.name, u.photo_path, u.elo, \
             ul.level, ul.total_xp, ul.current_xp, ul.xp_for_next_level \
             FROM minigame_scores ms \
             JOIN users u ON u.id = ms.user_id AND u.deleted_at IS NULL AND u.is_guest = false \
             LEFT JOIN user_levels ul ON ul.user_id = u.id \
             GROUP BY ms.user_id, u.name, u.photo_path, u.elo, \
             ul.level, ul.total_xp, ul.current_xp, ul.xp_for_next_level \
             ORDER BY total_score DESC LIMIT $1 OFFSET $2",
        )
        .bind(MINIGAMES_PER_PAGE)
        .bind((page - 1) * MINIGAMES_PER_PAGE)
        .fetch_all(&self.pool)
        .await?;
        let data: Vec<Value> = rows
            .iter()
            .map(|r| {
                let level: Option<i64> = r.get("level");
                json!({
                    "user": {
                        "id": r.get::<i64, _>("user_id"),
                        "name": r.get::<String, _>("name"),
                        "photo_path": r.get::<Option<String>, _>("photo_path"),
                        "elo": r.get::<Option<i64>, _>("elo").unwrap_or(1500),
                        "userLevel": level.map(|l| json!({
                            "level": l,
                            "total_xp": r.get::<Option<i64>, _>("total_xp").unwrap_or(0),
                            "current_xp": r.get::<Option<i64>, _>("current_xp").unwrap_or(0),
                            "xp_for_next_level": r.get::<Option<i64>, _>("xp_for_next_level").unwrap_or(100),
                        })),
                    },
                    "total_score": r.get::<i64, _>("total_score"),
                })
            })
            .collect();
        Ok(json!({
            "data": data,
            "total": total,
            "per_page": MINIGAMES_PER_PAGE,
            "current_page": page,
            "last_page": ((total + MINIGAMES_PER_PAGE - 1) / MINIGAMES_PER_PAGE).max(1),
        }))
    }

    pub async fn minigames_user_context(&self, user_id: i64, is_guest: bool) -> Result<Value, sqlx::Error> {
        if is_guest {
            return Ok(json!({"position": null, "score": 0}));
        }
        let score: i64 = sqlx::query_scalar(
            "SELECT COALESCE(CAST(SUM(score) AS bigint), 0) FROM minigame_scores WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        if score <= 0 {
            return Ok(json!({"position": null, "score": 0}));
        }
        let position: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) + 1 FROM (SELECT user_id, SUM(score) AS s \
             FROM minigame_scores GROUP BY user_id HAVING SUM(score) > $1) x",
        )
        .bind(score)
        .fetch_one(&self.pool)
        .await?;
        Ok(json!({"position": position, "score": score}))
    }

    // ---- level metrics --------------------------------------------------------

    pub async fn user_level_metrics(&self, user_id: i64) -> Result<Value, sqlx::Error> {
        let row = sqlx::query(
            "SELECT level, current_xp, xp_for_next_level, total_xp, \
             score_public_rooms, minigame_scores_total, months_seniority, \
             consecutive_days_streak, rooms_created_count, playlists_created_count, \
             tracks_liked_count, season_bonus_xp \
             FROM user_levels WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        let has_team: bool = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT team_id FROM users WHERE id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        .flatten()
        .is_some();
        let Some(r) = row else {
            return Ok(json!({
                "level": 1, "current_xp": 0, "xp_for_next_level": 100, "total_xp": 0,
                "level_metrics": null,
            }));
        };
        Ok(json!({
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
                "season_bonus_xp": r.get::<i64, _>("season_bonus_xp"),
                "has_team": has_team,
            },
        }))
    }
}
