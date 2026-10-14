//! `donations` — `DonationGoalService` + `DonorPerkService` port.
//! Month keys are `Y-m` in `Europe/Paris`; the monthly goal carries surplus
//! forward (`carryover`).

use chrono::{Datelike, Duration, TimeZone};
use chrono_tz::Europe::Paris;
use serde_json::{json, Value};
use sqlx::postgres::PgPool;
use sqlx::Row;

#[derive(Clone)]
pub struct DonationRepo {
    pool: PgPool,
}

/// `month_key` for a UTC instant in Europe/Paris.
pub fn month_key(ts: chrono::DateTime<chrono::Utc>) -> String {
    ts.with_timezone(&Paris).format("%Y-%m").to_string()
}

pub fn current_month_key() -> String {
    month_key(chrono::Utc::now())
}

/// First instant (UTC) of a `Y-m` month in Europe/Paris.
fn month_start(key: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    let (y, m) = key.split_once('-')?;
    Paris
        .with_ymd_and_hms(y.parse().ok()?, m.parse().ok()?, 1, 0, 0, 0)
        .single()
        .map(|d| d.with_timezone(&chrono::Utc))
}

/// `Y-m` of the previous month.
pub fn prev_month_key(key: &str) -> Option<String> {
    let start = month_start(key)?;
    let prev = start - Duration::days(1);
    Some(month_key(prev))
}

impl DonationRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// `raisedCentsForMonth`.
    pub async fn raised_cents(&self, month_key: &str) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT CAST(COALESCE(SUM(amount_cents), 0) AS bigint) FROM donations WHERE month_key = $1",
        )
        .bind(month_key)
        .fetch_one(&self.pool)
        .await
    }

    /// `carryoverCentsBeforeMonth` — walk from the first donation month to
    /// `month_key`, accumulating `max(0, available - goal)`.
    pub async fn carryover_cents_before_month(
        &self,
        month_key: &str,
        goal_cents: i64,
    ) -> Result<i64, sqlx::Error> {
        let first: Option<String> = sqlx::query_scalar(
            "SELECT month_key FROM donations ORDER BY month_key LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        let Some(first) = first.filter(|s| !s.is_empty()) else {
            return Ok(0);
        };
        if first.as_str() >= month_key {
            return Ok(0);
        }
        let mut carryover = 0i64;
        let mut cursor = first;
        while cursor.as_str() < month_key {
            let available = carryover + self.raised_cents(&cursor).await?;
            carryover = (available - goal_cents).max(0);
            let Some(next) = next_month_key(&cursor) else {
                break;
            };
            cursor = next;
        }
        Ok(carryover)
    }

    /// `monthProgress` — raised + carryover + goal flags.
    pub async fn month_progress(
        &self,
        month_key: &str,
        goal_cents: i64,
    ) -> Result<Value, sqlx::Error> {
        let raised = self.raised_cents(month_key).await?;
        let carryover = self.carryover_cents_before_month(month_key, goal_cents).await?;
        Ok(progress_json(month_key, raised, carryover, goal_cents))
    }

    /// `Donation::create` on `checkout.session.completed` — idempotent on
    /// `stripe_checkout_session_id`. Returns `(id, is_new)`; duplicates give
    /// `(existing_id, false)` so callers skip the thank-you notify like PHP.
    pub async fn record_checkout_session(
        &self,
        session_id: &str,
        amount_cents: i64,
        currency: &str,
        user_id: Option<i64>,
        donor_email: Option<&str>,
        donated_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(i64, bool), sqlx::Error> {
        if let Some(existing) = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM donations WHERE stripe_checkout_session_id = $1",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?
        {
            return Ok((existing, false));
        }
        let now = chrono::Utc::now();
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO donations (stripe_checkout_session_id, amount_cents, \
             currency, month_key, user_id, donor_email, donated_at, \
             created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) RETURNING id",
        )
        .bind(session_id)
        .bind(amount_cents)
        .bind(currency.to_lowercase())
        .bind(month_key(donated_at))
        .bind(user_id)
        .bind(donor_email)
        .bind(donated_at)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        .map(|r| (r, true))
    }

    /// `resolveUser` — by `client_reference_id` then by lowercased email;
    /// guests never match. Returns the user id.
    pub async fn resolve_donor(
        &self,
        client_reference_id: Option<&str>,
        donor_email: Option<&str>,
    ) -> Result<Option<i64>, sqlx::Error> {
        if let Some(id) = client_reference_id {
            if id.chars().all(|c| c.is_ascii_digit()) && !id.is_empty() {
                let uid: Option<i64> = sqlx::query_scalar(
                    "SELECT id FROM users WHERE id = $1 AND is_guest = false AND deleted_at IS NULL",
                )
                .bind(id.parse::<i64>().unwrap_or(0))
                .fetch_optional(&self.pool)
                .await?;
                if uid.is_some() {
                    return Ok(uid);
                }
            }
        }
        let Some(email) = donor_email else {
            return Ok(None);
        };
        sqlx::query_scalar::<_, i64>(
            "SELECT id FROM users WHERE LOWER(email) = $1 AND is_guest = false \
             AND deleted_at IS NULL LIMIT 1",
        )
        .bind(email.to_lowercase())
        .fetch_optional(&self.pool)
        .await
    }

    /// `userIsSupporter`.
    pub async fn user_is_supporter(&self, user_id: i64, month_key: &str) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM donations \
             WHERE month_key = $1 AND user_id = $2)::int::bigint",
        )
        .bind(month_key)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?
            == 1)
    }

    /// `perkMapForUserIds` — supporter ids among `user_ids` this month.
    pub async fn supporter_ids(
        &self,
        user_ids: &[i64],
        month_key: &str,
    ) -> Result<Vec<i64>, sqlx::Error> {
        if user_ids.is_empty() {
            return Ok(vec![]);
        }
        sqlx::query_scalar::<_, i64>(
            "SELECT DISTINCT user_id FROM donations \
             WHERE month_key = $1 AND user_id = ANY($2)",
        )
        .bind(month_key)
        .bind(user_ids.to_vec())
        .fetch_all(&self.pool)
        .await
    }

    /// `monthlySupporters` — distinct users who donated this month, latest
    /// first, plus carryover supporters (handled by caller via
    /// `post_goal_supporters`).
    pub async fn month_donors(&self, month_key: &str) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT d.user_id, d.donated_at, u.name, u.photo_path \
             FROM donations d JOIN users u ON u.id = d.user_id \
             WHERE d.month_key = $1 AND d.user_id IS NOT NULL \
             ORDER BY d.donated_at DESC, d.id DESC",
        )
        .bind(month_key)
        .fetch_all(&self.pool)
        .await?;
        let mut seen = std::collections::HashSet::new();
        Ok(rows
            .iter()
            .filter(|r| seen.insert(r.get::<i64, _>("user_id")))
            .map(|r| {
                json!({
                    "id": r.get::<i64, _>("user_id"),
                    "name": r.get::<String, _>("name"),
                    "photo_path": r.get::<Option<String>, _>("photo_path"),
                })
            })
            .collect())
    }

    /// `postGoalSupporters` — donors whose contribution landed at/after the
    /// goal line (running total including carryover), unique per user.
    pub async fn post_goal_supporters(
        &self,
        month_key: &str,
        goal_cents: i64,
    ) -> Result<Vec<Value>, sqlx::Error> {
        let mut running = self.carryover_cents_before_month(month_key, goal_cents).await?;
        let rows = sqlx::query(
            "SELECT d.user_id, d.amount_cents, d.donated_at, d.id, \
             u.name, u.photo_path \
             FROM donations d LEFT JOIN users u ON u.id = d.user_id \
             WHERE d.month_key = $1 ORDER BY d.donated_at, d.id",
        )
        .bind(month_key)
        .fetch_all(&self.pool)
        .await?;
        let mut post_goal: Vec<(i64, chrono::DateTime<chrono::Utc>, i64, Value)> = vec![];
        for r in &rows {
            let before = running;
            running += r.get::<i64, _>("amount_cents");
            let uid: Option<i64> = r.get("user_id");
            if let Some(uid) = uid {
                if before >= goal_cents || running > goal_cents {
                    post_goal.push((
                        uid,
                        r.get("donated_at"),
                        r.get("id"),
                        json!({
                            "id": uid,
                            "name": r.get::<Option<String>, _>("name"),
                            "photo_path": r.get::<Option<String>, _>("photo_path"),
                        }),
                    ));
                }
            }
        }
        post_goal.sort_by(|a, b| (b.1, b.2).cmp(&(a.1, a.2)));
        let mut seen = std::collections::HashSet::new();
        Ok(post_goal
            .into_iter()
            .filter(|(uid, _, _, _)| seen.insert(*uid))
            .map(|(_, _, _, v)| v)
            .collect())
    }

    /// `monthlyHistory` — per-month aggregates (desc), caller walks carryover
    /// forward like PHP does.
    pub async fn month_aggregates(
        &self,
        since_key: &str,
    ) -> Result<Vec<(String, i64, i64)>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT month_key, CAST(SUM(amount_cents) AS bigint) AS raised, COUNT(*) AS cnt \
             FROM donations WHERE month_key >= $1 GROUP BY month_key ORDER BY month_key",
        )
        .bind(since_key)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                (
                    r.get::<String, _>("month_key"),
                    r.get::<i64, _>("raised"),
                    r.get::<i64, _>("cnt"),
                )
            })
            .collect())
    }

    /// `recentDonations` — latest donations with optional user join.
    pub async fn recent_donations(&self, limit: i64) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT d.id, d.amount_cents, d.currency, d.month_key, d.donated_at, \
             d.user_id, u.name, u.photo_path \
             FROM donations d LEFT JOIN users u ON u.id = d.user_id \
             ORDER BY d.donated_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(donation_json).collect())
    }

    /// `userDonationHistory`.
    pub async fn user_history(&self, user_id: i64, limit: i64) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT d.id, d.amount_cents, d.currency, d.month_key, d.donated_at, \
             d.user_id, NULL AS name, NULL AS photo_path \
             FROM donations d WHERE d.user_id = $1 \
             ORDER BY d.donated_at DESC LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(donation_json).collect())
    }

    /// `userDonationSummary` aggregates.
    pub async fn user_summary(&self, user_id: i64) -> Result<Value, sqlx::Error> {
        let r = sqlx::query(
            "SELECT CAST(COALESCE(SUM(amount_cents),0) AS bigint) AS total, COUNT(*) AS cnt, \
             COUNT(DISTINCT month_key) AS months, MIN(donated_at) AS first_at, \
             MIN(month_key) AS first_key \
             FROM donations WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        let first_at: Option<chrono::DateTime<chrono::Utc>> = r.get("first_at");
        // `diffInMonths(first→now, floor)` — months between calendar months.
        let duration_months = first_at.map_or(0, |f| {
            let f = f.with_timezone(&Paris);
            let now = chrono::Utc::now().with_timezone(&Paris);
            let months = (now.year() - f.year()) * 12 + (now.month() as i32 - f.month() as i32);
            months.max(1)
        });
        Ok(json!({
            "total_cents": r.get::<i64, _>("total"),
            "donation_count": r.get::<i64, _>("cnt"),
            "months_supported": r.get::<i64, _>("months"),
            "supporter_duration_months": duration_months,
            "first_donated_at": first_at.map(|d| d.to_rfc3339()),
            "first_supported_month_key": r.get::<Option<String>, _>("first_key"),
        }))
    }
}

fn donation_json(r: &sqlx::postgres::PgRow) -> Value {
    let uid: Option<i64> = r.get("user_id");
    let mut v = json!({
        "id": r.get::<i64, _>("id"),
        "amount_cents": r.get::<i64, _>("amount_cents"),
        "currency": r.get::<String, _>("currency"),
        "month_key": r.get::<String, _>("month_key"),
        "donated_at": r.get::<chrono::DateTime<chrono::Utc>, _>("donated_at")
            .to_rfc3339(),
        "anonymous": uid.is_none(),
    });
    if let Some(uid) = uid {
        v["user"] = json!({
            "id": uid,
            "name": r.get::<Option<String>, _>("name"),
            "photo_path": r.get::<Option<String>, _>("photo_path"),
        });
    }
    v
}

/// `monthProgress` shape (shared by month_progress + history).
pub fn progress_json(month_key: &str, raised: i64, carryover: i64, goal: i64) -> Value {
    let effective = carryover + raised;
    json!({
        "month_key": month_key,
        "raised_cents": raised,
        "carryover_cents": carryover,
        "effective_cents": effective,
        "surplus_cents": (effective - goal).max(0),
        "goal_cents": goal,
        "percent": ((effective as f64 / goal.max(1) as f64) * 100.0).round().min(100.0) as i64,
        "goal_reached": effective >= goal,
    })
}

/// `Y-m` of the month after `key`.
pub fn next_month_key(key: &str) -> Option<String> {
    let start = month_start(key)?;
    let next = start + Duration::days(32);
    Some(month_key(next))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_keys() {
        // 2025-01-31 23:30 UTC = 2025-02-01 00:30 Paris → "2025-02".
        let ts = chrono::Utc.with_ymd_and_hms(2025, 1, 31, 23, 30, 0).unwrap();
        assert_eq!(month_key(ts), "2025-02");
        assert_eq!(prev_month_key("2025-03"), Some("2025-02".into()));
        assert_eq!(prev_month_key("2025-01"), Some("2024-12".into()));
        assert_eq!(next_month_key("2025-12"), Some("2026-01".into()));
    }
}
