use crate::auth::CurrentUser;
use crate::state::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const MAX_ATTEMPTS: usize = 5;
const DECAY: Duration = Duration::from_secs(60);

#[derive(Default)]
pub struct LoginThrottle {
    hits: Mutex<HashMap<String, Vec<Instant>>>,
}

impl LoginThrottle {
    fn attempts(&self, key: &str) -> Vec<Instant> {
        let map = self.hits.lock().unwrap();
        map.get(key)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|t| t.elapsed() < DECAY)
            .collect()
    }

    pub fn too_many_attempts(&self, key: &str) -> Option<u64> {
        let a = self.attempts(key);
        if a.len() >= MAX_ATTEMPTS {
            let oldest = a.into_iter().min().unwrap();
            Some(DECAY.saturating_sub(oldest.elapsed()).as_secs().max(1))
        } else {
            None
        }
    }

    pub fn hit(&self, key: &str) {
        let mut a = self.attempts(key);
        a.push(Instant::now());
        self.hits.lock().unwrap().insert(key.to_string(), a);
    }

    pub fn clear(&self, key: &str) {
        self.hits.lock().unwrap().remove(key);
    }
}

fn throttle_key(email: &str, ip: &str) -> String {
    format!("{}|{}", email.trim().to_lowercase(), ip)
}

pub(crate) fn client_ip(headers: &HeaderMap, fallback: &str) -> String {
    let h = |n: &str| {
        headers
            .get(n)
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    };
    if let Some(ip) = h("cf-connecting-ip") {
        return ip.to_string();
    }
    if let Some(xff) = h("x-forwarded-for") {
        if let Some(first) = xff
            .split(',')
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            return first.to_string();
        }
    }
    h("x-real-ip").unwrap_or(fallback).to_string()
}

fn validation_error(field: &str, msg: &str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({
            "message": msg,
            "errors": { field: [msg] },
        })),
    )
        .into_response()
}

fn user_json(u: &blindall_db::entities::User, is_public_moderator: bool) -> Value {
    json!({
        "id": u.id,
        "name": u.name,
        "photo_path": u.photo_path,
        "elo": u.elo,
        "is_guest": u.is_guest,
        "is_administrator": u.is_administrator,
        "is_public_moderator": is_public_moderator,
        "team_id": u.team_id,
    })
}

pub(crate) async fn enriched_user_json(
    state: &AppState,
    u: &blindall_db::entities::User,
    is_public_moderator: bool,
) -> Value {
    let mut base = user_json(u, is_public_moderator);
    base["photo"] = json!(crate::profiles::photo_url(
        state,
        u.photo_path.as_deref().unwrap_or(""),
        &u.name,
    ));
    base["team"] = match u.team_id {
        Some(tid) => state
            .teams
            .find(tid)
            .await
            .ok()
            .flatten()
            .map(|t| json!({"id": t.id, "name": t.name}))
            .unwrap_or(Value::Null),
        None => Value::Null,
    };
    base["custom_tag"] = json!(
        state
            .auth
            .custom_tag_of(u.id)
            .await
            .ok()
            .flatten()
            .unwrap_or_default()
    );
    let plans = crate::billing::entitlements(state, u.id).await;
    base["plans"] = json!(plans);
    base["is_plus"] = json!(
        plans
            .iter()
            .any(|p| p == blindall_db::subscriptions::PLAN_PLUS)
    );
    base["is_pro"] = json!(
        plans
            .iter()
            .any(|p| p == blindall_db::subscriptions::PLAN_PRO)
    );
    let snap = state.levels.snapshot(u.id).await;
    base["level"] = snap
        .as_ref()
        .and_then(|s| s["level"].as_i64())
        .map_or(json!(1), |v| v.into());
    base["current_xp"] = snap
        .as_ref()
        .map(|s| s["current_xp"].clone())
        .unwrap_or(json!(0));
    base["xp_for_next_level"] = snap
        .as_ref()
        .map(|s| s["xp_for_next_level"].clone())
        .unwrap_or(json!(100));
    base["total_xp"] = snap
        .as_ref()
        .map(|s| s["total_xp"].clone())
        .unwrap_or(json!(0));
    base["level_metrics"] = snap
        .map(|s| s["level_metrics"].clone())
        .unwrap_or(Value::Null);
    let is_supporter = !u.is_guest
        && state
            .donations_repo
            .user_is_supporter(u.id, &blindall_db::donations::current_month_key())
            .await
            .unwrap_or(false);
    let perks: Vec<&String> = if is_supporter {
        state.donations.supporter_perks.iter().collect()
    } else {
        vec![]
    };
    base["donor_perks"] = json!(perks);
    base["is_supporter"] = json!(!perks.is_empty());
    base
}

pub(crate) fn validate_name(name: &str, moderator_names: &[String]) -> Result<(), &'static str> {
    const RESERVED: &[&str] = &[
        "admin",
        "admins",
        "mchev",
        "administrateur",
        "administrator",
        "superadmin",
        "blindallmusic.com",
        "moderator",
        "moderateur",
        "test",
        "playlist",
        "room",
        "team",
    ];
    let lower = name.to_lowercase();
    if RESERVED.contains(&lower.as_str()) {
        return Err("validation.reserved");
    }
    for bad in BAD_WORDS {
        if lower.contains(bad) {
            return Err("validation.reserved");
        }
    }
    for m in moderator_names {
        if !m.is_empty() && lower.contains(m.as_str()) {
            return Err("validation.reserved");
        }
    }
    Ok(())
}

fn valid_email(email: &str) -> bool {
    email.len() <= 255
        && email.matches('@').count() == 1
        && email.split('@').all(|p| !p.is_empty())
        && email.rsplit('@').next().is_some_and(|d| d.contains('.'))
}

#[derive(Deserialize)]
pub struct RegisterBody {
    name: String,
    email: String,
    password: String,
    password_confirmation: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginBody {
    email: String,
    password: String,
    #[allow(dead_code)]
    remember: Option<bool>,
}

pub async fn register(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Json(body): Json<RegisterBody>,
) -> Response {
    let ip = client_ip(&headers, &addr.ip().to_string());
    if state.auth.is_ip_banned(&ip).await {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Access from your IP address is restricted."})),
        )
            .into_response();
    }
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 25 {
        return validation_error(
            "name",
            "The name field is required and must not exceed 25 characters.",
        );
    }
    let moderator_names = state
        .auth
        .reserved_moderator_names()
        .await
        .unwrap_or_default();
    if let Err(msg) = validate_name(name, &moderator_names) {
        return validation_error("name", msg);
    }
    if state.auth.name_taken(name).await.unwrap_or(false) {
        return validation_error("name", "The name has already been taken.");
    }
    let email = body.email.trim();
    if !valid_email(email) {
        return validation_error("email", "The email field must be a valid email address.");
    }
    if email.chars().count() > 50 {
        return validation_error("email", "The email field must not exceed 50 characters.");
    }
    if state.auth.email_taken(email).await.unwrap_or(false) {
        return validation_error("email", "The email has already been taken.");
    }
    if body.password.len() < 8 {
        return validation_error(
            "password",
            "The password field must be at least 8 characters.",
        );
    }
    if body.password_confirmation.as_deref() != Some(body.password.as_str()) {
        return validation_error(
            "password",
            "The password field confirmation does not match.",
        );
    }

    let hash = match bcrypt::hash(&body.password, 12) {
        Ok(h) => h,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let user_id = match state.auth.create_user(name, email, &hash, Some(&ip)).await {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!(error = %e, "register: create_user failed");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    {
        let (mail, email, name) = (state.mail.clone(), email.to_string(), name.to_string());
        tokio::spawn(async move {
            if let Err(e) = crate::mail::send_welcome(&mail, &email, &name).await {
                tracing::warn!(error = %e, "welcome mail failed");
            }
        });
    }
    let token = match state.auth.issue_token(user_id, "api").await {
        Ok(t) => t,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let user = blindall_db::entities::User {
        id: user_id,
        name: name.to_string(),
        photo_path: None,
        elo: 1500,
        is_guest: false,
        is_administrator: false,
        team_id: None,
    };
    (
        StatusCode::CREATED,
        Json(json!({ "token": token, "user": user_json(&user, false) })),
    )
        .into_response()
}

pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Json(body): Json<LoginBody>,
) -> Response {
    let ip = client_ip(&headers, &addr.ip().to_string());
    let key = throttle_key(&body.email, &ip);
    if let Some(seconds) = state.throttle.too_many_attempts(&key) {
        let msg = format!("Too many login attempts. Please try again in {seconds} seconds.");
        return validation_error("email", &msg);
    }

    let found = match state.auth.find_for_login(body.email.trim()).await {
        Ok(f) => f,
        Err(e) => {
            tracing::warn!(error = %e, "login: find_for_login failed");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let Some((user, password_hash, banned)) = found else {
        state.throttle.hit(&key);
        return validation_error("email", "These credentials do not match our records.");
    };
    let ok = password_hash
        .as_deref()
        .is_some_and(|h| bcrypt::verify(&body.password, h).unwrap_or(false));
    if !ok {
        state.throttle.hit(&key);
        return validation_error("email", "These credentials do not match our records.");
    }
    state.throttle.clear(&key);
    if banned {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "banned"}))).into_response();
    }
    state.auth.update_ip(user.id, &ip).await;
    state
        .levels
        .dispatch(user.id, blindall_db::LevelTrigger::Login);
    let token = match state.auth.issue_token(user.id, "api").await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(error = %e, "login: issue_token failed");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let is_mod = state.auth.is_public_moderator(user.id).await;
    Json(json!({ "token": token, "user": enriched_user_json(&state, &user, is_mod).await }))
        .into_response()
}

/// Sliding-window per-key rate limiter (PHP throttle:N,1 equivalent).
#[derive(Default)]
pub struct RateBucket {
    hits: std::sync::Mutex<std::collections::HashMap<String, Vec<Instant>>>,
}

impl RateBucket {
    /// Records a hit; returns seconds to wait if over `limit` in `window_secs`.
    pub fn retry_after(&self, key: &str, limit: usize, window_secs: u64) -> Option<u64> {
        let window = Duration::from_secs(window_secs);
        let mut map = self.hits.lock().unwrap();
        let a = map.entry(key.to_string()).or_default();
        a.retain(|t| t.elapsed() < window);
        if a.len() >= limit {
            let oldest = a.iter().min().unwrap();
            Some(window.saturating_sub(oldest.elapsed()).as_secs().max(1))
        } else {
            a.push(Instant::now());
            None
        }
    }
}

#[derive(Default)]
pub struct GuestThrottle {
    hits: Mutex<HashMap<String, Vec<Instant>>>,
}

impl GuestThrottle {
    fn prune(&self, key: &str, window: Duration) -> Vec<Instant> {
        let map = self.hits.lock().unwrap();
        map.get(key)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|t| t.elapsed() < window)
            .collect()
    }

    pub fn retry_after(&self, key: &str) -> Option<u64> {
        const LIMITS: &[(usize, Duration)] = &[
            (3, Duration::from_secs(60)),
            (20, Duration::from_secs(3600)),
            (50, Duration::from_secs(86400)),
        ];
        let mut wait = 0u64;
        for (max, window) in LIMITS {
            let hits = self.prune(key, *window);
            if hits.len() >= *max {
                let oldest = hits.iter().min().unwrap();
                wait = wait.max(window.saturating_sub(oldest.elapsed()).as_secs().max(1));
            }
        }
        (wait > 0).then_some(wait)
    }

    pub fn hit(&self, key: &str) {
        let mut a = self.prune(key, Duration::from_secs(86400));
        a.push(Instant::now());
        self.hits.lock().unwrap().insert(key.to_string(), a);
    }
}

pub async fn guest(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Response {
    if let Some(t) = bearer_token_str(&headers) {
        if let Ok(Some(u)) = state.auth.user_for_token(&t).await {
            let is_mod = state.auth.is_public_moderator(u.id).await;
            return Json(
                json!({ "token": t, "user": enriched_user_json(&state, &u, is_mod).await }),
            )
            .into_response();
        }
    }
    let ip = client_ip(&headers, &addr.ip().to_string());
    if state.auth.is_ip_banned(&ip).await {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Access from your IP address is restricted."})),
        )
            .into_response();
    }
    let key = format!("guest|{ip}");
    if let Some(seconds) = state.guest_throttle.retry_after(&key) {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"message": format!("Guest creation throttle : retry in {seconds}s"), "retry_after": seconds})),
        )
            .into_response();
    }

    use rand::Rng;
    use rand::distr::Alphanumeric;
    let name = format!("Guest-{}", rand::rng().random_range(10000..=99999));
    let email = format!(
        "guest-{}@bam.test",
        rand::rng()
            .sample_iter(&Alphanumeric)
            .take(8)
            .map(char::from)
            .collect::<String>()
    );
    let guest_token = uuid::Uuid::now_v7().to_string();
    let user_id = match state
        .auth
        .create_guest(&name, &email, &guest_token, Some(&ip))
        .await
    {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!(error = %e, "guest create failed");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    state.guest_throttle.hit(&key);
    let token = match state.auth.issue_token(user_id, "api").await {
        Ok(t) => t,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let user = blindall_db::entities::User {
        id: user_id,
        name,
        photo_path: None,
        elo: 1500,
        is_guest: true,
        is_administrator: false,
        team_id: None,
    };
    (
        StatusCode::CREATED,
        Json(json!({ "token": token, "user": user_json(&user, false) })),
    )
        .into_response()
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(t) = bearer_token_str(&headers) {
        let _ = state.auth.revoke_token(&t).await;
    }
    StatusCode::NO_CONTENT.into_response()
}

/// Short-lived one-time tickets for the WebSocket upgrade (keeps bearer
/// tokens out of URLs — and therefore out of any URL log/history).
#[derive(Default)]
pub struct WsTickets {
    issued: std::sync::Mutex<std::collections::HashMap<String, (i64, Instant)>>,
}

const WS_TICKET_TTL: Duration = Duration::from_secs(60);

impl WsTickets {
    pub fn issue(&self, user_id: i64) -> String {
        use rand::Rng;
        const CH: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let mut rng = rand::rng();
        let t: String = (0..48)
            .map(|_| CH[rng.random_range(0..CH.len())] as char)
            .collect();
        let mut map = self.issued.lock().unwrap();
        map.retain(|_, (_, t0)| t0.elapsed() < WS_TICKET_TTL);
        map.insert(t.clone(), (user_id, Instant::now()));
        t
    }

    /// One-time: consumes the ticket, returns the bound user_id if fresh.
    pub fn consume(&self, ticket: &str) -> Option<i64> {
        let mut map = self.issued.lock().unwrap();
        map.retain(|_, (_, t0)| t0.elapsed() < WS_TICKET_TTL);
        map.remove(ticket).map(|(uid, _)| uid)
    }
}

pub async fn ws_ticket(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> Response {
    let t = state.ws_tickets.issue(user.id);
    Json(json!({ "ticket": t, "expires_in": 60 })).into_response()
}

fn bearer_token_str(headers: &HeaderMap) -> Option<String> {
    headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::to_string)
}

pub async fn me(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> Response {
    let is_mod = state.auth.is_public_moderator(user.id).await;
    Json(enriched_user_json(&state, &user, is_mod).await).into_response()
}

pub(crate) const BAD_WORDS: &[&str] = &[
    "abruti",
    "andouille",
    "anal",
    "anus",
    "baise",
    "baltringue",
    "batard",
    "biatch",
    " bite",
    "bordel",
    "bouffon",
    "bouffonne",
    "bougnoul",
    "bougnoule",
    "branlette",
    "branleur",
    "bride",
    "burne",
    "branquignole",
    "cageot",
    "casse-couille",
    "casse-couilles",
    "chagasse",
    "charogne",
    "chiennasse",
    "chier",
    "chieur",
    "chieuse",
    "chinetoc",
    "chinetoque",
    "chinetok",
    "chleuh",
    " con ",
    "conchie",
    "conchier",
    "connard",
    "connasse",
    "couille",
    "couilles",
    "couiller",
    "couillon",
    "couillonne",
    "crote",
    " cul ",
    "debile",
    "ducon",
    "dugland",
    "emmerde",
    "emmerder",
    "emmerdeur",
    "emmerdeuse",
    "encule",
    "enculer",
    "enflure",
    "enfoire",
    "etron",
    "fiotte",
    "fouteur",
    "foutre",
    "fumier",
    "gland",
    "glandeur",
    "glandeuse",
    "glandu",
    "gogol",
    "gogole",
    "gourdasse",
    "gourgandine",
    "lavette",
    "lopette",
    "mange-merde",
    "merde",
    "merdeux",
    "merdeuse",
    "michto",
    "michtoneur",
    "michtoneuse",
    "mongol",
    "mongole",
    "mongolien",
    "mongolienne",
    "nazi",
    "nichon",
    "nique",
    "niquer",
    "niquez",
    "orchidoclaste",
    "pd",
    "pede",
    "pequenaud",
    "petasse",
    "pouffe",
    "pouffiasse",
    "putain",
    "pute",
    "raclure",
    "salaud",
    "salop",
    "salope",
    "sexe",
    "schlague",
    "schleu",
    "schnock",
    "schnoque",
    "sous-merde",
    "tafiole",
    "tantouse",
    "tantouze",
    "tarlouse",
    "tarlouze",
    "tarlouz",
    "tebe",
    "teub",
    "teube",
    "tepu",
    "tocard",
    "trouduc",
    "vaurien",
    "vide-couille",
    "vide-couilles",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_static_names_are_rejected() {
        for n in ["admin", "Admin", "MCHEV", "test", "room"] {
            assert!(validate_name(n, &[]).is_err(), "{n}");
        }
        assert!(validate_name("martin", &[]).is_ok());
    }

    #[test]
    fn bad_word_substring_is_rejected() {
        assert!(validate_name("superconnard42", &[]).is_err());
        assert!(validate_name("kevin", &[]).is_ok());
    }

    #[test]
    fn moderator_name_substring_is_rejected() {
        let mods = vec!["jean-modo".to_string()];
        assert!(validate_name("pasjean-modo", &mods).is_err());
        assert!(validate_name("jean", &mods).is_ok());
    }

    #[test]
    fn throttle_allows_then_locks() {
        let t = LoginThrottle::default();
        for _ in 0..MAX_ATTEMPTS {
            assert!(t.too_many_attempts("a@b.c|1.2.3.4").is_none());
            t.hit("a@b.c|1.2.3.4");
        }
        assert!(t.too_many_attempts("a@b.c|1.2.3.4").is_some());
        t.clear("a@b.c|1.2.3.4");
        assert!(t.too_many_attempts("a@b.c|1.2.3.4").is_none());
        assert!(t.too_many_attempts("a@b.c|5.6.7.8").is_none());
    }

    #[test]
    fn email_shape() {
        assert!(valid_email("a@b.com"));
        assert!(!valid_email("a@b"));
        assert!(!valid_email("a@@b.com"));
        assert!(!valid_email("@b.com"));
    }
}
