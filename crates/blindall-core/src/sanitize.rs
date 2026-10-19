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

static IMPORT_JUNK_PAREN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(concat!(
        r"(?i)\s*[\(\[]\s*",
        r"(?:re-?master(?:ed|is[eé])?|remastered \d+|radio edit|single version|album version|",
        r"original (?:version|mix|recording)|mono|stereo|edit|version|demo|bonus track(?:s)?|",
        r"deluxe|live(?: at [^\)\]]*)?|acoustic|instrumental|karaoke(?: version)?|",
        r"extended(?: version| mix| edit)?|remix|remixed|mix|re-?recorded|re-?enregistr[eé]|",
        r"\d{4} (?:remaster|remix|version|mix)|digital remaster|explicit|clean)",
        r"[^\)\]]*[\)\]]"
    ))
    .unwrap()
});
static IMPORT_BRACKET: Lazy<Regex> = Lazy::new(|| Regex::new(r"\[[^\]]*\]").unwrap());
static IMPORT_FEAT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)[\(\[]?\s*\b(?:feat\.?|featuring|ft\.?)\s+([^)\[\]]+?)(?:\s*[\)\]]|\s+[-:]\s|$)",
    )
    .unwrap()
});
static IMPORT_MEDLEY: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+/\s+").unwrap());

/// Nettoie un titre provider avant import : retire les suffixes "(Remastered)",
/// "(Radio Edit)", "[feat. X]"… et retourne les artistes en featuring detectes
/// ainsi que les segments d'un medley "A / B / C" (a utiliser comme alias).
/// Ne touche PAS aux references de source "(De "X")" / "(from "X")".
pub fn clean_import_title(input: &str) -> (String, Vec<String>, Vec<String>) {
    let mut title = input.trim().to_string();
    let mut feats: Vec<String> = Vec::new();

    // extrait les featurings avant tout stripping de parentheses.
    // la capture brute est rendue telle quelle — l'éclatement en artistes
    // individuels se fait a l'import, la ou on peut verifier qu'une forme
    // jointe ("Earth, Wind & Fire") existe deja comme artiste.
    while let Some(c) = IMPORT_FEAT.captures(&title.clone()) {
        let cap = c.get(0).unwrap().range();
        let inner = c.get(1).unwrap().as_str().trim().to_string();
        if inner.len() > 1 && !feats.iter().any(|x| x.eq_ignore_ascii_case(&inner)) {
            feats.push(inner);
        }
        title = format!("{}{}", &title[..cap.start], &title[cap.end..]);
    }

    title = IMPORT_JUNK_PAREN.replace_all(&title, "").into_owned();
    title = IMPORT_BRACKET.replace_all(&title, "").into_owned();

    // re-equilibre les parentheses orphelines (ex: "Ocarina of Time")" )
    while title.matches(')').count() > title.matches('(').count() {
        if let Some(pos) = title.rfind(')') {
            title.remove(pos);
        } else {
            break;
        }
    }
    title = title
        .trim_end_matches(['"', '\'', '»', '“', '”'])
        .trim_end()
        .to_string();
    while title.matches('(').count() > title.matches(')').count() {
        if let Some(pos) = title.rfind('(') {
            title.truncate(pos);
            title = title.trim_end().to_string();
        } else {
            break;
        }
    }

    let medley: Vec<String> = IMPORT_MEDLEY
        .split(&title)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();

    let title = WHITESPACE_RUN.replace_all(title.trim(), " ").into_owned();
    (title, feats, medley)
}

static FEAT_PAREN: Lazy<Regex> = Lazy::new(|| Regex::new(r"[\(\[][^\)\]]*[\)\]]").unwrap());

/// Eclate une capture "feat. A & B, C" en noms d'artistes candidats.
/// Les annotations entre parentheses/crochets ("Olly Alexander (Years & Years)")
/// sont retirees d'abord pour ne pas produire de fragments orphelins.
/// La decision de garder une forme jointe ("Earth, Wind & Fire") revient a
/// l'appelant, qui peut verifier son existence comme artiste dans le catalogue.
pub fn split_feat_artists(raw: &str) -> Vec<String> {
    let stripped = FEAT_PAREN.replace_all(raw, "").trim().to_string();
    let has_parens = FEAT_PAREN.is_match(raw);
    let basis = if stripped.len() > 1 {
        stripped
    } else if has_parens {
        // le feat n'etait qu'une annotation parenthetique — rien d'exploitable
        return Vec::new();
    } else {
        raw.trim().to_string()
    };
    let mut out: Vec<String> = Vec::new();
    for seg in basis
        .split(|ch: char| ch == ',' || ch == '&')
        .flat_map(|p| p.split(" et "))
        .flat_map(|p| p.split(" and "))
    {
        let seg = seg.trim();
        if seg.len() > 1 && !out.iter().any(|x| x.eq_ignore_ascii_case(seg)) {
            out.push(seg.to_string());
        }
    }
    if out.is_empty() && basis.trim().len() > 1 {
        out.push(basis.trim().to_string());
    }
    out
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

    #[test]
    fn import_title_strips_provider_junk() {
        let (t, f, m) = clean_import_title("Yeah! [feat. Lil Jon & Ludacris]");
        assert_eq!(t, "Yeah!");
        assert_eq!(f, vec!["Lil Jon & Ludacris"]);
        assert!(m.len() <= 1);

        let (t, f, _) = clean_import_title("Song (Remastered 2011) (Radio Edit)");
        assert_eq!(t, "Song");
        assert!(f.is_empty());

        let (t, f, _) = clean_import_title("Defeat at Midnight");
        assert_eq!(t, "Defeat at Midnight");
        assert!(f.is_empty());
    }

    #[test]
    fn split_feat_artists_segments() {
        assert_eq!(
            split_feat_artists("Lil Jon & Ludacris"),
            vec!["Lil Jon", "Ludacris"]
        );
        assert_eq!(
            split_feat_artists("Ciara & Fat Man Scoop"),
            vec!["Ciara", "Fat Man Scoop"]
        );
        assert_eq!(
            split_feat_artists("Pharrell Williams, Katy Perry & Big Sean"),
            vec!["Pharrell Williams", "Katy Perry", "Big Sean"]
        );
        // les annotations entre parentheses ne produisent pas de fragments
        assert_eq!(
            split_feat_artists("Olly Alexander (Years & Years)"),
            vec!["Olly Alexander"]
        );
        assert!(split_feat_artists("(Years & Years)").is_empty());
        // un nom simple revient tel quel
        assert_eq!(split_feat_artists("Mase"), vec!["Mase"]);
    }

    #[test]
    fn import_title_keeps_media_refs() {
        let (t, _, _) = clean_import_title("Shallow (De \"A Star Is Born\")");
        assert_eq!(t, "Shallow (De \"A Star Is Born\")");
        let (t, _, _) = clean_import_title("My Heart Will Go On (Love Theme from \"Titanic\")");
        assert_eq!(t, "My Heart Will Go On (Love Theme from \"Titanic\")");
    }

    #[test]
    fn import_title_fixes_orphan_parens_and_medleys() {
        let (t, _, _) = clean_import_title("Ocarina of Time\")");
        assert_eq!(t, "Ocarina of Time");
        let (_, _, m) = clean_import_title("Intro / Verse / Chorus");
        assert_eq!(m, vec!["Intro", "Verse", "Chorus"]);
    }
}
