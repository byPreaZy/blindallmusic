use crate::levenshtein::php_levenshtein;
use crate::sanitize::sanitize_string;
use crate::types::TrackAnswer;
use once_cell::sync::Lazy;
use regex::Regex;

static NUMERIC_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[+-]?((\d+(\.\d*)?)|(\.\d+))([eE][+-]?\d+)?$").unwrap());

fn php_is_numeric(s: &str) -> bool {
    NUMERIC_RE.is_match(s.trim())
}

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

pub fn evaluate_match(sanitized: &str, user_words: &[String], raw_line: &str) -> MatchVerdict {
    let trimmed_raw = raw_line.trim();
    let value = sanitize_string(trimmed_raw);

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

        for word in &answer_words {
            if php_is_numeric(word) {
                let found = user_words.iter().any(|uw| php_is_numeric(uw) && uw == word);
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
        let v = evaluate_match("bili jean", &words(&["bili", "jean"]), "Billie Jean");
        assert!(v.complete);
    }

    #[test]
    fn typo_per_word() {
        let v = evaluate_match(
            "thriler michael jackson",
            &words(&["thriler", "michael", "jackson"]),
            "Thriller Michael Jackson",
        );
        assert!(v.complete);
    }

    #[test]
    fn short_words_need_exact() {
        let v = evaluate_match("x romance", &words(&["x", "romance"]), "Bad Romance");
        assert!(!v.complete && v.almost);
        let v = evaluate_match("bda romance", &words(&["bda", "romance"]), "Bad Romance");
        assert!(v.complete);
    }

    #[test]
    fn almost_when_half_words_found() {
        let v = evaluate_match(
            "red hot peppers",
            &words(&["red", "hot", "peppers"]),
            "Red Hot Chili Peppers",
        );
        assert!(!v.complete && v.almost);
    }

    #[test]
    fn numeric_words_must_be_verbatim() {
        let v = evaluate_match("prince 1998", &words(&["prince", "1998"]), "Prince 1999");
        assert!(!v.complete);
        let v = evaluate_match("prince 1999", &words(&["prince", "1999"]), "Prince 1999");
        assert!(v.complete);
    }

    #[test]
    fn empty_parenthesized_value_matches_anything_nonempty() {
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
        assert_eq!(
            candidate_lines(&answer),
            vec!["Hello".to_string(), "Hi".to_string()]
        );
    }
}
