use crate::auth::{CurrentUser, OptionalUser};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use blindall_db::RoomOptions;
use blindall_game::{GameStore, PlayerInfo, RoomCommand};
use serde::Deserialize;
use serde_json::{Map, Value, json};

fn forbidden() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({"error": "unauthorized"})),
    )
        .into_response()
}

fn ip_banned_response() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({"error": "Access from your IP address is restricted."})),
    )
        .into_response()
}

fn validation(field: &str, msg: &str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"message": msg, "errors": {field: [msg]}})),
    )
        .into_response()
}

pub(crate) fn require_not_guest(user: &blindall_db::entities::User) -> Option<Response> {
    user.is_guest.then(|| {
        (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "guest_forbidden"})),
        )
            .into_response()
    })
}

pub async fn list(State(state): State<AppState>) -> Json<Value> {
    let rooms = state.store.public_playing_rooms().await.unwrap_or_default();
    let now = chrono::Utc::now().timestamp();
    let mut out = Vec::with_capacity(rooms.len());
    for r in rooms {
        let members = state.presence.member_count(r.id, now).await.unwrap_or(0);
        out.push(json!({
            "id": r.id,
            "name": r.name,
            "slug": r.slug,
            "is_playing": r.is_playing,
            "member_count": members,
            "tracks_by_round": r.tracks_by_round,
        }));
    }
    Json(json!({ "rooms": out }))
}

#[derive(serde::Deserialize)]
pub struct ShowParams {
    password: Option<String>,
}

pub async fn show(
    State(state): State<AppState>,
    Path(key): Path<String>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    axum::extract::Query(params): axum::extract::Query<ShowParams>,
    user: OptionalUser,
) -> Result<Json<Value>, Response> {
    let ip = crate::auth_session::client_ip(&headers, &addr.ip().to_string());
    if state.auth.is_ip_banned(&ip).await {
        return Err(ip_banned_response());
    }
    if let Some(u) = &user.0 {
        if state.moderation.is_banned(u.id).await.unwrap_or(false) {
            return Err(ip_banned_response());
        }
    }
    let row = match key.parse::<i64>() {
        Ok(id) => state.rooms.find(id).await.ok().flatten(),
        Err(_) => state.rooms.find_by_slug(&key).await.ok().flatten(),
    }
    .ok_or_else(|| StatusCode::NOT_FOUND.into_response())?;
    let id = row.id;
    let cfg = state
        .store
        .room_config(id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND.into_response())?;

    let room_password = state.rooms.password_of(id).await.ok().flatten();
    if let Some(expected) = &room_password {
        let given = params.password.as_deref().unwrap_or("");
        let ok = if expected.starts_with("$2") {
            bcrypt::verify(given, expected).unwrap_or(false)
        } else {
            given == expected
        };
        if !ok {
            return Err((
                StatusCode::FORBIDDEN,
                Json(json!({
                    "error": "password_required",
                    "room": {"id": row.id, "name": row.name, "slug": row.slug},
                })),
            )
                .into_response());
        }
    }

    if let Some(u) = &user.0 {
        let _ = state.presence.grant_participation(id, u.id).await;
    }

    let can_edit = match &user.0 {
        Some(u) => {
            state
                .rooms
                .has_control(&row, u.id, u.is_administrator)
                .await
        }
        None => false,
    };
    let is_bookmarked = match &user.0 {
        Some(u) => state.rooms.is_bookmarked(u.id, id).await.unwrap_or(false),
        None => false,
    };

    let snapshot = state.manager.snapshot(id).await.ok().flatten();
    let now = chrono::Utc::now().timestamp();
    let member_count = state.presence.member_count(id, now).await.unwrap_or(0);
    let is_playing = snapshot
        .as_ref()
        .map(|s| s.is_playing)
        .unwrap_or(row.is_playing);

    let photo = row.photo_path.as_deref().map(|p| state.storage.url(p));
    let slug = row.slug.clone().unwrap_or_else(|| id.to_string());
    let url = format!("/rooms/{slug}");
    let moderators: Vec<Value> = state
        .rooms
        .moderators_detail(id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(mid, name, photo_path)| {
            json!({
                "id": mid,
                "name": name,
                "photo": photo_path.as_deref().map(|p| state.storage.url(p)),
            })
        })
        .collect();
    let tracks_count = state.rooms.tracks_count(id).await.unwrap_or(0);
    let rounds_count = state.rooms.rounds_count(id).await.unwrap_or(0);
    let owner = state
        .rooms
        .owner(id)
        .await
        .ok()
        .flatten()
        .map(|(oid, name, photo_path)| {
            json!({
                "id": oid,
                "name": name,
                "photo": photo_path.as_deref().map(|p| state.storage.url(p)),
            })
        });
    let category = match row.category_id {
        Some(cid) => state
            .rooms
            .category_name_slug(cid)
            .await
            .ok()
            .flatten()
            .map(|(name, cslug)| json!({ "id": cid, "name": name, "slug": cslug })),
        None => None,
    };
    let public_rooms: Vec<Value> = state
        .rooms
        .public_room_cards()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(slug, name, photo_path)| {
            json!({
                "slug": slug,
                "name": name,
                "photo": photo_path.as_deref().map(|p| state.storage.url(p)),
            })
        })
        .collect();

    let seo =
        if row.is_public && room_password.is_none() && crate::official_seo::is_official_slug(&slug)
        {
            let mut breadcrumbs = vec![json!({"label": "Home", "href": "/"})];
            if let Some(cat) = &category {
                if let (Some(cname), Some(cslug)) = (cat["name"].as_str(), cat["slug"].as_str()) {
                    breadcrumbs.push(json!({
                        "label": cname,
                        "href": format!("/blind-test-{cslug}"),
                    }));
                }
            }
            breadcrumbs.push(json!({"label": row.name, "href": null}));

            let similar_rooms: Vec<Value> = match row.category_id {
                Some(cid) => state
                    .rooms
                    .official_similar_rooms(cid, id, crate::official_seo::slugs(), 4)
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(rslug, name, photo_path)| {
                        json!({
                            "slug": rslug,
                            "name": name,
                            "photo": photo_path.as_deref().map(|p| state.storage.url(p)),
                        })
                    })
                    .collect(),
                None => Vec::new(),
            };

            let intro = crate::official_seo::configured_intro(&slug, "fr");
            json!({
                "breadcrumbs": breadcrumbs,
                "stats": {
                    "rounds": rounds_count,
                    "tracks": tracks_count,
                    "players_online": member_count,
                },
                "similar_rooms": similar_rooms,
                "content": {
                    "intro": intro.as_ref().and_then(|i| i.intro.clone()),
                    "intro_secondary": intro.as_ref().and_then(|i| i.intro_secondary.clone()),
                },
            })
        } else {
            Value::Null
        };

    Ok(Json(json!({
        "seo": seo,
        "room": {
            "id": cfg.id,
            "name": cfg.name,
            "description": row.description,
            "photo": photo,
            "slug": slug,
            "url": url,
            "can_edit": can_edit,
            "is_bookmarked": is_bookmarked,
            "is_playing": is_playing,
            "is_chat_active": row.is_chat_active,
            "is_random": row.is_random,
            "track_duration": cfg.track_duration,
            "pause_between_tracks": cfg.pause_between_tracks,
            "pause_between_rounds": cfg.pause_between_rounds,
            "tracks_by_round": cfg.tracks_by_round,
            "is_public": cfg.is_public,
            "is_autostart": cfg.is_autostart,
            "is_active": row.is_active,
            "member_count": member_count,
            "tracks_count": tracks_count,
            "rounds_count": rounds_count,
            "moderators": moderators,
            "owner": owner,
            "category": category,
        },
        "public_rooms": public_rooms,
        "game": snapshot.map(|s| json!({
            "is_playing": s.is_playing,
            "round": s.round.map(|r| blindall_game::events::round_payload(&r)),
            "track": s.current_track.map(|t| json!({
                "id": t.id,
                "provider": t.provider,
                "preview_url": t.preview_url,
                "audio": t.audio,
                "hint": t.hint,
                "answers": t.answers.iter().map(|a| json!({
                    "id": a.id, "name": a.type_name,
                })).collect::<Vec<_>>(),
            })),
            "played_track_ids": s.played_track_ids,
        })),
    })))
}

// ---- presence + hydration ---------------------------------------------------

pub(crate) fn player_info_json(
    state: &AppState,
    p: &PlayerInfo,
    score_total: f64,
    perks: &[String],
) -> Value {
    json!({
        "id": p.id,
        "name": p.name,
        "is_guest": p.is_guest,
        "team": p.team_id.map(|id| json!({
            "id": id,
            "name": p.team_name.clone().unwrap_or_default(),
        })),
        "photo": crate::profiles::photo_url(state, p.photo.as_deref().unwrap_or(""), &p.name),
        "elo": p.elo,
        "level": p.level,
        "current_xp": p.current_xp,
        "xp_for_next_level": p.xp_for_next_level,
        "total_xp": p.total_xp,
        "userLevel": {
            "level": p.level,
            "rounds_played_count": p.rounds_played_count,
        },
        "public_rounds_played_count": p.public_rounds_played_count,
        "level_metrics": {
            "score_public_rooms": p.score_public_rooms,
            "seniority_months": p.months_seniority,
            "consecutive_days_streak": p.consecutive_days_streak,
            "rooms_created_count": p.rooms_created_count,
            "playlists_created_count": p.playlists_created_count,
            "tracks_liked_count": p.tracks_liked_count,
            "has_team": p.team_id.is_some(),
        },
        "score": {
            "answers": [],
            "points": 0,
            "total": score_total,
        },
        "donor_perks": perks,
        "is_supporter": !perks.is_empty(),
        "role": if p.is_administrator { "admin" } else if p.is_moderator { "moderator" } else { "" },
        "custom_tag": p.custom_tag.as_deref().unwrap_or(""),
    })
}

async fn room_state(state: &AppState, room_id: i64) -> Value {
    let now = chrono::Utc::now().timestamp();
    let member_ids = state
        .presence
        .member_ids(room_id, now)
        .await
        .unwrap_or_default();
    let info = state
        .store
        .players_info(&member_ids)
        .await
        .unwrap_or_default();

    let snapshot = state.manager.snapshot(room_id).await.ok().flatten();
    let (round_id, raw_scores) = match snapshot.as_ref().and_then(|s| s.round.as_ref()) {
        Some(r) if r.finished_at.is_none() => {
            let sc = state.scores.all_scores(r.id).await.unwrap_or_default();
            (Some(r.id), sc)
        }
        _ => (None, Default::default()),
    };
    let mut scores = Map::new();
    let raw_map: std::collections::HashMap<i64, f64> = raw_scores.into_iter().collect();
    for (uid, s) in &raw_map {
        scores.insert(uid.to_string(), json!(s));
    }

    let supporter_ids: std::collections::HashSet<i64> = state
        .donations_repo
        .supporter_ids(&member_ids, &blindall_db::donations::current_month_key())
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();
    let configured: Vec<String> = state.donations.supporter_perks.clone();

    let users: Vec<Value> = member_ids
        .iter()
        .filter_map(|uid| info.get(uid))
        .map(|p| {
            let perks: &[String] = if supporter_ids.contains(&p.id) {
                &configured
            } else {
                &[]
            };
            player_info_json(state, p, raw_map.get(&p.id).copied().unwrap_or(0.0), perks)
        })
        .collect();

    json!({
        "users": users,
        "scores": scores,
        "roundId": round_id,
    })
}

async fn broadcast_room_state(state: &AppState, room_id: i64) {
    let channel = blindall_game::events::room_channel(room_id);
    let st = room_state(state, room_id).await;
    state.hub.broadcast(&channel, "RoomState", st, None).await;
}

async fn broadcast_public_state(state: &AppState, cfg: &blindall_game::store::RoomConfig) {
    let now = chrono::Utc::now().timestamp();
    let count = state.presence.member_count(cfg.id, now).await.unwrap_or(0);
    let snapshot = state.manager.snapshot(cfg.id).await.ok().flatten();
    let (idx, playing) = snapshot
        .as_ref()
        .and_then(|s| s.round.as_ref())
        .map(|r| (r.current, r.is_playing))
        .unwrap_or((0, false));
    let payload = blindall_game::events::room_public_state(cfg, count, idx, playing);
    state
        .hub
        .broadcast(
            &blindall_game::events::public_channel(cfg.id),
            "RoomPublicState",
            payload,
            None,
        )
        .await;
}

pub async fn joined(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    user: CurrentUser,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let row = load_room(&state, id).await.map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "room_not_found"})),
        )
    })?;
    let cfg = state.store.room_config(id).await.map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "room_not_found"})),
        )
    })?;

    if !state
        .presence
        .has_participation(id, user.id())
        .await
        .unwrap_or(false)
    {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error": "room_access_required"})),
        ));
    }

    if !row.is_playing && cfg.is_autostart {
        let _ = state
            .manager
            .send(
                id,
                RoomCommand::Start {
                    user_id: Some(user.id()),
                },
            )
            .await;
    }

    let st = room_state(&state, id).await;
    let snapshot = state.manager.snapshot(id).await.ok().flatten();

    let played: Vec<Value> = match &snapshot {
        Some(s) => {
            let mut v = Vec::new();
            for tid in &s.played_track_ids {
                if let Ok(Some(t)) = state.store.load_track(*tid).await {
                    let mut p = blindall_game::events::track_playlist_payload(&t);
                    if let Ok(vp) = state.votes.payload(*tid, user.id()).await {
                        p["user_voted_up"] = vp["user_voted_up"].clone();
                        p["user_voted_down"] = vp["user_voted_down"].clone();
                    }
                    v.push(p);
                }
            }
            v
        }
        None => vec![],
    };

    let mut track = Value::Null;
    let mut round = Value::Null;
    let mut start_time = 0.0_f64;
    if let Some(s) = &snapshot {
        if let Some(r) = &s.round {
            if r.finished_at.is_none() {
                round = blindall_game::events::round_payload(r);
                if let Some(t) = &s.current_track {
                    track = json!({
                        "id": t.id,
                        "provider": t.provider,
                        "preview_url": t.preview_url,
                        "audio": t.audio,
                        "hint": t.hint,
                        "answers": t.answers.iter().map(|a| json!({
                            "id": a.id, "name": a.type_name,
                        })).collect::<Vec<_>>(),
                    });
                    if let Some(started) = r.current_track_started_at {
                        let elapsed =
                            (chrono::Utc::now() - started).num_milliseconds() as f64 / 1000.0;
                        start_time = elapsed.max(0.0).min(cfg.track_duration as f64);
                    }
                }
            }
        }
    }

    Ok(Json(json!({
        "round": round,
        "track": track,
        "room": {"id": cfg.id, "is_playing": row.is_playing},
        "startTime": start_time,
        "scores": st["scores"],
        "users": st["users"],
        "roundId": st["roundId"],
        "playedTracks": played,
    })))
}

pub async fn presence_joined(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    user: CurrentUser,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if let Some(secs) = state
        .presence_rate
        .retry_after(&format!("presence|{}", user.id()), 10, 60)
    {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "rate_limited", "retry_after": secs})),
        ));
    }

    let cfg = state.store.room_config(id).await.map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "room_not_found"})),
        )
    })?;
    let now = chrono::Utc::now().timestamp();
    let _ = state.presence.add_member(id, user.id(), now).await;
    let st = room_state(&state, id).await;
    state
        .hub
        .broadcast(
            &blindall_game::events::room_channel(id),
            "RoomState",
            st.clone(),
            None,
        )
        .await;
    broadcast_public_state(&state, &cfg).await;
    Ok(Json(json!({
        "ok": true,
        "users": st["users"],
        "scores": st["scores"],
        "roundId": st["roundId"],
    })))
}

pub async fn presence_left(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    user: CurrentUser,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if let Some(secs) = state
        .presence_rate
        .retry_after(&format!("presence|{}", user.id()), 10, 60)
    {
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "rate_limited", "retry_after": secs})),
        ));
    }

    let cfg = state.store.room_config(id).await.map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "room_not_found"})),
        )
    })?;
    let _ = state.presence.remove_member(id, user.id()).await;
    broadcast_room_state(&state, id).await;
    broadcast_public_state(&state, &cfg).await;
    Ok(Json(json!({"ok": true})))
}

pub async fn public_state(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    user: OptionalUser,
) -> Result<Json<Value>, StatusCode> {
    let ip = crate::auth_session::client_ip(&headers, &addr.ip().to_string());
    if state.auth.is_ip_banned(&ip).await {
        return Err(StatusCode::FORBIDDEN);
    }
    if let Some(u) = &user.0 {
        if state.moderation.is_banned(u.id).await.unwrap_or(false) {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    let cfg = state
        .store
        .room_config(id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let now = chrono::Utc::now().timestamp();
    let count = state.presence.member_count(id, now).await.unwrap_or(0);
    let snapshot = state.manager.snapshot(id).await.ok().flatten();
    let (idx, playing) = snapshot
        .as_ref()
        .and_then(|s| s.round.as_ref())
        .map(|r| (r.current, r.is_playing))
        .unwrap_or((0, false));
    let mut out = blindall_game::events::room_public_state(&cfg, count, idx, playing);
    if playing {
        if let (Some(t), Some(started)) = (
            snapshot.as_ref().and_then(|s| s.current_track.as_ref()),
            snapshot
                .as_ref()
                .and_then(|s| s.round.as_ref())
                .and_then(|r| r.current_track_started_at),
        ) {
            out["current_track"] = blindall_game::events::salon_track_started(
                t,
                started,
                cfg.track_duration,
                idx,
                cfg.tracks_by_round.into(),
            );
        }
    }
    Ok(Json(out))
}

/// Salon/TV live view state: current track audio, running round standings
/// and reveal data. Open for public rooms; participants only otherwise.
pub async fn live_state(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    user: OptionalUser,
) -> Result<Json<Value>, StatusCode> {
    let ip = crate::auth_session::client_ip(&headers, &addr.ip().to_string());
    if state.auth.is_ip_banned(&ip).await {
        return Err(StatusCode::FORBIDDEN);
    }
    if let Some(u) = &user.0 {
        if state.moderation.is_banned(u.id).await.unwrap_or(false) {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    let cfg = state
        .store
        .room_config(id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    if !cfg.is_public {
        let allowed = match &user.0 {
            Some(u) => {
                state
                    .presence
                    .has_participation(id, u.id)
                    .await
                    .unwrap_or(false)
                    || state.rooms.is_moderator(id, u.id).await
                    || u.is_administrator
            }
            None => false,
        };
        if !allowed {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    let now = chrono::Utc::now().timestamp();
    let count = state.presence.member_count(id, now).await.unwrap_or(0);
    let snapshot = state.manager.snapshot(id).await.ok().flatten();
    let round_any = snapshot.as_ref().and_then(|s| s.round.as_ref());
    let round = round_any.filter(|r| r.finished_at.is_none());
    let playing = round.map(|r| r.is_playing).unwrap_or(false);

    // Keep showing the just-finished round's scores until the snapshot clears.
    let mut standings: Vec<Value> = vec![];
    if let Some(r) = round_any {
        let raw = state.scores.all_scores(r.id).await.unwrap_or_default();
        let ids: Vec<i64> = raw.iter().map(|(u, _)| *u).collect();
        let infos = state.store.players_info(&ids).await.unwrap_or_default();
        let mut rows: Vec<Value> = raw
            .into_iter()
            .map(|(uid, score)| {
                let info = infos.get(&uid);
                json!({
                    "user_id": uid,
                    "name": info.map(|p| p.name.clone()).unwrap_or_default(),
                    "photo": info.and_then(|p| p.photo.clone()),
                    "score": score,
                })
            })
            .collect();
        rows.sort_by(|a, b| {
            b["score"]
                .as_f64()
                .unwrap_or(0.0)
                .partial_cmp(&a["score"].as_f64().unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        standings = rows;
    }

    let can_control = match &user.0 {
        Some(u) => u.is_administrator || state.rooms.is_moderator(id, u.id).await,
        None => false,
    };
    let paused = snapshot.as_ref().map(|s| s.paused).unwrap_or(false);
    let mut phase = "idle";
    let mut out = json!({
        "room_id": id,
        "name": cfg.name,
        "venue_name": cfg.venue_name,
        "is_playing": playing,
        "can_control": can_control,
        "member_count": count,
        "current_index": round.map(|r| r.current).unwrap_or(0),
        "tracks_total": cfg.tracks_by_round,
        "standings": standings,
    });
    if paused {
        // Never reveal answers while the host has the room paused.
        phase = "paused";
    } else if playing {
        if let (Some(t), Some(started)) = (
            snapshot.as_ref().and_then(|s| s.current_track.as_ref()),
            round.and_then(|r| r.current_track_started_at),
        ) {
            let elapsed = (chrono::Utc::now() - started).num_seconds();
            if elapsed < cfg.track_duration as i64 {
                phase = "playing";
                out["current_track"] = blindall_game::events::salon_track_started(
                    t,
                    started,
                    cfg.track_duration,
                    out["current_index"].as_i64().unwrap_or(0),
                    cfg.tracks_by_round.into(),
                );
            } else {
                // Track's window elapsed but the round still points at it:
                // inter-track pause → the answers can be revealed on screen.
                phase = "reveal";
                out["reveal_track"] = blindall_game::events::track_playlist_payload(t);
            }
        }
    }
    out["phase"] = json!(phase);
    Ok(Json(out))
}

/// QR code SVG pointing at the room join URL — used by the salon/TV view.
pub async fn qr_code(State(state): State<AppState>, Path(id): Path<i64>) -> Response {
    if state.store.room_config(id).await.is_err() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let url = format!("{}/rooms/{}", state.mail.app_url.trim_end_matches('/'), id);
    let Ok(code) = qrcode::QrCode::new(url.as_bytes()) else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let svg = code
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(512, 512)
        .build();
    ([(axum::http::header::CONTENT_TYPE, "image/svg+xml")], svg).into_response()
}

// ---- management ------------------------------------------------------------

fn room_json(r: &blindall_db::RoomRow) -> Value {
    json!({
        "id": r.id,
        "user_id": r.user_id,
        "name": r.name,
        "slug": r.slug,
        "description": r.description,
        "category_id": r.category_id,
        "photo_path": r.photo_path,
        "tracks_by_round": r.tracks_by_round,
        "track_duration": r.track_duration,
        "pause_between_tracks": r.pause_between_tracks,
        "pause_between_rounds": r.pause_between_rounds,
        "is_public": r.is_public,
        "is_random": r.is_random,
        "is_chat_active": r.is_chat_active,
        "is_playing": r.is_playing,
        "is_autostart": r.is_autostart,
        "is_active": r.is_active,
        "is_featured": r.is_featured,
        "color": r.color,
        "venue_name": r.venue_name,
        "has_password": r.has_password,
        "user_count": r.user_count,
        "deleted_at": r.deleted_at,
    })
}

async fn load_room(state: &AppState, id: i64) -> Result<blindall_db::RoomRow, Response> {
    state
        .rooms
        .find(id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
        .ok_or_else(|| StatusCode::NOT_FOUND.into_response())
}

async fn load_room_by_key(state: &AppState, key: &str) -> Result<blindall_db::RoomRow, Response> {
    let row = match key.parse::<i64>() {
        Ok(id) => state.rooms.find(id).await,
        Err(_) => state.rooms.find_by_slug(key).await,
    };
    row.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())?
        .ok_or_else(|| StatusCode::NOT_FOUND.into_response())
}

pub async fn edit(
    State(state): State<AppState>,
    Path(key): Path<String>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let room = match load_room_by_key(&state, &key).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    let id = room.id;
    if !state
        .rooms
        .has_control(&room, user.id, user.is_administrator)
        .await
    {
        return forbidden();
    }
    let mods = state.rooms.moderators(id).await.unwrap_or_default();
    let pls = state.rooms.room_playlists(id).await.unwrap_or_default();
    let mut v = room_json(&room);
    v["moderators"] = json!(
        mods.iter()
            .map(|(id, n)| json!({"id": id, "name": n}))
            .collect::<Vec<_>>()
    );
    v["playlists"] = json!(
        pls.iter()
            .map(|(id, n)| json!({"id": id, "name": n}))
            .collect::<Vec<_>>()
    );
    Json(json!({
        "room": v,
        "owned": user.is_administrator || room.user_id == user.id,
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct CreateRoom {
    name: String,
    category_id: i64,
}

pub async fn store(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<CreateRoom>,
) -> Response {
    if let Some(r) = require_not_guest(&user) {
        return r;
    }
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 25 {
        return validation(
            "name",
            "The name field is required and must not exceed 25 characters.",
        );
    }
    let mods = state
        .auth
        .reserved_moderator_names()
        .await
        .unwrap_or_default();
    if crate::auth_session::validate_name(name, &mods).is_err() {
        return validation("name", "validation.reserved");
    }
    if state.rooms.name_taken(name, None).await.unwrap_or(false) {
        return validation("name", "The name has already been taken.");
    }
    if !state
        .rooms
        .category_exists(body.category_id)
        .await
        .unwrap_or(false)
    {
        return validation("category_id", "The selected category id is invalid.");
    }
    match state.rooms.create(user.id, name, body.category_id).await {
        Ok(id) => {
            state
                .levels
                .dispatch(user.id, blindall_db::LevelTrigger::RoomsCount);
            match state.rooms.find(id).await {
                Ok(Some(room)) => {
                    (StatusCode::CREATED, Json(json!({"room": room_json(&room)}))).into_response()
                }
                _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "room create failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct UpdateRoom {
    name: String,
    description: Option<String>,
    category_id: i64,
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<UpdateRoom>,
) -> Response {
    if let Some(r) = require_not_guest(&user) {
        return r;
    }
    let room = match load_room(&state, id).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    if !state
        .rooms
        .has_control(&room, user.id, user.is_administrator)
        .await
    {
        return forbidden();
    }
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 25 {
        return validation(
            "name",
            "The name field is required and must not exceed 25 characters.",
        );
    }
    let mods = state
        .auth
        .reserved_moderator_names()
        .await
        .unwrap_or_default();
    if !name.eq_ignore_ascii_case(&room.name)
        && crate::auth_session::validate_name(name, &mods).is_err()
    {
        return validation("name", "validation.reserved");
    }
    if state
        .rooms
        .name_taken(name, Some(id))
        .await
        .unwrap_or(false)
    {
        return validation("name", "The name has already been taken.");
    }
    if !state
        .rooms
        .category_exists(body.category_id)
        .await
        .unwrap_or(false)
    {
        return validation("category_id", "The selected category id is invalid.");
    }
    match state
        .rooms
        .update_fields(id, name, body.description.as_deref(), body.category_id)
        .await
    {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct UpdateOptions {
    password: Option<String>,
    has_password: Option<bool>,
    tracks_by_round: i64,
    track_duration: i64,
    pause_between_tracks: i64,
    pause_between_rounds: i64,
    is_chat_active: bool,
    is_autostart: bool,
    is_random: bool,
    is_active: Option<bool>,
    color: Option<String>,
    venue_name: Option<String>,
    is_featured: Option<bool>,
}

pub async fn update_options(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<UpdateOptions>,
) -> Response {
    if let Some(r) = require_not_guest(&user) {
        return r;
    }
    let room = match load_room(&state, id).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    if !state
        .rooms
        .has_control(&room, user.id, user.is_administrator)
        .await
    {
        return forbidden();
    }
    if !(1..=50).contains(&body.tracks_by_round) {
        return validation(
            "tracks_by_round",
            "The tracks by round field must be between 1 and 50.",
        );
    }
    if !(5..=30).contains(&body.track_duration) {
        return validation(
            "track_duration",
            "The track duration field must be between 5 and 30.",
        );
    }
    if !(0..=60).contains(&body.pause_between_tracks) {
        return validation(
            "pause_between_tracks",
            "The pause between tracks field must be between 0 and 60.",
        );
    }
    if !(0..=60).contains(&body.pause_between_rounds) {
        return validation(
            "pause_between_rounds",
            "The pause between rounds field must be between 0 and 60.",
        );
    }
    let venue_name = body
        .venue_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string);
    if venue_name.as_deref().map(str::len).unwrap_or(0) > 80 {
        return validation(
            "venue_name",
            "The venue name field must be at most 80 characters.",
        );
    }
    let options = RoomOptions {
        password: body.password,
        has_password: body.has_password.unwrap_or(false),
        tracks_by_round: body.tracks_by_round,
        track_duration: body.track_duration,
        pause_between_tracks: body.pause_between_tracks,
        pause_between_rounds: body.pause_between_rounds,
        is_chat_active: body.is_chat_active,
        is_autostart: body.is_autostart,
        is_random: body.is_random,
        is_active: body.is_active,
        color: body.color,
        venue_name,
        is_featured: user.is_administrator.then_some(body.is_featured).flatten(),
    };
    match state.rooms.update_options(id, &options).await {
        Ok(()) => {
            if let Ok(cfg) = state.store.room_config(id).await {
                let _ = state
                    .manager
                    .send(id, blindall_game::RoomCommand::UpdateConfig { cfg })
                    .await;
            }
            Json(json!({"ok": true})).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn destroy(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = require_not_guest(&user) {
        return r;
    }
    let room = match load_room(&state, id).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    if !state
        .rooms
        .has_control(&room, user.id, user.is_administrator)
        .await
    {
        return forbidden();
    }
    if let Some(p) = room.photo_path.as_deref() {
        if !p.is_empty() {
            state.storage.delete(p).await;
        }
    }
    // Kill the actor while the room config still loads — a deleted room must
    // not keep timers/broadcasts alive (Shutdown was dead code before this).
    let _ = state.manager.send(id, RoomCommand::Shutdown).await;
    match state.rooms.soft_delete(id).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn restore(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = require_not_guest(&user) {
        return r;
    }
    let room = match load_room(&state, id).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    if !state
        .rooms
        .has_control(&room, user.id, user.is_administrator)
        .await
    {
        return forbidden();
    }
    match state.rooms.restore(id).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn mine(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(q): Query<MineParams>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    match state
        .rooms
        .moderated_rooms(user.id, q.search.as_deref(), q.trashed.as_deref())
        .await
    {
        Ok(rooms) => {
            let mut out = Vec::with_capacity(rooms.len());
            for (r, category_name, rounds_count) in &rooms {
                let mut v = room_json(r);
                let mods = state.rooms.moderators(r.id).await.unwrap_or_default();
                let pls = state.rooms.room_playlists(r.id).await.unwrap_or_default();
                v["moderators"] = json!(
                    mods.iter()
                        .map(|(id, n)| json!({"id": id, "name": n}))
                        .collect::<Vec<_>>()
                );
                v["playlists"] = json!(
                    pls.iter()
                        .map(|(id, n)| json!({"id": id, "name": n}))
                        .collect::<Vec<_>>()
                );
                v["rounds_count"] = json!(rounds_count);
                v["category"] = match (r.category_id, category_name) {
                    (Some(id), Some(name)) => json!({"id": id, "name": name}),
                    _ => Value::Null,
                };
                out.push(v);
            }
            Json(json!({"rooms": out})).into_response()
        }
        Err(e) => {
            tracing::warn!(error = %e, "rooms/mine failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct MineParams {
    search: Option<String>,
    trashed: Option<String>,
}

#[derive(Deserialize)]
pub struct PlaylistIdBody {
    playlist_id: i64,
}

pub async fn attach_playlist(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<PlaylistIdBody>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let room = match load_room(&state, id).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    if !(user.is_administrator || room.user_id == user.id) {
        return forbidden();
    }
    if !state
        .rooms
        .playlist_exists(body.playlist_id)
        .await
        .unwrap_or(false)
    {
        return validation("playlist_id", "The selected playlist id is invalid.");
    }
    match state.rooms.attach_playlist(id, body.playlist_id).await {
        Ok(created) => {
            if created {
                if let Ok(Some(playlist)) = state.playlists.find(body.playlist_id).await {
                    if playlist.user_id != user.id {
                        let _ = state
                            .notifications
                            .notify(
                                playlist.user_id,
                                "App\\Notifications\\PlaylistUsed",
                                json!({
                                    "room": {"id": room.id, "name": room.name, "slug": room.slug},
                                    "playlist": {"id": playlist.id, "name": playlist.name},
                                    "user": {"id": user.id, "name": user.name},
                                    "created_at": notif_timestamp(),
                                    "message": format!("{} utilise ta playlist {} dans la room {}", user.name, playlist.name, room.name),
                                }),
                            )
                            .await;
                    }
                }
            }
            Json(json!({"ok": true})).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn detach_playlist(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<PlaylistIdBody>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let room = match load_room(&state, id).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    if !(user.is_administrator || room.user_id == user.id) {
        return forbidden();
    }
    match state.rooms.detach_playlist(id, body.playlist_id).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct UserIdBody {
    user_id: i64,
}

pub async fn attach_moderator(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<UserIdBody>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let room = match load_room(&state, id).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    if !(user.is_administrator || room.user_id == user.id) {
        return forbidden();
    }
    if !state.rooms.user_exists(body.user_id).await.unwrap_or(false) {
        return validation("user_id", "The selected user id is invalid.");
    }
    match state.rooms.attach_moderator(id, body.user_id).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn detach_moderator(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<UserIdBody>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    if let Err(e) = load_room(&state, id).await {
        return e;
    }
    let allowed = user.is_administrator || state.rooms.is_moderator(id, user.id).await;
    if !allowed {
        return forbidden();
    }
    match state.rooms.detach_moderator(id, body.user_id).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn bookmark(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let room = match load_room(&state, id).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    match state.rooms.bookmark(user.id, id).await {
        Ok(created) => {
            if created && room.user_id != user.id {
                let _ = state
                    .notifications
                    .notify(
                        room.user_id,
                        "App\\Notifications\\RoomBookmarked",
                        json!({
                            "room": {"id": room.id, "name": room.name, "slug": room.slug},
                            "user": {"id": user.id, "name": user.name},
                            "created_at": notif_timestamp(),
                            "message": format!("{} a ajouté {} à ses favoris", user.name, room.name),
                        }),
                    )
                    .await;
            }
            Json(json!({"is_bookmarked": true})).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn unbookmark(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    match state.rooms.unbookmark(user.id, id).await {
        Ok(()) => Json(json!({"is_bookmarked": false})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn scores(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    OptionalUser(user): OptionalUser,
) -> Response {
    match state.rooms.leaderboards(id).await {
        Ok(boards) => {
            let user_snap = match user {
                Some(u) => state
                    .rooms
                    .user_snapshot(id, u.id, &boards)
                    .await
                    .unwrap_or(serde_json::Value::Null),
                None => serde_json::Value::Null,
            };
            let mut v = boards;
            v["user"] = user_snap;
            Json(v).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct UserCountBody {
    count: i64,
}

pub async fn update_user_count(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<UserCountBody>,
) -> Response {
    if let Some(r) = require_not_guest(&user) {
        return r;
    }
    // PHP throttle:10,1 on update-user-count.
    if let Some(secs) =
        state
            .presence_rate
            .retry_after(&format!("ucount|{id}|{}", user.id), 10, 60)
    {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({"error": "rate_limited", "retry_after": secs})),
        )
            .into_response();
    }
    match state.rooms.update_user_count(id, body.count).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn categories(State(state): State<AppState>) -> Response {
    match state.rooms.categories().await {
        Ok(rows) => Json(json!({
            "categories": rows.iter().map(|(id, n)| json!({"id": id, "name": n})).collect::<Vec<_>>()
        }))
        .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn category_landing(State(state): State<AppState>, Path(slug): Path<String>) -> Response {
    let Some((cat_id, cat_name, cat_slug)) =
        state.rooms.category_by_slug(&slug).await.ok().flatten()
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let rooms = state
        .rooms
        .public_for_category(cat_id)
        .await
        .unwrap_or_default();
    if rooms.is_empty() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let now = chrono::Utc::now().timestamp();
    let mut room_json = Vec::with_capacity(rooms.len());
    let mut names = Vec::with_capacity(rooms.len());
    for r in &rooms {
        names.push(r.name.clone());
        let members = state.presence.member_count(r.id, now).await.unwrap_or(0);
        room_json.push(json!({
            "id": r.id,
            "name": r.name,
            "slug": r.slug,
            "photo_path": r.photo_path,
            "is_playing": r.is_playing,
            "member_count": members,
            "tracks_by_round": r.tracks_by_round,
        }));
    }
    let count = rooms.len() as i64;
    let rooms_joined = names.join(", ");
    let c = crate::category_content::content(&cat_slug, &cat_name, count, &rooms_joined);
    Json(json!({
        "category": {"id": cat_id, "name": cat_name, "slug": cat_slug},
        "content": {
            "title": c.title,
            "meta_title": c.meta_title,
            "meta_description": c.meta_description,
            "heading": c.heading,
            "intro": c.intro,
            "intro_secondary": c.intro_secondary,
            "rooms_heading": c.rooms_heading,
            "faq_title": c.faq_title,
            "faq": c.faq.iter().map(|(q, a)| json!({"question": q, "answer": a})).collect::<Vec<_>>(),
        },
        "rooms": room_json,
        "rooms_count": count,
    }))
    .into_response()
}

// ---- track votes --------------------------------------------------------------

async fn broadcast_track_voted(state: &AppState, room_id: i64, track_id: i64) {
    let track = state
        .votes
        .payload(track_id, 0)
        .await
        .map(|v| {
            json!({
                "id": track_id,
                "upvotes": v["upvotes"],
                "downvotes": v["downvotes"],
            })
        })
        .unwrap_or_else(|_| json!({"id": track_id}));
    state
        .hub
        .broadcast(
            &format!("room:{room_id}"),
            "TrackVoted",
            json!({"track": track}),
            None,
        )
        .await;
}

pub async fn upvote_track(
    State(state): State<AppState>,
    Path((room_id, track_id)): Path<(i64, i64)>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    if let Err(e) = load_room(&state, room_id).await {
        return e;
    }
    if let Err(_) = state.votes.apply_upvote(user.id, track_id).await {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    state
        .levels
        .dispatch(user.id, blindall_db::LevelTrigger::LikesCount);
    broadcast_track_voted(&state, room_id, track_id).await;
    match state.votes.payload(track_id, user.id).await {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct DownvoteBody {
    reason: Option<String>,
}

const DOWNVOTE_REASONS: &[&str] = &[
    "sound_quality",
    "difficulty",
    "passage_choice",
    "personal_taste",
    "controversial_artist",
    "other",
];

pub async fn downvote_track(
    State(state): State<AppState>,
    Path((room_id, track_id)): Path<(i64, i64)>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<DownvoteBody>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    if let Err(e) = load_room(&state, room_id).await {
        return e;
    }
    let reason = body.reason.as_deref();
    if let Some(r) = reason {
        if !DOWNVOTE_REASONS.contains(&r) {
            return validation("reason", "The selected reason is invalid.");
        }
    }
    match state.votes.apply_downvote(user.id, track_id, reason).await {
        Ok(Ok(())) => {}
        Ok(Err(())) => return validation("reason", "A downvote reason is required."),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
    state
        .levels
        .dispatch(user.id, blindall_db::LevelTrigger::LikesCount);
    broadcast_track_voted(&state, room_id, track_id).await;
    match state.votes.payload(track_id, user.id).await {
        Ok(v) => Json(v).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ---- alerts / suggestions -----------------------------------------------------

fn notif_timestamp() -> String {
    chrono::Utc::now().format("%d/%m/%Y %H:%M").to_string()
}

#[derive(Deserialize)]
pub struct AlertBody {
    message: Option<String>,
}

pub async fn alert(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<AlertBody>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let room = match load_room(&state, id).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    let message = body.message.unwrap_or_default();
    if message.chars().count() > 255 {
        return validation("message", "The message must not exceed 255 characters.");
    }
    let moderators = state.auth.public_moderator_ids().await.unwrap_or_default();
    let text = if message.is_empty() {
        format!("{} reports a problem on the chat {}", user.name, room.name)
    } else {
        format!(
            "{} reports a problem on the chat {} : {}",
            user.name, room.name, message
        )
    };
    for mod_id in moderators {
        let _ = state
            .notifications
            .notify(
                mod_id,
                "App\\Notifications\\NewRoomAlert",
                json!({
                    "room": {"id": room.id, "name": room.name},
                    "created_at": notif_timestamp(),
                    "message": text,
                }),
            )
            .await;
    }
    Json(json!({"ok": true})).into_response()
}

#[derive(Deserialize)]
pub struct SuggestionBody {
    suggestion: String,
    #[serde(default)]
    track_id: Option<i64>,
}

pub async fn suggestion(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<SuggestionBody>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let room = match load_room(&state, id).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    if body.suggestion.is_empty() {
        return validation("suggestion", "The suggestion field is required.");
    }
    let veteran = user.is_administrator
        || state.auth.is_public_moderator(user.id).await
        || match state.auth.created_at_and_total_score(user.id).await {
            Ok(Some((Some(created), total))) => {
                created < chrono::Utc::now() - chrono::Duration::days(60) && total > 500.0
            }
            _ => false,
        };
    // Fresh accounts may still report broken data (they are the ones who find
    // it), but throttled so the notification queue cannot be flooded.
    if !veteran {
        if let Some(secs) =
            state
                .suggestion_rate
                .retry_after(&format!("sugg|{}", user.id), 10, 3600)
        {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({"error": "rate_limited", "retry_after": secs})),
            )
                .into_response();
        }
    }
    let room_playlists = state
        .rooms
        .room_playlists(room.id)
        .await
        .unwrap_or_default();
    let playlists: Vec<String> = room_playlists
        .iter()
        .map(|(_, name)| name.clone())
        .collect();
    let playlist_list: Vec<Value> = room_playlists
        .iter()
        .map(|(pid, name)| json!({"id": pid, "name": name}))
        .collect();
    let mut track_json = Value::Null;
    if let Some(tid) = body.track_id {
        let answers = state
            .playlists
            .track_answers_labeled(tid)
            .await
            .unwrap_or_default();
        if !answers.is_empty() {
            track_json = json!({"id": tid, "answers": answers});
        }
    }
    for (mod_id, _name) in state.rooms.moderators(room.id).await.unwrap_or_default() {
        let _ = state
            .notifications
            .notify(
                mod_id,
                "App\\Notifications\\NewSuggestion",
                json!({
                    "room": {
                        "id": room.id,
                        "name": room.name,
                        "playlists": playlists.join(","),
                        "playlist_list": playlist_list,
                    },
                    "user": {"id": user.id, "name": user.name},
                    "track": track_json,
                    "created_at": notif_timestamp(),
                    "message": body.suggestion,
                }),
            )
            .await;
    }
    Json(json!({"ok": true})).into_response()
}

// ---- room photo (`HasPicture`) + track search ---------------------------------

pub async fn update_photo(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    mut multipart: axum::extract::Multipart,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let room = match load_room(&state, id).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    if !state
        .rooms
        .has_control(&room, user.id, user.is_administrator)
        .await
    {
        return forbidden();
    }
    let can_picture = user.is_administrator
        || room.is_pro
        || room.is_public
        || state
            .auth
            .can_change_picture(user.id)
            .await
            .unwrap_or(false);
    if !can_picture {
        return forbidden();
    }
    let mut bytes = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("photo") {
            match field.bytes().await {
                Ok(b) if b.len() <= 10 * 1024 * 1024 => bytes = Some(b),
                _ => return validation("photo", "The photo may not be larger than 10 MB."),
            }
        }
    }
    let Some(bytes) = bytes else {
        return validation("photo", "The photo field is required.");
    };
    let webp = match crate::storage::image_to_webp_contain(&bytes, 350, 350) {
        Ok(w) => w,
        Err(()) => return validation("photo", "The photo must be an image."),
    };
    let path = format!("rooms/{}.webp", uuid::Uuid::now_v7().simple());
    if state.storage.put(&path, &webp).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    match state.rooms.set_photo(id, &path).await {
        Ok(prev) => {
            if let Some(p) = prev {
                state.storage.delete(&p).await;
            }
            Json(json!({"ok": true, "photo_path": path})).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn delete_picture(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let room = match load_room(&state, id).await {
        Ok(r) => r,
        Err(e) => return e,
    };
    if !state
        .rooms
        .has_control(&room, user.id, user.is_administrator)
        .await
    {
        return forbidden();
    }
    match state.rooms.clear_photo(id).await {
        Ok(prev) => {
            if let Some(p) = prev {
                state.storage.delete(&p).await;
            }
            Json(json!({"ok": true})).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct TrackSearchQuery {
    search: Option<String>,
}

pub async fn search_room_tracks(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<TrackSearchQuery>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = require_not_guest(&user) {
        return r;
    }
    if load_room(&state, id).await.is_err() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let term = q.search.unwrap_or_default();
    let ids = state
        .rooms
        .search_track_ids(id, &term)
        .await
        .unwrap_or_default();
    let mut out = Vec::with_capacity(ids.len());
    for tid in ids {
        // PHP renders `answer.type.name : answer.value` — keep labeled answers.
        let answers = state
            .playlists
            .track_answers_labeled(tid)
            .await
            .unwrap_or_default();
        out.push(json!({
            "id": tid,
            "answers": answers,
        }));
    }
    Json(json!(out)).into_response()
}

// ---- in-room track moderation ------------------------------------------------
//
// Scoped moderation for tracks as they appear in a room's playlist: admins and
// public moderators can fix answers (add/edit/delete) or remove a track that
// doesn't belong, straight from the room page. Everything is scoped to tracks
// actually attached to this room's playlists (track_in_room).

async fn require_track_mod(
    state: &AppState,
    user: &blindall_db::entities::User,
) -> Result<(), Response> {
    if user.is_administrator || state.auth.is_public_moderator(user.id).await {
        Ok(())
    } else {
        Err(forbidden())
    }
}

async fn track_in_room(state: &AppState, room_id: i64, track_id: i64) -> Result<(), Response> {
    match state.rooms.track_in_room(room_id, track_id).await {
        Ok(true) => Ok(()),
        Ok(false) => Err((
            StatusCode::NOT_FOUND,
            Json(json!({"error": "track_not_found"})),
        )
            .into_response()),
        Err(e) => {
            tracing::warn!(error = %e, "track_in_room failed");
            Err(StatusCode::INTERNAL_SERVER_ERROR.into_response())
        }
    }
}

async fn track_mod_gate(
    state: &AppState,
    user: &blindall_db::entities::User,
    room_id: i64,
    track_id: i64,
) -> Result<(), Response> {
    if let Some(r) = require_not_guest(user) {
        return Err(r);
    }
    require_track_mod(state, user).await?;
    track_in_room(state, room_id, track_id).await
}

/// GET /api/v1/rooms/{room}/tracks/{track}/answers — full answer rows incl. aliases
/// and type ids, so the moderator form can edit without losing data.
pub async fn mod_track_answers(
    State(state): State<AppState>,
    Path((room_id, track_id)): Path<(i64, i64)>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Err(r) = track_mod_gate(&state, &user, room_id, track_id).await {
        return r;
    }
    match state.playlists.track_answers(track_id).await {
        Ok(rows) => Json(json!({
            "answers": rows
                .iter()
                .map(|a| json!({
                    "id": a.id,
                    "answer_type_id": a.answer_type_id,
                    "value": a.value,
                    "score": a.score,
                    "aliases": a.aliases,
                }))
                .collect::<Vec<_>>()
        }))
        .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "mod_track_answers failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// POST /api/v1/rooms/{room}/tracks/{track}/answers
pub async fn mod_track_store_answer(
    State(state): State<AppState>,
    Path((room_id, track_id)): Path<(i64, i64)>,
    CurrentUser(user): CurrentUser,
    Json(b): Json<crate::playlists::AnswerBody>,
) -> Response {
    if let Err(r) = track_mod_gate(&state, &user, room_id, track_id).await {
        return r;
    }
    let known: Vec<i64> = state
        .playlists
        .answer_types()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    if !known.contains(&b.answer_type_id) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "invalid_answer_type"})),
        )
            .into_response();
    }
    let aliases = match crate::playlists::validate_answer(&b) {
        Ok(a) => a,
        Err(r) => return r,
    };
    match state
        .playlists
        .create_answer(
            track_id,
            b.answer_type_id,
            b.value.trim(),
            b.score.unwrap_or(0.5),
            aliases,
        )
        .await
    {
        Ok(id) => (StatusCode::CREATED, Json(json!({ "id": id }))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "mod_track_store_answer failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// PUT /api/v1/rooms/{room}/tracks/{track}/answers/{aid}
pub async fn mod_track_update_answer(
    State(state): State<AppState>,
    Path((room_id, track_id, answer_id)): Path<(i64, i64, i64)>,
    CurrentUser(user): CurrentUser,
    Json(b): Json<crate::playlists::AnswerBody>,
) -> Response {
    if let Err(r) = track_mod_gate(&state, &user, room_id, track_id).await {
        return r;
    }
    if !state.playlists.answer_belongs_to(track_id, answer_id).await {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "answer_not_found"})),
        )
            .into_response();
    }
    let known: Vec<i64> = state
        .playlists
        .answer_types()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    if !known.contains(&b.answer_type_id) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "invalid_answer_type"})),
        )
            .into_response();
    }
    let aliases = match crate::playlists::validate_answer(&b) {
        Ok(a) => a,
        Err(r) => return r,
    };
    match state
        .playlists
        .update_answer(
            answer_id,
            b.answer_type_id,
            b.value.trim(),
            b.score.unwrap_or(0.5),
            aliases,
        )
        .await
    {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "mod_track_update_answer failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// DELETE /api/v1/rooms/{room}/tracks/{track}/answers/{aid}
pub async fn mod_track_delete_answer(
    State(state): State<AppState>,
    Path((room_id, track_id, answer_id)): Path<(i64, i64, i64)>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Err(r) = track_mod_gate(&state, &user, room_id, track_id).await {
        return r;
    }
    if !state.playlists.answer_belongs_to(track_id, answer_id).await {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"error": "answer_not_found"})),
        )
            .into_response();
    }
    match state.playlists.delete_answer(answer_id).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "mod_track_delete_answer failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// DELETE /api/v1/rooms/{room}/tracks/{track} — removes the track from the room's
/// playlist entirely (with the standard moderator/Discord notifications).
pub async fn mod_track_destroy(
    State(state): State<AppState>,
    Path((room_id, track_id)): Path<(i64, i64)>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Err(r) = track_mod_gate(&state, &user, room_id, track_id).await {
        return r;
    }
    match state
        .store
        .delete_track(track_id, Some(user.name.clone()))
        .await
    {
        Ok(_) => Json(json!({ "ok": true })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "mod_track_destroy failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
