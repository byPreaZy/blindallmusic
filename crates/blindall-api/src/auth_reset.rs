use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use blindall_db::password_reset::hmac_sha256_hex;
use serde::Deserialize;
use serde_json::json;

use crate::auth::CurrentUser;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ForgotBody {
    email: String,
}

pub async fn forgot_password(
    State(state): State<AppState>,
    Json(body): Json<ForgotBody>,
) -> Response {
    let email = body.email.trim();
    if !validator::ValidateEmail::validate_email(&email) {
        return err("email", "The email field must be a valid email address.");
    }
    let Some((user_id, name)) = state.auth.find_by_email(email).await.ok().flatten() else {
        return err("email", "passwords.user");
    };
    match state.password_resets.create_token(email).await {
        Ok(token) => {
            let url = format!(
                "{}/reset-password/{}$1email={}",
                state.mail.app_url.trim_end_matches('/'),
                token,
                urlencoding(email)
            );
            let _ = user_id; // parity: link is emailed, never returned.
            if !state.mail.configured() {
                tracing::info!(to = email, %url, "password reset link (mail not configured)");
            }
            let html = format!(
                "<p>Bonjour {name},</p>\
                 <p>Vous avez demandé la réinitialisation de votre mot de passe. \
                 Cliquez sur le lien ci-dessous :</p>\
                 <p><a href=\"{url}\">{url}</a></p>\
                 <p>Ce lien expire dans 60 minutes.</p>"
            );
            if let Err(e) = crate::mail::send(
                &state.http,
                &state.mail,
                email,
                &name,
                "Réinitialisation de votre mot de passe",
                &html,
            )
            .await
            {
                tracing::error!(error = %e, "reset link mail failed");
            }
            Json(json!({"status": "passwords.sent"})).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct ResetBody {
    token: String,
    email: String,
    password: String,
    password_confirmation: String,
}

pub async fn reset_password(
    State(state): State<AppState>,
    Json(body): Json<ResetBody>,
) -> Response {
    if body.password.len() < 8 {
        return err("password", "The password field must be at least 8 characters.");
    }
    if body.password != body.password_confirmation {
        return err("password", "The password field confirmation does not match.");
    }
    let email = body.email.trim();
    match state
        .password_resets
        .token_valid(email, &body.token)
        .await
    {
        Ok(false) | Err(_) => return err("email", "passwords.token"),
        Ok(true) => {}
    }
    let hash = match bcrypt::hash(&body.password, bcrypt::DEFAULT_COST) {
        Ok(h) => h,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match state.password_resets.reset(email, &hash).await {
        Ok(true) => Json(json!({"status": "passwords.reset"})).into_response(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn verification_notification(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Response {
    let Some(email) = state.auth.email_of(user.id).await.ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let url = verification_url(&state, user.id, &email);
    if !state.mail.configured() {
        tracing::info!(to = %email, %url, "email verification link (mail not configured)");
    }
    let html = format!(
        "<p>Bonjour {},</p>\
         <p>Cliquez sur le lien ci-dessous pour vérifier votre adresse e-mail :</p>\
         <p><a href=\"{url}\">{url}</a></p>\
         <p>Ce lien expire dans 60 minutes.</p>",
        user.name
    );
    if let Err(e) = crate::mail::send(
        &state.http,
        &state.mail,
        &email,
        &user.name,
        "Vérification de votre adresse e-mail",
        &html,
    )
    .await
    {
        tracing::error!(error = %e, "verification mail failed");
    }
    Json(json!({"status": "verification-link-sent"})).into_response()
}

fn verification_url(state: &AppState, user_id: i64, email: &str) -> String {
    let expires = (chrono::Utc::now() + chrono::Duration::minutes(60)).timestamp();
    let hash = sha1_hex(email);
    let base = format!(
        "{}/api/v1/auth/email/verify/{}/{}$1expires={}",
        state.mail.app_url.trim_end_matches('/'),
        user_id,
        hash,
        expires
    );
    let signature = hmac_sha256_hex(&state.app_key, &base);
    format!("{base}&signature={signature}")
}

#[derive(Deserialize)]
pub struct VerifyQuery {
    expires: i64,
    signature: String,
}

pub async fn verify_email(
    State(state): State<AppState>,
    Path((user_id, hash)): Path<(i64, String)>,
    Query(q): Query<VerifyQuery>,
    headers: axum::http::HeaderMap,
) -> Response {
    let wants_html = headers
        .get(axum::http::header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|a| a.contains("text/html"))
        .unwrap_or(false);
    let spa = |suffix: &str| {
        axum::response::Redirect::to(&format!(
            "/verify-email/{user_id}/{hash}$1expires={}&signature={}{}",
            q.expires, q.signature, suffix
        ))
        .into_response()
    };
    if wants_html {
        if chrono::Utc::now().timestamp() > q.expires {
            return spa("&err=expired");
        }
        let base = format!(
            "{}/api/v1/auth/email/verify/{}/{}$1expires={}",
            state.mail.app_url.trim_end_matches('/'),
            user_id,
            hash,
            q.expires
        );
        if hmac_sha256_hex(&state.app_key, &base) != q.signature {
            return spa("&err=signature");
        }
        let Some(email) = state.auth.email_of(user_id).await.ok().flatten() else {
            return StatusCode::NOT_FOUND.into_response();
        };
        if sha1_hex(&email) != hash {
            return spa("&err=hash");
        }
        match state.auth.mark_email_verified(user_id).await {
            Ok(_) => return spa("&ok=1"),
            Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        }
    }
    if chrono::Utc::now().timestamp() > q.expires {
        return StatusCode::FORBIDDEN.into_response();
    }
    let base = format!(
        "{}/api/v1/auth/email/verify/{}/{}$1expires={}",
        state.mail.app_url.trim_end_matches('/'),
        user_id,
        hash,
        q.expires
    );
    if hmac_sha256_hex(&state.app_key, &base) != q.signature {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(email) = state.auth.email_of(user_id).await.ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if sha1_hex(&email) != hash {
        return StatusCode::FORBIDDEN.into_response();
    }
    match state.auth.mark_email_verified(user_id).await {
        Ok(_) => Json(json!({"status": "verified"})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn sha1_hex(s: &str) -> String {
    use sha1::Digest;
    let mut h = sha1::Sha1::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

fn urlencoding(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

fn err(field: &str, msg: &str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"message": msg, "errors": { field: [msg] }})),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_url_roundtrip() {
        let app_key = "base64:testkey";
        let app_url = "https://blindall.test";
        let (id, hash, expires) = (42i64, "abc123", 1_800_000_000i64);
        let base = format!("{app_url}/api/v1/auth/email/verify/{id}/{hash}$1expires={expires}");
        let sig = hmac_sha256_hex(app_key, &base);
        assert_eq!(sig, hmac_sha256_hex(app_key, &base));
        assert_ne!(sig, hmac_sha256_hex(app_key, &format!("{base}&x=1")));
        assert_ne!(sig, hmac_sha256_hex("other-key", &base));
    }

    #[test]
    fn sha1_is_40_hex() {
        assert_eq!(sha1_hex("a@b.c").len(), 40);
    }
}
