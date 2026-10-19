use crate::auth_session::client_ip;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const STATE_TTL: Duration = Duration::from_secs(600);

#[derive(Clone, Default)]
pub struct OAuthConfig {
    pub providers: HashMap<String, OAuthProviderEntry>,
}

#[derive(Clone)]
pub struct OAuthProviderEntry {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
}

#[derive(Default)]
pub struct OAuthStates {
    issued: Mutex<HashMap<String, Instant>>,
}

impl OAuthStates {
    pub fn issue(&self) -> String {
        use rand::Rng;
        const CH: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        let mut rng = rand::rng();
        let state: String = (0..40)
            .map(|_| CH[rng.random_range(0..CH.len())] as char)
            .collect();
        let mut map = self.issued.lock().unwrap();
        map.retain(|_, t| t.elapsed() < STATE_TTL);
        map.insert(state.clone(), Instant::now());
        state
    }

    pub fn consume(&self, state: &str) -> bool {
        let mut map = self.issued.lock().unwrap();
        map.retain(|_, t| t.elapsed() < STATE_TTL);
        map.remove(state).is_some()
    }
}

fn provider_meta(name: &str) -> Option<(&'static str, &'static str, &'static str, &'static str)> {
    match name {
        "google" => Some((
            "https://accounts.google.com/o/oauth2/auth",
            "https://oauth2.googleapis.com/token",
            "https://openidconnect.googleapis.com/v1/userinfo",
            "openid email profile",
        )),
        "discord" => Some((
            "https://discord.com/oauth2/authorize",
            "https://discord.com/api/oauth2/token",
            "https://discord.com/api/users/@me",
            "identify email",
        )),
        _ => None,
    }
}

pub async fn redirect(State(state): State<AppState>, Path(provider): Path<String>) -> Response {
    match authorize_url(&state, &provider) {
        Ok(url) => Json(json!({ "url": url })).into_response(),
        Err(r) => r,
    }
}

pub async fn redirect_compat(
    State(state): State<AppState>,
    Path(provider): Path<String>,
) -> Response {
    match authorize_url(&state, &provider) {
        Ok(url) => axum::response::Redirect::to(&url).into_response(),
        Err(r) => r,
    }
}

pub async fn callback_compat(
    Path(provider): Path<String>,
    axum::extract::RawQuery(query): axum::extract::RawQuery,
) -> Response {
    let q = query.map(|q| format!("?{q}")).unwrap_or_default();
    axum::response::Redirect::to(&format!("/oauth/{provider}/callback{q}")).into_response()
}

fn authorize_url(state: &AppState, provider: &str) -> Result<String, Response> {
    let Some((auth_url, _, _, scope)) = provider_meta(provider) else {
        return Err(StatusCode::NOT_FOUND.into_response());
    };
    let Some(cfg) = state.oauth.providers.get(provider) else {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({"error": "provider_not_configured"})),
        )
            .into_response());
    };
    let oauth_state = state.oauth_states.issue();
    let q: Vec<(String, String)> = vec![
        ("client_id".into(), cfg.client_id.clone()),
        ("redirect_uri".into(), cfg.redirect_uri.clone()),
        ("response_type".into(), "code".to_string()),
        ("state".into(), oauth_state),
        ("scope".into(), scope.to_string()),
    ];
    Ok(format!("{auth_url}?{}", serde_urlencoded(&q)))
}

fn serde_urlencoded(pairs: &[(String, String)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", urlenc(k), urlenc(v)))
        .collect::<Vec<_>>()
        .join("&")
}

fn urlenc(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => c.to_string(),
            _ => format!("%{:02X}", c as u32),
        })
        .collect()
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

pub async fn callback(
    State(state): State<AppState>,
    Path(provider): Path<String>,
    Query(q): Query<CallbackQuery>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
) -> Response {
    let Some((_, token_url, user_url, _)) = provider_meta(&provider) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(cfg) = state.oauth.providers.get(&provider) else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    if q.error.is_some() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "oauth_denied"})),
        )
            .into_response();
    }
    let (Some(code), Some(oauth_state)) = (q.code, q.state) else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "missing_code_or_state"})),
        )
            .into_response();
    };
    if !state.oauth_states.consume(&oauth_state) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_state"})),
        )
            .into_response();
    }

    let t0 = Instant::now();
    let token = match exchange_code(&state.http, cfg, token_url, &code).await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(provider, error = %e, "oauth token exchange failed");
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "oauth_exchange_failed"})),
            )
                .into_response();
        }
    };
    let t_exchange = t0.elapsed();
    let t1 = Instant::now();
    let info = match fetch_profile(&state.http, &provider, user_url, &token).await {
        Ok(i) => i,
        Err(e) => {
            tracing::warn!(provider, error = %e, "oauth profile fetch failed");
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": "oauth_profile_failed"})),
            )
                .into_response();
        }
    };
    let t_profile = t1.elapsed();
    let Some(email) = info.email.filter(|e| !e.is_empty()) else {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "oauth_no_email"})),
        )
            .into_response();
    };

    let user = match state.auth.find_by_provider_id(&provider, &info.id).await {
        Ok(Some(u)) => u,
        Ok(None) => match state.auth.find_for_login(&email).await {
            Ok(Some((existing, _, _))) => {
                let _ = state
                    .auth
                    .link_provider(existing.id, &provider, &info.id)
                    .await;
                existing
            }
            Ok(None) => {
                let name = unique_name(&state, &info.name).await;
                match state
                    .auth
                    .create_social_user(&name, &email, &provider, &info.id)
                    .await
                {
                    Ok(id) => {
                        let mail = state.mail.clone();
                        let (e2, n2) = (email.clone(), name.clone());
                        tokio::spawn(async move {
                            let _ = crate::mail::send_welcome(&mail, &e2, &n2).await;
                        });
                        blindall_db::entities::User {
                            id,
                            name,
                            photo_path: None,
                            elo: 1500,
                            is_guest: false,
                            is_administrator: false,
                            team_id: None,
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "oauth create_user failed");
                        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                    }
                }
            }
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    if state.auth.is_user_banned(user.id).await {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "banned"}))).into_response();
    }
    let t_resolve = t0.elapsed() - t_exchange - t_profile;
    let ip = client_ip(&headers, &addr.ip().to_string());
    state.auth.update_ip(user.id, &ip).await;
    let token = match state.auth.issue_token(user.id, &provider).await {
        Ok(t) => t,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let is_mod = state.auth.is_public_moderator(user.id).await;
    let t2 = Instant::now();
    let payload = crate::auth_session::enriched_user_json(&state, &user, is_mod).await;
    tracing::info!(
        provider,
        exchange_ms = t_exchange.as_millis() as u64,
        profile_ms = t_profile.as_millis() as u64,
        resolve_ms = t_resolve.as_millis() as u64,
        enrich_ms = t2.elapsed().as_millis() as u64,
        total_ms = t0.elapsed().as_millis() as u64,
        "oauth callback timing"
    );
    Json(json!({
        "token": token,
        "user": payload,
    }))
    .into_response()
}

struct SocialProfile {
    id: String,
    email: Option<String>,
    name: String,
}

async fn unique_name(state: &AppState, base: &str) -> String {
    let base = if base.trim().is_empty() {
        "Player"
    } else {
        base.trim()
    };
    let mut name = base.to_string();
    let mut count = 0;
    while state.auth.name_taken(&name).await.unwrap_or(false) {
        name = format!("{base}-{count}");
        count += 1;
    }
    name
}

async fn exchange_code(
    http: &reqwest::Client,
    cfg: &OAuthProviderEntry,
    token_url: &str,
    code: &str,
) -> anyhow::Result<String> {
    let form = vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", code.to_string()),
        ("redirect_uri", cfg.redirect_uri.clone()),
        ("client_id", cfg.client_id.clone()),
        ("client_secret", cfg.client_secret.clone()),
    ];
    let resp = http.post(token_url).form(&form).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("token endpoint returned {}", resp.status());
    }
    let body: Value = resp.json().await?;
    body.get("access_token")
        .and_then(|t| t.as_str())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("no access_token in response"))
}

async fn fetch_profile(
    http: &reqwest::Client,
    provider: &str,
    user_url: &str,
    token: &str,
) -> anyhow::Result<SocialProfile> {
    let resp = http.get(user_url).bearer_auth(token).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("profile endpoint returned {}", resp.status());
    }
    let v: Value = resp.json().await?;
    let pick = |keys: &[&str]| -> Option<String> {
        for k in keys {
            match v.get(*k) {
                Some(Value::String(s)) => return Some(s.clone()),
                Some(Value::Number(n)) => return Some(n.to_string()),
                _ => {}
            }
        }
        None
    };
    let id = match provider {
        "google" => pick(&["sub", "id"]),
        _ => pick(&["id"]),
    }
    .ok_or_else(|| anyhow::anyhow!("profile missing id"))?;
    let name = match provider {
        "discord" => pick(&["username", "global_name"]),
        _ => pick(&["name"]),
    }
    .unwrap_or_else(|| "Player".to_string());
    Ok(SocialProfile {
        id,
        email: pick(&["email"]),
        name,
    })
}
