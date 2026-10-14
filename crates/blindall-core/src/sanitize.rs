use once_cell::sync::Lazy;
use regex::Regex;

const ARTICLES: [&str; 4] = ["the ", "and ", "& ", "et "];

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

pub fn sanitize_string(input: &str) -> String {
    let mut s = input.to_string();

    while PAREN_PAIR.is_match(&s) {
        s = PAREN_PAIR.replace_all(&s, "").into_owned();
    }
    s = s.replace(['(', ')'], "");

    s = slug_with_space(&s);

    s = replace_numbers(&s);

    for article in ARTICLES {
        s = s.replace(article, "");
    }

    s
}

pub fn replace_numbers(s: &str) -> String {
    let mut out = s.to_string();
    for (find, replace) in NUMBER_WORDS {
        out = out.replace(find, replace);
    }
    out
}

pub fn slug_with_space(title: &str) -> String {
    let mut t = deunicode::deunicode(title);

    t = t.replace('-', " ");

    t = t.replace('@', " at ");

    let t = NON_SLUG_CHARS.replace_all(&t, "");

    let t = WHITESPACE_RUN.replace_all(&t, " ");

    t.trim().to_lowercase()
}

pub fn str_slug(title: &str, separator: char) -> String {
    let mut t = deunicode::deunicode(title);
    let flip = if separator == '-' { '_' } else { '-' };
    t = t.replace(flip, &separator.to_string());
    t = t.replace('@', &format!("{separator}at{separator}"));
    let kept: String = t
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace() || *c == separator)
        .collect();
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

    #[test]
    fn str_slug_matches_laravel() {
        assert_eq!(str_slug("Ma Room Tést!", '-'), "ma-room-test");
        assert_eq!(str_slug("Crème brûlée 100%", '-'), "creme-brulee-100");
        assert_eq!(str_slug("my_room_name", '-'), "my-room-name");
        assert_eq!(str_slug("a@b.com", '-'), "a-at-bcom");
        assert_eq!(str_slug("  --x--  ", '-'), "x");
    }

    #[test]
    fn strips_parentheses_including_nested() {
        assert_eq!(sanitize_string("Song (feat. X)"), "song");
        assert_eq!(sanitize_string("Title (a (b c) d)"), "title");
        assert_eq!(sanitize_string("A (x) B (y)"), "a b");
    }

    #[test]
    fn transliterates_french_accents() {
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
        assert_eq!(sanitize_string("il était une fois"), "il etait 1e fois");
        assert_eq!(sanitize_string("twenty one pilots"), "twenty 1 pilots");
        assert_eq!(sanitize_string("brune"), "br1e");
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
