use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::auth::CurrentUser;
use crate::state::AppState;

const POINTS_CORRECT: i64 = 1;
const ANAGRAM_TTL: Duration = Duration::from_secs(300);

pub const TYPE_QUIZ: &str = "quiz";
pub const TYPE_WHO_SANG: &str = "who_sang";
pub const TYPE_ALBUM_COVER: &str = "album_cover";
pub const TYPE_FIRST_LETTER: &str = "first_letter";
pub const TYPE_ANAGRAM: &str = "anagram";

#[derive(Default)]
pub struct AnagramQuestions {
    map: Mutex<HashMap<String, (String, Instant)>>,
}

impl AnagramQuestions {
    pub fn put(&self, artist: String) -> String {
        let id = uuid::Uuid::now_v7().to_string();
        let mut m = self.map.lock().unwrap();
        if m.len() > 50_000 {
            let now = Instant::now();
            m.retain(|_, (_, t)| now.duration_since(*t) < ANAGRAM_TTL);
        }
        m.insert(id.clone(), (artist, Instant::now()));
        id
    }

    pub fn pull(&self, id: &str) -> Option<String> {
        let mut m = self.map.lock().unwrap();
        let (artist, t) = m.remove(id)?;
        (Instant::now().duration_since(t) < ANAGRAM_TTL).then_some(artist)
    }
}

fn no_track() -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"error": "No track available for this minigame."})),
    )
        .into_response()
}

fn expired_question() -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"error": "Question expired or already answered."})),
    )
        .into_response()
}

// ---- index -----------------------------------------------------------------

pub async fn index(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> Response {
    Json(json!({
        "games": games_list(&state, Some(user.id)).await
    }))
    .into_response()
}

pub async fn games_list(state: &AppState, user_id: Option<i64>) -> Vec<Value> {
    let totals = match user_id {
        Some(id) => state.minigames.totals_for_user(id).await.unwrap_or_default(),
        None => vec![],
    };
    let score = |t: &str| {
        totals
            .iter()
            .find(|(g, _)| g == t)
            .map(|(_, s)| *s)
            .unwrap_or(0)
    };
    vec![
        json!({"type": TYPE_QUIZ, "score": score(TYPE_QUIZ)}),
        json!({"type": TYPE_WHO_SANG, "score": score(TYPE_WHO_SANG)}),
        json!({"type": TYPE_ANAGRAM, "score": score(TYPE_ANAGRAM)}),
        json!({"type": TYPE_FIRST_LETTER, "score": score(TYPE_FIRST_LETTER)}),
        json!({"type": TYPE_ALBUM_COVER, "score": score(TYPE_ALBUM_COVER)}),
    ]
}

pub async fn leaderboard(
    State(state): State<AppState>,
    Path(game_type): Path<String>,
    CurrentUser(_user): CurrentUser,
) -> Response {
    let game_type = game_type.replace('-', "_");
    match state.minigames.leaderboard(&game_type, 10).await {
        Ok(rows) => Json(json!({"leaderboard": rows})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ---- next ------------------------------------------------------------------

pub async fn next(
    State(state): State<AppState>,
    Path(game): Path<String>,
    CurrentUser(_user): CurrentUser,
) -> Response {
    let game = game.replace('-', "_");
    match game.as_str() {
        TYPE_QUIZ | TYPE_WHO_SANG | TYPE_ALBUM_COVER => {
            match state.minigames.track_with_choices(&game).await {
                Ok(Some(q)) => Json(json!({
                    "track": q.track,
                    "choices": q.choices,
                    "correct_value": q.correct_value, // PHP sends it too.
                }))
                .into_response(),
                Ok(None) => no_track(),
                Err(e) => {
                    tracing::warn!(error = %e, "minigame next failed");
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        }
        TYPE_FIRST_LETTER => match state.minigames.track_with_title().await {
            Ok(Some(q)) => Json(json!({
                "track": q.track,
                "correct_value": q.correct_value,
            }))
            .into_response(),
            Ok(None) => no_track(),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        TYPE_ANAGRAM => match state.minigames.random_anagram() {
            Some((scrambled, artist)) => {
                let qid = state.anagram_questions.put(artist);
                Json(json!({"question_id": qid, "scrambled_artist": scrambled})).into_response()
            }
            None => no_track(),
        },
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

// ---- check -----------------------------------------------------------------

#[derive(Deserialize)]
pub struct CheckBody {
    track_id: Option<i64>,
    question_id: Option<String>,
    chosen_value: String,
}

pub async fn check(
    State(state): State<AppState>,
    Path(game): Path<String>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<CheckBody>,
) -> Response {
    let game = game.replace('-', "_");
    let (correct, correct_value, meta): (bool, Option<String>, Value) = match game.as_str() {
        TYPE_QUIZ | TYPE_FIRST_LETTER => {
            let Some(track_id) = body.track_id else {
                return err_track_id();
            };
            let answer_type = blindall_db::minigames::ANSWER_TYPE_TITLE;
            let raw = state
                .minigames
                .answer_of(track_id, answer_type)
                .await
                .ok()
                .flatten();
            let is_correct = if game == TYPE_QUIZ {
                raw.as_deref().is_some_and(|t| t.trim() == body.chosen_value.trim())
            } else {
                let given = blindall_core::sanitize::sanitize_string(body.chosen_value.trim());
                let correct = raw.as_deref().map(blindall_core::sanitize::sanitize_string);
                correct.as_deref().is_some_and(|c| {
                    !c.is_empty()
                        && !given.is_empty()
                        && blindall_core::sanitize::str_slug(&given, '-')
                            == blindall_core::sanitize::str_slug(c, '-')
                })
            };
            (is_correct, raw.map(|v| if game == TYPE_QUIZ { v } else { blindall_core::sanitize::sanitize_string(&v) }), json!({"track_id": track_id}))
        }
        TYPE_WHO_SANG | TYPE_ALBUM_COVER => {
            let Some(track_id) = body.track_id else {
                return err_track_id();
            };
            let raw = state
                .minigames
                .answer_of(track_id, blindall_db::minigames::ANSWER_TYPE_ARTIST)
                .await
                .ok()
                .flatten();
            let is_correct = raw
                .as_deref()
                .is_some_and(|a| a.trim() == body.chosen_value.trim());
            (is_correct, raw, json!({"track_id": track_id}))
        }
        TYPE_ANAGRAM => {
            let Some(qid) = body.question_id.as_deref() else {
                return err_track_id();
            };
            let Some(artist) = state.anagram_questions.pull(qid) else {
                return expired_question();
            };
            let is_correct = body.chosen_value.trim().eq_ignore_ascii_case(artist.trim());
            (is_correct, Some(artist), json!({}))
        }
        _ => return StatusCode::NOT_FOUND.into_response(),
    };

    if correct {
        let _ = state
            .minigames
            .record_score(user.id, &game, POINTS_CORRECT, meta)
            .await;
        state
            .levels
            .dispatch(user.id, blindall_db::LevelTrigger::Score);
    }
    Json(json!({
        "correct": correct,
        "points": if correct { POINTS_CORRECT } else { 0 },
        "correct_value": correct_value,
    }))
    .into_response()
}

fn err_track_id() -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"error": "track_id or question_id required"})),
    )
        .into_response()
}
