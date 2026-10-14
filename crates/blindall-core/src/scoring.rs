use crate::matching::{candidate_lines, evaluate_match};
use crate::sanitize::sanitize_string;
use crate::types::TrackAnswer;
use std::collections::HashSet;

pub trait ScoreBackend {
    fn found_answer_ids(&mut self, user_id: i64, track_id: i64) -> HashSet<i64>;
    fn record_track_details(
        &mut self,
        user_id: i64,
        track_id: i64,
        response_time: Option<f64>,
        score: Option<f64>,
        answer_id: Option<i64>,
    ) -> bool;
    fn players_who_found(&mut self, track_id: i64, answer_id: i64, exclude_user: i64) -> usize;
    fn add_score(&mut self, user_id: i64, score: f64);
    fn get_user_score(&mut self, user_id: i64) -> f64;
    fn count_found_for_user(&mut self, user_id: i64, track_id: i64) -> usize;
}

pub struct CheckRequest {
    pub user_id: i64,
    pub track_id: i64,
    pub text: String,
    pub client_words: Vec<String>,
    pub current_time: f64,
    pub track_duration: u32,
    pub hint: Option<String>,
    pub answers: Vec<TrackAnswer>,
    pub total_track_answers: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FoundAnswer {
    pub answer_id: i64,
    pub order: usize,
    pub speed_bonus: bool,
    pub type_name: String,
    pub svg_icon: Option<String>,
    pub value: String,
    pub score: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CheckOutcome {
    InvalidTime,
    Hint(String),
    NoHint,
    Good {
        answers: Vec<FoundAnswer>,
        total_score: f64,
        found_all: bool,
    },
    AlreadyFound,
    Almost,
    Bad,
}

/// NOTE on ordering: PHP computes `order` (playersWhoFound + 1) BEFORE
pub fn check<B: ScoreBackend>(req: &CheckRequest, backend: &mut B) -> CheckOutcome {
    let max_allowed = req.track_duration as f64 + 8.0;
    if req.current_time < 0.0 || req.current_time > max_allowed {
        return CheckOutcome::InvalidTime;
    }

    let text_lower = req.text.trim().to_lowercase();
    if text_lower == "!indice" || text_lower == "!hint" {
        return match &req.hint {
            Some(h) => CheckOutcome::Hint(h.clone()),
            None => CheckOutcome::NoHint,
        };
    }

    let speed_bonus = req.current_time < (req.track_duration as f64 * 0.18);
    let sanitized = sanitize_string(&req.text);

    let mut user_words: Vec<String> = sanitized
        .split(' ')
        .filter(|w| !w.trim().is_empty())
        .map(str::to_string)
        .collect();
    for w in &req.client_words {
        let t = w.trim();
        if !t.is_empty() && !user_words.iter().any(|u| u == t) {
            user_words.push(t.to_string());
        }
    }
    user_words.sort();
    user_words.dedup();

    let already_found = backend.found_answer_ids(req.user_id, req.track_id);
    let remaining: Vec<&TrackAnswer> = req
        .answers
        .iter()
        .filter(|a| !already_found.contains(&a.id))
        .collect();

    let mut good: Vec<FoundAnswer> = Vec::new();
    let mut duplicates = false;
    let mut almost = false;

    for answer in &remaining {
        let mut line_complete = false;
        let mut line_almost = false;

        for raw_line in candidate_lines(answer) {
            let verdict = evaluate_match(&sanitized, &user_words, &raw_line);
            if verdict.complete {
                line_complete = true;
                break;
            }
            if verdict.almost {
                line_almost = true;
            }
        }

        if line_complete {
            let order = backend.players_who_found(req.track_id, answer.id, req.user_id) + 1;

            let mut score = answer.score;
            let mut actual_speed_bonus = false;
            if answer.score > 0.0 {
                if order < 4 {
                    score += 0.5;
                }
                if speed_bonus {
                    score += 0.5;
                    actual_speed_bonus = true;
                }
            }

            let was_new = backend.record_track_details(
                req.user_id,
                req.track_id,
                Some(req.current_time),
                Some(score),
                Some(answer.id),
            );

            if !was_new {
                duplicates = true;
                continue;
            }

            backend.add_score(req.user_id, score);

            good.push(FoundAnswer {
                answer_id: answer.id,
                order,
                speed_bonus: actual_speed_bonus,
                type_name: answer.type_name.clone(),
                svg_icon: answer.svg_icon.clone(),
                value: answer.value.clone(),
                score,
            });
        } else if line_almost {
            almost = true;
        }
    }

    if !good.is_empty() {
        let total_score = backend.get_user_score(req.user_id);
        let found_all = backend.count_found_for_user(req.user_id, req.track_id)
            == req.total_track_answers;
        return CheckOutcome::Good {
            answers: good,
            total_score,
            found_all,
        };
    }
    if duplicates {
        return CheckOutcome::AlreadyFound;
    }
    if almost {
        return CheckOutcome::Almost;
    }
    CheckOutcome::Bad
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    struct MemBackend {
        found: HashMap<(i64, i64), HashSet<i64>>,   // (user,track) -> answer ids
        answer_players: HashMap<i64, HashSet<i64>>, // answer_id -> user ids
        totals: HashMap<i64, f64>,                  // user -> total
        stale_reads: bool,
    }

    impl MemBackend {
        fn new() -> Self {
            Self {
                found: HashMap::new(),
                answer_players: HashMap::new(),
                totals: HashMap::new(),
                stale_reads: false,
            }
        }
    }

    impl ScoreBackend for MemBackend {
        fn found_answer_ids(&mut self, u: i64, t: i64) -> HashSet<i64> {
            if self.stale_reads {
                return HashSet::new();
            }
            self.found.get(&(u, t)).cloned().unwrap_or_default()
        }
        fn record_track_details(
            &mut self,
            u: i64,
            t: i64,
            _rt: Option<f64>,
            _s: Option<f64>,
            a: Option<i64>,
        ) -> bool {
            if let Some(aid) = a {
                let set = self.found.entry((u, t)).or_default();
                if !set.insert(aid) {
                    return false;
                }
                self.answer_players.entry(aid).or_default().insert(u);
            }
            true
        }
        fn players_who_found(&mut self, _t: i64, a: i64, exclude: i64) -> usize {
            let mut set = self.answer_players.get(&a).cloned().unwrap_or_default();
            set.remove(&exclude);
            set.len()
        }
        fn add_score(&mut self, u: i64, s: f64) {
            *self.totals.entry(u).or_insert(0.0) += s;
        }
        fn get_user_score(&mut self, u: i64) -> f64 {
            self.totals.get(&u).copied().unwrap_or(0.0)
        }
        fn count_found_for_user(&mut self, u: i64, t: i64) -> usize {
            self.found.get(&(u, t)).map(|s| s.len()).unwrap_or(0)
        }
    }

    fn answers() -> Vec<TrackAnswer> {
        vec![
            TrackAnswer {
                id: 1,
                type_name: "Titre".into(),
                svg_icon: None,
                value: "Billie Jean".into(),
                aliases: vec![],
                score: 1.0,
            },
            TrackAnswer {
                id: 2,
                type_name: "Artiste".into(),
                svg_icon: None,
                value: "Michael Jackson".into(),
                aliases: vec!["MJ".into()],
                score: 0.5,
            },
        ]
    }

    fn req(text: &str, time: f64) -> CheckRequest {
        CheckRequest {
            user_id: 7,
            track_id: 42,
            text: text.into(),
            client_words: vec![],
            current_time: time,
            track_duration: 30,
            hint: None,
            answers: answers(),
            total_track_answers: 2,
        }
    }

    #[test]
    fn good_first_answer_with_both_bonuses() {
        let mut b = MemBackend::new();
        match check(&req("billie jean", 4.0), &mut b) {
            CheckOutcome::Good {
                answers,
                total_score,
                found_all,
            } => {
                assert_eq!(answers.len(), 1);
                assert_eq!(answers[0].score, 2.0);
                assert!(answers[0].speed_bonus);
                assert_eq!(answers[0].order, 1);
                assert_eq!(total_score, 2.0);
                assert!(!found_all);
            }
            other => panic!("expected Good, got {other:?}"),
        }
    }

    #[test]
    fn finding_everything_sets_found_all() {
        let mut b = MemBackend::new();
        match check(&req("billie jean michael jackson", 10.0), &mut b) {
            CheckOutcome::Good { answers, found_all, .. } => {
                assert_eq!(answers.len(), 2);
                assert!(found_all);
            }
            other => panic!("expected Good, got {other:?}"),
        }
    }

    #[test]
    fn resubmit_of_found_answer_is_bad() {
        let mut b = MemBackend::new();
        check(&req("billie jean", 4.0), &mut b);
        assert_eq!(check(&req("billie jean", 6.0), &mut b), CheckOutcome::Bad);
    }

    #[test]
    fn concurrent_submit_is_already_found() {
        let mut b = MemBackend::new();
        check(&req("billie jean", 4.0), &mut b);
        b.stale_reads = true;
        assert_eq!(check(&req("billie jean", 6.0), &mut b), CheckOutcome::AlreadyFound);
    }

    #[test]
    fn second_player_gets_order_two_and_bonus() {
        let mut b = MemBackend::new();
        check(&req("billie jean", 4.0), &mut b); // user 7 first
        let mut r = req("billie jean", 4.0);
        r.user_id = 9;
        match check(&r, &mut b) {
            CheckOutcome::Good { answers, .. } => {
                assert_eq!(answers[0].order, 2);
                assert_eq!(answers[0].score, 2.0); // still <4 order bonus
            }
            other => panic!("expected Good, got {other:?}"),
        }
    }

    #[test]
    fn late_answer_loses_speed_bonus() {
        let mut b = MemBackend::new();
        match check(&req("billie jean", 20.0), &mut b) {
            CheckOutcome::Good { answers, .. } => {
                assert_eq!(answers[0].score, 1.5); // order bonus only
                assert!(!answers[0].speed_bonus);
            }
            other => panic!("expected Good, got {other:?}"),
        }
    }

    #[test]
    fn hint_commands() {
        let mut b = MemBackend::new();
        let mut r = req("!indice", 1.0);
        assert_eq!(check(&r, &mut b), CheckOutcome::NoHint);
        r.hint = Some("Chanteur des années 80".into());
        assert_eq!(
            check(&r, &mut b),
            CheckOutcome::Hint("Chanteur des années 80".into())
        );
    }

    #[test]
    fn invalid_time_rejected() {
        let mut b = MemBackend::new();
        assert_eq!(check(&req("billie jean", 39.0), &mut b), CheckOutcome::InvalidTime);
        assert_eq!(check(&req("billie jean", -1.0), &mut b), CheckOutcome::InvalidTime);
        assert!(matches!(
            check(&req("billie jean", 38.0), &mut b),
            CheckOutcome::Good { .. }
        ));
    }

    #[test]
    fn almost_and_bad() {
        let mut b = MemBackend::new();
        assert_eq!(check(&req("michael", 10.0), &mut b), CheckOutcome::Almost);
        assert_eq!(check(&req("xyz", 10.0), &mut b), CheckOutcome::Bad);
    }

    #[test]
    fn alias_matches() {
        let mut b = MemBackend::new();
        match check(&req("mj", 10.0), &mut b) {
            CheckOutcome::Good { answers, .. } => assert_eq!(answers[0].answer_id, 2),
            other => panic!("expected Good via alias, got {other:?}"),
        }
    }

    #[test]
    fn zero_score_answer_gets_no_bonus() {
        let mut b = MemBackend::new();
        let mut r = req("billie jean", 2.0);
        r.answers = vec![TrackAnswer {
            id: 5,
            type_name: "Featuring".into(),
            svg_icon: None,
            value: "(feat. X)".into(),
            aliases: vec![],
            score: 0.0,
        }];
        r.total_track_answers = 1;
        match check(&r, &mut b) {
            CheckOutcome::Good { answers, .. } => {
                assert_eq!(answers[0].score, 0.0);
                assert!(!answers[0].speed_bonus);
            }
            other => panic!("expected Good, got {other:?}"),
        }
    }
}
