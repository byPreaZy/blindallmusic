-- Refund resolution: charge.refunded only carries the Stripe invoice id
-- (in_...), never the subscription id. Keep the latest invoice per
-- subscription so a full refund can be mapped back to the right row —
-- checkout.session.completed stores the first invoice, and
-- invoice.payment_succeeded refreshes it on every renewal.
ALTER TABLE user_subscriptions
    ADD COLUMN IF NOT EXISTS stripe_invoice_id CITEXT NULL;

CREATE INDEX IF NOT EXISTS user_subscriptions_invoice_idx
    ON user_subscriptions (stripe_invoice_id)
    WHERE stripe_invoice_id IS NOT NULL;
