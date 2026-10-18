use crate::auth::CurrentUser;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

pub fn photo_url(state: &AppState, photo_path: &str, name: &str) -> String {
    if !photo_path.is_empty() {
        return state.storage.url(photo_path);
    }
    crate::avatar::fallback_url(name)
}

pub async fn show(State(state): State<AppState>, Path(user_id): Path<i64>) -> Response {
    let mut profile = match state.profiles.header(user_id).await {
        Ok(Some(p)) => p,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let key = blindall_db::donations::current_month_key();
    let is_supporter = state
        .donations_repo
        .user_is_supporter(user_id, &key)
        .await
        .unwrap_or(false);
    let perks: Vec<String> = if is_supporter {
        state.donations.supporter_perks.clone()
    } else {
        vec![]
    };
    let summary = state
        .donations_repo
        .user_summary(user_id)
        .await
        .unwrap_or(json!({}));
    let show_donations = profile["show_donation_history_on_profile"]
        .as_bool()
        .unwrap_or(false)
        && summary["donation_count"].as_i64().unwrap_or(0) > 0;
    if let Some(o) = profile.as_object_mut() {
        let name = o["name"].as_str().unwrap_or("").to_string();
        let path = o["photo"].as_str().unwrap_or("").to_string();
        o.insert("photo".into(), json!(photo_url(&state, &path, &name)));
        if let Some(team) = o.get_mut("team").and_then(|t| t.as_object_mut()) {
            let tp = team["photo"].as_str().unwrap_or("").to_string();
            let tname = team["name"].as_str().unwrap_or("").to_string();
            team.insert("photo".into(), json!(photo_url(&state, &tp, &tname)));
        }
        o.insert("donor_perks".into(), json!(perks));
        o.insert("is_supporter".into(), json!(is_supporter));
        if show_donations {
            o.insert("donation_summary".into(), summary);
        }
    }
    Json(profile).into_response()
}

#[derive(Deserialize)]
pub struct ScoresQuery {
    page: Option<i64>,
    sort: Option<String>,
    direction: Option<String>,
}

pub async fn scores(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
    Query(q): Query<ScoresQuery>,
) -> Response {
    let sort = q.sort.as_deref().unwrap_or("updated_at");
    let direction = q.direction.as_deref().unwrap_or("desc");
    match state
        .profiles
        .scores(user_id, q.page.unwrap_or(1), sort, direction)
        .await
    {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct PageQuery {
    page: Option<i64>,
}

pub async fn likes(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
    Query(q): Query<PageQuery>,
) -> Response {
    match state.profiles.likes(user_id, q.page.unwrap_or(1)).await {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn bookmarks(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
    Query(q): Query<PageQuery>,
) -> Response {
    match state.profiles.bookmarks(user_id, q.page.unwrap_or(1)).await {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn profile_minigames(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
    Query(q): Query<PageQuery>,
) -> Response {
    let page = q.page.unwrap_or(1).max(1);
    let totals = state
        .minigames
        .totals_for_user(user_id)
        .await
        .unwrap_or_default();
    let total_of = |t: &str| {
        totals
            .iter()
            .find(|(g, _)| g == t)
            .map(|(_, s)| *s)
            .unwrap_or(0)
    };
    let defs = [
        ("quiz", "Minigame quiz label", "/minigames/quiz"),
        ("who_sang", "Who sang?", "/minigames/who-sang"),
        ("anagram", "Anagram", "/minigames/anagram"),
        ("first_letter", "First letter", "/minigames/first-letter"),
        ("album_cover", "Album cover", "/minigames/album-cover"),
    ];
    let games: Vec<Value> = defs
        .iter()
        .map(|(ty, label_key, play_url)| {
            json!({"type": ty, "label_key": label_key, "play_url": play_url, "total": total_of(ty)})
        })
        .collect();
    let label_of = |ty: &str| {
        defs.iter()
            .find(|(d, _, _)| *d == ty)
            .map(|(_, l, _)| *l)
            .unwrap_or("Minigame")
    };
    let history = match state.minigames.history_for_user(user_id, page, 15).await {
        Ok(mut h) => {
            if let Some(rows) = h["data"].as_array_mut() {
                for r in rows.iter_mut() {
                    let ty = r["game_type"].as_str().unwrap_or_default().to_string();
                    r["label_key"] = json!(label_of(&ty));
                }
            }
            h
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let ctx = state
        .rankings
        .minigames_user_context(user_id, false)
        .await
        .unwrap_or_else(|_| json!({"position": null, "score": 0}));
    Json(json!({
        "minigames": {
            "games": games,
            "history": history,
            "rankings_url": "/rankings/minigames",
            "user_rank": ctx["position"],
            "user_total_score": ctx["score"],
        }
    }))
    .into_response()
}

pub async fn score_evolution(State(state): State<AppState>, Path(user_id): Path<i64>) -> Response {
    match state.profiles.score_evolution(user_id).await {
        Ok(v) => Json(json!({"score_evolution": v})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn highlights(State(state): State<AppState>, Path(user_id): Path<i64>) -> Response {
    match state.profiles.highlights(user_id).await {
        Ok(mut v) => {
            if let Some(rooms) = v["top_rooms"].as_array_mut() {
                for r in rooms.iter_mut() {
                    let path = r["room"]["photo"].as_str().unwrap_or("").to_string();
                    let name = r["room"]["name"].as_str().unwrap_or("").to_string();
                    r["room"]["photo"] = json!(room_photo_url(&state, &path, &name));
                }
            }
            let month = blindall_db::donations::current_month_key();
            let is_supporter = state
                .donations_repo
                .user_is_supporter(user_id, &month)
                .await
                .unwrap_or(false);
            let elo_rank = v["rank"]["position"].as_i64();
            let m = &v["badge_metrics"];
            let g = |k: &str| m[k].as_i64().unwrap_or(0);
            let mut badges = vec![
                badge(
                    "supporter",
                    "crown",
                    is_supporter,
                    is_supporter as i64,
                    1,
                    "/docs/support",
                ),
                badge(
                    "creator",
                    "sparkles",
                    g("rooms_created_count") >= 1,
                    g("rooms_created_count"),
                    1,
                    "/rooms/create",
                ),
                badge(
                    "explorer",
                    "map-pin",
                    g("unique_rooms_played_count") >= 5,
                    g("unique_rooms_played_count").min(5),
                    5,
                    "/",
                ),
                badge(
                    "regular",
                    "flame",
                    g("consecutive_days_streak") >= 7,
                    g("consecutive_days_streak").min(7),
                    7,
                    "/",
                ),
                badge(
                    "minigamer",
                    "gamepad",
                    g("minigame_scores_total") > 0,
                    g("minigame_scores_total").min(1),
                    1,
                    "/games",
                ),
                badge(
                    "veteran",
                    "badge-check",
                    g("months_seniority") >= 12 || g("rounds_played_count") >= 100,
                    (g("months_seniority").min(12)).max(g("rounds_played_count").min(100)),
                    100,
                    "/",
                ),
                badge(
                    "top_elo",
                    "trophy",
                    elo_rank.is_some_and(|r| r <= 100),
                    match elo_rank {
                        Some(r) if r <= 100 => 100,
                        Some(r) => (100 - (r - 100)).max(0),
                        None => 0,
                    },
                    100,
                    "/rankings?sort=elo",
                ),
                badge(
                    "collector",
                    "heart",
                    g("tracks_liked_count") >= 50,
                    g("tracks_liked_count").min(50),
                    50,
                    &format!("/profile/{user_id}?tab=likes"),
                ),
            ];
            badges.sort_by(|a, b| b["earned"].as_bool().cmp(&a["earned"].as_bool()));
            v["badges"] = json!(badges);
            v.as_object_mut().map(|o| o.remove("badge_metrics"));
            Json(v).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn room_photo_url(state: &AppState, path: &str, name: &str) -> String {
    if !path.is_empty() {
        if path.starts_with("http") {
            return path.to_string();
        }
        return state.storage.url(path);
    }
    crate::avatar::fallback_url(name)
}

fn badge(id: &str, icon: &str, earned: bool, current: i64, target: i64, url: &str) -> Value {
    let key = id.replace('_', " ");
    json!({
        "id": id,
        "label_key": format!("Profile badge {key}"),
        "description_key": format!("Profile badge {key} desc"),
        "icon": icon,
        "earned": earned,
        "current": current,
        "target": target,
        "progress_url": url,
    })
}

pub async fn unlike_track(
    State(state): State<AppState>,
    Path(track_id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    match state.profiles.unlike_track(user.id, track_id).await {
        Ok(()) => {
            state
                .levels
                .dispatch(user.id, blindall_db::LevelTrigger::LikesCount);
            Json(json!({"ok": true})).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
