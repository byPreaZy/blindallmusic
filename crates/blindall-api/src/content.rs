//! Misc public endpoints — `PageController::show/bannedUser`,
//! `ContactController::send` port.

use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;

/// `GET /api/v1/pages/{slug}` — `PageController::show` (latest revision).
/// `page.date` — `revised_at->translatedFormat('l j F Y H:i:s')` localized
/// from the request `X-Locale` header.
pub async fn page(
    State(state): State<AppState>,
    Path(slug): Path<String>,
    headers: axum::http::HeaderMap,
) -> Response {
    match state.admin.page_by_slug(&slug).await {
        Ok(Some(mut page)) => {
            let locale = match headers
                .get("x-locale")
                .and_then(|v| v.to_str().ok())
                .map(str::trim)
            {
                Some(l @ ("fr" | "en" | "es")) => l,
                _ => "fr",
            };
            if let Some(dt) = page["revised_at"].as_str().and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .map(|d| d.with_timezone(&chrono::Utc))
                    .ok()
            }) {
                page["date"] = json!(format_long_date(dt, locale));
            }
            Json(json!({"page": page})).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `translatedFormat('l j F Y H:i:s')` — "lundi 13 octobre 2026 14:30:00".
fn format_long_date(dt: chrono::DateTime<chrono::Utc>, locale: &str) -> String {
    use chrono::Datelike;
    const DAYS: [[&str; 7]; 3] = [
        ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"],
        ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"],
        ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"],
    ];
    const MONTHS: [[&str; 12]; 3] = [
        ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"],
        ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"],
        ["enero", "febrero", "marzo", "abril", "mayo", "junio", "julio", "agosto", "septiembre", "octubre", "noviembre", "diciembre"],
    ];
    let idx = match locale { "fr" => 1, "es" => 2, _ => 0 };
    let wd = dt.weekday().num_days_from_monday() as usize;
    let mo = dt.month0() as usize;
    format!(
        "{} {} {} {} {}",
        DAYS[idx][wd], dt.day(), MONTHS[idx][mo], dt.year(),
        dt.format("%H:%M:%S")
    )
}

/// `GET /api/v1/user/banned` — `bannedUser`: `{"ban": null}` when the
/// authed user isn't banned (Laravel renders the page with a null prop).
/// Unauthenticated guests 403 like PHP's `abort(403)`.
pub async fn banned_user(
    State(state): State<AppState>,
    user: crate::auth::OptionalUser,
) -> Response {
    // `PageController::bannedUser` — the route is outside `logout.banned` so
    // a banned token still resolves here; guests get PHP's `abort(403)`.
    let Some(user) = user.0 else {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "banned"}))).into_response();
    };
    let ban = match state.moderation.is_banned(user.id).await {
        Ok(true) => state.moderation.latest_ban(user.id).await.ok().flatten(),
        Ok(false) => None,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    Json(json!({
        "ban": ban.map(|b| json!({
            "comment": b["comment"].as_str().unwrap_or("Votre adresse IP a été bloquée"),
            // `$ban->expired_at->diffForHumans() ?? 'jamais'` — FR diff.
            "expired_at": b["expired_at"]
                .as_str()
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| diff_for_humans_fr_in(dt.with_timezone(&chrono::Utc)))
                .unwrap_or_else(|| "jamais".to_string()),
        }))
    }))
    .into_response()
}

/// Carbon `diffForHumans()` on a FUTURE date — "dans 3 jours".
fn diff_for_humans_fr_in(d: chrono::DateTime<chrono::Utc>) -> String {
    let secs = (d - chrono::Utc::now()).num_seconds().max(1);
    let (n, unit) = if secs < 60 {
        (secs, "seconde")
    } else if secs < 3_600 {
        (secs / 60, "minute")
    } else if secs < 86_400 {
        (secs / 3_600, "heure")
    } else if secs < 604_800 {
        (secs / 86_400, "jour")
    } else if secs < 2_592_000 {
        (secs / 604_800, "semaine")
    } else if secs < 31_536_000 {
        (secs / 2_592_000, "mois")
    } else {
        (secs / 31_536_000, "an")
    };
    let s = if n > 1 && unit != "mois" { "s" } else { "" };
    format!("dans {n} {unit}{s}")
}

#[derive(Deserialize)]
pub struct ContactBody {
    email: String,
    message: String,
}

/// `POST /api/v1/contact` — `ContactController::send`: mail to the
/// `mail.from.address` mailbox, from/reply-to the sender.
pub async fn contact(
    State(state): State<AppState>,
    user: crate::auth::OptionalUser,
    Json(body): Json<ContactBody>,
) -> Response {
    let email = body.email.trim();
    if email.is_empty() || !email.contains('@') || body.message.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"message": "validation"})),
        )
            .into_response();
    }
    let esc = |s: &str| {
        s.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('\n', "<br>")
    };
    // `ContactMessage::toMail` — subject uses the sender name (or
    // "Visiteur"), body starts with "name: email" when authed.
    let subject = match &user.0 {
        Some(u) => format!("Message de {}", u.name),
        None => "Message de Visiteur".to_string(),
    };
    let user_email = match &user.0 {
        Some(u) => state.auth.email_of(u.id).await.ok().flatten(),
        None => None,
    };
    let body_html = match (&user.0, &user_email) {
        (Some(u), Some(ue)) => format!(
            "<p>{}: {}</p><p>{}</p>",
            esc(&u.name),
            esc(ue),
            esc(&body.message)
        ),
        _ => format!("<p>{}</p>", esc(&body.message)),
    };
    // PHP notifies the User row owning `mail.from.address` — the mailbox.
    let _ = crate::mail::send_opts(
        &state.http,
        &state.mail,
        &state.mail.from_email,
        &state.mail.from_name,
        &subject,
        &body_html,
        Some(email),
    )
    .await;
    Json(json!({"ok": true})).into_response()
}
