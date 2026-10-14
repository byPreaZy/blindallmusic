use crate::auth::CurrentUser;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use blindall_core::messages;
use blindall_core::scoring::CheckOutcome;
use blindall_game::{GameStore, RoomCommand};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct CheckBody {
    text: String,
    #[serde(default)]
    words: Vec<String>,
    #[serde(rename = "currentTime")]
    current_time: f64,
}

pub async fn check(
    State(state): State<AppState>,
    Path((round_id, track_id)): Path<(i64, i64)>,
    headers: axum::http::HeaderMap,
    user: CurrentUser,
    Json(body): Json<CheckBody>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if body.text.trim().is_empty() || body.text.len() > 255 {
        return Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "validation", "field": "text"})),
        ));
    }

    let round = state
        .store
        .round_row(round_id)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "server_error"))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "round_not_found"))?;
    if round.finished_at.is_some() {
        return Err(err(StatusCode::CONFLICT, "round_ended"));
    }
    if !participates(&state, round.room_id, user.id()).await {
        return Err(err(StatusCode::FORBIDDEN, "room_access_required"));
    }
    let idx = (round.current - 1).max(0) as usize;
    if round.tracks.get(idx).copied() != Some(track_id) {
        return Err(err(StatusCode::CONFLICT, "track_mismatch"));
    }

    let (reply, rx) = tokio::sync::oneshot::channel();
    state
        .manager
        .send(
            round.room_id,
            RoomCommand::CheckAnswer {
                user_id: user.id(),
                track_id,
                text: body.text.clone(),
                client_words: body.words.clone(),
                current_time: body.current_time,
                reply,
            },
        )
        .await
        .map_err(|_| err(StatusCode::SERVICE_UNAVAILABLE, "room_unavailable"))?;

    let outcome = rx
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "server_error"))?;

    let words = merged_words(&body);
    let locale = locale_of(&headers);
    let msg = |t: &'static str, body: &str| json!({"type": t, "body": body});

    Ok(Json(match outcome {
        CheckOutcome::InvalidTime => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "invalid_time",
                    "message": tr(locale, "Answer window closed"),
                })),
            ));
        }
        CheckOutcome::Hint(h) => json!({
            "message": msg("hint", &h),
            "words": body.words,
            "good_answers": [],
        }),
        CheckOutcome::NoHint => json!({
            "message": msg("bad", tr(locale, "No hint available")),
            "words": body.words,
            "good_answers": [],
        }),
        CheckOutcome::Good {
            answers,
            total_score,
            found_all: _,
        } => {
            let good: Vec<Value> = answers
                .iter()
                .map(|a| {
                    json!({
                        "id": a.answer_id,
                        "order": a.order,
                        "speedBonus": a.speed_bonus,
                        "name": a.type_name,
                        "type": {"name": a.type_name, "svg_icon": a.svg_icon},
                        "value": a.value,
                        "score": a.score,
                    })
                })
                .collect();
            json!({
                "words": words,
                "good_answers": good,
                "total_score": total_score,
                "message": pick_message("good", locale),
            })
        }
        CheckOutcome::AlreadyFound => json!({
            "words": words,
            "good_answers": [],
            "message": msg("good", "You have already found these answers"),
        }),
        CheckOutcome::Almost => json!({
            "words": words,
            "good_answers": [],
            "message": pick_message("almost", locale),
        }),
        CheckOutcome::Bad => json!({
            "words": words,
            "good_answers": [],
            "message": pick_message("bad", locale),
        }),
    }))
}

pub async fn track_listened(
    State(state): State<AppState>,
    Path((round_id, track_id)): Path<(i64, i64)>,
    user: CurrentUser,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let round = state
        .store
        .round_row(round_id)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "server_error"))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "round_not_found"))?;
    if !participates(&state, round.room_id, user.id()).await {
        return Err(err(StatusCode::FORBIDDEN, "room_access_required"));
    }
    if !round.tracks.contains(&track_id) {
        return Err(err(StatusCode::BAD_REQUEST, "track_not_in_round"));
    }
    if round.finished_at.is_some() {
        return Err(err(StatusCode::BAD_REQUEST, "round_finished"));
    }
    state
        .manager
        .send(
            round.room_id,
            RoomCommand::TrackListened {
                user_id: user.id(),
                track_id,
            },
        )
        .await
        .map_err(|_| err(StatusCode::SERVICE_UNAVAILABLE, "room_unavailable"))?;
    Ok(Json(json!({"success": true})))
}

pub async fn scores(
    State(state): State<AppState>,
    Path(round_id): Path<i64>,
    user: CurrentUser,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let round = state
        .store
        .round_row(round_id)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "server_error"))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "round_not_found"))?;
    if !participates(&state, round.room_id, user.id()).await {
        return Err(err(StatusCode::FORBIDDEN, "room_access_required"));
    }
    let scores = state.scores.all_scores(round_id).await.unwrap_or_default();
    Ok(Json(json!({
        "scores": scores
            .into_iter()
            .map(|(uid, s)| json!({"user_id": uid, "score": s}))
            .collect::<Vec<_>>(),
    })))
}

async fn participates(state: &AppState, room_id: i64, user_id: i64) -> bool {
    state
        .presence
        .has_participation(room_id, user_id)
        .await
        .unwrap_or(false)
}

pub async fn start(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    user: CurrentUser,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if user.0.is_guest {
        return Err((StatusCode::FORBIDDEN, Json(json!({"message": "guest_forbidden"}))));
    }

    state
        .manager
        .send(id, RoomCommand::Start {
            user_id: Some(user.id()),
        })
        .await
        .map_err(|_| err(StatusCode::SERVICE_UNAVAILABLE, "room_unavailable"))?;
    Ok(Json(json!({"success": true})))
}

async fn require_room_control(
    state: &AppState,
    room_id: i64,
    user: &blindall_db::entities::User,
) -> Result<(), (StatusCode, Json<Value>)> {
    let room = state
        .rooms
        .find(room_id)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db"))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "room_not_found"))?;
    if !state
        .rooms
        .has_control(&room, user.id, user.is_administrator)
        .await
    {
        return Err(err(StatusCode::FORBIDDEN, "unauthorized"));
    }
    Ok(())
}

pub async fn pause(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if user.is_guest {
        return Err((StatusCode::FORBIDDEN, Json(json!({"message": "guest_forbidden"}))));
    }

    require_room_control(&state, id, &user).await?;
    state
        .manager
        .send(id, RoomCommand::Pause)
        .await
        .map_err(|_| err(StatusCode::SERVICE_UNAVAILABLE, "room_unavailable"))?;
    Ok(Json(json!({"success": true})))
}

pub async fn resume(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if user.is_guest {
        return Err((StatusCode::FORBIDDEN, Json(json!({"message": "guest_forbidden"}))));
    }

    require_room_control(&state, id, &user).await?;
    state
        .manager
        .send(id, RoomCommand::Resume)
        .await
        .map_err(|_| err(StatusCode::SERVICE_UNAVAILABLE, "room_unavailable"))?;
    Ok(Json(json!({"success": true})))
}

pub async fn stop(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if user.is_guest {
        return Err((StatusCode::FORBIDDEN, Json(json!({"message": "guest_forbidden"}))));
    }

    require_room_control(&state, id, &user).await?;
    state
        .manager
        .send(id, RoomCommand::Stop)
        .await
        .map_err(|_| err(StatusCode::SERVICE_UNAVAILABLE, "room_unavailable"))?;
    Ok(Json(json!({"success": true})))
}

fn err(code: StatusCode, error: &'static str) -> (StatusCode, Json<Value>) {
    (code, Json(json!({"error": error})))
}

fn merged_words(body: &CheckBody) -> Vec<String> {
    let sanitized = blindall_core::sanitize::sanitize_string(&body.text);
    let mut words: Vec<String> = sanitized
        .split(' ')
        .filter(|w| !w.trim().is_empty())
        .map(str::to_string)
        .collect();
    for w in &body.words {
        let t = w.trim();
        if !t.is_empty() && !words.iter().any(|u| u == t) {
            words.push(t.to_string());
        }
    }
    words.sort();
    words.dedup();
    words
}

fn locale_of(headers: &axum::http::HeaderMap) -> &str {
    match headers
        .get("x-locale")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
    {
        Some(l @ ("fr" | "en" | "es")) => l,
        _ => "fr",
    }
}

fn tr<'a>(locale: &str, key: &'a str) -> &'a str {
    match (locale, key) {
        ("fr", "Answer window closed") => "Fenêtre de réponse fermée",
        ("fr", "No hint available") => "Aucun indice disponible",
        ("es", "No hint available") => "No hay pista disponible",
        _ => key,
    }
}

fn pick_message(kind: &'static str, locale: &str) -> Value {
    let idx = (rand::random::<u32>() as usize) % 1024;
    json!({
        "type": kind,
        "body": messages::random_message(kind, locale, idx),
    })
}
