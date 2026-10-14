//! Broadcast payload builders — key-for-key parity with the PHP events'
//! `broadcastWith()` (snake_case field names preserved).

use crate::store::{GameTrack, RoomConfig, RoundRow};
use blindall_core::types::TrackAnswer;
use chrono::{DateTime, SecondsFormat, Utc};
use serde_json::{json, Value};

/// Channel name for a room (our protocol uses `room:{id}` for the merged
/// presence+events channel).
pub fn room_channel(room_id: i64) -> String {
    format!("room:{room_id}")
}

pub fn public_channel(room_id: i64) -> String {
    format!("room.public:{room_id}")
}

/// `Round::toArray()` equivalent for broadcast payloads.
pub fn round_payload(r: &RoundRow) -> Value {
    json!({
        "id": r.id,
        "room_id": r.room_id,
        "tracks": r.tracks,
        "current": r.current,
        "is_playing": r.is_playing,
        "finished_at": r.finished_at.map(|t| t.to_rfc3339_opts(SecondsFormat::Micros, true)),
        "current_track_started_at": r
            .current_track_started_at
            .map(|t| t.to_rfc3339_opts(SecondsFormat::Micros, true)),
    })
}

/// `TrackPlayed::broadcastWith`.
pub fn track_played(round: &RoundRow, room: &Value, track: &GameTrack) -> Value {
    let mut r = round_payload(round);
    // PHP sets current_track_started_at explicitly in the payload.
    if let Some(t) = round.current_track_started_at {
        r["current_track_started_at"] =
            Value::String(t.to_rfc3339_opts(SecondsFormat::Micros, true));
    }
    json!({
        "round": r,
        "room": room,
        "track": {
            "id": track.id,
            "provider": track.provider,
            "preview_url": track.preview_url,
            "audio": track.audio,
            "hint": track.hint,
            "answers": track.answers.iter().map(|a| json!({
                "id": a.id,
                "name": a.type_name,
            })).collect::<Vec<_>>(),
        },
    })
}

/// `TrackEnded::broadcastWith` — includes next-track prefetch fields and
/// the revealed answer card for the track that just finished.
pub fn track_ended(
    round: &RoundRow,
    pause_between_tracks: u32,
    finished_track_playlist_payload: Option<Value>,
    next: Option<&GameTrack>,
    now: DateTime<Utc>,
) -> Value {
    let is_last = next.is_none();
    let mut payload = json!({
        "round": round_payload(round),
        "next_track_at": if is_last {
            Value::Null
        } else {
            Value::String(
                (now + chrono::Duration::seconds(pause_between_tracks as i64))
                    .to_rfc3339_opts(SecondsFormat::Micros, true),
            )
        },
        "next_track_id": next.map(|t| Value::from(t.id)).unwrap_or(Value::Null),
        "next_track_preview_url": next
            .and_then(|t| t.preview_url.clone())
            .map_or(Value::Null, Value::String),
        "next_track_audio_url": next
            .and_then(|t| t.audio.clone())
            .map_or(Value::Null, Value::String),
    });
    if let Some(tp) = finished_track_playlist_payload {
        payload["track"] = tp;
    }
    payload
}

/// `RoundStarted::broadcastWith` — PHP serializes the round with room.
pub fn round_started(round: &RoundRow, room: &Value) -> Value {
    let mut r = round_payload(round);
    r["room"] = room.clone();
    json!({ "round": r })
}

/// `RoundFinished::broadcastWith` — podium data comes from finalization.
pub fn round_finished(round: &RoundRow, room: &Value, users_podium: Value, teams_podium: Value) -> Value {
    let mut r = round_payload(round);
    r["room"] = room.clone();
    json!({
        "round": r,
        "users_podium": users_podium,
        "teams_podium": teams_podium,
    })
}

/// `RoomPublicState::broadcastWith` → `Room::publicStatePayload()`.
pub fn room_public_state(
    room: &RoomConfig,
    member_count: i64,
    current_track_index: i64,
    is_playing: bool,
) -> Value {
    json!({
        "roomId": room.id,
        "memberCount": member_count,
        "currentTrackIndex": current_track_index,
        "tracksByRound": room.tracks_by_round,
        "isPlaying": is_playing,
    })
}

/// `NewScore` payload — same shape as `RoundController::check` broadcast.
pub fn new_score(
    room_id: i64,
    user_id: i64,
    track_id: i64,
    answers: &[blindall_core::scoring::FoundAnswer],
    total: f64,
    time: f64,
) -> Value {
    json!({
        "room_id": room_id,
        "user_id": user_id,
        "track_id": track_id,
        "answers": answers.iter().map(|a| json!({
            "id": a.answer_id,
            "order": a.order,
            "speedBonus": a.speed_bonus,
            "name": a.type_name,
            "value": a.value,
        })).collect::<Vec<_>>(),
        "total": total,
        "time": time,
    })
}

/// `UserHasFoundAllTheAnswers` payload — PHP serializes the public props
/// `{room, user}`; the client reads `e.user.{id,name,photo,time}`.
/// `photo` must be the accessor-resolved URL (`storage.url(path)` or the
/// ui-avatars fallback), which `resolve_photo_url` builds from the
/// storage prefix the manager carries.
pub fn user_found_all(room: &Value, user_name: &str, user_id: i64, photo: &str, time: f64) -> Value {
    json!({
        "room": room,
        "user": {
            "name": user_name,
            "id": user_id,
            "photo": photo,
            "time": time,
        },
    })
}

/// `User::photo` accessor — `Storage::url(photo_path)` or the ui-avatars
/// initials fallback (`HasPicture::defaultPhotoUrl` for users).
pub fn resolve_photo_url(storage_prefix: &str, photo_path: Option<&str>, name: &str) -> String {
    match photo_path {
        Some(p) if !p.is_empty() => {
            format!("{}/{}", storage_prefix.trim_end_matches('/'), p)
        }
        _ => {
            let initials = name
                .split_whitespace()
                .filter_map(|seg| seg.chars().next())
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            format!(
                "https://ui-avatars.com/api/?name={}&color=7F9CF5&background=1f2937&size=300",
                url::form_urlencoded::byte_serialize(initials.trim().as_bytes())
                    .collect::<String>()
            )
        }
    }
}

/// Track answer card after reveal (`playlistPayloadForRoom` equivalent,
/// simplified: id, value, type name, score).
/// `user_voted_*` is false here — broadcast payloads can't be per-user
/// (PHP passes `$request->user()`, null in queue context). The `joined`
/// hydration post-processes these payloads with real vote state.
pub fn track_playlist_payload(track: &GameTrack) -> Value {
    json!({
        "id": track.id,
        "provider": track.provider,
        "preview_url": track.preview_url,
        "artwork_url": track.artwork_url,
        "album_name": Value::Null,
        "hint": track.hint,
        "upvotes": format_votes(track.upvotes),
        "downvotes": format_votes(track.downvotes),
        "user_voted_up": false,
        "user_voted_down": false,
        "answers": track.answers.iter().map(|a: &TrackAnswer| json!({
            "id": a.id,
            "value": a.value,
            "type": {"name": a.type_name},
            "name": a.type_name,
            "score": a.score,
        })).collect::<Vec<_>>(),
    })
}

/// `formatVoteNumbers` — ≥1000 → "X.Yk", else the raw int.
fn format_votes(n: i64) -> Value {
    if n >= 1000 {
        let k = ((n as f64 / 1000.0) * 10.0).round() / 10.0;
        Value::String(format!("{k}k"))
    } else {
        Value::from(n)
    }
}
