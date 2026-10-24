use crate::auth::{CurrentUser, OptionalUser};
use crate::state::AppState;
use axum::{
    Json,
    body::Bytes,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde_json::{Value, json};

pub const DONOR_PERKS: [&str; 3] = ["ad_free", "avatar_crown", "supporter_reactions"];

#[derive(Clone)]
pub struct DonationsConfig {
    pub monthly_goal_cents: i64,
    pub stripe_payment_url: String,
    pub stripe_plus_payment_url: String,
    pub stripe_pro_payment_url: String,
    pub stripe_webhook_secret: String,
    pub supporter_perks: Vec<String>,
    pub supporter_reaction_emojis: Vec<String>,
}

/// Perks granted while a Plus subscription is active — independent of the
/// monthly donation (donating also gives `supporter_perks` for that month).
pub const PLUS_PERKS: [&str; 2] = ["ad_free", "daily_revive"];

impl Default for DonationsConfig {
    fn default() -> Self {
        Self {
            monthly_goal_cents: 6_000,
            stripe_payment_url: String::new(),
            stripe_plus_payment_url: String::new(),
            stripe_pro_payment_url: String::new(),
            stripe_webhook_secret: String::new(),
            supporter_perks: DONOR_PERKS.iter().map(|s| s.to_string()).collect(),
            supporter_reaction_emojis: ["☕", "💎", "🙏", "✨", "🎁", "🫶"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }
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
        &state.donations.stripe_webhook_secret,
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
            let r = if obj["mode"].as_str() == Some("subscription") {
                record_subscription_session(&state, obj).await
            } else {
                record_session(&state, obj).await
            };
            if let Err(r) = r {
                return r;
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
            state
                .donations_repo
                .resolve_donor(None, email.as_deref())
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
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

async fn record_session(state: &AppState, session: &Value) -> Result<i64, Response> {
    let Some(id) = session["id"].as_str().filter(|s| !s.is_empty()) else {
        return Ok(0);
    };
    if session["payment_status"].as_str() != Some("paid") {
        return Ok(0);
    }
    let amount = session["amount_total"].as_i64().unwrap_or(0);
    if amount <= 0 {
        return Ok(0);
    }
    let donated_at = session["created"]
        .as_i64()
        .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
        .unwrap_or_else(chrono::Utc::now);
    let donor_email = session["customer_details"]["email"]
        .as_str()
        .map(|e| e.to_lowercase());
    let user_id = state
        .donations_repo
        .resolve_donor(
            session["client_reference_id"].as_str(),
            donor_email.as_deref(),
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    let (_, is_new) = state
        .donations_repo
        .record_checkout_session(
            id,
            amount,
            session["currency"].as_str().unwrap_or("eur"),
            user_id,
            donor_email.as_deref(),
            donated_at,
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?;
    use fred::interfaces::KeysInterface;
    let key = format!(
        "{}donation_goal_progress:v2:{}",
        state.cache_prefix,
        blindall_db::donations::month_key(donated_at)
    );
    let _: Result<(), _> = state.redis.del(&key).await;
    if is_new {
        if let Some(uid) = user_id {
            if let Ok(Some(email)) = state.auth.email_of(uid).await {
                let amount = format!("{:.2}", amount as f64 / 100.0).replace('.', ",");
                let _ = crate::mail::send(
                    &state.mail,
                    &email,
                    "",
                    "Merci pour votre don",
                    &format!(
                        "<p>Merci pour votre don de {amount} €, il nous aide à garder \
                         Blind'All Music en ligne.</p>"
                    ),
                )
                .await;
            }
        }
    }
    Ok(0)
}

pub fn payment_url_for_user(
    cfg: &DonationsConfig,
    user: Option<&blindall_db::entities::User>,
    email: Option<&str>,
    locale: Option<&str>,
) -> String {
    let mut query: Vec<(String, String)> = vec![];
    if let Some(l) = locale {
        if ["fr", "en", "es"].contains(&l) {
            query.push(("locale".into(), l.into()));
        }
    }
    if let Some(u) = user {
        if !u.is_guest {
            query.push(("client_reference_id".into(), u.id.to_string()));
            if let Some(e) = email.filter(|e| e.contains('@')) {
                query.push(("locked_prefilled_email".into(), e.into()));
            }
        }
    }
    finish_payment_url(&cfg.stripe_payment_url, query)
}

/// Payment Link for a subscription plan — `client_reference_id` carries
/// "<user_id>:<plan>" so the webhook can attach the subscription.
pub fn plan_payment_url_for_user(
    cfg: &DonationsConfig,
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

fn enrich(
    state: &AppState,
    mut supporter: Value,
    perks_map: &std::collections::HashSet<i64>,
) -> Value {
    let uid = supporter["id"].as_i64().unwrap_or(0);
    let photo_path = supporter["photo_path"].as_str().unwrap_or("").to_string();
    let name = supporter["name"].as_str().unwrap_or("").to_string();
    let perks: Vec<String> = if perks_map.contains(&uid) {
        state.donations.supporter_perks.clone()
    } else {
        vec![]
    };
    let photo = crate::profiles::photo_url(state, &photo_path, &name);
    let is_supporter = !perks.is_empty();
    if let Some(o) = supporter.as_object_mut() {
        o.remove("photo_path");
        o.insert("photo".into(), json!(photo));
        o.insert("donor_perks".into(), json!(perks));
        o.insert("is_supporter".into(), json!(is_supporter));
    }
    supporter
}

async fn perks_map(state: &AppState, supporter_ids: &[i64]) -> std::collections::HashSet<i64> {
    state
        .donations_repo
        .supporter_ids(supporter_ids, &blindall_db::donations::current_month_key())
        .await
        .unwrap_or_default()
        .into_iter()
        .collect()
}

pub async fn progress(State(state): State<AppState>, OptionalUser(user): OptionalUser) -> Response {
    let key = blindall_db::donations::current_month_key();
    let mut progress = match state
        .donations_repo
        .month_progress(&key, state.donations.monthly_goal_cents)
        .await
    {
        Ok(p) => p,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let email = match &user {
        Some(u) => state.auth.email_of(u.id).await.ok().flatten(),
        None => None,
    };
    let days_remaining = days_remaining_in_month(&key);
    let raised = progress["raised_cents"].as_i64().unwrap_or(0);
    let carryover = progress["carryover_cents"].as_i64().unwrap_or(0);
    let goal = state.donations.monthly_goal_cents;
    // Ad-free perk is personal: only this month's supporters browse ad-free.
    // Reaching the monthly goal does NOT disable ads for everyone.
    let user_supporter = match &user {
        Some(u) if !u.is_guest => state
            .donations_repo
            .user_is_supporter(u.id, &key)
            .await
            .unwrap_or(false),
        _ => false,
    };
    // Ad-free perk covers the active paid plan as well as this month's donors.
    let plan = match &user {
        Some(u) if !u.is_guest => state
            .subscriptions
            .active_plan(u.id)
            .await
            .ok()
            .flatten(),
        _ => None,
    };
    if let Some(o) = progress.as_object_mut() {
        o.insert("days_remaining".into(), json!(days_remaining));
        o.insert("ads_disabled".into(), json!(user_supporter || plan.is_some()));
        o.insert("plan".into(), json!(plan));
        o.insert(
            "plus_offered".into(),
            json!(!state.donations.stripe_plus_payment_url.is_empty()
                || !state.donations.stripe_pro_payment_url.is_empty()),
        );
        o.insert("adsense_client".into(), json!(state.adsense_client));
        o.insert("adsense_slot".into(), json!(state.adsense_slot));
        o.insert(
            "progress_segments".into(),
            progress_segments(raised, carryover, goal),
        );
    }
    progress["payment_url"] = json!(payment_url_for_user(
        &state.donations,
        user.as_ref(),
        email.as_deref(),
        None
    ));
    progress["plus_url"] = json!(plan_payment_url_for_user(
        &state.donations,
        blindall_db::subscriptions::PLAN_PLUS,
        user.as_ref(),
        email.as_deref(),
        None
    ));
    progress["pro_url"] = json!(plan_payment_url_for_user(
        &state.donations,
        blindall_db::subscriptions::PLAN_PRO,
        user.as_ref(),
        email.as_deref(),
        None
    ));
    let supporters = monthly_supporters(&state).await.unwrap_or_default();
    progress["monthly_supporters"] = json!(supporters);
    Json(progress).into_response()
}

pub async fn enrich_messages(state: &AppState, messages: &mut [Value]) {
    let ids: Vec<i64> = messages
        .iter()
        .filter_map(|m| m["user"]["id"].as_i64())
        .collect();
    let map = perks_map(state, &ids).await;
    for m in messages.iter_mut() {
        if let Some(uid) = m["user"]["id"].as_i64() {
            let perks: Vec<String> = if map.contains(&uid) {
                state.donations.supporter_perks.clone()
            } else {
                vec![]
            };
            m["user"]["donor_perks"] = json!(perks);
            m["user"]["is_supporter"] = json!(!perks.is_empty());
        }
    }
}

async fn monthly_supporters(state: &AppState) -> Option<Vec<Value>> {
    let key = blindall_db::donations::current_month_key();
    let goal = state.donations.monthly_goal_cents;
    let donors = state.donations_repo.month_donors(&key).await.ok()?;
    let mut supporters = donors;
    let mut existing: std::collections::HashSet<i64> =
        supporters.iter().filter_map(|s| s["id"].as_i64()).collect();
    if state
        .donations_repo
        .carryover_cents_before_month(&key, goal)
        .await
        .unwrap_or(0)
        > 0
    {
        if let Some(prev) = blindall_db::donations::prev_month_key(&key) {
            if let Ok(extra) = state.donations_repo.post_goal_supporters(&prev, goal).await {
                for s in extra {
                    if existing.insert(s["id"].as_i64().unwrap_or(0)) {
                        supporters.push(s);
                    }
                }
            }
        }
    }
    let ids: Vec<i64> = supporters.iter().filter_map(|s| s["id"].as_i64()).collect();
    let map = perks_map(state, &ids).await;
    Some(
        supporters
            .into_iter()
            .map(|s| enrich(state, s, &map))
            .collect(),
    )
}

fn progress_segments(raised: i64, carryover: i64, goal: i64) -> Value {
    if goal <= 0 {
        return json!({"carryover_percent": 0, "raised_percent": 0, "surplus_percent": 0});
    }
    let effective = carryover + raised;
    let carryover_toward = carryover.min(goal);
    let raised_toward = raised.min((goal - carryover_toward).max(0));
    let surplus = (effective - goal).max(0);
    json!({
        "carryover_percent": ((carryover_toward as f64 / goal as f64) * 100.0).round() as i64,
        "raised_percent": ((raised_toward as f64 / goal as f64) * 100.0).round() as i64,
        "surplus_percent": (((surplus as f64 / goal as f64) * 100.0).round() as i64).min(25),
    })
}

fn days_remaining_in_month(key: &str) -> i64 {
    let now = chrono::Utc::now().with_timezone(&chrono_tz::Europe::Paris);
    let today = now.date_naive();
    let Some((y, m)) = key
        .split_once('-')
        .and_then(|(y, m)| Some((y.parse::<i32>().ok()?, m.parse::<u32>().ok()?)))
    else {
        return 0;
    };
    let (ny, nm) = if m == 12 { (y + 1, 1) } else { (y, m + 1) };
    let Some(end) = chrono::NaiveDate::from_ymd_opt(ny, nm, 1).map(|d| d.pred_opt().unwrap_or(d))
    else {
        return 0;
    };
    (end - today).num_days().max(0)
}

pub async fn history(State(state): State<AppState>) -> Response {
    let goal = state.donations.monthly_goal_cents;
    let now_key = blindall_db::donations::current_month_key();
    let mut start_key = now_key.clone();
    for _ in 0..11 {
        start_key = blindall_db::donations::prev_month_key(&start_key).unwrap_or(start_key);
    }
    let aggregates: std::collections::HashMap<String, (i64, i64)> =
        match state.donations_repo.month_aggregates(&start_key).await {
            Ok(v) => v.into_iter().map(|(k, r, c)| (k, (r, c))).collect(),
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        };
    let mut carryover = state
        .donations_repo
        .carryover_cents_before_month(&start_key, goal)
        .await
        .unwrap_or(0);
    let mut history = vec![];
    let mut cursor = start_key;
    while cursor <= now_key {
        let (raised, count) = aggregates.get(&cursor).copied().unwrap_or((0, 0));
        let mut p = blindall_db::donations::progress_json(&cursor, raised, carryover, goal);
        p["donation_count"] = json!(count);
        carryover = p["surplus_cents"].as_i64().unwrap_or(0);
        history.push(p);
        let Some(next) = blindall_db::donations::next_month_key(&cursor) else {
            break;
        };
        cursor = next;
    }
    history.reverse();
    Json(json!({ "history": history })).into_response()
}

pub async fn supporters(State(state): State<AppState>) -> Response {
    match monthly_supporters(&state).await {
        Some(s) => Json(json!({ "supporters": s })).into_response(),
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn recent(State(state): State<AppState>) -> Response {
    let donations = match state.donations_repo.recent_donations(30).await {
        Ok(d) => d,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let ids: Vec<i64> = donations
        .iter()
        .filter_map(|d| d["user"]["id"].as_i64())
        .collect();
    let map = perks_map(&state, &ids).await;
    let donations: Vec<Value> = donations
        .into_iter()
        .map(|mut d| {
            if let Some(uid) = d["user"]["id"].as_i64() {
                let perks: Vec<String> = if map.contains(&uid) {
                    state.donations.supporter_perks.clone()
                } else {
                    vec![]
                };
                d["user"]["donor_perks"] = json!(perks);
                d["user"]["is_supporter"] = json!(!perks.is_empty());
            }
            d
        })
        .collect();
    Json(json!({ "donations": donations })).into_response()
}

pub async fn user_donations(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if user.id != id && !user.is_administrator {
        return StatusCode::FORBIDDEN.into_response();
    }
    let history = state
        .donations_repo
        .user_history(id, 50)
        .await
        .unwrap_or_default();
    let summary = state
        .donations_repo
        .user_summary(id)
        .await
        .unwrap_or(json!({}));
    Json(json!({"donations": history, "summary": summary})).into_response()
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
    fn payment_url_building() {
        let cfg = DonationsConfig {
            stripe_payment_url: "https://donate.stripe.com/x?foo=1#f".into(),
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
        let url = payment_url_for_user(&cfg, Some(&user), Some("u@e.fr"), Some("fr"));
        assert!(url.starts_with("https://donate.stripe.com/x?foo=1&"));
        assert!(url.contains("locale=fr"));
        assert!(url.contains("client_reference_id=7"));
        assert!(url.contains("locked_prefilled_email=u%40e.fr"));
        assert!(url.ends_with("#f"));
        let guest = blindall_db::entities::User {
            is_guest: true,
            ..user
        };
        let url = payment_url_for_user(&cfg, Some(&guest), None, Some("en"));
        assert_eq!(url, "https://donate.stripe.com/x?foo=1&locale=en#f");
    }

    #[test]
    fn segments_math() {
        let s = progress_segments(5000, 3000, 6000);
        assert_eq!(s["carryover_percent"], 50);
        assert_eq!(s["raised_percent"], 50);
        assert_eq!(s["surplus_percent"], 33_i64.min(25));
    }
}
