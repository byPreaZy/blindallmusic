use sqlx::PgPool;

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
            "SELECT plan::text FROM user_subscriptions \
             WHERE user_id = $1 \
               AND status IN ('active', 'trialing', 'past_due') \
               AND (current_period_end IS NULL OR current_period_end > now()) \
             ORDER BY updated_at DESC NULLS LAST, id DESC LIMIT 1",
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
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE user_subscriptions \
             SET status = $2, current_period_end = $3, \
                 canceled_at = CASE WHEN $4 THEN COALESCE(canceled_at, $5) \
                                    ELSE NULL END, \
                 updated_at = $5 \
             WHERE stripe_subscription_id = $1",
        )
        .bind(subscription_id)
        .bind(status)
        .bind(current_period_end)
        .bind(cancel_at_period_end)
        .bind(chrono::Utc::now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// customer.subscription.deleted — hard-cancel now.
    pub async fn cancel(&self, subscription_id: &str) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now();
        sqlx::query(
            "UPDATE user_subscriptions SET status = 'canceled', \
             canceled_at = COALESCE(canceled_at, $2), updated_at = $2 \
             WHERE stripe_subscription_id = $1",
        )
        .bind(subscription_id)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
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
