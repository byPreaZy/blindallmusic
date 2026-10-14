//! Port of `RoundController::candidateLinesForTrackAnswer` and
//! `RoundController::evaluateMatchForAnswerLine`.
//!
//! All comparisons happen on *sanitized* strings for the answer side; user
//! words are only trimmed (same as PHP — they may stay multi-byte UTF-8,
//! which is why distances use the byte-level `php_levenshtein`).

use crate::levenshtein::php_levenshtein;
use crate::sanitize::sanitize_string;
use crate::types::TrackAnswer;
use once_cell::sync::Lazy;
use regex::Regex;

/// PHP `is_numeric()`: decimal or exponent form, optional sign. NOT Rust's
/// `f64::parse` (which accepts "inf", "nan"…). Hex rejected, as in PHP ≥ 7.
static NUMERIC_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[+-]?((\d+(\.\d*)?)|(\.\d+))([eE][+-]?\d+)?$").unwrap());

fn php_is_numeric(s: &str) -> bool {
    NUMERIC_RE.is_match(s.trim())
}

/// Candidate match lines for one answer: `value` + each non-empty trimmed
/// alias, deduplicated by lowercase key (order preserved).
pub fn candidate_lines(answer: &TrackAnswer) -> Vec<String> {
    let mut lines = vec![answer.value.trim().to_string()];

    for alias in &answer.aliases {
        let trimmed = alias.trim();
        if !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
    }

    let mut seen = std::collections::HashSet::new();
    let mut unique = Vec::with_capacity(lines.len());
    for line in lines {
        if seen.insert(line.to_lowercase()) {
            unique.push(line);
        }
    }
    unique
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MatchVerdict {
    pub complete: bool,
    pub almost: bool,
}

/// Port of `evaluateMatchForAnswerLine`.
///
/// - `sanitized`: `sanitize_string(user_input)` once per request.
/// - `user_words`: union of sanitized input words + client `words` (trimmed).
/// - `raw_line`: one candidate line (raw, will be sanitized inside).
pub fn evaluate_match(sanitized: &str, user_words: &[String], raw_line: &str) -> MatchVerdict {
    let trimmed_raw = raw_line.trim();
    let value = sanitize_string(trimmed_raw);

    // Value only inside parentheses (stripped by sanitize) — e.g. 0pt
    // "Featuring" hints. Historically any single letter matched via
    // levenshtein(sanitized, '') < 3. Parity preserved.
    if value.is_empty() && !trimmed_raw.is_empty() {
        return MatchVerdict {
            complete: !sanitized.is_empty() || !user_words.is_empty(),
            almost: false,
        };
    }
    if value.is_empty() {
        return MatchVerdict::default();
    }

    let answer_words: Vec<String> = value
        .split(' ')
        .filter(|w| !w.is_empty())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .map(str::to_string)
        .collect();

    if answer_words.is_empty() {
        return MatchVerdict::default();
    }

    let mut good_words: Vec<String>;

    if sanitized == value {
        good_words = answer_words.clone();
    } else if php_levenshtein(sanitized, &value) < 3 {
        let mut can_accept_levenshtein = true;

        // Numeric words must all be present verbatim in user words.
        for word in &answer_words {
            if php_is_numeric(word) {
                let found = user_words
                    .iter()
                    .any(|uw| php_is_numeric(uw) && uw == word);
                if !found {
                    can_accept_levenshtein = false;
                    break;
                }
            }
        }

        if can_accept_levenshtein {
            good_words = answer_words.clone();
        } else {
            good_words = per_word_matches(&answer_words, user_words);
        }
    } else {
        good_words = per_word_matches(&answer_words, user_words);
    }

    good_words.sort();
    good_words.dedup();

    let complete = answer_words.len() == good_words.len();
    let almost = !complete && (good_words.len() as f64) >= (answer_words.len() as f64 / 2.0);

    MatchVerdict { complete, almost }
}

/// Port of the duplicated per-word matching loop in the PHP method:
/// numeric pairs must be exactly equal; short user words (< 5 bytes) need
/// exact equality; longer ones tolerate levenshtein < 1.55 (i.e. ≤ 1).
fn per_word_matches(answer_words: &[String], user_words: &[String]) -> Vec<String> {
    let mut good = Vec::new();

    for word in answer_words {
        for user_word in user_words {
            let uw_numeric = php_is_numeric(user_word);
            let w_numeric = php_is_numeric(word);

            if uw_numeric && w_numeric {
                if user_word == word {
                    good.push(word.clone());
                }
            } else if user_word.len() < 5 {
                // PHP strlen() = byte length — `.len()` is parity.
                if user_word == word {
                    good.push(word.clone());
                }
            } else if (php_levenshtein(user_word, word) as f64) < 1.55 {
                good.push(word.clone());
            }
        }
    }

    good
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn exact_sanitized_match() {
        let v = evaluate_match("billie jean", &words(&["billie", "jean"]), "Billie Jean");
        assert!(v.complete && !v.almost);
    }

    #[test]
    fn levenshtein_under_3_accepts() {
        // "bili jean" -> distance 1 to "billie jean"? actually distance 2 — still < 3.
        let v = evaluate_match("bili jean", &words(&["bili", "jean"]), "Billie Jean");
        assert!(v.complete);
    }

    #[test]
    fn typo_per_word() {
        // "thriler" (len 7) vs "thriller": levenshtein 1 < 1.55 -> good word.
        let v = evaluate_match("thriler michael jackson", &words(&["thriler", "michael", "jackson"]), "Thriller Michael Jackson");
        assert!(v.complete);
    }

    #[test]
    fn short_words_need_exact() {
        // When the global levenshtein is ≥3, per-word rules apply: "x"
        // (len<5) is not exactly "bad" → only "romance" found → almost.
        let v = evaluate_match("x romance", &words(&["x", "romance"]), "Bad Romance");
        assert!(!v.complete && v.almost);
        // But a global levenshtein <3 accepts the whole line first (PHP
        // checks it BEFORE the per-word pass) — "bda romance" completes.
        let v = evaluate_match("bda romance", &words(&["bda", "romance"]), "Bad Romance");
        assert!(v.complete);
    }

    #[test]
    fn almost_when_half_words_found() {
        // 4 answer words, 2 found -> almost, not complete.
        let v = evaluate_match("red hot peppers", &words(&["red", "hot", "peppers"]), "Red Hot Chili Peppers");
        assert!(!v.complete && v.almost);
    }

    #[test]
    fn numeric_words_must_be_verbatim() {
        // Levenshtein("1999", "1998") = 1 < 3 globally, but numeric words
        // must appear verbatim — "1998" in input does NOT satisfy "1999".
        let v = evaluate_match("prince 1998", &words(&["prince", "1998"]), "Prince 1999");
        assert!(!v.complete);
        let v = evaluate_match("prince 1999", &words(&["prince", "1999"]), "Prince 1999");
        assert!(v.complete);
    }

    #[test]
    fn empty_parenthesized_value_matches_anything_nonempty() {
        // "(Featuring X)" -> sanitized "" -> any non-empty input completes.
        let v = evaluate_match("anything", &words(&["anything"]), "(Featuring X)");
        assert!(v.complete && !v.almost);
        let v = evaluate_match("", &[], "(Featuring X)");
        assert!(!v.complete);
    }

    #[test]
    fn candidate_lines_dedup_lowercase() {
        let answer = TrackAnswer {
            id: 1,
            type_name: "Titre".into(),
            svg_icon: None,
            value: "Hello".into(),
            aliases: vec!["hello".into(), "  HELLO ".into(), "Hi".into(), "".into()],
            score: 0.5,
        };
        assert_eq!(candidate_lines(&answer), vec!["Hello".to_string(), "Hi".to_string()]);
    }
}
