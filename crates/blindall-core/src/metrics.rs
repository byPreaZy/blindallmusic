//! Port of `RoundPerformanceMetricsService` (pure parts).

use crate::types::{PerformanceMetrics, TrackHistoryEntry};
use std::collections::{HashMap, HashSet};

/// `speedBonusThresholdForRoom`: `room.track_duration * 0.18`.
pub fn speed_bonus_threshold(track_duration: u32) -> f64 {
    track_duration as f64 * 0.18
}

/// `trackIdsFromRound` — int-cast, filter 0, unique.
pub fn track_ids_from_round(tracks: &[i64]) -> Vec<i64> {
    tracks
        .iter()
        .copied()
        .filter(|id| *id != 0)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect()
}

/// `fromTracksHistory`: completion time per track = max response_time of its
/// found answers; a track counts as "completed" only when all required
/// answers were found. Average over completed tracks; fast = under threshold.
pub fn from_tracks_history(
    history: &[TrackHistoryEntry],
    required_answers_per_track: &HashMap<i64, usize>,
    speed_bonus_threshold: f64,
) -> PerformanceMetrics {
    if history.is_empty() {
        return PerformanceMetrics::default();
    }

    struct Acc {
        answer_ids: HashSet<i64>,
        max_response_time: Option<f64>,
    }

    let mut by_track: HashMap<i64, Acc> = HashMap::new();
    for entry in history {
        if entry.track_id == 0 {
            continue;
        }
        let acc = by_track.entry(entry.track_id).or_insert_with(|| Acc {
            answer_ids: HashSet::new(),
            max_response_time: None,
        });
        if let Some(aid) = entry.answer_id {
            acc.answer_ids.insert(aid);
        }
        if let Some(rt) = entry.response_time {
            acc.max_response_time = Some(acc.max_response_time.map_or(rt, |m| m.max(rt)));
        }
    }

    let mut completion_times: Vec<f64> = Vec::new();
    let mut total_answers = 0i64;

    for (track_id, acc) in &by_track {
        let found = acc.answer_ids.len();
        total_answers += found as i64;
        let required = required_answers_per_track.get(track_id).copied().unwrap_or(0);
        if required == 0 || found < required {
            continue;
        }
        if let Some(max_rt) = acc.max_response_time {
            completion_times.push(max_rt);
        }
    }

    let average = if completion_times.is_empty() {
        None
    } else {
        let avg = completion_times.iter().sum::<f64>() / completion_times.len() as f64;
        Some((avg * 1000.0).round() / 1000.0) // PHP round($x, 3)
    };

    PerformanceMetrics {
        average_response_time: average,
        fast_answers_count: completion_times
            .iter()
            .filter(|&&t| t < speed_bonus_threshold)
            .count() as i64,
        total_answers_count: total_answers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_tracks_only() {
        let history = vec![
            TrackHistoryEntry {
                track_id: 1,
                answer_id: Some(10),
                response_time: Some(3.0),
                ..Default::default()
            },
            TrackHistoryEntry {
                track_id: 1,
                answer_id: Some(11),
                response_time: Some(9.0),
                ..Default::default()
            },
            // track 2 incomplete: 1/2 answers
            TrackHistoryEntry {
                track_id: 2,
                answer_id: Some(20),
                response_time: Some(1.0),
                ..Default::default()
            },
        ];
        let required = HashMap::from([(1i64, 2usize), (2i64, 2usize)]);
        let m = from_tracks_history(&history, &required, 5.4);
        assert_eq!(m.average_response_time, Some(9.0)); // max rt of completed
        assert_eq!(m.fast_answers_count, 0); // 9.0 > 5.4
        assert_eq!(m.total_answers_count, 3);
    }

    #[test]
    fn fast_answers() {
        let history = vec![TrackHistoryEntry {
            track_id: 1,
            answer_id: Some(10),
            response_time: Some(2.0),
            ..Default::default()
        }];
        let required = HashMap::from([(1i64, 1usize)]);
        let m = from_tracks_history(&history, &required, 5.4);
        assert_eq!(m.fast_answers_count, 1);
    }
}
