use crate::auth::OptionalUser;
use crate::state::AppState;
use axum::{
    Json,
    body::Bytes,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

/// Plan perks shared by Plus and Pro ("all supporter perks" + these).
pub const PLUS_PERKS: [&str; 2] = ["ad_free", "daily_revive"];

#[derive(Clone)]
pub struct PaymentsConfig {
    pub stripe_plus_payment_url: String,
    pub stripe_pro_payment_url: String,
    pub stripe_webhook_secret: String,
    pub supporter_perks: Vec<String>,
    pub supporter_reaction_emojis: Vec<String>,
}

impl Default for PaymentsConfig {
    fn default() -> Self {
        Self {
            stripe_plus_payment_url: String::new(),
            stripe_pro_payment_url: String::new(),
            stripe_webhook_secret: String::new(),
            supporter_perks: ["ad_free", "avatar_crown", "supporter_reactions"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            supporter_reaction_emojis: ["☕", "💎", "🙏", "✨", "🎁", "🫶"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
}

/// Perks granted to any active paid plan: the supporter pack plus the
/// plan's own perks. `None` plan -> no perks.
pub fn perks_for_plan(cfg: &PaymentsConfig, plan: Option<&str>) -> Vec<String> {
    if plan.is_none() {
        return vec![];
    }
    let mut perks = cfg.supporter_perks.clone();
    for p in PLUS_PERKS {
        if !perks.iter().any(|x| x == p) {
            perks.push(p.to_string());
        }
    }
    perks
}

pub fn verify_stripe_signature(
    secret: &str,
    signature_header: &str,
    payload: &[u8],
    now_unix: i64,
) -> Result<(), ()> {
    if secret.is_empty() {
        return Err(());
    }
    let mut timestamp: Option<i64> = None;
    let mut signatures: Vec<&str> = vec![];
    for part in signature_header.split(',') {
        let mut kv = part.trim().splitn(2, '=');
        match (kv.next(), kv.next()) {
            (Some("t"), Some(v)) => timestamp = v.trim().parse().ok(),
            (Some("v1"), Some(v)) => signatures.push(v.trim()),
            _ => {}
        }
    }
    let Some(t) = timestamp else {
        return Err(());
    };
    if signatures.is_empty() || (now_unix - t).abs() > 300 {
        return Err(());
    }
    use hmac::{Hmac, Mac};
    let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).map_err(|_| ())?;
    mac.update(t.to_string().as_bytes());
    mac.update(b".");
    mac.update(payload);
    let expected = hex::encode(mac.finalize().into_bytes());
    let ok = signatures.iter().any(|s| {
        s.len() == expected.len()
            && s.bytes()
                .zip(expected.bytes())
                .fold(0u8, |acc, (x, y)| acc | (x ^ y))
                == 0
    });
    if ok { Ok(()) } else { Err(()) }
}

pub async fn stripe_webhook(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let sig = headers
        .get("stripe-signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if verify_stripe_signature(
        &state.payments.stripe_webhook_secret,
        sig,
        &body,
        chrono::Utc::now().timestamp(),
    )
    .is_err()
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Invalid signature"})),
        )
            .into_response();
    }
    let Ok(event) = serde_json::from_slice::<Value>(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let obj = &event["data"]["object"];
    match event["type"].as_str() {
        // async_payment_succeeded arrives when an async payment method (SEPA,
        // bank transfer) clears after checkout.session.completed=unpaid.
        Some("checkout.session.completed") | Some("checkout.session.async_payment_succeeded") => {
            // Only subscription-mode checkouts are recorded — one-time
            // payments (the former donation flow) are ignored.
            if obj["mode"].as_str() == Some("subscription") {
                if let Err(r) = record_subscription_session(&state, obj).await {
                    return r;
                }
            }
        }
        Some("customer.subscription.updated") | Some("customer.subscription.created") => {
            sync_subscription(&state, obj).await;
        }
        Some("customer.subscription.deleted") => {
            if let Some(sid) = obj["id"].as_str() {
                if let Ok(Some(uid)) = state.subscriptions.cancel(sid).await {
                    strip_venue_perks_unless_pro(&state, uid).await;
                }
            }
        }
        // Keep the latest invoice id on the subscription row — a later
        // charge.refunded only carries the invoice id, not the sub id.
        // Newer Stripe API versions nest it under parent.subscription_details.
        Some("invoice.payment_succeeded") | Some("invoice.paid") => {
            let sub_id = obj["subscription"]
                .as_str()
                .or_else(|| {
                    obj["parent"]["subscription_details"]["subscription"].as_str()
                });
            if let (Some(sid), Some(inv)) = (sub_id, obj["id"].as_str()) {
                if let Err(e) = state.subscriptions.set_invoice(sid, inv).await {
                    tracing::warn!(error = %e, "invoice id sync failed");
                }
            }
        }
        // Full refund of a subscription charge (droit de rétractation,
        // duplicate charge, chargeback) revokes the plan — Stripe does not
        // cancel the subscription itself on refund.
        Some("charge.refunded") => {
            handle_charge_refunded(&state, obj).await;
        }
        _ => {}
    }
    Json(json!({"received": true})).into_response()
}

/// checkout.session.completed for a Payment Link in subscription mode:
/// `client_reference_id` = "<user_id>:<plan>".
async fn record_subscription_session(state: &AppState, session: &Value) -> Result<i64, Response> {
    let Some(session_id) = session["id"].as_str().filter(|s| !s.is_empty()) else {
        return Ok(0);
    };
    if !matches!(
        session["payment_status"].as_str(),
        Some("paid") | Some("no_payment_required")
    ) {
        return Ok(0);
    }
    let reference = session["client_reference_id"].as_str().unwrap_or("");
    let (ref_uid, ref_plan) = reference.split_once(':').unwrap_or((reference, "plus"));
    let plan = match ref_plan {
        blindall_db::subscriptions::PLAN_PRO => blindall_db::subscriptions::PLAN_PRO,
        _ => blindall_db::subscriptions::PLAN_PLUS,
    };
    let user_id = match ref_uid.parse::<i64>() {
        Ok(id) => Some(id),
        Err(_) => {
            let email = session["customer_details"]["email"]
                .as_str()
                .map(|e| e.to_lowercase());
            match email.as_deref() {
                Some(e) => state
                    .auth
                    .member_id_by_email(e)
                    .await
                    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?,
                None => None,
            }
        }
    };
    let Some(uid) = user_id else {
        tracing::warn!(%session_id, "subscription checkout without resolvable user");
        return Ok(0);
    };
    match state
        .subscriptions
        .record_checkout(
            uid,
            plan,
            session_id,
            session["customer"].as_str(),
            session["subscription"].as_str(),
            session["invoice"].as_str(),
        )
        .await
    {
        Ok(true) => {
            if let Ok(Some(email)) = state.auth.email_of(uid).await {
                let _ = crate::mail::send(
                    &state.mail,
                    &email,
                    "",
                    "Bienvenue dans Blind'All Plus",
                    &format!(
                        "<p>Merci pour votre abonnement Blind'All {} !</p>\
                         <p>Vos avantages sont actifs : {}</p>\
                         <p><small>Abonnement mensuel sans engagement — résiliable à \
                         tout moment depuis vos paramètres ou par e-mail. Voir nos \
                         <a href=\"https://blindallmusic.clerely.fr/pages/conditions-generales-vente\">CGV</a>.</small></p>",
                        if plan == "pro" { "Pro" } else { "Plus" },
                        if plan == "pro" {
                            "branding de salle (nom d'établissement sur l'écran TV) \
                             pour vos soirées, plus tous les avantages supporter."
                        } else {
                            "plus de pubs, un revive quotidien sur le daily \
                             challenge, et tous les avantages supporter."
                        }
                    ),
                )
                .await;
            }
        }
        Ok(false) => {}
        Err(e) => {
            tracing::warn!(error = %e, "subscription record failed");
            return Err(StatusCode::INTERNAL_SERVER_ERROR.into_response());
        }
    }
    Ok(0)
}

async fn sync_subscription(state: &AppState, obj: &Value) {
    let Some(sid) = obj["id"].as_str() else { return };
    let status = obj["status"].as_str().unwrap_or("active");
    let period_end = obj["current_period_end"]
        .as_i64()
        .and_then(|t| chrono::DateTime::from_timestamp(t, 0));
    let cancel_at_period_end = obj["cancel_at_period_end"].as_bool().unwrap_or(false)
        || !obj["canceled_at"].is_null();
    match state
        .subscriptions
        .sync(sid, status, period_end, cancel_at_period_end)
        .await
    {
        Ok(Some(user_id)) => {
            // A definitively non-entitled status (canceled/unpaid/expired)
            // drops the venue branding the moment the perk lapses.
            if !blindall_db::subscriptions::SubscriptionRepo::entitled_status(status) {
                strip_venue_perks_unless_pro(state, user_id).await;
            }
        }
        Ok(None) => {}
        Err(e) => tracing::warn!(error = %e, "subscription sync failed"),
    }
}

/// charge.refunded: only a FULL refund revokes the plan (partial refunds
/// are goodwill adjustments, not a withdrawal). Resolution order:
/// invoice id -> subscription row; failing that, the Stripe customer — but
/// only when it maps to exactly one entitled subscription. Charges with no
/// invoice at all (one-time payments) never reach the fallback: they are
/// not subscription-backed.
async fn handle_charge_refunded(state: &AppState, charge: &Value) {
    if charge["refunded"].as_bool() != Some(true) {
        return;
    }
    let Some(invoice) = charge["invoice"].as_str().filter(|s| !s.is_empty()) else {
        return;
    };
    let mut user_id: Option<i64> = None;
    match state.subscriptions.cancel_by_invoice(invoice).await {
        Ok(uid) => user_id = uid,
        Err(e) => tracing::warn!(error = %e, "refund cancel by invoice failed"),
    }
    if user_id.is_none() {
        if let Some(customer) = charge["customer"].as_str().filter(|s| !s.is_empty()) {
            match state.subscriptions.subs_by_customer(customer).await {
                // Exactly one subscription row for this customer AND it is
                // still entitled -> safe to revoke. Several rows (e.g. an
                // old canceled sub next to a fresh one) mean the refund
                // probably targets history, not the live plan.
                Ok(subs) if subs.len() == 1 => {
                    let (sid, _, status) = &subs[0];
                    if blindall_db::subscriptions::SubscriptionRepo::entitled_status(status)
                        && let Ok(Some(uid)) = state.subscriptions.cancel(sid).await
                    {
                        user_id = Some(uid);
                    }
                }
                Ok(subs) if subs.len() > 1 => {
                    tracing::warn!(
                        customer,
                        count = subs.len(),
                        "ambiguous refund: several subs, manual review needed"
                    );
                }
                _ => {}
            }
        }
    }
    let Some(uid) = user_id else { return };
    strip_venue_perks_unless_pro(state, uid).await;
    if let Ok(Some(email)) = state.auth.email_of(uid).await {
        let _ = crate::mail::send(
            &state.mail,
            &email,
            "",
            "Votre abonnement Blind'All a été remboursé",
            "<p>Le paiement de votre abonnement Blind'All a été intégralement \
             remboursé. Votre abonnement est donc résilié et les avantages \
             associés ne sont plus actifs.</p>\
             <p><small>Si vous n'êtes pas à l'origine de ce remboursement, \
             contactez le support.</small></p>",
        )
        .await;
    }
}

/// Venue branding is a Pro perk: strip it only when the user's remaining
/// active plan is not `pro`. A user holding several subscriptions (e.g. an
/// old plus sub beside an upgraded pro one) keeps the venue while any pro
/// row stays entitled — and loses it when downgraded to plain plus.
async fn strip_venue_perks_unless_pro(state: &AppState, user_id: i64) {
    let still_pro = state
        .subscriptions
        .active_plan(user_id)
        .await
        .ok()
        .flatten()
        .as_deref()
        == Some(blindall_db::subscriptions::PLAN_PRO);
    if !still_pro {
        let _ = state.subscriptions.strip_venue_perks(user_id).await;
    }
}

/// Payment Link for a subscription plan — `client_reference_id` carries
/// "<user_id>:<plan>" so the webhook can attach the subscription.
pub fn plan_payment_url_for_user(
    cfg: &PaymentsConfig,
    plan: &str,
    user: Option<&blindall_db::entities::User>,
    email: Option<&str>,
    locale: Option<&str>,
) -> String {
    let base = match plan {
        blindall_db::subscriptions::PLAN_PRO => &cfg.stripe_pro_payment_url,
        _ => &cfg.stripe_plus_payment_url,
    };
    if base.is_empty() {
        return String::new();
    }
    // Anonymous/guest checkout can't carry client_reference_id — the
    // subscription couldn't be attached. Only members get a link; the UI
    // falls back to /login.
    let Some(u) = user.filter(|u| !u.is_guest) else {
        return String::new();
    };
    let mut query: Vec<(String, String)> = vec![];
    if let Some(l) = locale {
        if ["fr", "en", "es"].contains(&l) {
            query.push(("locale".into(), l.into()));
        }
    }
    query.push((
        "client_reference_id".into(),
        format!("{}:{plan}", u.id),
    ));
    if let Some(e) = email.filter(|e| e.contains('@')) {
        query.push(("locked_prefilled_email".into(), e.into()));
    }
    finish_payment_url(base, query)
}

fn finish_payment_url(base: &str, query: Vec<(String, String)>) -> String {
    if base.is_empty() {
        return String::new();
    }
    if query.is_empty() {
        return base.to_string();
    }
    let (base, fragment) = match base.split_once('#') {
        Some((b, f)) => (b.to_string(), format!("#{f}")),
        None => (base.to_string(), String::new()),
    };
    let sep = if base.contains('?') { "&" } else { "?" };
    let qs: Vec<String> = query
        .iter()
        .map(|(k, v)| {
            format!(
                "{k}={}",
                url::form_urlencoded::byte_serialize(v.as_bytes()).collect::<String>()
            )
        })
        .collect();
    format!("{base}{sep}{}{fragment}", qs.join("&"))
}

/// Billing overview for the /premium page, settings and the ads gate:
/// current plan, Stripe payment links and ad state in one payload.
pub async fn billing_status(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
) -> Response {
    let plan = match &user {
        Some(u) if !u.is_guest => state
            .subscriptions
            .active_plan(u.id)
            .await
            .ok()
            .flatten(),
        _ => None,
    };
    let email = match &user {
        Some(u) => state.auth.email_of(u.id).await.ok().flatten(),
        None => None,
    };
    Json(json!({
        "plan": plan,
        "plus_url": plan_payment_url_for_user(
            &state.payments,
            blindall_db::subscriptions::PLAN_PLUS,
            user.as_ref(),
            email.as_deref(),
            None
        ),
        "pro_url": plan_payment_url_for_user(
            &state.payments,
            blindall_db::subscriptions::PLAN_PRO,
            user.as_ref(),
            email.as_deref(),
            None
        ),
        "plus_offered": !state.payments.stripe_plus_payment_url.is_empty()
            || !state.payments.stripe_pro_payment_url.is_empty(),
        // Ad-free is a paid-plan perk.
        "ads_disabled": plan.is_some(),
        "adsense_client": state.adsense_client,
        "adsense_slot": state.adsense_slot,
    }))
    .into_response()
}

/// Batch-enrich chat message authors with their plan perks — `donor_perks`
/// /`is_supporter` now reflect the active subscription (the one-time
/// donation flow was removed).
pub async fn enrich_messages(state: &AppState, messages: &mut [Value]) {
    let ids: Vec<i64> = messages
        .iter()
        .filter_map(|m| m["user"]["id"].as_i64())
        .collect();
    let plans = state
        .subscriptions
        .active_plans(&ids)
        .await
        .unwrap_or_default();
    for m in messages.iter_mut() {
        if let Some(uid) = m["user"]["id"].as_i64() {
            let perks =
                perks_for_plan(&state.payments, plans.get(&uid).map(String::as_str));
            m["user"]["donor_perks"] = json!(perks);
            m["user"]["is_supporter"] = json!(!perks.is_empty());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmac::{Hmac, Mac};

    fn sign(secret: &str, t: i64, payload: &[u8]) -> String {
        let mut mac = Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(t.to_string().as_bytes());
        mac.update(b".");
        mac.update(payload);
        format!("t={t},v1={}", hex::encode(mac.finalize().into_bytes()))
    }

    #[test]
    fn stripe_signature_roundtrip() {
        let payload = br#"{"type":"checkout.session.completed"}"#;
        let t = 1_700_000_000;
        let header = sign("whsec_x", t, payload);
        assert!(verify_stripe_signature("whsec_x", &header, payload, t + 60).is_ok());
        assert!(verify_stripe_signature("whsec_y", &header, payload, t).is_err());
        assert!(verify_stripe_signature("whsec_x", &header, payload, t + 400).is_err());
        assert!(verify_stripe_signature("whsec_x", "t=1", payload, t).is_err());
        assert!(verify_stripe_signature("", &header, payload, t).is_err());
    }

    #[test]
    fn plan_payment_url_building() {
        let cfg = PaymentsConfig {
            stripe_plus_payment_url: "https://buy.stripe.com/plus#frag".into(),
            ..Default::default()
        };
        let user = blindall_db::entities::User {
            id: 7,
            name: "a".into(),
            photo_path: None,
            elo: 0,
            is_guest: false,
            is_administrator: false,
            team_id: None,
        };
        let url = plan_payment_url_for_user(
            &cfg,
            blindall_db::subscriptions::PLAN_PLUS,
            Some(&user),
            Some("u@e.fr"),
            Some("fr"),
        );
        assert!(url.starts_with("https://buy.stripe.com/plus?"));
        assert!(url.contains("client_reference_id=7%3Aplus"));
        assert!(url.contains("locked_prefilled_email=u%40e.fr"));
        assert!(url.ends_with("#frag"));
        // Guests get no link — the subscription couldn't be attached.
        let guest = blindall_db::entities::User {
            is_guest: true,
            ..user
        };
        assert_eq!(
            plan_payment_url_for_user(
                &cfg,
                blindall_db::subscriptions::PLAN_PLUS,
                Some(&guest),
                None,
                Some("en"),
            ),
            ""
        );
    }

    #[test]
    fn perks_require_plan() {
        let cfg = PaymentsConfig::default();
        assert!(perks_for_plan(&cfg, None).is_empty());
        let perks = perks_for_plan(&cfg, Some("plus"));
        assert!(perks.contains(&"ad_free".to_string()));
        assert!(perks.contains(&"avatar_crown".to_string()));
        assert!(perks.contains(&"daily_revive".to_string()));
    }
}
