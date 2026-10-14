use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
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
        .banned_users(q.search.as_deref(), q.page(), q.per_page(), sensitive(&user))
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
        .users(q.search.as_deref(), q.page(), q.per_page(), sensitive(&user))
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
    match state.moderation.user_detail(user_id, sensitive(&user)).await {
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
            Json(json!({"message": "La durée minimale doit être de 1 minute.",
                        "errors": {"duration": ["La durée minimale doit être de 1 minute."]}})),
        )
            .into_response();
    }
    match state.moderation.is_staff(user_id).await {
        Ok(true) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": "Moderation cannot ban staff"})),
            )
                .into_response()
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
            crate::chat::cleanup_banned_user_messages(&state, user_id).await;
            Json(json!({"ok": true})).into_response()
        }
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
