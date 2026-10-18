use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

use crate::auth::CurrentUser;
use crate::state::AppState;

async fn require_moderator(state: &AppState, user: &blindall_db::entities::User) -> bool {
    user.is_administrator || state.auth.is_public_moderator(user.id).await
}

fn forbidden() -> Response {
    StatusCode::FORBIDDEN.into_response()
}

#[derive(Deserialize)]
pub struct ListQuery {
    search: Option<String>,
    page: Option<i64>,
    per_page: Option<i64>,
}

impl ListQuery {
    fn page(&self) -> i64 {
        self.page.unwrap_or(1).max(1)
    }
    fn per_page(&self) -> i64 {
        self.per_page.unwrap_or(20).clamp(1, 100)
    }
}

fn sensitive(user: &blindall_db::entities::User) -> bool {
    user.is_administrator
}

// ---- dashboard -------------------------------------------------------------

pub async fn suggestions(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !require_moderator(&state, &user).await {
        return forbidden();
    }
    let list = state
        .notifications
        .suggestions(user.id, 200)
        .await
        .unwrap_or_default();
    Json(json!({"suggestions": list})).into_response()
}

pub async fn dashboard(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> Response {
    if !require_moderator(&state, &user).await {
        return forbidden();
    }
    match state.moderation.stats().await {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ---- trashed messages ------------------------------------------------------

pub async fn trashed_messages(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !require_moderator(&state, &user).await {
        return forbidden();
    }
    match state
        .moderation
        .trashed_messages(q.search.as_deref(), q.page(), q.per_page())
        .await
    {
        Ok(p) => Json(p.to_json()).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn restore_message(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !require_moderator(&state, &user).await {
        return forbidden();
    }
    match state.moderation.restore_message(id).await {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "Message is not deleted."})),
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn destroy_message(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !require_moderator(&state, &user).await {
        return forbidden();
    }
    match state.moderation.force_delete_message(id).await {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "Message is not deleted."})),
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ---- banned users ----------------------------------------------------------

pub async fn banned_users(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !require_moderator(&state, &user).await {
        return forbidden();
    }
    match state
        .moderation
        .banned_users(
            q.search.as_deref(),
            q.page(),
            q.per_page(),
            sensitive(&user),
        )
        .await
    {
        Ok(p) => Json(json!({
            "bannedUsers": p.to_json(),
            "canViewSensitiveData": sensitive(&user),
        }))
        .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn unban(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !require_moderator(&state, &user).await {
        return forbidden();
    }
    match state.moderation.unban(user_id).await {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "Moderation user is not banned"})),
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ---- user management -------------------------------------------------------

pub async fn users(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !require_moderator(&state, &user).await {
        return forbidden();
    }
    match state
        .moderation
        .users(
            q.search.as_deref(),
            q.page(),
            q.per_page(),
            sensitive(&user),
        )
        .await
    {
        Ok(p) => Json(json!({
            "users": p.to_json(),
            "canViewSensitiveData": sensitive(&user),
        }))
        .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn user_show(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !require_moderator(&state, &user).await {
        return forbidden();
    }
    match state
        .moderation
        .user_detail(user_id, sensitive(&user))
        .await
    {
        Ok(Some(v)) => Json(json!({
            "user": v,
            "canViewSensitiveData": sensitive(&user),
        }))
        .into_response(),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct BanBody {
    reason: String,
    duration: Option<i64>,
}

pub async fn ban(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<BanBody>,
) -> Response {
    if !require_moderator(&state, &user).await {
        return forbidden();
    }
    let reason = body.reason.trim();
    if reason.is_empty() || reason.len() > 255 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"message": "La raison du bannissement est requise.",
                        "errors": {"reason": ["La raison du bannissement est requise."]}})),
        )
            .into_response();
    }
    if body.duration.is_some_and(|d| d < 1) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(
                json!({"message": "La durée minimale doit être de 1 minute.",
                        "errors": {"duration": ["La durée minimale doit être de 1 minute."]}}),
            ),
        )
            .into_response();
    }
    match state.moderation.is_staff(user_id).await {
        Ok(true) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": "Moderation cannot ban staff"})),
            )
                .into_response();
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        Ok(false) => {}
    }
    let ip = match state.moderation.user_detail(user_id, true).await {
        Ok(Some(v)) => v["ip"].as_str().map(|s| s.to_string()),
        _ => None,
    };
    match state
        .moderation
        .ban(user_id, user.id, reason, body.duration, ip.as_deref())
        .await
    {
        Ok(()) => {
            let _ = state
                .notifications
                .notify(
                    user_id,
                    "App\\Notifications\\UserBanned",
                    json!({
                        "reason": reason,
                        "duration": body.duration,
                        "type": "ban",
                        "message": format!("Ton compte a été suspendu : {reason}"),
                    }),
                )
                .await;
            crate::chat::cleanup_banned_user_messages(&state, user_id).await;
            Json(json!({"ok": true})).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ---- track rematch (Deezer) -------------------------------------------------

#[derive(Deserialize)]
pub struct RematchSearchQuery {
    q: Option<String>,
}

/// GET /api/v1/moderation/tracks/{id}/rematch?q=
/// Deezer candidates for a track — the default query is "Artist Title" built
/// from its stored answers; moderators can pass an explicit q.
pub async fn track_rematch_candidates(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<RematchSearchQuery>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !require_moderator(&state, &user).await {
        return forbidden();
    }
    let term = match q.q.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(t) => t.to_string(),
        None => {
            let answers = state
                .playlists
                .track_answers_labeled(id)
                .await
                .unwrap_or_default();
            let find = |ty: &str| {
                answers
                    .iter()
                    .find(|a| a["type"].as_str() == Some(ty))
                    .and_then(|a| a["value"].as_str())
                    .unwrap_or("")
                    .to_string()
            };
            let term = format!("{} {}", find("Artist"), find("Title"));
            term.trim().to_string()
        }
    };
    if term.is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "Track has no artist/title to search with."})),
        )
            .into_response();
    }
    let (results, errors) = state.providers.search(&term, &["deezer"], true).await;
    Json(json!({"tracks": results, "term": term, "errors": errors})).into_response()
}

#[derive(Deserialize)]
pub struct RematchBody {
    provider: String,
    provider_id: String,
    provider_url: Option<String>,
    artwork_url: Option<String>,
    track_name: Option<String>,
    artist_name: Option<String>,
}

/// POST /api/v1/moderation/tracks/{id}/rematch
/// Repoint the track at the chosen Deezer resource and sync its Title/Artist
/// answers with the new version (keeps media answers and scores).
pub async fn track_rematch_apply(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<RematchBody>,
) -> Response {
    if !require_moderator(&state, &user).await {
        return forbidden();
    }
    if body.provider != "deezer" || body.provider_id.trim().is_empty() {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "Only deezer rematches are supported."})),
        )
            .into_response();
    }
    let preview = format!("deezer://{}", body.provider_id.trim());
    match state
        .playlists
        .rematch_track(
            id,
            "deezer",
            body.provider_id.trim(),
            body.provider_url.as_deref(),
            &preview,
            body.artwork_url.as_deref(),
            body.track_name.as_deref(),
            body.artist_name.as_deref(),
        )
        .await
    {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ---- moderators (admin only) -----------------------------------------------

pub async fn moderators(
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.moderation.moderators(q.page(), q.per_page()).await {
        Ok(p) => Json(json!({"moderators": p.to_json()})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn detach_room_moderator(
    State(state): State<AppState>,
    Path((room_id, user_id)): Path<(i64, i64)>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.rooms.detach_moderator(room_id, user_id).await {
        Ok(_) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn detach_playlist_moderator(
    State(state): State<AppState>,
    Path((playlist_id, user_id)): Path<(i64, i64)>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.playlists.detach_moderator(playlist_id, user_id).await {
        Ok(_) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
