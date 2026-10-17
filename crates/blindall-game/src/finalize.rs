use crate::events;
use crate::store::{GameStore, RoomConfig, StoreError};
use blindall_core::elo::{self, EloPlayerInput};
use blindall_core::metrics;
use blindall_core::types::{PodiumEntry, Standing, TrackHistoryEntry};
use crate::store::{PresenceStore, ScoreStore};
use blindall_realtime::Hub;
use chrono::Utc;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;

pub fn spawn(
    round_id: i64,
    room: RoomConfig,
    store: Arc<dyn GameStore>,
    scores: Arc<dyn ScoreStore>,
    hub: Hub,
    presence: Arc<dyn PresenceStore>,
    photo_prefix: String,
) {
    tokio::spawn(async move {
        if let Err(e) = finalize(round_id, &room, &*store, &scores, &hub, &presence, &photo_prefix).await {
            tracing::error!(round_id, error = %e, "round finalization failed");
        }
    });
}

#[allow(clippy::too_many_arguments)]
async fn finalize(
    round_id: i64,
    room: &RoomConfig,
    store: &dyn GameStore,
    scores_store: &Arc<dyn ScoreStore>,
    hub: &Hub,
    presence: &Arc<dyn PresenceStore>,
    photo_prefix: &str,
) -> crate::store::Result<()> {
    if store.round_has_standings(round_id).await? {
        tracing::info!(round_id, "finalization skipped : standings exist");
        return Ok(());
    }

    let redis_scores = scores_store.all_scores(round_id).await;
    let (scores, mut tracks_history, from_redis) = if !redis_scores.is_empty() {
        let history = scores_store
            .all_tracks_history(round_id)
            .await
            .into_iter()
            .collect::<HashMap<i64, Vec<_>>>()
            .into_iter()
            .map(|(u, v)| {
                (
                    u,
                    v.into_iter()
                        .map(|r| TrackHistoryEntry {
                            track_id: r.track_id,
                            answer_id: r.answer_id,
                            response_time: r.response_time,
                            position: r.position,
                            score: r.score,
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        (
            redis_scores.into_iter().collect::<HashMap<i64, f64>>(),
            history,
            true,
        )
    } else {
        (
            store.scores_from_db(round_id).await?,
            store.tracks_history_from_db(round_id).await?,
            false,
        )
    };

    if scores.is_empty() {
        tracing::info!(round_id, "no scores to finalize");
        return Ok(());
    }

    let user_ids: Vec<i64> = scores.keys().copied().collect();
    let users = store.players_info(&user_ids).await?;

    let mut podium: Vec<PodiumEntry> = scores
        .iter()
        .map(|(&uid, &total)| PodiumEntry {
            user_id: uid,
            total,
            team_id: users.get(&uid).and_then(|p| p.team_id),
        })
        .collect();
    podium.sort_by(|a, b| b.total.partial_cmp(&a.total).unwrap_or(std::cmp::Ordering::Equal));

    calculate_track_positions(&mut tracks_history);

    let round_row = store.round_row(round_id).await?.ok_or(StoreError::NotFound("round"))?;
    if round_row.finished_at.is_none() {
        tracing::warn!(round_id, "finalization skipped : round not finished");
        return Ok(());
    }
    let total_tracks = round_row.tracks.len();

    let track_ids: Vec<i64> = tracks_history
        .values()
        .flat_map(|h| h.iter().map(|e| e.track_id))
        .collect();
    let required = store.required_answers_per_track(&track_ids).await?;
    let threshold = metrics::speed_bonus_threshold(room.track_duration);

    let mut players = HashMap::new();
    for (idx, entry) in podium.iter().enumerate() {
        let uid = entry.user_id;
        let Some(info) = users.get(&uid) else {
            continue;
        };
        let history = tracks_history.get(&uid).cloned().unwrap_or_default();
        let tracks_played = history
            .iter()
            .map(|e| e.track_id)
            .collect::<std::collections::HashSet<_>>()
            .len();
        let m = metrics::from_tracks_history(&history, &required, threshold);
        let win_streak = if idx == 0 {
            store.win_streak(room.id, uid, round_id).await.unwrap_or(0)
        } else {
            0
        };
        players.insert(
            uid,
            EloPlayerInput {
                user_id: uid,
                elo: info.elo,
                is_guest: info.is_guest,
                team_id: info.team_id,
                tracks_played,
                placement_rounds_played: store
                    .placement_rounds_played(uid)
                    .await
                    .unwrap_or(0) as usize,
                win_streak,
                tracks_history: history,
                average_response_time: m.average_response_time,
                fast_answers_count: m.fast_answers_count,
                total_answers_count: m.total_answers_count,
            },
        );
    }

    let elo::EloRoundResult {
        standings,
        elo_updates,
    } = elo::update_elos_for_round(
        round_id,
        room.id,
        room.is_public,
        &podium,
        total_tracks.max(1),
        &players,
    );

    if standings.is_empty() {
        tracing::warn!(round_id, "no standings created");
        return Ok(());
    }

    store.write_standings(&standings, &elo_updates).await?;

    for (uid, score) in &scores {
        let Some(info) = users.get(uid) else {
            continue;
        };
        store
            .bump_total_score(room.id, *uid, info.team_id, *score)
            .await?;
        if room.is_public {
            match store.recalc_user_level(*uid).await {
                Ok(Some(payload)) => {
                    hub.broadcast(
                        &format!("user:{}", uid),
                        "UserLevelUpdated",
                        payload,
                        None,
                    )
                    .await;
                }
                Ok(None) => {}
                Err(e) => tracing::warn!(user_id = uid, error = %e, "level recalc failed"),
            }
        }
    }

    if from_redis {
        let _ = scores_store.cleanup(round_id).await;
    }
    if !from_redis || store.has_legacy_scores(round_id).await.unwrap_or(false) {
        let _ = store.delete_round_scores(round_id).await;
    }

    let room_v = json!({
        "id": room.id,
        "is_public": room.is_public,
        "tracks_by_round": room.tracks_by_round,
        "track_duration": room.track_duration,
        "pause_between_tracks": room.pause_between_tracks,
        "pause_between_rounds": room.pause_between_rounds,
        "is_playing": false,
    });
    let channel = events::room_channel(room.id);
    for (uid, elo_after) in &elo_updates {
        hub.broadcast(
            &channel,
            "UserEloUpdated",
            json!({
                "user": podium_user_json(*uid, &users, photo_prefix),
                "room": room_v,
                "elo": elo_after,
            }),
            None,
        )
        .await;
    }

    hub.broadcast(
        &channel,
        "RoundFinished",
        events::round_finished(
            &round_row,
            &room_v,
            users_podium_json(&standings, &users, photo_prefix),
            teams_podium_json(&standings, &users, photo_prefix),
        ),
        None,
    )
    .await;

    let count = presence
        .member_count(room.id, Utc::now().timestamp())
        .await;
    hub.broadcast(
        &events::public_channel(room.id),
        "RoomPublicState",
        events::room_public_state(room, count, 0, false),
        None,
    )
    .await;

    Ok(())
}

fn calculate_track_positions(history: &mut HashMap<i64, Vec<TrackHistoryEntry>>) {
    let mut by_track: HashMap<i64, Vec<(i64, f64, Option<f64>)>> = HashMap::new();
    for (&uid, entries) in history.iter() {
        for e in entries {
            by_track.entry(e.track_id).or_default().push((
                uid,
                e.score.unwrap_or(0.0),
                e.response_time,
            ));
        }
    }
    for (track_id, mut players) in by_track {
        players.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(match (a.2, b.2) {
                    (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
                    _ => std::cmp::Ordering::Equal,
                })
        });
        for (pos, (uid, _, _)) in players.iter().enumerate() {
            if let Some(entries) = history.get_mut(uid) {
                for e in entries.iter_mut().filter(|e| e.track_id == track_id) {
                    e.position = Some(pos as i64 + 1);
                }
            }
        }
    }
}

fn podium_user_json(
    uid: i64,
    users: &HashMap<i64, crate::store::PlayerInfo>,
    photo_prefix: &str,
) -> serde_json::Value {
    match users.get(&uid) {
        Some(p) => json!({
            "id": p.id,
            "name": p.name,
            "photo": events::resolve_photo_url(photo_prefix, p.photo.as_deref(), &p.name),
            "elo": p.elo,
            "role": if p.is_administrator { "admin" } else if p.is_moderator { "moderator" } else { "" },
            "custom_tag": p.custom_tag.as_deref().unwrap_or(""),
        }),
        None => json!({ "id": uid }),
    }
}

fn users_podium_json(
    standings: &[Standing],
    users: &HashMap<i64, crate::store::PlayerInfo>,
    photo_prefix: &str,
) -> serde_json::Value {
    serde_json::Value::Array(
        standings
            .iter()
            .map(|s| {
                json!({
                    "user_id": s.user_id,
                    "total": s.total_score,
                    "team_id": s.team_id,
                    "user": podium_user_json(s.user_id, users, photo_prefix),
                })
            })
            .collect(),
    )
}

fn teams_podium_json(
    standings: &[Standing],
    users: &HashMap<i64, crate::store::PlayerInfo>,
    photo_prefix: &str,
) -> serde_json::Value {
    let mut totals: HashMap<i64, (f64, Option<String>, Option<String>)> = HashMap::new();
    for s in standings {
        if let Some(t) = s.team_id {
            let entry = totals.entry(t).or_insert((0.0, None, None));
            entry.0 += s.total_score;
            if entry.1.is_none() {
                if let Some(u) = users.values().find(|u| u.team_id == Some(t)) {
                    entry.1 = u.team_name.clone();
                    entry.2 = u.team_photo.clone();
                }
            }
        }
    }
    let mut v: Vec<_> = totals.into_iter().collect();
    v.sort_by(|a, b| b.1 .0.partial_cmp(&a.1 .0).unwrap_or(std::cmp::Ordering::Equal));
    serde_json::Value::Array(
        v.into_iter()
            .map(|(team_id, (total, name, photo_path))| {
                let name = name.unwrap_or_default();
                json!({
                    "team_id": team_id,
                    "total": total,
                    "team": {
                        "id": team_id,
                        "name": name,
                        "photo": events::resolve_photo_url(photo_prefix, photo_path.as_deref(), &name),
                    },
                })
            })
            .collect(),
    )
}
