//! Realtime matchmaking for the minigames: players queue per game type,
//! the server forms a match as soon as 2 players are waiting (up to 6),
//! then drives a shared question sequence on the `match.{id}` channel —
//! same engine as the room party (MatchQuestion → MatchReveal →
//! MatchFinished), scores race live. In-memory: a match is ephemeral.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicI64, Ordering};
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
const COUNTDOWN_SECS: u64 = 4;
const MATCH_QUESTIONS: usize = 10;
const MIN_PLAYERS: usize = 2;
const MAX_PLAYERS: usize = 6;
const QUEUE_STALE_SECS: i64 = 120;
/// Finished matches stay readable this long (results screen / rematch),
/// then a sweeper drops them.
const MATCH_KEEP_SECS: u64 = 600;

#[derive(Clone)]
pub struct Queued {
    pub user_id: i64,
    pub name: String,
    pub joined_at: i64,
}

pub struct Match {
    pub game_type: String,
    pub players: Vec<Queued>,
    pub questions: Vec<Value>, // correct_value kept server-side
    pub idx: i64,
    pub phase: &'static str, // "countdown" | "question" | "reveal" | "finished"
    pub ends_at: i64,
    pub scores: HashMap<i64, i64>,
    pub answered: HashMap<i64, HashSet<i64>>,
    pub finished_at: Option<i64>,
}

#[derive(Default)]
pub struct MatchStore {
    matches: Mutex<HashMap<i64, Match>>,
    queues: Mutex<HashMap<String, Vec<Queued>>>,
    next_id: AtomicI64,
}

impl MatchStore {
    pub fn is_participant(&self, match_id: i64, user_id: i64) -> bool {
        self.matches
            .lock()
            .unwrap()
            .get(&match_id)
            .is_some_and(|m| m.players.iter().any(|p| p.user_id == user_id))
    }

    /// Queue position (1-based) for the user, whichever type they queued for.
    fn queue_status(&self, user_id: i64) -> Option<(String, usize)> {
        let queues = self.queues.lock().unwrap_or_else(|e| e.into_inner());
        for (ty, list) in queues.iter() {
            if let Some(pos) = list.iter().position(|q| q.user_id == user_id) {
                return Some((ty.clone(), pos + 1));
            }
        }
        None
    }

    /// The user's latest *unfinished* match, if any. Finished matches are
    /// only reachable via `show` (results screen) — join/status never
    /// route back to them, or rematch would loop into the stale game.
    fn active_match(&self, user_id: i64) -> Option<i64> {
        self.matches
            .lock()
            .unwrap()
            .iter()
            .filter(|(_, m)| m.players.iter().any(|p| p.user_id == user_id))
            .filter(|(_, m)| m.phase != "finished")
            .map(|(id, _)| *id)
            .max()
    }

    fn snapshot(&self, match_id: i64, viewer_id: i64) -> Option<Value> {
        let map = self.matches.lock().unwrap_or_else(|e| e.into_inner());
        let m = map.get(&match_id)?;
        Some(match_json(match_id, m, viewer_id))
    }

    fn remove(&self, match_id: i64) {
        self.matches.lock().unwrap_or_else(|e| e.into_inner()).remove(&match_id);
    }
}

fn match_json(match_id: i64, m: &Match, viewer_id: i64) -> Value {
    let current = if m.phase == "question" && m.idx >= 0 {
        m.questions.get(m.idx as usize).map(|q| {
            strip_questions(std::slice::from_ref(q))
                .into_iter()
                .next()
                .unwrap()
        })
    } else {
        None
    };
    let reveal_value = if m.phase == "reveal" && m.idx >= 0 {
        m.questions
            .get(m.idx as usize)
            .and_then(|q| q["correct_value"].as_str())
            .map(str::to_string)
    } else {
        None
    };
    json!({
        "id": match_id,
        "game_type": m.game_type,
        "phase": m.phase,
        "idx": m.idx,
        "total": m.questions.len(),
        "ends_at": m.ends_at,
        "question": current,
        "reveal_value": reveal_value,
        "players": m.players.iter()
            .map(|p| json!({"id": p.user_id, "name": p.name}))
            .collect::<Vec<_>>(),
        "scores": m.players.iter()
            .map(|p| json!({"user_id": p.user_id, "name": p.name,
                            "score": m.scores.get(&p.user_id).copied().unwrap_or(0)}))
            .collect::<Vec<_>>(),
        "my_answered": m.answered.get(&viewer_id)
            .map(|s| s.iter().copied().collect::<Vec<_>>())
            .unwrap_or_default(),
    })
}

fn channel(match_id: i64) -> String {
    format!("match.{match_id}")
}

fn user_channel(user_id: i64) -> String {
    format!("user:{user_id}")
}

fn err(status: StatusCode, msg: &str) -> Response {
    (status, Json(json!({"error": msg}))).into_response()
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Drop queue entries older than QUEUE_STALE_SECS (disconnected clients
/// never send a leave).
fn prune_queues(store: &MatchStore) {
    let cutoff = now() - QUEUE_STALE_SECS;
    let mut queues = store.queues.lock().unwrap_or_else(|e| e.into_inner());
    for list in queues.values_mut() {
        list.retain(|q| q.joined_at >= cutoff);
    }
    queues.retain(|_, list| !list.is_empty());
}

/// Pop up to MAX_PLAYERS queued users for `game_type` and spawn a match:
/// frozen question set, server-side driver, MatchFound ping on each
/// player's user channel (the app already subscribes to it).
async fn try_form(state: &AppState, game_type: &str) {
    let players: Vec<Queued> = {
        let mut queues = state.matchmaking.queues.lock().unwrap_or_else(|e| e.into_inner());
        let Some(list) = queues.get_mut(game_type) else {
            return;
        };
        if list.len() < MIN_PLAYERS {
            return;
        }
        let take = list.len().min(MAX_PLAYERS);
        let players: Vec<Queued> = list.drain(..take).collect();
        if list.is_empty() {
            queues.remove(game_type);
        }
        players
    };

    // Stale queue entries can name a player already inside an active
    // match (tiny race between their queue drain and the match insert) —
    // never double-match someone.
    let players: Vec<Queued> = players
        .into_iter()
        .filter(|p| state.matchmaking.active_match(p.user_id).is_none())
        .collect();
    if players.len() < MIN_PLAYERS {
        let mut queues = state.matchmaking.queues.lock().unwrap_or_else(|e| e.into_inner());
        let list = queues.entry(game_type.to_string()).or_default();
        let mut front = players;
        front.append(list);
        *list = front;
        return;
    }

    let questions = state
        .minigames
        .gen_question_set(game_type, MATCH_QUESTIONS)
        .await;
    if questions.len() < 3 {
        // Can't build a set right now — requeue everyone at the front so
        // a later join retries the formation.
        let mut queues = state.matchmaking.queues.lock().unwrap_or_else(|e| e.into_inner());
        let list = queues.entry(game_type.to_string()).or_default();
        let mut front = players;
        front.append(list);
        *list = front;
        return;
    }

    let match_id = state.matchmaking.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    {
        state.matchmaking.matches.lock().unwrap_or_else(|e| e.into_inner()).insert(
            match_id,
            Match {
                game_type: game_type.to_string(),
                players: players.clone(),
                questions,
                idx: -1,
                phase: "countdown",
                ends_at: now() + COUNTDOWN_SECS as i64,
                scores: HashMap::new(),
                answered: HashMap::new(),
                finished_at: None,
            },
        );
    }

    for p in &players {
        state
            .hub
            .broadcast(
                &user_channel(p.user_id),
                "MatchFound",
                json!({"match_id": match_id, "game_type": game_type}),
                None,
            )
            .await;
    }

    tokio::spawn(run_match(state.clone(), match_id));
}

/// Server-side driver: countdown → per-question broadcast → reveal →
/// finished. Mirrors the party driver; clients only display.
async fn run_match(state: AppState, match_id: i64) {
    tokio::time::sleep(Duration::from_secs(COUNTDOWN_SECS)).await;
    let total = {
        state
            .matchmaking
            .matches
            .lock()
            .unwrap()
            .get(&match_id)
            .map(|m| m.questions.len() as i64)
            .unwrap_or(0)
    };
    for i in 0..total {
        let payload = {
            let mut map = state.matchmaking.matches.lock().unwrap_or_else(|e| e.into_inner());
            let Some(m) = map.get_mut(&match_id) else {
                return;
            };
            m.idx = i;
            m.phase = "question";
            m.ends_at = now() + QUESTION_SECS as i64;
            match_json(match_id, m, 0)
        };
        state
            .hub
            .broadcast(&channel(match_id), "MatchQuestion", payload, None)
            .await;
        tokio::time::sleep(Duration::from_secs(QUESTION_SECS)).await;

        let payload = {
            let mut map = state.matchmaking.matches.lock().unwrap_or_else(|e| e.into_inner());
            let Some(m) = map.get_mut(&match_id) else {
                return;
            };
            m.phase = "reveal";
            m.ends_at = now() + REVEAL_SECS as i64;
            match_json(match_id, m, 0)
        };
        state
            .hub
            .broadcast(&channel(match_id), "MatchReveal", payload, None)
            .await;
        tokio::time::sleep(Duration::from_secs(REVEAL_SECS)).await;
    }

    let payload = {
        let mut map = state.matchmaking.matches.lock().unwrap_or_else(|e| e.into_inner());
        let Some(m) = map.get_mut(&match_id) else {
            return;
        };
        m.phase = "finished";
        m.finished_at = Some(now());
        match_json(match_id, m, 0)
    };
    state
        .hub
        .broadcast(&channel(match_id), "MatchFinished", payload, None)
        .await;

    // Keep the result readable for a while, then sweep.
    tokio::time::sleep(Duration::from_secs(MATCH_KEEP_SECS)).await;
    state.matchmaking.remove(match_id);
}

// ---- endpoints ---------------------------------------------------------------

#[derive(Deserialize)]
pub struct JoinBody {
    game_type: Option<String>,
}

pub async fn join(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<JoinBody>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }
    if state.moderation.is_banned(user.id).await.unwrap_or(false) {
        return err(StatusCode::FORBIDDEN, "banned");
    }
    let game_type = body
        .game_type
        .unwrap_or_else(|| "quiz".into())
        .replace('-', "_");
    if !crate::minigames::VALID_TYPES.contains(&game_type.as_str()) {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "Invalid game type.");
    }

    prune_queues(&state.matchmaking);

    // Already in an active match → send them back to it.
    if let Some(id) = state.matchmaking.active_match(user.id) {
        return Json(json!({"status": "matched", "match_id": id})).into_response();
    }

    // One queue slot per user: a new join switches queues.
    {
        let mut queues = state.matchmaking.queues.lock().unwrap_or_else(|e| e.into_inner());
        for list in queues.values_mut() {
            list.retain(|q| q.user_id != user.id);
        }
        queues.entry(game_type.clone()).or_default().push(Queued {
            user_id: user.id,
            name: user.name.clone(),
            joined_at: now(),
        });
    }

    try_form(&state, &game_type).await;

    // Maybe the join completed a queue and formed a match instantly.
    if let Some(id) = state.matchmaking.active_match(user.id) {
        return Json(json!({"status": "matched", "match_id": id})).into_response();
    }
    let position = state
        .matchmaking
        .queue_status(user.id)
        .map(|(_, p)| p)
        .unwrap_or(1);
    Json(json!({"status": "queued", "game_type": game_type, "position": position})).into_response()
}

pub async fn leave(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> Response {
    let mut queues = state.matchmaking.queues.lock().unwrap_or_else(|e| e.into_inner());
    for list in queues.values_mut() {
        list.retain(|q| q.user_id != user.id);
    }
    queues.retain(|_, list| !list.is_empty());
    Json(json!({"status": "idle"})).into_response()
}

pub async fn status(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }
    prune_queues(&state.matchmaking);
    if let Some(id) = state.matchmaking.active_match(user.id) {
        return Json(json!({"status": "matched", "match_id": id})).into_response();
    }
    match state.matchmaking.queue_status(user.id) {
        Some((ty, pos)) => {
            Json(json!({"status": "queued", "game_type": ty, "position": pos})).into_response()
        }
        None => Json(json!({"status": "idle"})).into_response(),
    }
}

pub async fn show(
    State(state): State<AppState>,
    Path(match_id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !state.matchmaking.is_participant(match_id, user.id) {
        return err(StatusCode::FORBIDDEN, "Not a participant.");
    }
    match state.matchmaking.snapshot(match_id, user.id) {
        Some(v) => Json(v).into_response(),
        None => err(StatusCode::NOT_FOUND, "Match not found."),
    }
}

#[derive(Deserialize)]
pub struct AnswerBody {
    idx: i64,
    value: String,
}

pub async fn answer(
    State(state): State<AppState>,
    Path(match_id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<AnswerBody>,
) -> Response {
    let (game_type, correct) = {
        let mut map = state.matchmaking.matches.lock().unwrap_or_else(|e| e.into_inner());
        let Some(m) = map.get_mut(&match_id) else {
            return err(StatusCode::NOT_FOUND, "Match not found.");
        };
        if !m.players.iter().any(|p| p.user_id == user.id) {
            return err(StatusCode::FORBIDDEN, "Not a participant.");
        }
        if m.phase != "question" || m.idx != body.idx {
            return err(StatusCode::UNPROCESSABLE_ENTITY, "Question closed.");
        }
        let Some(q) = m.questions.get(body.idx as usize) else {
            return err(StatusCode::UNPROCESSABLE_ENTITY, "Invalid question index.");
        };
        let Some(correct_value) = q["correct_value"].as_str() else {
            return err(StatusCode::UNPROCESSABLE_ENTITY, "Invalid question.");
        };
        let already = m.answered.entry(user.id).or_default();
        if !already.insert(body.idx) {
            return err(StatusCode::UNPROCESSABLE_ENTITY, "Already answered.");
        }
        let ok = question_correct(&m.game_type, correct_value, &body.value);
        if ok {
            *m.scores.entry(user.id).or_insert(0) += 1;
        }
        (m.game_type.clone(), (ok, correct_value.to_string()))
    };
    let (ok, correct_value) = correct;
    if ok {
        let _ = state
            .minigames
            .record_score(user.id, &game_type, 1, json!({"match": match_id}))
            .await;
        state
            .levels
            .dispatch(user.id, blindall_db::LevelTrigger::Score);
        let score = state
            .matchmaking
            .matches
            .lock()
            .unwrap()
            .get(&match_id)
            .and_then(|m| m.scores.get(&user.id).copied())
            .unwrap_or(0);
        state
            .hub
            .broadcast(
                &channel(match_id),
                "MatchScore",
                json!({"match_id": match_id, "user_id": user.id,
                       "name": user.name, "idx": body.idx, "score": score}),
                None,
            )
            .await;
    }
    Json(json!({"correct": ok, "correct_value": correct_value})).into_response()
}
