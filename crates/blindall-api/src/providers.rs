//! `GET /api/v1/providers/search?term=…&providers=a,b,c` — merged
//! multi-provider search (PHP: `MusicProvidersService::search` +
//! `TrackController::search`'s response shape).

use crate::auth::CurrentUser;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::response::Json;
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub struct SearchParams {
    term: Option<String>,
    providers: Option<String>,
}

pub async fn search(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<SearchParams>,
) -> Json<Value> {
    let term = q.term.unwrap_or_default();
    let providers: Vec<&str> = q
        .providers
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();

    if term.trim().is_empty() || providers.is_empty() {
        return Json(json!({ "results": [], "errors": [] }));
    }

    // PHP picks the YouTube scraper only for public-room moderators.
    let is_public_moderator = state.auth.is_public_moderator(user.id()).await;

    let (results, errors) = state
        .providers
        .search(&term, &providers, is_public_moderator)
        .await;

    Json(json!({ "results": results, "errors": errors }))
}
