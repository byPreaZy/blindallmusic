//! Pro space backend: per-room stats and setup state for the /pro page.

use crate::auth::CurrentUser;
use crate::state::AppState;
use axum::Json;
use axum::extract::State;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Everything the Pro dashboard needs in one payload: the user's plan,
/// their rooms with activity stats, and current Free-tier usage.
pub async fn overview(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }
    let plan = state
        .subscriptions
        .active_plan(user.id)
        .await
        .ok()
        .flatten();
    let rooms = state.rooms.owned_with_stats(user.id).await.unwrap_or_default();
    let limited = crate::limits::limited_for(plan.as_deref(), user.is_administrator);
    let limits = crate::limits::usage(&state, user.id, limited).await;
    Json(json!({
        "plan": plan,
        "is_admin": user.is_administrator,
        "limits": limits,
        "rooms": rooms,
    }))
    .into_response()
}
