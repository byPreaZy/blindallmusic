use crate::auth::{CurrentUser, OptionalUser};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const CACHE_TTL: Duration = Duration::from_secs(600);

#[derive(Default)]
pub struct RankingsCache {
    map: Mutex<HashMap<String, (Value, Instant)>>,
}

impl RankingsCache {
    pub fn get(&self, key: &str) -> Option<Value> {
        let m = self.map.lock().ok()?;
        let (v, t) = m.get(key)?;
        (Instant::now().duration_since(*t) < CACHE_TTL).then(|| v.clone())
    }

    pub fn put(&self, key: String, value: Value) {
        if let Ok(mut m) = self.map.lock() {
            if m.len() > 500 {
                let now = Instant::now();
                m.retain(|_, (_, t)| now.duration_since(*t) < CACHE_TTL);
            }
            m.insert(key, (value, Instant::now()));
        }
    }
}

fn db_error() -> Response {
    StatusCode::INTERNAL_SERVER_ERROR.into_response()
}

pub(crate) fn enrich_photos(state: &AppState, payload: &mut Value) {
    let Some(items) = payload["data"].as_array_mut() else {
        return;
    };
    for entry in items.iter_mut() {
        for key in ["user", "team"] {
            if let Some(u) = entry[key].as_object_mut() {
                let path = u
                    .get("photo_path")
                    .and_then(|p| p.as_str())
                    .unwrap_or("")
                    .to_string();
                let name = u.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let photo = crate::profiles::photo_url(state, &path, name);
                u.remove("photo_path");
                u.insert("photo".into(), json!(photo));
            }
        }
    }
}

#[derive(Deserialize)]
pub struct PlayersQuery {
    sort: Option<String>,
    page: Option<i64>,
    room: Option<i64>,
}

pub async fn players(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Query(q): Query<PlayersQuery>,
) -> Response {
    let sort = q.sort.as_deref().unwrap_or("elo");
    let page = q.page.unwrap_or(1).max(1);
    let room_id = match state.rankings.resolve_official_room_id(q.room).await {
        Ok(v) => v,
        Err(_) => return db_error(),
    };
    let cache_room = room_id.map(|r| r.to_string()).unwrap_or_else(|| "all".into());
    let lb_key = format!("global_leaderboard:v3:{sort}:room:{cache_room}:page:{page}");
    let mut leaderboard = state.rankings_cache.get(&lb_key);
    if leaderboard.is_none() {
        match state.rankings.leaderboard(sort, page, room_id).await {
            Ok(mut v) => {
                enrich_photos(&state, &mut v);
                state.rankings_cache.put(lb_key, v.clone());
                leaderboard = Some(v);
            }
            Err(e) => {
                tracing::warn!(error = %e, "rankings leaderboard failed");
                return db_error();
            }
        }
    }
    let rooms = state.rankings.official_rooms().await.unwrap_or_default();
    let mut user_context = Value::Null;
    if let Some(u) = user {
        let ctx_key =
            format!("global_leaderboard:v3:user:{}:sort:{sort}:room:{cache_room}:context", u.id);
        let mut ctx = state.rankings_cache.get(&ctx_key);
        if ctx.is_none() {
            match state.rankings.user_context(u.id, sort, room_id).await {
                Ok(mut v) => {
                    let mut wrap = json!({"data": [v["entry"].clone()]});
                    enrich_photos(&state, &mut wrap);
                    v["entry"] = wrap["data"][0].clone();
                    state.rankings_cache.put(ctx_key, v.clone());
                    ctx = Some(v);
                }
                Err(_) => ctx = Some(Value::Null),
            }
        }
        user_context = ctx.unwrap_or(Value::Null);
    }
    Json(json!({
        "leaderboard": leaderboard.unwrap_or(json!({"data": [], "total": 0})),
        "sort": sort,
        "sorts": [
            {"id": "level", "label": "Level"},
            {"id": "score", "label": "Score"},
            {"id": "elo", "label": "ELO"},
            {"id": "week", "label": "Top Week"},
            {"id": "avg_time", "label": "Avg. response time"},
            {"id": "best_round", "label": "Best round"},
        ],
        "official_rooms": rooms,
        "room_id": room_id,
        "user_context": user_context,
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct PageQuery {
    page: Option<i64>,
}

pub async fn teams(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Query(q): Query<PageQuery>,
) -> Response {
    let page = q.page.unwrap_or(1).max(1);
    let key = format!("teams_leaderboard:v1:page:{page}");
    let mut top = state.rankings_cache.get(&key);
    if top.is_none() {
        match state.rankings.teams_leaderboard(page).await {
            Ok(mut v) => {
                enrich_photos(&state, &mut v);
                state.rankings_cache.put(key, v.clone());
                top = Some(v);
            }
            Err(_) => return db_error(),
        }
    }
    let mut position = Value::Null;
    if let Some(u) = user {
        position = state
            .rankings
            .user_team_position(u.id)
            .await
            .unwrap_or(Value::Null);
    }
    Json(json!({
        "top_teams": top.unwrap_or(json!({"data": [], "total": 0})),
        "user_position": position,
    }))
    .into_response()
}

pub async fn minigames(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Query(q): Query<PageQuery>,
) -> Response {
    let page = q.page.unwrap_or(1).max(1);
    let key = format!("minigame_leaderboard:v1:page:{page}");
    let mut top = state.rankings_cache.get(&key);
    if top.is_none() {
        match state.rankings.minigames_leaderboard(page).await {
            Ok(mut v) => {
                enrich_photos(&state, &mut v);
                state.rankings_cache.put(key, v.clone());
                top = Some(v);
            }
            Err(_) => return db_error(),
        }
    }
    let mut user_context = Value::Null;
    if let Some(u) = user {
        let ctx_key = format!("minigame_leaderboard:v1:user:{}:context", u.id);
        let mut ctx = state.rankings_cache.get(&ctx_key);
        if ctx.is_none() {
            ctx = state
                .rankings
                .minigames_user_context(u.id, u.is_guest)
                .await
                .ok()
                .map(|v| {
                    state.rankings_cache.put(ctx_key, v.clone());
                    v
                });
        }
        user_context = ctx.unwrap_or(Value::Null);
    }
    Json(json!({
        "top_by_minigames": top.unwrap_or(json!({"data": [], "total": 0})),
        "user_context": user_context,
    }))
    .into_response()
}

pub async fn user_level_metrics(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
    CurrentUser(_user): CurrentUser,
) -> Response {
    match state.rankings.user_level_metrics(user_id).await {
        Ok(v) => Json(v).into_response(),
        Err(_) => db_error(),
    }
}
