use crate::auth::CurrentUser;
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
pub struct ListQuery {
    search: Option<String>,
    page: Option<i64>,
}

pub async fn index(State(state): State<AppState>, Query(q): Query<ListQuery>) -> Response {
    match state
        .faqs
        .list(q.search.as_deref(), q.page.unwrap_or(1), 6)
        .await
    {
        Ok((faqs, total)) => Json(json!({
            "data": faqs,
            "total": total,
            "per_page": 6,
            "current_page": q.page.unwrap_or(1).max(1),
        }))
        .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn vote(
    State(state): State<AppState>,
    Path((id, direction)): Path<(i64, String)>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if user.is_guest {
        return StatusCode::FORBIDDEN.into_response();
    }
    let value = match direction.as_str() {
        "up" => 1,
        "down" => -1,
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    match state.faqs.vote(user.id, id, value).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
