//! HTTP client for `/api/v1` — Sanctum bearer from localStorage.

use gloo_net::http::{Method, RequestBuilder};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;

pub const TOKEN_KEY: &str = "blindall_token";

/// Mirrors PHP `config('blindall.discord_invite_url')` — Inertia shares it on
/// every page; here it's a build-time constant with the same default.
pub const DISCORD_INVITE_URL: &str = "https://discord.gg/FVJRmKtScG";

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

pub fn stored_token() -> Option<String> {
    storage()?.get_item(TOKEN_KEY).ok()?
}

pub fn store_token(token: Option<&str>) {
    if let Some(s) = storage() {
        match token {
            Some(t) => {
                let _ = s.set_item(TOKEN_KEY, t);
            }
            None => {
                let _ = s.remove_item(TOKEN_KEY);
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: u16,
    pub message: String,
    pub errors: HashMap<String, Vec<String>>,
    /// Raw decoded body — endpoints may carry extra fields (e.g. the room
    /// name inside `password_required` 403s).
    pub body: Value,
}

impl ApiError {
    pub fn first(&self, field: &str) -> Option<String> {
        self.errors.get(field)?.first().cloned()
    }
}

fn authed(req: RequestBuilder) -> RequestBuilder {
    // `X-Locale` mirrors PHP's session locale (`SetLocale`) — the API uses
    // it for localized payloads (check messages, mails).
    let req = req.header("X-Locale", crate::i18n::locale());
    match stored_token() {
        Some(token) => req.header("Authorization", &format!("Bearer {token}")),
        None => req,
    }
}

async fn finish(
    resp: Result<gloo_net::http::Response, gloo_net::Error>,
) -> Result<gloo_net::http::Response, ApiError> {
    let resp = resp.map_err(|e| ApiError {
        status: 0,
        body: Value::Null,
        message: e.to_string(),
        errors: HashMap::new(),
    })?;
    if resp.ok() {
        return Ok(resp);
    }
    let status = resp.status();
    let body: Value = resp.json().await.unwrap_or_default();
    // banhammer `logout.banned`/`auth.banned` — any API call answered with a
    // banned-403 routes the SPA to `/user/banned` (the page's own fetch is
    // exempt by the pathname guard, no redirect loop).
    if body["error"].as_str() == Some("banned") {
        if let Some(w) = web_sys::window() {
            if w.location().pathname().ok().as_deref() != Some("/user/banned") {
                let _ = w.location().set_href("/user/banned");
            }
        }
    }
    let message = body["message"]
        .as_str()
        .or_else(|| body["error"].as_str())
        .unwrap_or("Request failed")
        .to_string();
    let mut errors = HashMap::new();
    if let Some(obj) = body["errors"].as_object() {
        for (k, v) in obj {
            if let Some(arr) = v.as_array() {
                errors.insert(
                    k.clone(),
                    arr.iter()
                        .filter_map(|m| m.as_str().map(str::to_string))
                        .collect(),
                );
            }
        }
    }
    Err(ApiError {
        status,
        message,
        errors,
        body,
    })
}

async fn send(req: RequestBuilder) -> Result<gloo_net::http::Response, ApiError> {
    finish(authed(req).send().await).await
}

/// Reads a JSON response, tolerating `204`/empty bodies — Laravel returns
/// `response()->noContent()` for many mutations. Empty parses as `null`
/// (works for `Value`/`Option<T>` callers; struct callers still error).
async fn parse<T: DeserializeOwned>(resp: gloo_net::http::Response) -> Result<T, ApiError> {
    let text = resp.text().await.map_err(|e| ApiError {
        status: 0,
        body: Value::Null,
        message: e.to_string(),
        errors: HashMap::new(),
    })?;
    let text = text.trim();
    let src = if text.is_empty() { "null" } else { text };
    serde_json::from_str::<T>(src).map_err(|e| ApiError {
        status: 0,
        body: Value::Null,
        message: e.to_string(),
        errors: HashMap::new(),
    })
}

pub async fn get<T: DeserializeOwned>(path: &str) -> Result<T, ApiError> {
    let resp = send(RequestBuilder::new(path).method(Method::GET)).await?;
    parse(resp).await
}

pub async fn post<T: DeserializeOwned, B: Serialize>(
    path: &str,
    body: &B,
) -> Result<T, ApiError> {
    let body = serde_json::to_string(body).map_err(|e| ApiError {
        status: 0,
        body: Value::Null,
        message: e.to_string(),
        errors: HashMap::new(),
    })?;
    let req = authed(RequestBuilder::new(path).method(Method::POST))
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| ApiError {
            status: 0,
        body: Value::Null,
            message: e.to_string(),
            errors: HashMap::new(),
        })?;
    let resp = finish(req.send().await).await?;
    parse(resp).await
}

pub async fn post_empty<T: DeserializeOwned>(path: &str) -> Result<T, ApiError> {
    post(path, &serde_json::json!({})).await
}

pub async fn put<T: DeserializeOwned, B: Serialize>(
    path: &str,
    body: &B,
) -> Result<T, ApiError> {
    let body = serde_json::to_string(body).map_err(|e| ApiError {
        status: 0,
        body: Value::Null,
        message: e.to_string(),
        errors: HashMap::new(),
    })?;
    let req = authed(RequestBuilder::new(path).method(Method::PUT))
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| ApiError {
            status: 0,
        body: Value::Null,
            message: e.to_string(),
            errors: HashMap::new(),
        })?;
    let resp = finish(req.send().await).await?;
    parse(resp).await
}

pub async fn patch<T: DeserializeOwned, B: Serialize>(
    path: &str,
    body: &B,
) -> Result<T, ApiError> {
    let body = serde_json::to_string(body).map_err(|e| ApiError {
        status: 0,
        body: Value::Null,
        message: e.to_string(),
        errors: HashMap::new(),
    })?;
    let req = authed(RequestBuilder::new(path).method(Method::PATCH))
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| ApiError {
            status: 0,
        body: Value::Null,
            message: e.to_string(),
            errors: HashMap::new(),
        })?;
    let resp = finish(req.send().await).await?;
    parse(resp).await
}

pub async fn delete<T: DeserializeOwned>(path: &str) -> Result<T, ApiError> {
    let resp = send(RequestBuilder::new(path).method(Method::DELETE)).await?;
    parse(resp).await
}

/// POST for endpoints answering `204 No Content` (chat store, logout…).
pub async fn post_no_content<B: Serialize>(path: &str, body: &B) -> Result<(), ApiError> {
    let body = serde_json::to_string(body).map_err(|e| ApiError {
        status: 0,
        body: Value::Null,
        message: e.to_string(),
        errors: HashMap::new(),
    })?;
    let req = authed(RequestBuilder::new(path).method(Method::POST))
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| ApiError {
            status: 0,
        body: Value::Null,
            message: e.to_string(),
            errors: HashMap::new(),
        })?;
    finish(req.send().await).await.map(|_| ())
}

/// DELETE for `204 No Content` endpoints.
pub async fn delete_no_content(path: &str) -> Result<(), ApiError> {
    send(RequestBuilder::new(path).method(Method::DELETE))
        .await
        .map(|_| ())
}

/// DELETE with a JSON body (Laravel `->detach($request->user_id)` style).
pub async fn delete_with_body<T: DeserializeOwned, B: Serialize>(
    path: &str,
    body: &B,
) -> Result<T, ApiError> {
    let body = serde_json::to_string(body).map_err(|e| ApiError {
        status: 0,
        body: Value::Null,
        message: e.to_string(),
        errors: HashMap::new(),
    })?;
    let req = authed(RequestBuilder::new(path).method(Method::DELETE))
        .header("Content-Type", "application/json")
        .body(body)
        .map_err(|e| ApiError {
            status: 0,
        body: Value::Null,
            message: e.to_string(),
            errors: HashMap::new(),
        })?;
    let resp = finish(req.send().await).await?;
    parse(resp).await
}
