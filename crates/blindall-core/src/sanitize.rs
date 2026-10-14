//! Port of `app/Http/Helpers.php` — `sanitizeString()` + `replaceNumbers()`
//! plus the `Str::slug($s, ' ')` transliteration pipeline.
//!
//! Deliberately preserved quirks (bug-for-bug parity):
//! - `replaceNumbers` and the article removal are **substring** replaces,
//!   applied sequentially in declaration order — mid-word corruption included
//!   (`"brune"` -> `"br1e"` because `"un"` is a plain substring).
//! - `Str::slug` transliterates to ASCII, drops every char that is not a
//!   Unicode letter/number/whitespace, collapses whitespace runs.
//!   Apostrophes are dropped *without* producing a separator:
//!   `"L'été"` -> `"lete"` (one word).

use once_cell::sync::Lazy;
use regex::Regex;

const ARTICLES: [&str; 4] = ["the ", "and ", "& ", "et "];

/// (find, replace) — identical order to the PHP arrays. Order matters:
/// `str_replace` applies each pair sequentially to the whole string.
const NUMBER_WORDS: [(&str, &str); 22] = [
    ("un", "1"),
    ("one", "1"),
    ("deux", "2"),
    ("trois", "3"),
    ("quatre", "4"),
    ("cinq", "5"),
    ("six", "6"),
    ("sept", "7"),
    ("huit", "8"),
    ("neuf", "9"),
    ("dix", "10"),
    ("vingt", "20"),
    ("trente", "30"),
    ("quarante", "40"),
    ("cinquante", "50"),
    ("fifty", "50"),
    ("soixante", "60"),
    ("soixantedix", "70"),
    ("quatrevingt", "80"),
    ("quatrevingtdix", "90"),
    ("cent", "100"),
    ("mille", "1000"),
];

static PAREN_PAIR: Lazy<Regex> = Lazy::new(|| Regex::new(r"\([^()]*\)").unwrap());
static NON_SLUG_CHARS: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^\p{L}\p{N}\s]+").unwrap());
static WHITESPACE_RUN: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());

/// Port of PHP `sanitizeString()`.
pub fn sanitize_string(input: &str) -> String {
    let mut s = input.to_string();

    // Remove all between parentheses, innermost first (nested parens).
    while PAREN_PAIR.is_match(&s) {
        s = PAREN_PAIR.replace_all(&s, "").into_owned();
    }
    // Remove any remaining opening or closing parentheses.
    s = s.replace(['(', ')'], "");

    // Slugify to clean special characters (separator: ' ').
    s = slug_with_space(&s);

    // Replace text numbers to numeric numbers.
    s = replace_numbers(&s);

    // Remove articles (substring replace — parity).
    for article in ARTICLES {
        s = s.replace(article, "");
    }

    s
}

/// Port of PHP `replaceNumbers()` — sequential substring replacement.
pub fn replace_numbers(s: &str) -> String {
    let mut out = s.to_string();
    for (find, replace) in NUMBER_WORDS {
        out = out.replace(find, replace);
    }
    out
}

/// `Str::slug($title, ' ')` equivalent:
///
/// 1. transliterate to ASCII (Laravel `Str::ascii` ≈ deunicode tables)
/// 2. flip '-' runs into ' '  (flip char because separator is ' ')
/// 3. '@' -> ' at '           (Laravel's slug dictionary)
/// 4. drop chars that are not letters, numbers or whitespace
/// 5. collapse every whitespace run to a single ' '
/// 6. lowercase + trim ' '
pub fn slug_with_space(title: &str) -> String {
    // 1. ASCII transliteration.
    let mut t = deunicode::deunicode(title);

    // 2. flip '-' -> ' '.
    t = t.replace('-', " ");

    // 3. '@' -> ' at '.
    t = t.replace('@', " at ");

    // 4. remove all chars that are not letters, numbers or whitespace.
    let t = NON_SLUG_CHARS.replace_all(&t, "");

    // 5. collapse whitespace runs.
    let t = WHITESPACE_RUN.replace_all(&t, " ");

    // 6. lowercase + trim.
    t.trim().to_lowercase()
}

/// `Str::slug($title, $separator)` — Laravel's exact pipeline:
/// ascii → flip char ('_' for '-', '-' otherwise) → '@' to `{sep}at{sep}`
/// → drop chars that are not letters/numbers/whitespace/separator
/// → collapse separator+whitespace runs → trim separator → lowercase.
pub fn str_slug(title: &str, separator: char) -> String {
    let mut t = deunicode::deunicode(title);
    let flip = if separator == '-' { '_' } else { '-' };
    t = t.replace(flip, &separator.to_string());
    t = t.replace('@', &format!("{separator}at{separator}"));
    // Drop chars that are not letters, numbers, whitespace or the separator.
    let kept: String = t
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == separator)
        .collect();
    // Collapse separator+whitespace runs into a single separator.
    let mut out = String::with_capacity(kept.len());
    let mut pending_sep = false;
    for c in kept.chars() {
        if c == separator || c.is_whitespace() {
            pending_sep = true;
        } else {
            if pending_sep && !out.is_empty() {
                out.push(separator);
            }
            pending_sep = false;
            out.push(c);
        }
    }
    out.trim_matches(separator).to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Expected values below were hand-derived by tracing the PHP code —
    // including its substring-replace quirks. The corpus fixture
    // (fixtures/sanitize_corpus.json, generated from PHP) locks parity.

    #[test]
    fn str_slug_matches_laravel() {
        // Str::slug("Ma Room Tést!") === "ma-room-test"
        assert_eq!(str_slug("Ma Room Tést!", '-'), "ma-room-test");
        assert_eq!(str_slug("Crème brûlée 100%", '-'), "creme-brulee-100");
        // '_' is the flip char for '-' separator: underscores become dashes.
        assert_eq!(str_slug("my_room_name", '-'), "my-room-name");
        assert_eq!(str_slug("a@b.com", '-'), "a-at-bcom");
        assert_eq!(str_slug("  --x--  ", '-'), "x");
    }

    #[test]
    fn strips_parentheses_including_nested() {
        // Innermost pair removed first, then the outer pair — the whole
        // parenthetical group disappears, inner words are NOT kept.
        assert_eq!(sanitize_string("Song (feat. X)"), "song");
        assert_eq!(sanitize_string("Title (a (b c) d)"), "title");
        assert_eq!(sanitize_string("A (x) B (y)"), "a b");
    }

    #[test]
    fn transliterates_french_accents() {
        // Laravel drops the apostrophe without a separator: "L'été" -> "lete".
        assert_eq!(sanitize_string("L'Été indien"), "lete indien");
        assert_eq!(sanitize_string("Garçon à côté"), "garcon a cote");
        assert_eq!(sanitize_string("Noël à Zürich"), "noel a zurich");
    }

    #[test]
    fn removes_articles_as_substrings() {
        assert_eq!(sanitize_string("the show and tell"), "show tell");
        assert_eq!(sanitize_string("rock et roll"), "rock roll");
    }

    #[test]
    fn number_words_become_digits_with_php_quirks() {
        // "une" -> "1e": 'un' is a substring replace, no word boundary.
        assert_eq!(sanitize_string("il était une fois"), "il etait 1e fois");
        assert_eq!(sanitize_string("twenty one pilots"), "twenty 1 pilots");
        assert_eq!(sanitize_string("brune"), "br1e");
        // Sequential order produces this exact PHP output.
        assert_eq!(
            sanitize_string("mille neuf cent quatre vingt dix"),
            "1000 9 100 4 20 10"
        );
        assert_eq!(
            sanitize_string("annee mille neuf cent quatrevingtdix"),
            "annee 1000 9 100 42010"
        );
    }

    #[test]
    fn handles_empty_and_special() {
        assert_eq!(sanitize_string(""), "");
        assert_eq!(sanitize_string("((( )))"), "");
        assert_eq!(sanitize_string("AC/DC"), "acdc");
        assert_eq!(sanitize_string("2Pac & Biggie"), "2pac biggie");
    }
}
