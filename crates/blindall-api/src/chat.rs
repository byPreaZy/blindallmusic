use crate::auth::CurrentUser;
use crate::auth_session::BAD_WORDS;
use crate::state::AppState;
use blindall_game::GameStore;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use regex::Regex;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const ROOM_FLOOD_PER_MINUTE: usize = 8;
const DUPLICATE_WINDOW: Duration = Duration::from_secs(30);
const MIN_CROSS_ROOM_BODY_LEN: usize = 8;
const CROSS_ROOM_WINDOW: Duration = Duration::from_secs(300);
const CROSS_ROOM_MIN_ROOMS: usize = 3;
const CROSS_ROOM_BAN_DAYS: i64 = 7;

static LINK_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"(?i)(?:https?://|www\.)\S+").unwrap());
static STRIP_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"[\s\p{P}\p{S}]+").unwrap());
static WS_RE: std::sync::LazyLock<Regex> =
    std::sync::LazyLock::new(|| Regex::new(r"\s+").unwrap());

struct Entry {
    at: Instant,
    room_id: i64,
}

#[derive(Default)]
pub struct ChatModeration {
    flood: Mutex<HashMap<(i64, i64), Vec<Instant>>>,
    duplicates: Mutex<HashMap<(i64, i64, u64), Instant>>,
    cross_room: Mutex<HashMap<(i64, u64), Vec<Entry>>>,
}

fn body_hash(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;
    let mut h = DefaultHasher::new();
    h.write(s.as_bytes());
    h.finish()
}

impl ChatModeration {
    pub fn filter_body(body: &str) -> String {
        let mut out = body.to_string();
        for w in BAD_WORDS {
            let w = w.trim();
            if w.is_empty() {
                continue;
            }
            let re = Regex::new(&format!(r"(?i)\b{}\b", regex::escape(w))).unwrap();
            out = re.replace_all(&out, "*****").into_owned();
        }
        out
    }

    fn normalize_body(body: &str) -> String {
        let trimmed = body.trim();
        if trimmed.is_empty() {
            return String::new();
        }
        let normalized = deunicode::deunicode(&trimmed.to_lowercase());
        let stripped = STRIP_RE.replace_all(&normalized, "");
        if !stripped.is_empty() {
            return stripped.into_owned();
        }
        WS_RE.replace_all(trimmed, "").into_owned()
    }

    pub async fn assert_can_send(
        &self,
        state: &AppState,
        user_id: i64,
        is_staff: bool,
        room_id: i64,
        body: &str,
    ) -> Result<(), &'static str> {
        if is_staff {
            return Ok(());
        }
        let normalized = Self::normalize_body(body);
        if normalized.is_empty() {
            return Err("Chat message empty");
        }
        {
            let mut flood = self.flood.lock().unwrap();
            let key = (user_id, room_id);
            let v = flood.entry(key).or_default();
            v.retain(|t| t.elapsed() < Duration::from_secs(60));
            if v.len() >= ROOM_FLOOD_PER_MINUTE {
                return Err("Chat message flood");
            }
        }
        {
            let dups = self.duplicates.lock().unwrap();
            let key = (user_id, room_id, body_hash(&normalized));
            if let Some(t) = dups.get(&key) {
                if t.elapsed() < DUPLICATE_WINDOW {
                    return Err("Chat message duplicate");
                }
            }
        }
        if LINK_RE.is_match(body) {
            return Err("Chat message links forbidden");
        }
        if self.cross_room_room_count(user_id, room_id, &normalized) >= CROSS_ROOM_MIN_ROOMS {
            if state.chat.is_banned(user_id).await {
                return Err("Chat message rejected spam");
            }
            let comment = format!("Chat auto ban cross room spam: {}", &body[..body.len().min(120)]);
            if state
                .chat
                .ban_user(user_id, CROSS_ROOM_BAN_DAYS, &comment)
                .await
                .is_ok()
            {
                cleanup_banned_user_messages(state, user_id).await;
            }
            return Err("Chat message banned spam");
        }
        Ok(())
    }

    fn cross_room_room_count(&self, user_id: i64, room_id: i64, normalized: &str) -> usize {
        if normalized.chars().count() < MIN_CROSS_ROOM_BODY_LEN {
            return 0;
        }
        let mut cross = self.cross_room.lock().unwrap();
        let key = (user_id, body_hash(normalized));
        let entries = cross.entry(key).or_default();
        entries.retain(|e| e.at.elapsed() < CROSS_ROOM_WINDOW);
        let mut rooms: Vec<i64> = entries.iter().map(|e| e.room_id).collect();
        rooms.push(room_id);
        rooms.sort_unstable();
        rooms.dedup();
        rooms.len()
    }

    pub fn record_sent(&self, user_id: i64, room_id: i64, body: &str) {
        let normalized = Self::normalize_body(body);
        if normalized.is_empty() {
            return;
        }
        self.flood
            .lock()
            .unwrap()
            .entry((user_id, room_id))
            .or_default()
            .push(Instant::now());
        self.duplicates
            .lock()
            .unwrap()
            .insert((user_id, room_id, body_hash(&normalized)), Instant::now());
        if normalized.chars().count() >= MIN_CROSS_ROOM_BODY_LEN {
            self.cross_room
                .lock()
                .unwrap()
                .entry((user_id, body_hash(&normalized)))
                .or_default()
                .push(Entry {
                    at: Instant::now(),
                    room_id,
                });
        }
    }
}

#[derive(Deserialize)]
pub struct MessageBody {
    body: String,
}

fn err(field: &str, msg: &str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"message": msg, "errors": { field: [msg] }})),
    )
        .into_response()
}

pub async fn cleanup_banned_user_messages(state: &AppState, user_id: i64) {
    match state.chat.soft_delete_user_messages(user_id).await {
        Ok(deleted) => {
            for (id, room_id) in deleted {
                state
                    .hub
                    .broadcast(
                        &chat_channel(room_id),
                        "MessageDeleted",
                        json!({"message": {"id": id}}),
                        None,
                    )
                    .await;
            }
        }
        Err(e) => tracing::warn!(error = %e, user_id, "banned-user message cleanup failed"),
    }
}

fn chat_channel(room_id: i64) -> String {
    format!("chat-room.{room_id}")
}

async fn message_json(state: &AppState, msg_id: i64, room_id: i64) -> Value {
    let msg = state.chat.find_message(msg_id).await.ok().flatten();
    let reports = state.chat.total_downvotes(msg_id).await.unwrap_or(0);
    let (user, body, created_at) = match &msg {
        Some(m) => {
            let info = state
                .store
                .players_info(&[m.user_id])
                .await
                .ok()
                .and_then(|map| map.get(&m.user_id).cloned());
            let team = match info.as_ref().and_then(|p| p.team_id) {
                Some(tid) => state
                    .teams
                    .find(tid)
                    .await
                    .ok()
                    .flatten()
                    .map(|t| json!({"id": t.id, "name": t.name}))
                    .unwrap_or(Value::Null),
                None => Value::Null,
            };
            (
                json!({
                    "id": m.user_id,
                    "name": info.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
                    "photo": crate::profiles::photo_url(
                        state,
                        info.as_ref().and_then(|p| p.photo.as_deref()).unwrap_or(""),
                        info.as_ref().map(|p| p.name.as_str()).unwrap_or(""),
                    ),
                    "team_id": info.as_ref().and_then(|p| p.team_id),
                    "team": team,
                }),
                m.body.clone(),
                m.created_at,
            )
        }
        None => (Value::Null, String::new(), chrono::Utc::now()),
    };
    json!({
        "id": msg_id,
        "messagable_id": room_id,
        "messagable_type": "App\\Models\\Room",
        "body": body,
        "reports": reports,
        "time": created_at.format("%H:%M").to_string(),
        "created_at": created_at,
        "user": user,
    })
}

pub async fn store(
    State(state): State<AppState>,
    Path(room_id): Path<i64>,
    headers: HeaderMap,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<MessageBody>,
) -> Response {
    if user.is_guest {
        return StatusCode::FORBIDDEN.into_response();
    }
    if !(user.is_administrator || state.auth.is_public_moderator(user.id).await) {
        if let Some(secs) = state.chat_rate.retry_after(&format!("chat|{}", user.id), 15) {
            return (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({"error": "rate_limited", "retry_after": secs})),
            )
                .into_response();
        }
    }
    let body_text = body.body.trim();
    if body_text.is_empty() || body_text.chars().count() > 255 {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"message": "validation", "errors": {"body": ["The body field is required and must not exceed 255 characters."]}})),
        )
            .into_response();
    }
    if state.chat.is_banned(user.id).await {
        return StatusCode::NO_CONTENT.into_response();
    }
    let is_staff = user.is_administrator || state.auth.is_public_moderator(user.id).await;
    if let Err(msg) = state
        .chat_moderation
        .assert_can_send(&state, user.id, is_staff, room_id, body_text)
        .await
    {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"message": msg, "errors": {"body": [msg]}})),
        )
            .into_response();
    }
    let ip = crate::auth_session::client_ip(&headers, &addr.ip().to_string());
    let filtered = ChatModeration::filter_body(body_text);
    let msg_id = match state.chat.create_message(room_id, user.id, &ip, &filtered).await {
        Ok(id) => id,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    state.chat_moderation.record_sent(user.id, room_id, body_text);
    let mut payload = message_json(&state, msg_id, room_id).await;
    crate::donations::enrich_messages(&state, std::slice::from_mut(&mut payload)).await;
    state
        .hub
        .broadcast(&chat_channel(room_id), "NewMessage", json!({"message": payload}), None)
        .await;
    StatusCode::NO_CONTENT.into_response()
}

pub async fn config(State(state): State<AppState>) -> Response {
    let public_moderator_ids = state
        .auth
        .public_moderator_ids()
        .await
        .unwrap_or_default();
    Json(json!({
        "reactions": {
            "standard": state.chat_reaction_emojis,
            "supporter": state.donations.supporter_reaction_emojis,
        },
        "public_moderator_ids": public_moderator_ids,
    }))
    .into_response()
}

pub async fn history(State(state): State<AppState>, Path(room_id): Path<i64>) -> Response {
    match state.chat.room_messages(room_id, 30).await {
        Ok(mut msgs) => {
            crate::donations::enrich_messages(&state, &mut msgs).await;
            Json(json!({"messages": msgs})).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ---- reactions (`MessageReactionController`) ---------------------------------

fn can_react(state: &AppState, user_is_supporter: bool, is_guest: bool, emoji: &str) -> Result<(), &'static str> {
    if emoji.is_empty() || emoji.chars().count() > 191 {
        return Err("validation");
    }
    let standard = &state.chat_reaction_emojis;
    let supporter = &state.donations.supporter_reaction_emojis;
    if standard.contains(&emoji.to_string()) {
        return if is_guest { Err("unauthorized") } else { Ok(()) };
    }
    if supporter.contains(&emoji.to_string()) {
        return if user_is_supporter { Ok(()) } else { Err("locked") };
    }
    Err("invalid")
}

pub async fn reactions(
    State(state): State<AppState>,
    Path(message_id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    let reactions = state.chat.grouped_reactions(message_id).await.unwrap_or_default();
    let user_reaction = state
        .chat
        .user_reaction(message_id, user.id)
        .await
        .unwrap_or(None);
    Json(json!({
        "reactions": reactions,
        "userReaction": user_reaction,
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct ReactionBody {
    emoji: String,
}

pub async fn toggle_reaction(
    State(state): State<AppState>,
    Path(message_id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<ReactionBody>,
) -> Response {
    if user.is_guest {
        return StatusCode::FORBIDDEN.into_response();
    }
    let is_supporter = state
        .donations_repo
        .user_is_supporter(user.id, &blindall_db::donations::current_month_key())
        .await
        .unwrap_or(false)
        && state
            .donations
            .supporter_perks
            .iter()
            .any(|p| p == "supporter_reactions");
    match can_react(&state, is_supporter, user.is_guest, &body.emoji) {
        Err("locked") => {
            return err("emoji", "Supporter reaction emoji locked");
        }
        Err(_) => {
            return err("emoji", "Invalid reaction emoji");
        }
        Ok(()) => {}
    }
    if state.chat.find_message(message_id).await.ok().flatten().is_none() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let removed = state
        .chat
        .delete_reaction(message_id, user.id, &body.emoji)
        .await
        .unwrap_or(0);
    let (removed_flag, user_reaction) = if removed > 0 {
        (true, state.chat.user_reaction(message_id, user.id).await.unwrap_or(None))
    } else {
        if state
            .chat
            .create_reaction(message_id, user.id, &body.emoji)
            .await
            .is_err()
        {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        (false, Some(body.emoji.clone()))
    };
    let reactions = state.chat.grouped_reactions(message_id).await.unwrap_or_default();
    state
        .hub
        .broadcast(
            &format!("chat.message.{message_id}"),
            "MessageReactionUpdated",
            json!({
                "messageId": message_id,
                "reactions": reactions,
                "userReaction": user_reaction,
            }),
            None,
        )
        .await;
    Json(json!({ if removed_flag { "removed" } else { "added" }: true })).into_response()
}

pub async fn report(
    State(state): State<AppState>,
    Path(message_id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if user.is_guest {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Some(msg) = state.chat.find_message(message_id).await.ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let total = state.chat.down_vote(message_id, user.id).await.unwrap_or(0);
    let channel = chat_channel(msg.room_id);
    let payload = message_json(&state, message_id, msg.room_id).await;
    state
        .hub
        .broadcast(&channel, "MessageReported", json!({"message": payload}), None)
        .await;
    if total < -2 {
        state
            .hub
            .broadcast(&channel, "MessageDeleted", json!({"message": {"id": message_id}}), None)
            .await;
        let _ = state.chat.soft_delete_message(message_id).await;
    }
    StatusCode::NO_CONTENT.into_response()
}

pub async fn destroy(
    State(state): State<AppState>,
    Path(message_id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let Some(msg) = state.chat.find_message(message_id).await.ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let allowed = user.is_administrator
        || state.auth.is_public_moderator(user.id).await
        || state.rooms.is_moderator(msg.room_id, user.id).await;
    if allowed {
        let channel = chat_channel(msg.room_id);
        state
            .hub
            .broadcast(&channel, "MessageDeleted", json!({"message": {"id": message_id}}), None)
            .await;
        let _ = state.chat.soft_delete_message(message_id).await;
    }
    StatusCode::NO_CONTENT.into_response()
}

pub async fn restore(
    State(state): State<AppState>,
    Path(message_id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let allowed = user.is_administrator || state.auth.is_public_moderator(user.id).await;
    if !allowed {
        return StatusCode::FORBIDDEN.into_response();
    }
    match state.chat.restore_message(message_id).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_body_censors_word_boundaries() {
        assert_eq!(ChatModeration::filter_body("t'es un connard"), "t'es un *****");
        let filtered = ChatModeration::filter_body("connasse");
        assert!(!filtered.contains("connasse") || filtered == "connasse");
        assert_eq!(ChatModeration::filter_body("bonjour"), "bonjour");
    }

    #[test]
    fn normalize_strips_everything() {
        assert_eq!(ChatModeration::normalize_body("  Héllo, Wörld!  "), "helloworld");
        assert_eq!(ChatModeration::normalize_body("   "), "");
    }

    #[test]
    fn link_detection() {
        assert!(LINK_RE.is_match("go to https://x.fr now"));
        assert!(LINK_RE.is_match("www.spam.com"));
        assert!(!LINK_RE.is_match("je t'aime bien"));
    }

    #[test]
    fn cross_room_count_needs_three_distinct_rooms() {
        let m = ChatModeration::default();
        let body = "same long message here";
        for room in [1, 2] {
            m.record_sent(7, room, body);
        }
        assert_eq!(m.cross_room_room_count(7, 3, &ChatModeration::normalize_body(body)), 3);
        assert_eq!(m.cross_room_room_count(7, 9, "hi"), 0);
    }
}
