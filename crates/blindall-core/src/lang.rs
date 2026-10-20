//! Cheap heuristic language detection for answer strings.
//!
//! Used by the minigame picker to keep distractors in the same language as
//! the correct answer — a French title with three English alternatives (or
//! the reverse) gives the answer away for free.
//!
//! Detection scores distinctive stopwords/word-forms per language, plus a
//! diacritic bonus for French. Titles with no signal ("Imagine") and
//! non-Latin scripts both land in `Other`, where the caller should fall
//! back to playlist-level coherence instead of a language filter.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Fr,
    En,
    Other,
}

/// Words that appear in French but are not English words.
/// Deliberately excludes forms that collide with English ("son", "pour",
/// "lit", "main", "car", "sale", "vent", "sang", "encore", "chance",
/// "dame", "route", "tour", "fort", "air", "mode", "rose", "image",
/// "note", "rage", "dire", "laid", "brave", "stage", "sage", "fin",
/// "comment", "ma", "pa", "as", "me", "on", "or", "a", "met", "dis",
/// "font", "mine", "coin", "use", "marche"→EN "march" differs…).
const FR_WORDS: &[&str] = &[
    "le",
    "les",
    "des",
    "du",
    "de",
    "et",
    "est",
    "je",
    "tu",
    "il",
    "elle",
    "ils",
    "elles",
    "nous",
    "vous",
    "mon",
    "mes",
    "ton",
    "ta",
    "tes",
    "ce",
    "cet",
    "cette",
    "ces",
    "dans",
    "sur",
    "avec",
    "sans",
    "sous",
    "chez",
    "ne",
    "pas",
    "plus",
    "que",
    "qui",
    "quoi",
    "quand",
    "comme",
    "amour",
    "jour",
    "nuit",
    "vie",
    "tout",
    "toute",
    "tous",
    "toutes",
    "toi",
    "se",
    "au",
    "aux",
    "oui",
    "non",
    "jamais",
    "toujours",
    "peux",
    "veux",
    "suis",
    "es",
    "sont",
    "ont",
    "été",
    "être",
    "etais",
    "était",
    "sera",
    "serai",
    "serais",
    "serait",
    "soit",
    "sois",
    "fais",
    "fait",
    "faites",
    "faire",
    "vais",
    "vas",
    "vont",
    "allons",
    "allez",
    "va",
    "viens",
    "vient",
    "venir",
    "venez",
    "dit",
    "dites",
    "disent",
    "prends",
    "prend",
    "prendre",
    "mets",
    "mettre",
    "donne",
    "donner",
    "sais",
    "sait",
    "savoir",
    "peut",
    "pouvoir",
    "pourrais",
    "pourrait",
    "veut",
    "vouloir",
    "voudrais",
    "voudrait",
    "dois",
    "doit",
    "devoir",
    "faut",
    "falloir",
    "aime",
    "aimer",
    "aimé",
    "aimée",
    "danse",
    "danser",
    "chante",
    "chanter",
    "chanson",
    "pars",
    "partir",
    "partie",
    "reste",
    "rester",
    "meurs",
    "mourir",
    "mort",
    "morte",
    "vivre",
    "vécue",
    "reviens",
    "revenir",
    "regarde",
    "regarder",
    "écoute",
    "écouter",
    "parle",
    "parler",
    "joue",
    "jouer",
    "pense",
    "penser",
    "crois",
    "croire",
    "sens",
    "sentir",
    "attends",
    "attendre",
    "cherche",
    "chercher",
    "trouve",
    "trouver",
    "perds",
    "perdre",
    "perdu",
    "gagne",
    "gagner",
    "pleure",
    "pleurer",
    "rire",
    "rêve",
    "rêver",
    "ciel",
    "soleil",
    "lune",
    "mer",
    "soir",
    "matin",
    "hier",
    "demain",
    "aujourd",
    "yeux",
    "tête",
    "cœur",
    "coeur",
    "âme",
    "chérie",
    "chéri",
    "homme",
    "femme",
    "déjà",
    "très",
    "peu",
    "bien",
    "mal",
    "ici",
    "là",
    "alors",
    "donc",
    "mais",
    "ni",
    "entre",
    "vers",
    "après",
    "avant",
    "depuis",
    "contre",
    "quelque",
    "chose",
    "choses",
    "deux",
    "trois",
    "moins",
    "trop",
    "assez",
    "presque",
    "bientôt",
    "aussi",
    "même",
    "autre",
    "autres",
    "quel",
    "quelle",
    "quels",
    "quelles",
    "leur",
    "leurs",
    "notre",
    "votre",
    "nos",
    "vos",
    "celui",
    "celle",
    "ceux",
    "belle",
    "beau",
    "grand",
    "grande",
    "petit",
    "petite",
    "jeune",
    "vieux",
    "vieille",
    "nouveau",
    "nouvelle",
    "premier",
    "première",
    "dernier",
    "dernière",
    "seul",
    "seule",
    "tel",
    "telle",
    "tant",
    "tellement",
    "pourquoi",
    "combien",
    "personne",
    "rien",
    "jours",
    "nuits",
    "mots",
    "bras",
    "corps",
    "sœur",
    "frère",
    "père",
    "mère",
    "enfant",
    "fille",
    "garçon",
    "ami",
    "amie",
    "copain",
    "histoire",
    "année",
    "années",
    "mois",
    "semaine",
    "heure",
    "heures",
    "seconde",
    "fois",
    "douce",
    "doux",
    "tendre",
    "triste",
    "heureux",
    "heureuse",
    "fou",
    "folle",
    "vrai",
    "vraie",
    "beau",
    "belles",
    "mieux",
    "pire",
    "qu", // "qu'il", "qu'on" fragments
];

/// Words that appear in English but are not French words (plus contraction
/// fragments left by apostrophe tokenization: "don't" → "don","t").
/// Excludes forms that collide with French (see FR_WORDS note).
const EN_WORDS: &[&str] = &[
    "the",
    "and",
    "of",
    "to",
    "my",
    "your",
    "you",
    "is",
    "are",
    "was",
    "were",
    "for",
    "with",
    "at",
    "by",
    "from",
    "in",
    "into",
    "out",
    "up",
    "down",
    "him",
    "her",
    "us",
    "them",
    "this",
    "that",
    "these",
    "those",
    "what",
    "when",
    "where",
    "who",
    "how",
    "not",
    "no",
    "yes",
    "all",
    "some",
    "love",
    "baby",
    "girl",
    "boy",
    "heart",
    "way",
    "time",
    "world",
    "man",
    "men",
    "woman",
    "women",
    "gonna",
    "wanna",
    "let",
    "go",
    "got",
    "get",
    "one",
    "two",
    "new",
    "old",
    "big",
    "bad",
    "good",
    "little",
    "back",
    "now",
    "just",
    "only",
    "over",
    "again",
    "never",
    "always",
    "till",
    "until",
    "night",
    "night",
    "day",
    "days",
    "moon",
    "sun",
    "sky",
    "rain",
    "fire",
    "water",
    "blood",
    "soul",
    "eyes",
    "hands",
    "head",
    "voice",
    "name",
    "street",
    "city",
    "town",
    "house",
    "door",
    "window",
    "room",
    "bed",
    "flower",
    "red",
    "blue",
    "white",
    "black",
    "green",
    "yellow",
    "grey",
    "gray",
    "beautiful",
    "pretty",
    "young",
    "first",
    "last",
    "alone",
    "another",
    "every",
    "their",
    "our",
    "his",
    "hers",
    "ours",
    "we",
    "they",
    "he",
    "she",
    "it",
    "i",
    "we",
    "if",
    "do",
    "be",
    "am",
    "off",
    "say",
    "see",
    "come",
    "take",
    "make",
    "know",
    "want",
    "need",
    "feel",
    "like",
    "kiss",
    "dance",
    "sing",
    "song",
    "music",
    "party",
    "money",
    "mama",
    "hey",
    "oh",
    "yeah",
    "tonight",
    "forever",
    "together",
    "ever",
    "maybe",
    "please",
    "sorry",
    "hello",
    "goodbye",
    "bye",
    "home",
    "mind",
    "life",
    "star",
    "rock",
    "roll",
    "gold",
    "golden",
    "silver",
    "wild",
    "free",
    "crazy",
    "sexy",
    "high",
    "low",
    "sweet",
    "cold",
    "hot",
    "warm",
    "deep",
    "hard",
    "easy",
    "far",
    "near",
    "right",
    "wrong",
    "true",
    "real",
    "gone",
    "dead",
    "alive",
    "born",
    "die",
    "dying",
    "live",
    "living",
    "run",
    "running",
    "walk",
    "walking",
    "fly",
    "flying",
    "fall",
    "falling",
    "cry",
    "crying",
    "smile",
    "dream",
    "dreaming",
    "dreams",
    "wake",
    "waking",
    "sleep",
    "sleeping",
    "stay",
    "staying",
    "leave",
    "leaving",
    "call",
    "calling",
    "hold",
    "holding",
    "touch",
    "kiss",
    "dont",
    "cant",
    "wont",
    "im",
    "ive",
    "ill",
    "youre",
    "youll",
    "youve",
    "youd",
    "hes",
    "shes",
    "its",
    "lets",
    "thats",
    "whats",
    "whos",
    "heres",
    "theres",
    "wheres",
    "whens",
    "whys",
    "hows",
    "itll",
    "itd",
    "id",
    "weve",
    "werent",
    "didnt",
    "doesnt",
    "isnt",
    "arent",
    "wasnt",
    "hasnt",
    "havent",
    "hadnt",
    "couldnt",
    "shouldnt",
    "wouldnt",
    "mustnt",
    "aint",
];

fn has_diacritics(s: &str) -> bool {
    s.chars().any(|c| {
        matches!(
            c,
            'à' | 'â'
                | 'ä'
                | 'ç'
                | 'é'
                | 'è'
                | 'ê'
                | 'ë'
                | 'î'
                | 'ï'
                | 'ô'
                | 'ö'
                | 'ù'
                | 'û'
                | 'ü'
                | 'ÿ'
                | 'œ'
                | 'æ'
                | 'À'
                | 'Â'
                | 'Ä'
                | 'Ç'
                | 'É'
                | 'È'
                | 'Ê'
                | 'Ë'
                | 'Î'
                | 'Ï'
                | 'Ô'
                | 'Ö'
                | 'Ù'
                | 'Û'
                | 'Ü'
                | 'Ÿ'
                | 'Œ'
                | 'Æ'
        )
    })
}

/// Any character outside Latin/ASCII-digits/punctuation — CJK, kana,
/// hangul, cyrillic, arabic, hebrew, greek…
fn has_non_latin(s: &str) -> bool {
    s.chars().any(|c| {
        matches!(c as u32,
            0x0370..=0x03FF      // greek
            | 0x0400..=0x04FF    // cyrillic
            | 0x0590..=0x05FF    // hebrew
            | 0x0600..=0x06FF    // arabic
            | 0x3040..=0x30FF    // hiragana + katakana
            | 0x3400..=0x9FFF    // CJK
            | 0xAC00..=0xD7AF    // hangul
            | 0x0E00..=0x0E7F    // thai
        )
    })
}

/// Tokenize on anything that isn't a letter — apostrophes split, so
/// "don't" → ["don","t"], "j'aime" → ["j","aime"], "c'est" → ["c","est"].
fn tokens(s: &str) -> impl Iterator<Item = String> + '_ {
    s.split(|c: char| !c.is_alphabetic())
        .filter(|w| w.len() > 1)
        .map(|w| w.to_lowercase())
}

pub fn detect(s: &str) -> Lang {
    if s.trim().is_empty() || has_non_latin(s) {
        return Lang::Other;
    }
    let mut fr = 0i32;
    let mut en = 0i32;
    if has_diacritics(s) {
        fr += 2;
    }
    for w in tokens(s) {
        if FR_WORDS.contains(&w.as_str()) {
            fr += 1;
        }
        if EN_WORDS.contains(&w.as_str()) {
            en += 1;
        }
    }
    if fr > en {
        Lang::Fr
    } else if en > fr {
        Lang::En
    } else {
        Lang::Other
    }
}

/// Can `candidate` serve as a distractor for `correct`? A known language
/// rejects candidates detected as the *other* language — that is the
/// tell that gives the answer away. `Other` candidates (proper nouns,
/// no signal words) betray nothing and are always acceptable.
pub fn compatible(correct: Lang, candidate: &str) -> bool {
    match correct {
        Lang::Other => true,
        want => {
            let d = detect(candidate);
            d == want || d == Lang::Other
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn french_titles() {
        for s in [
            "La vie en rose",
            "Je t'aime",
            "L'amour est bleu",
            "Ne me quitte pas",
            "Les feuilles mortes",
            "Comme d'habitude",
            "Pour que tu m'aimes encore", // "encore" excluded (EN too) — still FR
            "Et si tu n'existais pas",
            "Je vole",
            "Tout doucement",
            "Aux champs-Élysées",
            "Mon vieux",
            "Quand on n'a que l'amour",
            "Le temps des cathédrales",
            "Sous le soleil de Bodega",
        ] {
            assert_eq!(detect(s), Lang::Fr, "expected Fr: {s}");
        }
    }

    #[test]
    fn english_titles() {
        for s in [
            "The Sound of Silence",
            "I Will Always Love You",
            "My Heart Will Go On",
            "Don't Stop Me Now",
            "We Are the Champions",
            "Born in the U.S.A.",
            "Stairway to Heaven",
            "Sweet Child o' Mine", // "o" skipped, sweet+child EN… wait child not in set — sweet is
            "Every Breath You Take",
            "With or Without You",
            "She Loves You",
            "Let It Be",
            "Can't Help Falling in Love",
            "It's My Life",
            "What Is Love",
            "Take On Me",
            "Never Gonna Give You Up",
        ] {
            assert_eq!(detect(s), Lang::En, "expected En: {s}");
        }
    }

    #[test]
    fn other_or_neutral() {
        // Proper nouns / no stopword signal → Other (caller falls back to
        // playlist coherence). Non-Latin scripts always Other.
        for s in [
            "Imagine",
            "Yesterday",
            "Hotel California",
            "愛してる",
            "Pyongyang",
        ] {
            assert_eq!(detect(s), Lang::Other, "expected Other: {s}");
        }
        assert_eq!(detect(""), Lang::Other);
        assert_eq!(detect("   "), Lang::Other);
    }

    #[test]
    fn distractor_compat() {
        assert!(compatible(Lang::En, "The Final Countdown"));
        assert!(!compatible(Lang::En, "La vie en rose"));
        assert!(compatible(Lang::Fr, "Hotel California")); // neutral — no tell
        assert!(compatible(Lang::Fr, "Je t'aime"));
        assert!(!compatible(Lang::Fr, "The Final Countdown"));
        assert!(compatible(Lang::Other, "anything at all"));
    }
}
