mod common;
use common::Client;
use reqwest::StatusCode;
use serde_json::{Value, json};

// ---- helpers ----------------------------------------------------------------

fn stripe_secret() -> Option<String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../.env");
    std::fs::read_to_string(path)
        .ok()?
        .lines()
        .find_map(|l| l.strip_prefix("STRIPE_WEBHOOK_SECRET=").map(str::to_string))
        .filter(|s| !s.is_empty() && s != "whsec_change_me")
}

fn sign(secret: &str, ts: i64, payload: &str) -> String {
    use hmac::{Hmac, Mac};
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(ts.to_string().as_bytes());
    mac.update(b".");
    mac.update(payload.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

async fn fire_webhook(payload: &Value) -> common::Resp {
    let body = payload.to_string();
    let ts = chrono_ts();
    let secret = stripe_secret().expect("STRIPE_WEBHOOK_SECRET must be set in .env");
    let sig = format!("t={ts},v1={}", sign(&secret, ts, &body));
    Client::anon()
        .post_raw("/api/v1/stripe/webhook", ("stripe-signature", &sig), body)
        .await
}

fn chrono_ts() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

fn sub_event(kind: &str, sub_id: &str, status: &str) -> Value {
    json!({
        "type": kind,
        "data": {"object": {
            "id": sub_id,
            "customer": "cus_e2e",
            "status": status,
            "current_period_end": chrono_ts() + 30 * 86400,
            "canceled_at": null,
        }},
    })
}

// ---- public + authed shapes ---------------------------------------------------

#[tokio::test]
async fn plans_public_shape() {
    let r = Client::anon()
        .get("/api/v1/billing/plans")
        .await
        .expect_ok("plans");
    assert!(r.json("/plus_available").is_boolean());
    assert!(r.json("/pro_available").is_boolean());
    assert!(
        r.json("/is_plus").is_null(),
        "anon must not get entitlements"
    );
}

#[tokio::test]
async fn billing_requires_auth() {
    let r = Client::anon().get("/api/v1/billing").await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);

    let (u, _) = Client::register("bill").await;
    let r = u.get("/api/v1/billing").await.expect_ok("billing");
    assert!(r.json("/plans").is_array());
    assert_eq!(r.json("/is_plus"), false);
    assert_eq!(r.json("/is_pro"), false);
    assert!(r.json("/subscriptions").is_array());
    // user payload carries entitlements too
    let me = u.get("/api/v1/users/me").await.expect_ok("me");
    assert_eq!(me.json("/account/is_plus"), false);
    assert_eq!(me.json("/account/is_pro"), false);
}

// ---- free-user gates ----------------------------------------------------------

#[tokio::test]
async fn free_user_plus_gates_rejected() {
    let (u, _) = Client::register("gate").await;
    let uid = u.user_id;
    let me = u.get("/api/v1/users/me").await.expect_ok("me");
    let name = me.json("/account/name").as_str().unwrap().to_string();
    let email = me.json("/account/email").as_str().unwrap().to_string();

    let r = u
        .post(
            "/api/v1/daily-challenge/revive",
            json!({"game_type": "quiz"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "revive: {}", r.text);
    assert_eq!(r.json("/error"), "plus_required");

    let r = u.get(&format!("/api/v1/users/{uid}/stats/advanced")).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "stats: {}", r.text);

    let r = u
        .post(
            &format!("/api/v1/users/{uid}"),
            json!({"name": name, "email": email, "custom_tag": "VIP"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "custom_tag: {}", r.text);
    assert_eq!(r.json("/error"), "plus_required");
}

// ---- webhook → entitlement → gates --------------------------------------------

#[tokio::test]
async fn webhook_grants_plus_then_revoke() {
    if stripe_secret().is_none() {
        eprintln!("SKIP: STRIPE_WEBHOOK_SECRET not configured");
        return;
    }
    let (u, _) = Client::register("plus").await;
    let uid = u.user_id;
    let me = u.get("/api/v1/users/me").await.expect_ok("me");
    let name = me.json("/account/name").as_str().unwrap().to_string();
    let email = me.json("/account/email").as_str().unwrap().to_string();
    let sub_id = format!("sub_e2e_{uid}");

    // checkout.session.completed (subscription mode) records the sub id.
    let r = fire_webhook(&json!({
        "type": "checkout.session.completed",
        "data": {"object": {
            "id": format!("cs_e2e_{uid}"),
            "mode": "subscription",
            "client_reference_id": format!("{uid}:plus"),
            "subscription": sub_id,
            "customer": "cus_e2e",
        }},
    }))
    .await;
    assert_eq!(r.status, StatusCode::OK, "session webhook: {}", r.text);

    // customer.subscription.created flips status to active → entitlement.
    let r = fire_webhook(&sub_event(
        "customer.subscription.created",
        &sub_id,
        "active",
    ))
    .await;
    assert_eq!(r.status, StatusCode::OK, "sub webhook: {}", r.text);

    let b = u.get("/api/v1/billing").await.expect_ok("billing after");
    assert_eq!(b.json("/is_plus"), true, "is_plus: {}", b.text);
    assert!(
        b.json("/plans")
            .as_array()
            .unwrap()
            .iter()
            .any(|p| p == "plus")
    );

    // Plus gates now open: custom_tag + revive + stats.
    let r = u
        .post(
            &format!("/api/v1/users/{uid}"),
            json!({"name": name, "email": email, "custom_tag": "VIP"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "custom_tag plus: {}", r.text);

    let r = u
        .post(
            "/api/v1/daily-challenge/revive",
            json!({"game_type": "quiz"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "revive: {}", r.text);
    // …and the second revive the same day is denied.
    let r = u
        .post(
            "/api/v1/daily-challenge/revive",
            json!({"game_type": "quiz"}),
        )
        .await;
    assert_eq!(
        r.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "revive2: {}",
        r.text
    );

    let r = u.get(&format!("/api/v1/users/{uid}/stats/advanced")).await;
    assert_eq!(r.status, StatusCode::OK, "stats: {}", r.text);
    assert!(r.json("/games").is_i64());

    // subscription.deleted → entitlement drops again.
    let r = fire_webhook(&sub_event(
        "customer.subscription.deleted",
        &sub_id,
        "canceled",
    ))
    .await;
    assert_eq!(r.status, StatusCode::OK);
    let b = u.get("/api/v1/billing").await.expect_ok("billing revoked");
    assert_eq!(b.json("/is_plus"), false);
}

#[tokio::test]
async fn webhook_ignores_donation_mode_and_bad_refs() {
    if stripe_secret().is_none() {
        eprintln!("SKIP: STRIPE_WEBHOOK_SECRET not configured");
        return;
    }
    // A donation-style session (mode "payment") must not create subscriptions.
    let r = fire_webhook(&json!({
        "type": "checkout.session.completed",
        "data": {"object": {
            "id": format!("cs_e2e_d_{}", chrono_ts()),
            "mode": "payment",
            "payment_status": "paid",
            "amount_total": 500,
            "created": chrono_ts(),
            "client_reference_id": "anon",
            "customer_details": {"email": "nobody@example.com"},
        }},
    }))
    .await;
    assert_eq!(r.status, StatusCode::OK, "donation path: {}", r.text);

    // subscription-mode session without a valid "id:plan" reference → ignored.
    let r = fire_webhook(&json!({
        "type": "checkout.session.completed",
        "data": {"object": {
            "id": format!("cs_e2e_x_{}", chrono_ts()),
            "mode": "subscription",
            "client_reference_id": "bogus",
        }},
    }))
    .await;
    assert_eq!(r.status, StatusCode::OK);
}
