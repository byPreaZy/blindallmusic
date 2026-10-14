use crate::authz::RoomChannelAuthorizer;
use crate::state::AppState;
use axum::extract::{Query, State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::Response;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Deserialize)]
pub struct WsQuery {
    token: Option<String>,
}

pub async fn handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(q): Query<WsQuery>,
) -> Result<Response, StatusCode> {
    let Some(token) = q.token else {
        return Err(StatusCode::UNAUTHORIZED);
    };
    let user = state
        .auth
        .user_for_token(&token)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?
        .ok_or(StatusCode::UNAUTHORIZED)?;
    if state.moderation.is_banned(user.id).await.unwrap_or(false) {
        return Err(StatusCode::FORBIDDEN);
    }

    let authz = Arc::new(RoomChannelAuthorizer {
        app: state.clone(),
    });
    let hub = state.hub.clone();
    Ok(ws.on_upgrade(move |socket| {
        blindall_realtime::ws::serve(socket, hub, authz, user.id)
    }))
}
