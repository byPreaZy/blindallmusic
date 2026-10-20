use sqlx::Row;

/// Stripe subscription state — the single source of truth for the
/// Plus (B2C) and Pro (B2B) entitlements. Rows are written by the webhook;
/// reads only need "is there an active subscription for this plan?".
#[derive(Clone)]
pub struct SubscriptionRepo {
    pool: sqlx::PgPool,
}

/// Stripe subscription statuses that still grant the perks. `past_due` keeps
/// the perks during Stripe's retry grace window; `canceled`/`incomplete` don't.
pub const ACTIVE_STATUSES: [&str; 3] = ["active", "trialing", "past_due"];

pub const PLAN_PLUS: &str = "plus";
pub const PLAN_PRO: &str = "pro";

#[derive(Clone, Debug)]
pub struct SubscriptionRow {
    pub id: i64,
    pub user_id: i64,
    pub plan: String,
    pub status: String,
    pub stripe_subscription_id: Option<String>,
    pub current_period_end: Option<chrono::DateTime<chrono::Utc>>,
    pub canceled_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl SubscriptionRepo {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Active plan slugs the user is currently entitled to ("plus" | "pro").
    /// Expired `current_period_end` is tolerated: Stripe's `customer.subscription
    /// .updated` event reconciles the status shortly after; treating the row as
    /// live avoids flapping perks around the renewal boundary.
    pub async fn entitlements(&self, user_id: i64) -> Result<Vec<String>, sqlx::Error> {
        sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT plan::text FROM user_subscriptions
             WHERE user_id = $1 AND status = ANY($2)",
        )
        .bind(user_id)
        .bind(&ACTIVE_STATUSES[..])
        .fetch_all(&self.pool)
        .await
    }

    pub async fn has_plan(&self, user_id: i64, plan: &str) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(
               SELECT 1 FROM user_subscriptions
               WHERE user_id = $1 AND plan = $2 AND status = ANY($3))::int::bigint",
        )
        .bind(user_id)
        .bind(plan)
        .bind(&ACTIVE_STATUSES[..])
        .fetch_one(&self.pool)
        .await
        .map(|v| v == 1)
    }

    /// Insert (or re-confirm, on Stripe retries) the row created by a
    /// `checkout.session.completed` subscription-mode event.
    /// `client_reference_id` carries "<user_id>:<plan>".
    pub async fn record_checkout(
        &self,
        user_id: i64,
        plan: &str,
        session_id: &str,
        subscription_id: Option<&str>,
        customer_id: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO user_subscriptions
               (user_id, plan, status, stripe_checkout_session_id,
                stripe_subscription_id, stripe_customer_id, created_at, updated_at)
             VALUES ($1, $2, 'active', $3, $4, $5, NOW(), NOW())
             ON CONFLICT (stripe_checkout_session_id)
             WHERE stripe_checkout_session_id IS NOT NULL
             DO UPDATE SET
               stripe_subscription_id = COALESCE(user_subscriptions.stripe_subscription_id,
                                                 EXCLUDED.stripe_subscription_id),
               stripe_customer_id = COALESCE(user_subscriptions.stripe_customer_id,
                                             EXCLUDED.stripe_customer_id),
               updated_at = NOW()",
        )
        .bind(user_id)
        .bind(plan)
        .bind(session_id)
        .bind(subscription_id)
        .bind(customer_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Sync status + period end from a `customer.subscription.*` event object.
    /// Rows without a stripe_subscription_id yet are backfilled when the ids
    /// match (checkout row created first, subscription event second — the
    /// normal Payment-Link ordering).
    pub async fn sync_status(
        &self,
        subscription_id: &str,
        customer_id: Option<&str>,
        status: &str,
        current_period_end: Option<chrono::DateTime<chrono::Utc>>,
        canceled_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<u64, sqlx::Error> {
        let res = sqlx::query(
            "UPDATE user_subscriptions SET
               status = $2,
               current_period_end = COALESCE($3, current_period_end),
               canceled_at = COALESCE($4, canceled_at),
               stripe_customer_id = COALESCE(stripe_customer_id, $5),
               updated_at = NOW()
             WHERE stripe_subscription_id = $1",
        )
        .bind(subscription_id)
        .bind(status)
        .bind(current_period_end)
        .bind(canceled_at)
        .bind(customer_id)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected())
    }

    pub async fn list_for_user(&self, user_id: i64) -> Result<Vec<SubscriptionRow>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, user_id, plan::text AS plan, status::text AS status,
                    stripe_subscription_id::text AS stripe_subscription_id,
                    current_period_end, canceled_at
             FROM user_subscriptions WHERE user_id = $1 ORDER BY id DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| SubscriptionRow {
                id: r.get("id"),
                user_id: r.get("user_id"),
                plan: r.get("plan"),
                status: r.get("status"),
                stripe_subscription_id: r.get("stripe_subscription_id"),
                current_period_end: r.get("current_period_end"),
                canceled_at: r.get("canceled_at"),
            })
            .collect())
    }

    /// Plus perk: one daily-challenge revive per (user, day, game_type).
    /// Returns false when the revive was already consumed — the UNIQUE
    /// constraint makes the check atomic (no TOCTOU).
    pub async fn consume_daily_revive(
        &self,
        user_id: i64,
        day: chrono::NaiveDate,
        game_type: &str,
    ) -> Result<bool, sqlx::Error> {
        let res = sqlx::query(
            "INSERT INTO user_plus_revives (user_id, day, game_type, created_at)
             VALUES ($1, $2, $3, NOW()) ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(day)
        .bind(game_type)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() == 1)
    }
}
