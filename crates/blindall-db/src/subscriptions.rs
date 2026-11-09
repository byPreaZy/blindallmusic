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
            // days, cutting perks instantly on a failed renewal is hostile.
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
    /// stripe_checkout_session_id (unique index). `invoice_id` is the first
    /// invoice (in_...), kept so a later charge.refunded can be mapped
    /// back to this subscription.
    pub async fn record_checkout(
        &self,
        user_id: i64,
        plan: &str,
        session_id: &str,
        customer_id: Option<&str>,
        subscription_id: Option<&str>,
        invoice_id: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        let now = chrono::Utc::now();
        let rows = sqlx::query(
            "INSERT INTO user_subscriptions \
             (user_id, plan, status, stripe_checkout_session_id, \
              stripe_customer_id, stripe_subscription_id, stripe_invoice_id, \
              created_at, updated_at) \
             VALUES ($1, $2, 'active', $3, $4, $5, $6, $7, $7) \
             ON CONFLICT (stripe_checkout_session_id) \
             WHERE stripe_checkout_session_id IS NOT NULL \
             DO NOTHING",
        )
        .bind(user_id)
        .bind(plan)
        .bind(session_id)
        .bind(customer_id)
        .bind(subscription_id)
        .bind(invoice_id)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(rows.rows_affected() == 1)
    }

    /// invoice.payment_succeeded, refresh the latest invoice id on the
    /// subscription row so charge.refunded resolves to the right sub even
    /// after renewals (each cycle issues a new invoice).
    pub async fn set_invoice(
        &self,
        subscription_id: &str,
        invoice_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE user_subscriptions SET stripe_invoice_id = $2, updated_at = $3 \
             WHERE stripe_subscription_id = $1",
        )
        .bind(subscription_id)
        .bind(invoice_id)
        .bind(chrono::Utc::now())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// charge.refunded, cancel the subscription behind a refunded invoice.
    /// Returns the user_id ONLY on an entitled -> canceled transition, so a
    /// replayed refund event neither re-emails nor falls through to the
    /// customer fallback.
    pub async fn cancel_by_invoice(&self, invoice_id: &str) -> Result<Option<i64>, sqlx::Error> {
        let now = chrono::Utc::now();
        sqlx::query(
            "UPDATE user_subscriptions SET status = 'canceled', \
             canceled_at = COALESCE(canceled_at, $2), updated_at = $2 \
             WHERE stripe_invoice_id = $1 \
               AND status IN ('active', 'trialing', 'past_due') \
             RETURNING user_id",
        )
        .bind(invoice_id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await
        .map(|r| r.map(|r| r.get::<i64, _>("user_id")))
    }

    /// Fallback refund resolution when the charge carries an invoice id we
    /// don't know (e.g. issued before invoice tracking): every subscription
    /// row of the Stripe customer as (stripe_subscription_id, user_id,
    /// status). The caller revokes only when the customer has exactly ONE
    /// row and it is still entitled, several rows mean the refund likely
    /// targets an old subscription and a blind cancel would over-revoke.
    pub async fn subs_by_customer(
        &self,
        customer_id: &str,
    ) -> Result<Vec<(String, i64, String)>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT stripe_subscription_id, user_id, status::text AS status \
             FROM user_subscriptions \
             WHERE stripe_customer_id = $1 \
               AND stripe_subscription_id IS NOT NULL",
        )
        .bind(customer_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                (
                    r.get::<String, _>("stripe_subscription_id"),
                    r.get::<i64, _>("user_id"),
                    r.get::<String, _>("status"),
                )
            })
            .collect())
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

    /// customer.subscription.updated, sync status / period / cancellation.
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

    /// customer.subscription.deleted, hard-cancel now. Returns user_id.
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

    /// Batch variant of `active_plan` for perk enrichment (chat messages,
    /// room rosters): returns user_id -> plan for every entitled id, with
    /// 'pro' outranking 'plus' per user.
    pub async fn active_plans(
        &self,
        user_ids: &[i64],
    ) -> Result<std::collections::HashMap<i64, String>, sqlx::Error> {
        use sqlx::Row;
        if user_ids.is_empty() {
            return Ok(Default::default());
        }
        let rows = sqlx::query(
            "SELECT DISTINCT ON (user_id) user_id, plan::text AS plan \
             FROM user_subscriptions \
             WHERE user_id = ANY($1) \
               AND status IN ('active', 'trialing', 'past_due') \
               AND (current_period_end IS NULL OR current_period_end > now()) \
             ORDER BY user_id, (plan = 'pro') DESC, updated_at DESC NULLS LAST, id DESC",
        )
        .bind(user_ids.to_vec())
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| (r.get::<i64, _>("user_id"), r.get::<String, _>("plan")))
            .collect())
    }

    // ---- admin comp grants -------------------------------------------------
    //
    // A "manual" row has every Stripe id NULL: webhook paths resolve rows
    // by stripe ids only, so comp grants are never touched by Stripe
    // events. The partial unique index user_subscriptions_manual_uidx
    // guarantees at most one manual row per user.

    const MANUAL_PREDICATE: &'static str = "stripe_subscription_id IS NULL \
        AND stripe_checkout_session_id IS NULL \
        AND stripe_customer_id IS NULL";

    /// All subscription rows of a user for the admin panel, manual grants
    /// first, each flagged `manual` / `entitled`.
    pub async fn list_for_user(&self, user_id: i64) -> Result<Vec<serde_json::Value>, sqlx::Error> {
        let sql = format!(
            "SELECT plan::text AS plan, status::text AS status, \
             current_period_end, created_at, updated_at, \
             (status IN ('active','trialing','past_due') \
              AND (current_period_end IS NULL OR current_period_end > now())) AS entitled, \
             ({}) AS manual \
             FROM user_subscriptions WHERE user_id = $1 \
             ORDER BY entitled DESC, (plan = 'pro') DESC, manual DESC, \
                      updated_at DESC NULLS LAST, id DESC",
            Self::MANUAL_PREDICATE
        );
        let rows = sqlx::query(&sql).bind(user_id).fetch_all(&self.pool).await?;
        Ok(rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "plan": r.get::<String, _>("plan"),
                    "status": r.get::<String, _>("status"),
                    "current_period_end": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("current_period_end"),
                    "created_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at"),
                    "updated_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("updated_at"),
                    "entitled": r.get::<bool, _>("entitled"),
                    "manual": r.get::<bool, _>("manual"),
                })
            })
            .collect())
    }

    /// Grant (or extend/replace) the admin comp plan: upserts the single
    /// manual row, `current_period_end` = now + `months` months.
    pub async fn grant(
        &self,
        user_id: i64,
        plan: &str,
        months: i32,
    ) -> Result<(), sqlx::Error> {
        let sql = format!(
            "INSERT INTO user_subscriptions \
             (user_id, plan, status, current_period_end, created_at, updated_at) \
             VALUES ($1, $2, 'active', \
                     now() + make_interval(months => $3), now(), now()) \
             ON CONFLICT (user_id) WHERE {} \
             DO UPDATE SET plan = EXCLUDED.plan, status = 'active', \
                 current_period_end = EXCLUDED.current_period_end, \
                 canceled_at = NULL, updated_at = now()",
            Self::MANUAL_PREDICATE
        );
        sqlx::query(&sql)
        .bind(user_id)
        .bind(plan)
        .bind(months)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Revoke the admin comp plan (if still entitled). Stripe-backed rows
    /// are untouched: they keep billing through Stripe as usual.
    pub async fn revoke_manual(&self, user_id: i64) -> Result<bool, sqlx::Error> {
        let sql = format!(
            "UPDATE user_subscriptions SET status = 'canceled', \
             canceled_at = COALESCE(canceled_at, now()), updated_at = now() \
             WHERE user_id = $1 AND {} \
               AND status IN ('active','trialing','past_due')",
            Self::MANUAL_PREDICATE
        );
        let rows = sqlx::query(&sql)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(rows.rows_affected() > 0)
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
    /// already taken, rely on the unique constraint for the race.
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
