use crate::types::{PodiumEntry, Standing, TrackHistoryEntry};
use std::collections::HashMap;

pub const PLACEMENT_ROUNDS: usize = 10;
pub const INITIAL_ELO: i64 = 1500;
pub const TARGET_AVERAGE_ELO: i64 = 1500;
pub const MIN_ELO: i64 = 100;
pub const MIN_TRACKS_PLAYED_PERCENTAGE: f64 = 0.8;

pub fn calculate_elo_change(
    user_elo: i64,
    opponent_elos: &[i64],
    position: usize,
    total_players: usize,
    placement_rounds_played: usize,
) -> i64 {
    if opponent_elos.is_empty() || total_players == 0 {
        return 0;
    }

    let actual = actual_score(position, total_players);
    let expected = opponent_elos
        .iter()
        .map(|&o| expected_score(user_elo, o as f64))
        .sum::<f64>()
        / opponent_elos.len() as f64;
    let k = k_factor(user_elo, placement_rounds_played);

    (k * (actual - expected)).round() as i64
}

fn k_factor(user_elo: i64, placement_rounds_played: usize) -> f64 {
    if placement_rounds_played < PLACEMENT_ROUNDS {
        let placement_k = 50.0 - (placement_rounds_played as f64 * 1.8);
        return placement_k.max(32.0);
    }
    if user_elo < 1200 {
        40.0
    } else if user_elo < 1600 {
        32.0
    } else if user_elo < 2000 {
        24.0
    } else {
        16.0
    }
}

// Multiplayer Elo : linear rank normalisation over [0,1] (1st -> 1.0,
// last -> 0.0). Sum of actuals over a field = N/2 = sum of expected
// pairwise scores for equal ratings, so the system is zero-sum instead
// of inflating +0.5*K every round like the old (pos+1)/N mapping did.
fn actual_score(position: usize, total_players: usize) -> f64 {
    if total_players <= 1 {
        return 0.5;
    }
    (total_players - position) as f64 / (total_players - 1) as f64
}

fn expected_score(user_elo: i64, opponent_elo: f64) -> f64 {
    1.0 / (1.0 + 10f64.powf((opponent_elo - user_elo as f64) / 400.0))
}

pub fn apply_inflation_protection(elo_change: i64, user_elo: i64) -> i64 {
    let mut change = elo_change as f64;
    if user_elo > 2000 && elo_change > 0 {
        change *= 0.9;
    }
    if user_elo < 1000 && elo_change < 0 {
        change *= 0.9;
    }
    change.round() as i64
}

#[derive(Debug, Clone)]
pub struct EloPlayerInput {
    pub user_id: i64,
    pub elo: i64,
    pub is_guest: bool,
    pub team_id: Option<i64>,
    pub tracks_played: usize,
    pub placement_rounds_played: usize,
    pub win_streak: i64,
    pub tracks_history: Vec<TrackHistoryEntry>,
    pub average_response_time: Option<f64>,
    pub fast_answers_count: i64,
    pub total_answers_count: i64,
}

pub struct EloRoundResult {
    pub standings: Vec<Standing>,
    pub elo_updates: Vec<(i64, i64)>,
}

pub fn update_elos_for_round(
    round_id: i64,
    room_id: i64,
    room_is_public: bool,
    podium: &[PodiumEntry],
    total_tracks_in_round: usize,
    players: &HashMap<i64, EloPlayerInput>,
) -> EloRoundResult {
    let mut elo_updates = Vec::new();

    if podium.is_empty() {
        return EloRoundResult {
            standings: vec![],
            elo_updates,
        };
    }

    let valid_user_ids: Vec<i64> = podium
        .iter()
        .filter(|p| {
            let input = players.get(&p.user_id);
            let pct = participation_pct(
                input.map(|i| i.tracks_played).unwrap_or(0),
                total_tracks_in_round,
            );
            pct >= MIN_TRACKS_PLAYED_PERCENTAGE && !input.map(|i| i.is_guest).unwrap_or(false)
        })
        .map(|p| p.user_id)
        .collect();

    let is_elo_counted = room_is_public && podium.len() >= 3 && valid_user_ids.len() >= 3;

    let valid_sorted: Vec<i64> = podium
        .iter()
        .filter(|p| valid_user_ids.contains(&p.user_id))
        .map(|p| p.user_id)
        .collect();

    let mut standings = Vec::with_capacity(podium.len());

    for (index, entry) in podium.iter().enumerate() {
        let user_id = entry.user_id;
        let Some(input) = players.get(&user_id) else {
            continue;
        };

        let position = (index + 1) as i64;
        let user_elo = input.elo;
        let pct = participation_pct(input.tracks_played, total_tracks_in_round);
        let counted = is_elo_counted && pct >= MIN_TRACKS_PLAYED_PERCENTAGE && !input.is_guest;

        let (elo_change, elo_after) = if counted {
            let opponents: Vec<i64> = valid_sorted
                .iter()
                .filter(|&&id| id != user_id)
                .map(|&id| players.get(&id).map(|p| p.elo).unwrap_or(INITIAL_ELO))
                .collect();
            let valid_position = valid_sorted
                .iter()
                .position(|&id| id == user_id)
                .map(|p| p + 1)
                .unwrap_or(position as usize);

            let mut change = calculate_elo_change(
                user_elo,
                &opponents,
                valid_position,
                valid_sorted.len(),
                input.placement_rounds_played,
            );
            change = apply_inflation_protection(change, user_elo);
            change = (change as f64 * pct).round() as i64;
            let after = (user_elo + change).max(MIN_ELO);
            (after - user_elo, after)
        } else {
            (0, user_elo)
        };

        if counted {
            elo_updates.push((user_id, elo_after));
        }

        standings.push(Standing {
            round_id,
            room_id,
            user_id,
            team_id: entry.team_id.or(input.team_id),
            position,
            total_score: entry.total,
            elo_before: user_elo,
            elo_after,
            elo_change,
            is_elo_counted: counted,
            average_response_time: input.average_response_time,
            fast_answers_count: input.fast_answers_count,
            total_answers_count: input.total_answers_count,
            tracks_history: input.tracks_history.clone(),
            win_streak: input.win_streak,
        });
    }

    EloRoundResult {
        standings,
        elo_updates,
    }
}

fn participation_pct(tracks_played: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        tracks_played as f64 / total as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actual_score_positions() {
        assert_eq!(actual_score(1, 4), 1.0);
        assert_eq!(actual_score(4, 4), 0.0);
        assert!((actual_score(2, 4) - 2.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn actual_scores_are_zero_sum_for_equal_elos() {
        let opp = [1500, 1500, 1500];
        let sum: i64 = (1..=4)
            .map(|pos| calculate_elo_change(1500, &opp, pos, 4, 10))
            .sum();
        assert_eq!(sum, 0);
    }

    #[test]
    fn k_factor_bands() {
        assert_eq!(k_factor(1500, 0), 50.0);
        assert_eq!(k_factor(1500, 10), 32.0); // placements done, mid ELO
        assert_eq!(k_factor(1100, 10), 40.0);
        assert_eq!(k_factor(2100, 10), 16.0);
    }

    #[test]
    fn winner_gains_loser_loses() {
        let opp = [1500, 1500, 1500];
        let win = calculate_elo_change(1500, &opp, 1, 4, 10);
        let lose = calculate_elo_change(1500, &opp, 4, 4, 10);
        assert!(win > 0 && lose < 0);
    }

    #[test]
    fn symmetric_values() {
        let opp = [1500, 1500, 1500];
        assert_eq!(calculate_elo_change(1500, &opp, 1, 4, 10), 16);
        assert_eq!(calculate_elo_change(1500, &opp, 4, 4, 10), -16);
    }

    #[test]
    fn favorite_loses_big_underdog_wins_big() {
        let weak = [1400, 1400, 1400];
        let strong = [1800, 1800, 1800];
        let fav_win = calculate_elo_change(1800, &weak, 1, 4, 10);
        let fav_lose = calculate_elo_change(1800, &weak, 4, 4, 10);
        let dog_win = calculate_elo_change(1400, &strong, 1, 4, 10);
        assert!(fav_win < dog_win);
        // Expected-score gap is symmetric (0.909 vs 0.091); the underdog's
        // bigger K (32 vs 24 at 1800) makes his win worth more in absolute
        // terms. The meaningful invariant: the favorite's downside dwarfs
        // his upside — he can lose ~22 but only gain ~2.
        assert!(fav_lose.abs() > fav_win * 5);
    }

    #[test]
    fn inflation_protection_dampens() {
        assert_eq!(apply_inflation_protection(100, 2100), 90);
        assert_eq!(apply_inflation_protection(-100, 900), -90);
        assert_eq!(apply_inflation_protection(100, 1500), 100);
    }
}
