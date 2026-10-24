use sqlx::{PgPool, Row};

pub const PLAN_PLUS: &str = "plus";
pub const PLAN_PRO: &str = "pro";

/// Stripe subscription state (Blind'All Plus / Pro). Rows are created by the
/// `checkout.session.completed` webhook (`client_reference_id` =
/// `"<user_id>:<plan>"`) and kept in sync by `customer.subscription.*` events.
#[derive(Clone)]
pub struct SubscriptionRepo {
    pool: PgPool,
}

impl SubscriptionRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Active paid plan for a user ("plus"/"pro"), if any. A cancelled
    /// subscription stays entitled until `current_period_end` (Stripe keeps
    /// status `active` in that window; the `deleted` event ends it).
    pub async fn active_plan(&self, user_id: i64) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar::<_, String>(
            // 'past_due' keeps entitlement: Stripe retries the charge for a few
            // days — cutting perks instantly on a failed renewal is hostile.
            // 'pro' outranks 'plus': an entitled pro row must win even when an
            // older plus row was touched more recently (e.g. a routine
            // customer.subscription.updated on the legacy sub would otherwise
            // downgrade a user who upgraded plus->pro).
            "SELECT plan::text FROM user_subscriptions \
             WHERE user_id = $1 \
               AND status IN ('active', 'trialing', 'past_due') \
               AND (current_period_end IS NULL OR current_period_end > now()) \
             ORDER BY (plan = 'pro') DESC, updated_at DESC NULLS LAST, id DESC \
             LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
    }

    /// checkout.session.completed with mode=subscription. Idempotent on
    /// stripe_checkout_session_id (unique index).
    pub async fn record_checkout(
        &self,
        user_id: i64,
        plan: &str,
        session_id: &str,
        customer_id: Option<&str>,
        subscription_id: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        let now = chrono::Utc::now();
        let rows = sqlx::query(
            "INSERT INTO user_subscriptions \
             (user_id, plan, status, stripe_checkout_session_id, \
              stripe_customer_id, stripe_subscription_id, created_at, updated_at) \
             VALUES ($1, $2, 'active', $3, $4, $5, $6, $6) \
             ON CONFLICT (stripe_checkout_session_id) \
             WHERE stripe_checkout_session_id IS NOT NULL \
             DO NOTHING",
        )
        .bind(user_id)
        .bind(plan)
        .bind(session_id)
        .bind(customer_id)
        .bind(subscription_id)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(rows.rows_affected() == 1)
    }

    /// Attach the subscription/customer ids once customer.subscription.created
    /// arrives (the checkout.session event does not always carry them first).
    pub async fn link_ids(
        &self,
        session_id: &str,
        customer_id: Option<&str>,
        subscription_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE user_subscriptions \
             SET stripe_subscription_id = $1, \
                 stripe_customer_id = COALESCE($2, stripe_customer_id), \
                 updated_at = $3 \
             WHERE stripe_checkout_session_id = $4 \
               AND (stripe_subscription_id IS NULL OR stripe_subscription_id = '')",
        )
        .bind(subscription_id)
        .bind(customer_id)
        .bind(chrono::Utc::now())
        .bind(session_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// customer.subscription.updated — sync status / period / cancellation.
    /// Stripe keeps `status='active'` while `cancel_at_period_end` is set, so
    /// the row stays entitled until the `deleted` event or period end.
    pub async fn sync(
        &self,
        subscription_id: &str,
        status: &str,
        current_period_end: Option<chrono::DateTime<chrono::Utc>>,
        cancel_at_period_end: bool,
    ) -> Result<Option<i64>, sqlx::Error> {
        sqlx::query(
            "UPDATE user_subscriptions \
             SET status = $2, current_period_end = $3, \
                 canceled_at = CASE WHEN $4 THEN COALESCE(canceled_at, $5) \
                                    ELSE NULL END, \
                 updated_at = $5 \
             WHERE stripe_subscription_id = $1 \
             RETURNING user_id",
        )
        .bind(subscription_id)
        .bind(status)
        .bind(current_period_end)
        .bind(cancel_at_period_end)
        .bind(chrono::Utc::now())
        .fetch_optional(&self.pool)
        .await
        .map(|r| r.map(|r| r.get::<i64, _>("user_id")))
    }

    /// customer.subscription.deleted — hard-cancel now. Returns user_id.
    pub async fn cancel(&self, subscription_id: &str) -> Result<Option<i64>, sqlx::Error> {
        let now = chrono::Utc::now();
        sqlx::query(
            "UPDATE user_subscriptions SET status = 'canceled', \
             canceled_at = COALESCE(canceled_at, $2), updated_at = $2 \
             WHERE stripe_subscription_id = $1 RETURNING user_id",
        )
        .bind(subscription_id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map(|r| r.map(|r| r.get::<i64, _>("user_id")))
    }

    /// A status that keeps paid perks (past_due = Stripe dunning retry window).
    pub fn entitled_status(status: &str) -> bool {
        matches!(status, "active" | "trialing" | "past_due")
    }

    /// Subscription ended -> drop venue branding on rooms the user owns.
    /// Only strips rooms that were actually branded (venue_name set) so an
    /// admin-flagged pro room without a venue is left alone.
    pub async fn strip_venue_perks(&self, user_id: i64) -> Result<u64, sqlx::Error> {
        Ok(sqlx::query(
            "UPDATE rooms SET is_pro = false, venue_name = NULL \
             WHERE user_id = $1 AND venue_name IS NOT NULL",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?
        .rows_affected())
    }

    // ---- Plus perk ledger: one daily-challenge revive per (day, game) ----

    pub async fn revive_used_today(
        &self,
        user_id: i64,
        game_type: &str,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM user_plus_revives \
             WHERE user_id = $1 AND day = CURRENT_DATE AND game_type = $2)",
        )
        .bind(user_id)
        .bind(game_type)
        .fetch_one(&self.pool)
        .await
    }

    /// Records the revive. Returns false when the (user, day, game) slot is
    /// already taken — rely on the unique constraint for the race.
    pub async fn record_revive(&self, user_id: i64, game_type: &str) -> Result<bool, sqlx::Error> {
        let rows = sqlx::query(
            "INSERT INTO user_plus_revives (user_id, day, game_type, created_at) \
             VALUES ($1, CURRENT_DATE, $2, $3) \
             ON CONFLICT (user_id, day, game_type) DO NOTHING",
        )
        .bind(user_id)
        .bind(game_type)
        .bind(chrono::Utc::now())
        .execute(&self.pool)
        .await?;
        Ok(rows.rows_affected() == 1)
    }
}
