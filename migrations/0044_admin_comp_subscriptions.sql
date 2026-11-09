-- Admin-granted comp subscriptions (press, offline B2B sales, support
-- cases): rows with every Stripe id NULL are invisible to all webhook
-- paths, which resolve subscriptions by stripe_subscription_id /
-- stripe_checkout_session_id / stripe_customer_id only. One manual row
-- per user keeps grant/revoke a simple upsert.
CREATE UNIQUE INDEX IF NOT EXISTS user_subscriptions_manual_uidx
    ON user_subscriptions (user_id)
    WHERE stripe_subscription_id IS NULL
      AND stripe_checkout_session_id IS NULL
      AND stripe_customer_id IS NULL;
