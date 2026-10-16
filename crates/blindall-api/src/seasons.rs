use crate::auth::OptionalUser;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

fn db_error() -> Response {
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}

#[derive(Deserialize)]
pub struct SeasonQuery {
    page: Option<i64>,
}

pub async fn index(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Query(q): Query<SeasonQuery>,
) -> Response {
    let (current, rewarded) = match state.seasons.ensure_current().await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "season ensure_current failed");
            return db_error();
        }
    };
    for uid in rewarded {
        state.levels.dispatch(uid, blindall_db::LevelTrigger::Score);
    }
    let season_id = current["id"].as_i64().unwrap_or(0);
    let page = q.page.unwrap_or(1).max(1);

    let mut board = match state.seasons.leaderboard(season_id, page).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, "season leaderboard failed");
            return db_error();
        }
    };
    crate::rankings::enrich_photos(&state, &mut board);

    let user_rank = match user {
        Some(u) => state
            .seasons
            .user_rank(season_id, u.id)
            .await
            .unwrap_or(Value::Null),
        None => Value::Null,
    };

    let past = state.seasons.past(4).await.unwrap_or_default();

    Json(json!({
        "current": current,
        "board": {
            "data": board["data"],
            "total": board["total"],
            "page": board["page"],
            "last_page": board["last_page"],
        },
        "user_rank": user_rank,
        "past": past,
    }))
    .into_response()
}
