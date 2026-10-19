use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};
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
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }
    Json(json!({
        "games": games_list(&state, Some(user.id)).await
    }))
    .into_response()
}

pub async fn games_list(state: &AppState, user_id: Option<i64>) -> Vec<Value> {
    let totals = match user_id {
        Some(id) => state
            .minigames
            .totals_for_user(id)
            .await
            .unwrap_or_default(),
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
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }
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
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }
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
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }
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
                raw.as_deref()
                    .is_some_and(|t| t.trim() == body.chosen_value.trim())
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
            (
                is_correct,
                raw.map(|v| {
                    if game == TYPE_QUIZ {
                        v
                    } else {
                        blindall_core::sanitize::sanitize_string(&v)
                    }
                }),
                json!({"track_id": track_id}),
            )
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

// ---- duels & daily challenge (async multiplayer) ----------------------------

const DEFAULT_QUESTIONS: usize = 10;
const MAX_QUESTIONS: usize = 20;

pub(crate) const VALID_TYPES: [&str; 5] = [
    TYPE_QUIZ,
    TYPE_WHO_SANG,
    TYPE_ALBUM_COVER,
    TYPE_FIRST_LETTER,
    TYPE_ANAGRAM,
];

/// Same validation rules as the solo `check`, applied to the frozen
/// `correct_value` stored in the duel/daily question.
pub(crate) fn question_correct(game_type: &str, correct_value: &str, given: &str) -> bool {
    match game_type {
        TYPE_QUIZ | TYPE_WHO_SANG | TYPE_ALBUM_COVER => correct_value.trim() == given.trim(),
        TYPE_FIRST_LETTER => {
            let g = blindall_core::sanitize::sanitize_string(given.trim());
            let c = blindall_core::sanitize::sanitize_string(correct_value.trim());
            !c.is_empty()
                && !g.is_empty()
                && blindall_core::sanitize::str_slug(&g, '-')
                    == blindall_core::sanitize::str_slug(&c, '-')
        }
        TYPE_ANAGRAM => correct_value.trim().eq_ignore_ascii_case(given.trim()),
        _ => false,
    }
}

/// Strips `correct_value` (server-side only) and adds the 0-based `idx`
/// used by the answer endpoints.
pub(crate) fn strip_questions(questions: &[Value]) -> Vec<Value> {
    questions
        .iter()
        .enumerate()
        .map(|(i, q)| {
            let mut q = q.clone();
            if let Some(o) = q.as_object_mut() {
                // first_letter needs its hint but not the raw answer: the
                // server computes it before stripping correct_value.
                let free_text = !o.contains_key("choices") && !o.contains_key("scrambled_artist");
                if free_text {
                    if let Some(cv) = o.get("correct_value").and_then(|v| v.as_str()) {
                        let hint = cv
                            .trim()
                            .split_whitespace()
                            .map(|w| {
                                let mut ch = w.chars();
                                let first = ch.next().map(|c| c.to_string()).unwrap_or_default();
                                format!("{}{}", first, "_".repeat(ch.count()))
                            })
                            .collect::<Vec<_>>()
                            .join(" ");
                        o.insert("hint".into(), Value::String(hint));
                    }
                }
                o.remove("correct_value");
            }
            json!({"idx": i, "question": q})
        })
        .collect()
}

fn duel_err(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({"error": msg}))).into_response()
}

fn notif_ts() -> String {
    chrono::Utc::now().format("%d/%m/%Y %H:%M").to_string()
}

fn duel_row_json(row: &Value, viewer_id: i64, with_questions: bool) -> Value {
    let mut out = row.clone();
    let creator_id = row["creator_id"].as_i64().unwrap_or_default();
    let my_answers = if viewer_id == creator_id {
        row["creator_answers"].clone()
    } else {
        row["opponent_answers"].clone()
    };
    out["my_answers"] = my_answers;
    out["question_count"] = json!(row["questions"].as_array().map(|q| q.len()).unwrap_or(0));
    // Answer lists are internal state — only the viewer's own list is exposed.
    out.as_object_mut().unwrap().remove("creator_answers");
    out.as_object_mut().unwrap().remove("opponent_answers");
    if with_questions {
        out["questions"] = json!(strip_questions(
            row["questions"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or(&[])
        ));
    } else {
        out.as_object_mut().unwrap().remove("questions");
    }
    out
}

pub async fn duels_index(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }
    match state.minigames.duels_for(user.id).await {
        Ok(d) => Json(json!({"duels": d})).into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "duels index failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn duels_leaderboard(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }
    match state.minigames.duel_leaderboard(20).await {
        Ok(rows) => Json(json!({"leaderboard": rows})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct CreateDuelBody {
    game_type: String,
    opponent_id: Option<i64>,
    count: Option<usize>,
}

pub async fn duel_create(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<CreateDuelBody>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }
    let game_type = body.game_type.replace('-', "_");
    if !VALID_TYPES.contains(&game_type.as_str()) {
        return duel_err(StatusCode::UNPROCESSABLE_ENTITY, "Invalid game type.");
    }
    if body.opponent_id == Some(user.id) {
        return duel_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Cannot challenge yourself.",
        );
    }
    if let Some(opp) = body.opponent_id {
        let challengeable = state.minigames.challengeable(opp).await.unwrap_or(false)
            && !state.moderation.is_banned(opp).await.unwrap_or(false);
        if !challengeable {
            return duel_err(StatusCode::UNPROCESSABLE_ENTITY, "Invalid opponent.");
        }
    }
    if state
        .minigames
        .pending_duel_count(user.id)
        .await
        .unwrap_or(0)
        >= 25
    {
        return duel_err(StatusCode::TOO_MANY_REQUESTS, "Too many pending duels.");
    }
    let n = body
        .count
        .unwrap_or(DEFAULT_QUESTIONS)
        .clamp(3, MAX_QUESTIONS);
    let questions = state.minigames.gen_question_set(&game_type, n).await;
    if questions.len() < 3 {
        return duel_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Not enough questions available.",
        );
    }
    match state
        .minigames
        .create_duel(user.id, body.opponent_id, &game_type, &json!(questions))
        .await
    {
        Ok(id) => {
            if let Some(opp) = body.opponent_id {
                let _ = state
                    .notifications
                    .notify(
                        opp,
                        "App\\Notifications\\DuelChallenge",
                        json!({
                            "duel": {"id": id, "game_type": game_type, "question_count": questions.len()},
                            "user": {"id": user.id, "name": user.name},
                            "created_at": notif_ts(),
                            "message": format!("{} te défie sur un duel {} !", user.name, game_type),
                        }),
                    )
                    .await;
            }
            Json(json!({"id": id, "question_count": questions.len()})).into_response()
        }
        Err(e) => {
            tracing::warn!(error = %e, "duel create failed");
            duel_err(StatusCode::UNPROCESSABLE_ENTITY, "Invalid opponent.")
        }
    }
}

pub async fn duel_show(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }
    match state.minigames.duel(id).await {
        Ok(Some(row)) => {
            let creator_id = row["creator_id"].as_i64().unwrap_or_default();
            let opponent_id = row["opponent_id"].as_i64();
            let participant = user.id == creator_id || opponent_id == Some(user.id);
            if !participant {
                return duel_err(StatusCode::FORBIDDEN, "Not a participant.");
            }
            Json(duel_row_json(&row, user.id, true)).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn duel_accept(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }
    match state.minigames.accept_duel(id, user.id).await {
        Ok(0) => duel_err(StatusCode::UNPROCESSABLE_ENTITY, "Duel cannot be accepted."),
        Ok(_) => {
            if let Ok(Some(row)) = state.minigames.duel(id).await {
                let creator_id = row["creator_id"].as_i64().unwrap_or_default();
                let _ = state
                    .notifications
                    .notify(
                        creator_id,
                        "App\\Notifications\\DuelAccepted",
                        json!({
                            "duel": {"id": id},
                            "user": {"id": user.id, "name": user.name},
                            "created_at": notif_ts(),
                            "message": format!("{} a accepté ton duel !", user.name),
                        }),
                    )
                    .await;
            }
            Json(json!({"accepted": true})).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct DuelAnswerBody {
    idx: usize,
    value: String,
}

pub async fn duel_answer(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<DuelAnswerBody>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }
    let Some(row) = state.minigames.duel(id).await.ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let game_type = row["game_type"].as_str().unwrap_or_default().to_string();
    let creator_id = row["creator_id"].as_i64().unwrap_or_default();
    let opponent_id = row["opponent_id"].as_i64();
    let side = if user.id == creator_id {
        "creator"
    } else if opponent_id == Some(user.id) {
        "opponent"
    } else {
        return duel_err(StatusCode::FORBIDDEN, "Not a participant.");
    };
    let Some(q) = row["questions"].as_array().and_then(|a| a.get(body.idx)) else {
        return duel_err(StatusCode::UNPROCESSABLE_ENTITY, "Invalid question index.");
    };
    let Some(correct_value) = q["correct_value"].as_str() else {
        return duel_err(StatusCode::UNPROCESSABLE_ENTITY, "Invalid question.");
    };
    let correct = question_correct(&game_type, correct_value, &body.value);
    match state
        .minigames
        .duel_record(
            id,
            side,
            user.id,
            body.idx as i64,
            correct,
            body.value.trim(),
        )
        .await
    {
        Ok(Some(updated)) => {
            if correct {
                let _ = state
                    .minigames
                    .record_score(user.id, &game_type, POINTS_CORRECT, json!({"duel_id": id}))
                    .await;
                state
                    .levels
                    .dispatch(user.id, blindall_db::LevelTrigger::Score);
            }
            if updated["status"].as_str() == Some("finished") {
                let other = if side == "creator" {
                    opponent_id
                } else {
                    Some(creator_id)
                };
                if let Some(oid) = other {
                    let (a, b) = if side == "creator" {
                        ("creator_score", "opponent_score")
                    } else {
                        ("opponent_score", "creator_score")
                    };
                    let my_score = updated[a].as_i64().unwrap_or_default();
                    let their_score = updated[b].as_i64().unwrap_or_default();
                    let result = match my_score.cmp(&their_score) {
                        std::cmp::Ordering::Greater => "victoire",
                        std::cmp::Ordering::Less => "défaite",
                        std::cmp::Ordering::Equal => "égalité",
                    };
                    let _ = state
                        .notifications
                        .notify(
                            oid,
                            "App\\Notifications\\DuelFinished",
                            json!({
                                "duel": {"id": id},
                                "user": {"id": user.id, "name": user.name},
                                "created_at": notif_ts(),
                                "message": format!("Duel terminé contre {} : {} ({}-{})",
                                    user.name, result, their_score, my_score),
                            }),
                        )
                        .await;
                }
            }
            Json(json!({
                "correct": correct,
                "correct_value": correct_value,
                "creator_score": updated["creator_score"],
                "opponent_score": updated["opponent_score"],
                "creator_done": updated["creator_done"],
                "opponent_done": updated["opponent_done"],
                "status": updated["status"],
            }))
            .into_response()
        }
        Ok(None) => duel_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Question already answered.",
        ),
        Err(e) => {
            tracing::warn!(error = %e, "duel answer failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// ---- daily -------------------------------------------------------------------

#[derive(Deserialize)]
pub struct DailyQuery {
    #[serde(rename = "type")]
    game_type: Option<String>,
}

pub async fn daily_show(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    axum::extract::Query(q): axum::extract::Query<DailyQuery>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }
    let game_type = q
        .game_type
        .unwrap_or_else(|| TYPE_QUIZ.to_string())
        .replace('-', "_");
    if !VALID_TYPES.contains(&game_type.as_str()) {
        return duel_err(StatusCode::UNPROCESSABLE_ENTITY, "Invalid game type.");
    }
    let today = chrono::Utc::now().date_naive();
    let questions = match state
        .minigames
        .daily_questions(today, &game_type, DAILY_QUESTIONS_CONST)
        .await
    {
        Ok(q) => q,
        Err(e) => {
            tracing::warn!(error = %e, "daily questions failed");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let (score, answered, my_answers) = state
        .minigames
        .daily_progress(user.id, today, &game_type)
        .await
        .ok()
        .flatten()
        .unwrap_or((0, 0, json!([])));
    // A day counts toward the streak once every question in *that day's* set
    // is answered — the set can be smaller than the nominal size.
    let streak = state
        .minigames
        .daily_streak(user.id, &game_type, questions.len() as i64, today)
        .await
        .unwrap_or(0);
    let leaderboard = state
        .minigames
        .daily_leaderboard(today, &game_type, 20)
        .await
        .unwrap_or_default();
    Json(json!({
        "date": today.to_string(),
        "game_type": game_type,
        "total": questions.len(),
        "score": score,
        "answered": answered,
        "my_answers": my_answers,
        "streak": streak,
        "questions": strip_questions(&questions),
        "leaderboard": leaderboard,
    }))
    .into_response()
}

const DAILY_QUESTIONS_CONST: usize = 10;

#[derive(Deserialize)]
pub struct DailyAnswerBody {
    game_type: String,
    idx: usize,
    value: String,
}

pub async fn daily_answer(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<DailyAnswerBody>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }
    let game_type = body.game_type.replace('-', "_");
    if !VALID_TYPES.contains(&game_type.as_str()) {
        return duel_err(StatusCode::UNPROCESSABLE_ENTITY, "Invalid game type.");
    }
    let today = chrono::Utc::now().date_naive();
    let questions = match state
        .minigames
        .daily_questions(today, &game_type, DAILY_QUESTIONS_CONST)
        .await
    {
        Ok(q) => q,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let Some(q) = questions.get(body.idx) else {
        return duel_err(StatusCode::UNPROCESSABLE_ENTITY, "Invalid question index.");
    };
    let Some(correct_value) = q["correct_value"].as_str() else {
        return duel_err(StatusCode::UNPROCESSABLE_ENTITY, "Invalid question.");
    };
    let correct = question_correct(&game_type, correct_value, &body.value);
    match state
        .minigames
        .daily_record(
            user.id,
            today,
            &game_type,
            body.idx as i64,
            correct,
            body.value.trim(),
        )
        .await
    {
        Ok(Some((score, answered))) => {
            if correct {
                let _ = state
                    .minigames
                    .record_score(
                        user.id,
                        &game_type,
                        POINTS_CORRECT,
                        json!({"daily": today.to_string()}),
                    )
                    .await;
                state
                    .levels
                    .dispatch(user.id, blindall_db::LevelTrigger::Score);
            }
            Json(json!({
                "correct": correct,
                "correct_value": correct_value,
                "score": score,
                "answered": answered,
                "total": questions.len(),
            }))
            .into_response()
        }
        Ok(None) => duel_err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Question already answered.",
        ),
        Err(e) => {
            tracing::warn!(error = %e, "daily answer failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
