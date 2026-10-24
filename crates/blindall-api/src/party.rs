//! Realtime "party" quiz inside a room: the server drives a shared
//! question sequence on the room channel — every participant sees the same
//! question at the same time and scores race live. In-memory per process
//! (a party is ephemeral by design; nothing needs to survive a restart).

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::Duration;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::auth::CurrentUser;
use crate::minigames::{question_correct, strip_questions};
use crate::state::AppState;

const QUESTION_SECS: u64 = 15;
const REVEAL_SECS: u64 = 4;
const PARTY_QUESTIONS: usize = 10;

pub struct Party {
    pub game_type: String,
    pub questions: Vec<Value>, // correct_value kept server-side
    pub idx: i64,
    pub phase: &'static str, // "question" | "reveal" | "finished"
    pub ends_at: i64,
    pub scores: HashMap<i64, i64>,
    pub answered: HashMap<i64, HashSet<i64>>,
    pub handle: Option<tokio::task::JoinHandle<()>>,
}

#[derive(Default)]
pub struct PartyStore {
    map: Mutex<HashMap<i64, Party>>,
}

impl PartyStore {
    fn snapshot(&self, room_id: i64, viewer_id: i64) -> Option<Value> {
        let map = self.map.lock().unwrap();
        let p = map.get(&room_id)?;
        Some(party_json(room_id, p, viewer_id))
    }

    fn finish(&self, room_id: i64) {
        let mut map = self.map.lock().unwrap();
        if let Some(p) = map.get_mut(&room_id) {
            p.phase = "finished";
        }
    }

    fn remove(&self, room_id: i64) -> Option<Party> {
        self.map.lock().unwrap().remove(&room_id)
    }
}

fn party_json(room_id: i64, p: &Party, viewer_id: i64) -> Value {
    let current = if p.phase == "question" && p.idx >= 0 {
        p.questions.get(p.idx as usize).map(|q| {
            strip_questions(std::slice::from_ref(q))
                .into_iter()
                .next()
                .unwrap()
        })
    } else {
        None
    };
    let reveal_value = if p.phase == "reveal" && p.idx >= 0 {
        p.questions
            .get(p.idx as usize)
            .and_then(|q| q["correct_value"].as_str())
            .map(|s| s.to_string())
    } else {
        None
    };
    json!({
        "room_id": room_id,
        "game_type": p.game_type,
        "phase": p.phase,
        "idx": p.idx,
        "total": p.questions.len(),
        "ends_at": p.ends_at,
        "question": current,
        "reveal_value": reveal_value,
        "scores": p.scores.iter().map(|(u, s)| json!({"user_id": u, "score": s}))
            .collect::<Vec<_>>(),
        "my_answered": p.answered.get(&viewer_id)
            .map(|s| s.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default(),
    })
}

fn channel(room_id: i64) -> String {
    blindall_game::events::room_channel(room_id)
}

fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({"error": msg}))).into_response()
}

async fn require_participant(state: &AppState, room_id: i64, user_id: i64) -> bool {
    state
        .presence
        .has_participation(room_id, user_id)
        .await
        .unwrap_or(false)
}

async fn can_host(state: &AppState, room_id: i64, user: &blindall_db::entities::User) -> bool {
    if user.is_guest {
        return false;
    }
    if user.is_administrator {
        return true;
    }
    if let Ok(Some((owner, _, _))) = state.rooms.owner(room_id).await {
        if owner == user.id {
            return true;
        }
    }
    state
        .rooms
        .moderators(room_id)
        .await
        .unwrap_or_default()
        .iter()
        .any(|(id, _)| *id == user.id)
}

/// Server-side driver: emits PartyQuestion → PartyReveal per question on the
/// room channel, then PartyFinished. Mirrors the authoritative-timing rule
/// used for rounds — clients only display.
fn spawn_driver(state: AppState, room_id: i64) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let total = {
            let parties = &state.parties;
            parties
                .map
                .lock()
                .unwrap()
                .get(&room_id)
                .map(|p| p.questions.len() as i64)
                .unwrap_or(0)
        };
        for i in 0..total {
            // question phase
            let payload = {
                let mut map = state.parties.map.lock().unwrap();
                let Some(p) = map.get_mut(&room_id) else {
                    return;
                };
                p.idx = i;
                p.phase = "question";
                p.ends_at = chrono::Utc::now().timestamp() + QUESTION_SECS as i64;
                party_json(room_id, p, 0)
            };
            state
                .hub
                .broadcast(&channel(room_id), "PartyQuestion", payload, None)
                .await;
            tokio::time::sleep(Duration::from_secs(QUESTION_SECS)).await;
            // reveal phase
            let payload = {
                let mut map = state.parties.map.lock().unwrap();
                let Some(p) = map.get_mut(&room_id) else {
                    return;
                };
                p.phase = "reveal";
                p.ends_at = chrono::Utc::now().timestamp() + REVEAL_SECS as i64;
                party_json(room_id, p, 0)
            };
            state
                .hub
                .broadcast(&channel(room_id), "PartyReveal", payload, None)
                .await;
            tokio::time::sleep(Duration::from_secs(REVEAL_SECS)).await;
        }
        state.parties.finish(room_id);
        let payload = {
            let map = state.parties.map.lock().unwrap();
            map.get(&room_id).map(|p| party_json(room_id, p, 0))
        };
        if let Some(payload) = payload {
            state
                .hub
                .broadcast(&channel(room_id), "PartyFinished", payload, None)
                .await;
        }
    })
}

#[derive(Deserialize)]
pub struct StartBody {
    game_type: Option<String>,
    count: Option<usize>,
}

pub async fn start(
    State(state): State<AppState>,
    Path(room_id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<StartBody>,
) -> Response {
    if !require_participant(&state, room_id, user.id).await {
        return err(StatusCode::FORBIDDEN, "Join the room first.");
    }
    if !can_host(&state, room_id, &user).await {
        return err(
            StatusCode::FORBIDDEN,
            "Only the room owner or a moderator can start a party.",
        );
    }
    let game_type = body
        .game_type
        .unwrap_or_else(|| "quiz".into())
        .replace('-', "_");
    if !crate::minigames::VALID_TYPES.contains(&game_type.as_str()) {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "Invalid game type.");
    }
    {
        if state.parties.map.lock().unwrap().contains_key(&room_id) {
            return err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "A party is already running.",
            );
        }
    }
    let n = body.count.unwrap_or(PARTY_QUESTIONS).clamp(3, 20);
    let questions = state.minigames.gen_question_set(&game_type, n).await;
    if questions.len() < 3 {
        return err(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Not enough questions available.",
        );
    }
    {
        let mut map = state.parties.map.lock().unwrap();
        if map.contains_key(&room_id) {
            return err(
                StatusCode::UNPROCESSABLE_ENTITY,
                "A party is already running.",
            );
        }
        map.insert(
            room_id,
            Party {
                game_type: game_type.clone(),
                questions,
                idx: -1,
                phase: "question",
                ends_at: 0,
                scores: HashMap::new(),
                answered: HashMap::new(),
                handle: None,
            },
        );
        let handle = spawn_driver(state.clone(), room_id);
        map.get_mut(&room_id).unwrap().handle = Some(handle);
    }
    state
        .hub
        .broadcast(
            &channel(room_id),
            "PartyStarted",
            json!({"room_id": room_id, "game_type": game_type, "by": user.name}),
            None,
        )
        .await;
    Json(json!({"ok": true, "total": n})).into_response()
}

pub async fn stop(
    State(state): State<AppState>,
    Path(room_id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !can_host(&state, room_id, &user).await {
        return err(
            StatusCode::FORBIDDEN,
            "Only the room owner or a moderator can stop a party.",
        );
    }
    let Some(p) = state.parties.remove(room_id) else {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "No party running.");
    };
    if let Some(h) = p.handle {
        h.abort();
    }
    state
        .hub
        .broadcast(
            &channel(room_id),
            "PartyStopped",
            json!({"room_id": room_id}),
            None,
        )
        .await;
    Json(json!({"ok": true})).into_response()
}

pub async fn show(
    State(state): State<AppState>,
    Path(room_id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !require_participant(&state, room_id, user.id).await {
        return err(StatusCode::FORBIDDEN, "Join the room first.");
    }
    match state.parties.snapshot(room_id, user.id) {
        Some(v) => Json(v).into_response(),
        None => err(StatusCode::NOT_FOUND, "No party running."),
    }
}

#[derive(Deserialize)]
pub struct AnswerBody {
    idx: i64,
    value: String,
}

pub async fn answer(
    State(state): State<AppState>,
    Path(room_id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<AnswerBody>,
) -> Response {
    if !require_participant(&state, room_id, user.id).await {
        return err(StatusCode::FORBIDDEN, "Join the room first.");
    }
    let (game_type, correct) = {
        let mut map = state.parties.map.lock().unwrap();
        let Some(p) = map.get_mut(&room_id) else {
            return err(StatusCode::NOT_FOUND, "No party running.");
        };
        if p.phase != "question" || p.idx != body.idx {
            return err(StatusCode::UNPROCESSABLE_ENTITY, "Question closed.");
        }
        let Some(q) = p.questions.get(body.idx as usize) else {
            return err(StatusCode::UNPROCESSABLE_ENTITY, "Invalid question index.");
        };
        let Some(correct_value) = q["correct_value"].as_str() else {
            return err(StatusCode::UNPROCESSABLE_ENTITY, "Invalid question.");
        };
        let already = p.answered.entry(user.id).or_default();
        if !already.insert(body.idx) {
            return err(StatusCode::UNPROCESSABLE_ENTITY, "Already answered.");
        }
        let ok = question_correct(&p.game_type, correct_value, &body.value);
        if ok {
            *p.scores.entry(user.id).or_insert(0) += 1;
        }
        (p.game_type.clone(), (ok, correct_value.to_string()))
    };
    let (ok, correct_value) = correct;
    if ok {
        let _ = state
            .minigames
            .record_score(user.id, &game_type, 1, json!({"party": room_id}))
            .await;
        state
            .levels
            .dispatch(user.id, blindall_db::LevelTrigger::Score);
        state
            .hub
            .broadcast(
                &channel(room_id),
                "PartyScore",
                json!({"room_id": room_id, "user_id": user.id,
                       "name": user.name, "idx": body.idx,
                       "score": state.parties.map.lock().unwrap()
                           .get(&room_id)
                           .and_then(|p| p.scores.get(&user.id).copied())
                           .unwrap_or(0)}),
                None,
            )
            .await;
    }
    Json(json!({"correct": ok, "correct_value": correct_value})).into_response()
}
