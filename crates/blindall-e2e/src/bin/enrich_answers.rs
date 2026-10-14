//! `enrich_answers` — derives media/featuring `track_answers` from the Deezer
//! API for themed rooms. Complements migration 0006 (parenthesized-title
//! extraction) for tracks whose title does not cite the work:
//!
//! - Media answer: `album.title` cleaned of soundtrack suffixes
//!   (« Spirited Away Soundtrack » → « Spirited Away »), typed from the
//!   room/playlist name (video-game keywords → Video game, anime → Anime,
//!   « anim » → Cartoon, series → Show, film/disney → Movie).
//! - Featuring answer: extra `contributors` beyond the stored Artist value.
//!
//! Idempotent — tracks already carrying a media-family or equal-value answer
//! are skipped. Usage:
//!
//!   DATABASE_URL=postgres://… cargo run -p blindall-e2e --bin enrich_answers -- [--dry-run] [--room=ID]

use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use std::time::Duration;

const RATE_DELAY: Duration = Duration::from_millis(130);

fn media_type_for(names: &[String]) -> Option<i64> {
    let mut best: Option<(i64, u8)> = None;
    for n in names {
        let l = n.to_lowercase();
        let (t, p) = if contains_word(&l, &["video game", "videogame", "gaming", "gamer", "jeu", "jeux", "games", "game"])
        {
            (12, 1)
        } else if contains_word(&l, &["anime", "animes", "manga", "mangas", "japan", "japon", "otaku", "naruto"]) {
            (8, 2)
        } else if l.contains("anim") {
            (9, 3)
        } else if contains_word(&l, &["série", "serie", "series", "séries", "show", "shows", "tv", "télé", "tele", "netflix", "feuilleton", "feuilletons"])
        {
            (7, 4)
        } else if contains_word(&l, &["film", "films", "movie", "movies", "cinema", "cinéma", "cinémas", "cine", "disney", "pixar", "dreamworks", "marvel"])
        {
            (6, 5)
        } else {
            continue;
        };
        if best.map(|(_, bp)| p < bp).unwrap_or(true) {
            best = Some((t, p));
        }
    }
    best.map(|(t, _)| t)
}

fn contains_word(s: &str, words: &[&str]) -> bool {
    let mut ok = false;
    for w in words {
        let mut start = 0;
        while let Some(i) = s[start..].find(w) {
            let i = i + start;
            let before = i == 0 || !s.as_bytes()[i - 1].is_ascii_alphanumeric();
            let after_i = i + w.len();
            let after = after_i >= s.len() || !s.as_bytes()[after_i].is_ascii_alphanumeric();
            if before && after {
                ok = true;
            }
            start = i + w.len().max(1);
        }
    }
    ok
}

/// Strip soundtrack/edition decorations from an album title, looping until
/// stable: « Ponyo … Image Album » → « Ponyo on the Cliff by the Sea ».
/// Cut at the first media marker anywhere in the string — « Laputa: Castle
/// in the Sky Soundtrack -The Mystery…- » → « Laputa: Castle in the Sky »,
/// « Nausicaä … Image Album: The Bird Man » → « Nausicaä … ».
fn cut_at_marker(s: &str) -> String {
    let l = s.to_lowercase();
    for marker in [
        " soundtrack", " image album", " image song", " bande originale",
        " bande sonore", " b.o.f", " o.s.t", " original score", " motion picture",
        "musique originale", "music from the", " music collection",
        " générique", " generique",
    ] {
        if let Some(i) = l.find(marker) {
            if i > 0 {
                return s[..i].trim_end_matches([' ', '-', '–', '—', ':', ',']).to_string();
            }
        }
    }
    s.to_string()
}

/// Whole-value generic results — compilation series names (« Level 3 »,
/// « Vol. 1 »), placeholder titles, or connector leftovers (« The Film »)
/// that are never a work name.
fn generic_whole(l: &str) -> bool {
    if ["unknown", "untitled", "the album", "album", "n/a", "none", "misc",
        "the film", "film", "the movie", "movie", "motion picture",
        "television series", "tv series", "the series", "series",
        "trailer", "main theme", "theme", "opening", "intro", "the",
    ]
    .iter()
    .any(|g| l == *g)
    {
        return true;
    }
    // « Level 3 », « Vol. 1 », « Volume 2 », « Track 4 », « Part 2 »
    let head = l.split(|c: char| c.is_ascii_digit()).next().unwrap_or("");
    let tail = l[head.len()..].trim_end_matches(|c: char| c.is_ascii_digit() || c == '.' || c == ' ');
    let numbered = matches!(
        head.trim_end(),
        "level" | "vol" | "vol." | "volume" | "track" | "part" | "pt" | "pt." | "disc" | "cd" | "chapter" | "episode" | "ep" | "ep."
    );
    numbered && tail.is_empty() && l.len() > head.len()
}

/// Compilation-style album titles that carry no usable work name
/// (« The Greatest Video Game Music », « Best of 8-Bit »…).
fn bad_album(album: &str) -> bool {
    let l = album.to_lowercase();
    [
        "video game music", "game music", "best of", "greatest", "tribute",
        "karaoke", "hits of", "the hits", "essential", "anthology",
        "compilation", "now that's", "various artists", "top tracks",
        "génériques", "generiques",
        "meilleures musiques", "meilleurs", "notre enfance",
        "l'histoire de la télé", "de la télé", "séries tv",
        "dessins animés", "feuilletons", "incontournables",
        "les plus grands thèmes", "thèmes de la télé",
        "chansons pour enfants", "l'intégrale", "les hits",
        "kitch", "reprises",
    ]
    .iter()
    .any(|k| l.contains(k))
}

/// Trailing volume markers: « Batman: Arkham Knight, Vol. 1 » → « … Knight ».
fn strip_volume(s: &str) -> String {
    let l = s.to_lowercase();
    for pat in [", vol.", ", vol ", " vol.", " volume "] {
        if let Some(i) = l.rfind(pat) {
            let rest = l[i + pat.len()..].trim_end_matches(|c: char| c == ')' || c == ']');
            // Digit run followed by end-of-string OR a subtitle separator
            // (« …, Vol. 2: Friends With Benefit », « Vol. 3 - Bonus »).
            let digits = rest.trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ' || c == '.');
            let all_digits = digits.is_empty() && rest.chars().any(|c| c.is_ascii_digit());
            let subtitled = digits.starts_with([':', '-', '–', '—', '(', '[', ',']);
            if all_digits || subtitled {
                return s[..i].trim_end_matches([' ', ',']).to_string();
            }
        }
    }
    s.to_string()
}

fn clean_work_name(album: &str) -> String {
    let mut s = strip_prefix(&cut_at_marker(&strip_volume(album.trim())));
    loop {
        let prev = s.clone();
        // Trailing (…)/( […] ) groups carrying soundtrack/edition keywords,
        // or a bare year/decade: « (2003 Remaster) », « (1989) ».
        if let Some(i) = s.rfind(['(', '[']) {
            let tail = s[i..].to_lowercase();
            let inner = tail.trim_matches(|c: char| "()[] ".contains(c));
            if !tail.contains(')') && !tail.contains(']') // unclosed after a mid-string marker cut
                || (inner.chars().all(|c| c.is_ascii_digit()) && inner.len() == 4)
                || tail.contains("original")
                || tail.contains("soundtrack")
                || tail.contains("ost")
                || tail.contains("score")
                || tail.contains("edition")
                || tail.contains("version")
                || tail.contains("deluxe")
                || tail.contains("remaster")
                || tail.contains("collection")
                || tail.contains("bande originale")
                || tail.contains("image album")
                || tail.contains("inspired")
                || tail.contains("motion picture")
                || tail.contains("b.o.f")
                || tail.contains("music")
                || tail.contains("musique")
                || tail.contains("générique")
                || tail.contains("generique")
                || tail.contains("thème")
                || tail.contains("theme")
                || tail.contains("officiel")
                || tail.contains("complete")
                || tail.contains("single")
            {
                s = s[..i].trim_end_matches([' ', '-', '–', '—', ':', ',']).to_string();
            }
        }
        s = strip_suffix(&s);
        if s == prev {
            break;
        }
    }
    s.trim_matches(|c: char| " -–—,/'\"".contains(c)).trim().to_string()
}

/// Leading « Music from … », « Bande originale de … », « B.O. du film … » —
/// the work name follows the connector.
fn strip_prefix(s: &str) -> String {
    let l = s.to_lowercase();
    for (prefix, connectors) in [
        ("music from", &["the motion picture", "the film", "the movie", "and inspired by", "the original", "the wb television series", "the television series", "the tv series", "the series", "the soundtrack of", "the soundtrack", "the "][..]),
        ("music of", &["the motion picture", "the film", "the "][..]),
        ("original music from", &["the motion picture", "the film", "the movie", "the "][..]),
        ("music inspired by", &["the motion picture", "the film", "the movie", "the "][..]),
        ("musique", &["originale de", "originale du", "de", "du", "du film", "du jeu"][..]),
        ("bande originale", &["du film", "de la série", "de la serie", "du jeu", "de", "du"][..]),
        ("bande sonore", &["de", "du"][..]),
        ("b.o.f.", &["de", "du"][..]),
        ("b.o.f", &["de", "du"][..]),
        ("ost", &["de", "du", "of"][..]),
    ] {
        if let Some(rest) = l.strip_prefix(prefix) {
            let rest = rest.trim_start();
            for c in connectors {
                if let Some(r) = rest.strip_prefix(c) {
                    let out = s[s.len() - r.len()..].trim_start().to_string();
                    if out.len() >= 2 {
                        return out;
                    }
                }
            }
            // « Music from Braid » — no connector, the whole rest is the
            // work name. Strip a leading « the » then take it wholesale.
            if prefix.contains("music") {
                let bare = rest.strip_prefix("the ").unwrap_or(rest);
                let out = s[s.len() - bare.len()..].trim_start().to_string();
                if out.chars().count() >= 2 {
                    return out;
                }
            }
        }
    }
    s.to_string()
}

fn strip_suffix(s: &str) -> String {
    const SUFFIXES: &[&str] = &[
        "original motion picture soundtrack",
        "original game soundtrack",
        "original video game soundtrack",
        "video game soundtrack",
        "game soundtrack",
        "original video game",
        "original game",
        "video game",
        "motion picture soundtrack",
        "original soundtrack",
        "the soundtrack",
        "soundtrack collection",
        "soundtrack",
        "bande originale du film",
        "bande originale de la série",
        "bande originale du jeu",
        "bande originale",
        "b.o.f.",
        "b.o.f",
        "bof",
        "o.s.t.",
        "o.s.t",
        "ost",
        "original score",
        "the score",
        "score",
        "image album",
        "image song collection",
        "image song",
        "music from the motion picture",
        "music from the film",
        "music from",
        "original music",
        "the musical",
        "musical",
        "quadrilogy",
        "trilogy",
        "saga",
        "for chamber orchestra",
        "chamber orchestra",
        "orchestra",
        "trailer",
        "songs",
        "music",
        "musique originale",
        "musique",
        "collection",
        "complete",
        "deluxe edition",
        "deluxe",
        "edition",
        "version",
        "remastered",
        "remaster",
        "original",
        "inspired by",
        "selection",
        "ep",
        "single",
    ];
    let mut out = s.to_string();
    for suf in SUFFIXES {
        let l = out.trim_end().to_lowercase();
        for sep in [" ", " - ", " – ", " — ", " : ", " / ", ", "] {
            let needle = format!("{sep}{suf}");
            if l.ends_with(&needle) {
                let cut = out.trim_end().len() - needle.len();
                out = out.trim_end()[..cut].trim_end_matches([' ', '-', '–', '—', ':', '/', ',']).to_string();
                return out;
            }
        }
        if l == *suf {
            return String::new();
        }
    }
    out
}

/// Compilation/generic album names that are not a work title.
fn generic_name(s: &str) -> bool {
    let l = s.to_lowercase();
    let l = l.trim();
    if l.chars().count() < 2 || l.chars().all(|c| c.is_ascii_digit() || c == '.' || c == ' ') {
        return true;
    }
    if generic_whole(l) {
        return true;
    }
    const STRONG: &[&str] = &[
        "best of", "greatest hits", "the essential", "compilation", "anthology",
        "the very best", "now that's", "karaoke", "tribute to", "tribute",
        "various artists", "collection", "hits",
    ];
    const WEAK: &[&str] = &[
        "covers", "various", "playlist", "session", "sessions", "live",
        "remixes", "soundtrack", "ost", "score", "music",
    ];
    STRONG.iter().any(|g| l == *g || l.starts_with(&format!("{g} ")))
        || WEAK
            .iter()
            .any(|g| l == *g || l.starts_with(&format!("{g} ")) && l.len() < g.len() + 8)
}

/// The album title carries an explicit soundtrack/media marker — then a
/// cleaned name equal to the track title is still a real work name
/// (« Ponyo on the Cliff by the Sea Image Album »).
fn has_media_marker(album: &str) -> bool {
    let l = album.to_lowercase();
    [
        "soundtrack", "b.o.f", "bof", "o.s.t", "ost", "score", "bande originale",
        "image album", "image song", "motion picture", "musique originale",
        "music from", "inspired by", "original music",
    ]
    .iter()
    .any(|k| l.contains(k))
}

/// Normalize for equality (mirrors `matching.rs` spirit): lowercase, keep
/// alphanumerics only.
fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let only_room: Option<i64> = args
        .iter()
        .find_map(|a| a.strip_prefix("--room=").and_then(|v| v.parse().ok()));
    // `--except=1,2,3` — exclude rooms (e.g. already-enriched themed rooms on
    // a featuring-only pass over the rest).
    let except: Vec<i64> = args
        .iter()
        .find_map(|a| a.strip_prefix("--except="))
        .map(|v| v.split(',').filter_map(|x| x.trim().parse().ok()).collect())
        .unwrap_or_default();
    let db = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:your-super-secret-and-long-postgres-password@localhost:5433/postgres".into());

    let pool = PgPoolOptions::new().max_connections(4).connect(&db).await?;
    let http = reqwest::Client::builder().timeout(Duration::from_secs(8)).build()?;

    // Media answer types that block enrichment (curated rows win).
    const MEDIA_FAMILY: &str =
        "('movie','show','anime','cartoon','video game','video games','videogame','jeu vidéo','jeux vidéo','game','musical','character','documentary','drama')";

    // Candidate tracks: every deezer track attached to a room. Media answers
    // require a themed name signal; Featuring dedup happens per value.
    let q = r#"
        SELECT t.id AS track_id, t.provider_id::text AS pid,
               array_agg(DISTINCT rn.name::text) AS names,
               (SELECT string_agg(value::text, ' / ') FROM track_answers WHERE track_id=t.id AND answer_type_id=1) AS artist,
               (SELECT string_agg(value::text, ' / ') FROM track_answers WHERE track_id=t.id AND answer_type_id=2) AS title,
               (SELECT string_agg(value::text, ' / ') FROM track_answers WHERE track_id=t.id AND answer_type_id=5) AS feats
        FROM tracks t
        JOIN playlist_room pr ON pr.playlist_id = t.playlist_id
        JOIN rooms rn ON rn.id = pr.room_id
        WHERE t.provider = 'deezer'
          AND ($1::bigint IS NULL OR rn.id = $1)
          AND NOT (rn.id = ANY($2::bigint[]))
        GROUP BY t.id, t.provider_id
        ORDER BY t.id
        "#
    .to_string();
    let candidates = sqlx::query(&q).bind(only_room).bind(&except).fetch_all(&pool).await?;
    println!("candidates: {}", candidates.len());

    let mut ok = 0u32;
    let mut media_added = 0u32;
    let mut feat_added = 0u32;
    let mut skipped = 0u32;

    for row in &candidates {
        let tid: i64 = row.get("track_id");
        let pid: String = row.get("pid");
        let names: Vec<String> = row.get("names");
        let artist: Option<String> = row.get("artist");
        let title: Option<String> = row.get("title");
        let feats: Option<String> = row.get("feats");

        // Non-themed rooms still get Featuring enrichment; media answers
        // require a name signal (no blind Movie fallback — singles would
        // produce wrong answers).
        let type_id = media_type_for(&names);
        if type_id.is_none() && feats.is_some() {
            skipped += 1;
            continue;
        }

        // Deezer track lookup with one retry on rate-limit.
        let mut track_json = None;
        for attempt in 0..3 {
            let resp = http
                .get(format!("https://api.deezer.com/track/{pid}"))
                .send()
                .await;
            match resp {
                Ok(r) if r.status() == reqwest::StatusCode::TOO_MANY_REQUESTS => {
                    tokio::time::sleep(Duration::from_secs(2 + attempt as u64 * 2)).await;
                    continue;
                }
                Ok(r) if r.status().is_success() => {
                    track_json = r.json::<serde_json::Value>().await.ok();
                    break;
                }
                Ok(_) => break,
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    continue;
                }
            }
        }
        ok += 1;
        let Some(j) = track_json else { continue };
        if j["readable"].as_bool() == Some(false) && j.get("error").is_some() {
            continue;
        }

        // — Media answer from album title —
        if let (Some(type_id), Some(album)) = (
            type_id,
            j["album"]["title"].as_str().filter(|a| !bad_album(a)),
        ) {
            let name = clean_work_name(album);
            // Album == track title with no soundtrack marker → a single;
            // nothing learned about the work → skip.
            let title_norm = title.as_deref().map(norm).unwrap_or_default();
            let is_single = norm(&name) == title_norm && !has_media_marker(album);
            // Very short names (« XO », « NI ») are only a real work when the
            // album carried a soundtrack marker (« Us (Original Motion
            // Picture Soundtrack) »); bare short album titles are music
            // releases, not works.
            let too_short = name.chars().count() < 3 && !has_media_marker(album);
            if !generic_name(&name) && !is_single && !too_short {
                if dry_run {
                    println!("[{tid}] type={type_id} album={album:?} → {name:?}");
                } else {
                    let guard = format!(
                        "INSERT INTO track_answers (track_id, answer_type_id, value, score, created_at, updated_at)
                         SELECT $1, $2, $3::citext, 0.5, NOW(), NOW()
                         WHERE NOT EXISTS (
                           SELECT 1 FROM track_answers x WHERE x.track_id=$1
                             AND x.answer_type_id IN (SELECT id FROM answer_types WHERE lower(name::text) IN {MEDIA_FAMILY}))"
                    );
                    let res = sqlx::query(&guard).bind(tid).bind(type_id).bind(&name).execute(&pool).await;
                    match res {
                        Ok(r) if r.rows_affected() > 0 => media_added += 1,
                        Ok(_) => {}
                        Err(e) => eprintln!("[{tid}] insert media failed: {e}"),
                    }
                }
            }
        }

        // — Featuring answers from extra contributors —
        let artist_norm = artist.as_deref().map(norm).unwrap_or_default();
        if let Some(contributors) = j["contributors"].as_array() {
            for c in contributors {
                let Some(cname) = c["name"].as_str() else { continue };
                let cn = norm(cname);
                if cn.is_empty() || cn == artist_norm {
                    continue;
                }
                let dup = feats
                    .as_deref()
                    .map(|f| f.split(" / ").any(|x| norm(x) == cn))
                    .unwrap_or(false);
                if dup {
                    continue;
                }
                if dry_run {
                    println!("[{tid}] feat → {cname:?}");
                } else {
                    let res = sqlx::query(
                        "INSERT INTO track_answers (track_id, answer_type_id, value, score, created_at, updated_at)
                         SELECT $1, 5, $2::citext, 0.5, NOW(), NOW()
                         WHERE NOT EXISTS (
                           SELECT 1 FROM track_answers x WHERE x.track_id=$1 AND x.answer_type_id=5
                             AND lower(btrim(x.value::text)) = lower($2))",
                    )
                    .bind(tid)
                    .bind(cname)
                    .execute(&pool)
                    .await;
                    match res {
                        Ok(r) if r.rows_affected() > 0 => feat_added += 1,
                        Ok(_) => {}
                        Err(e) => eprintln!("[{tid}] insert feat failed: {e}"),
                    }
                }
            }
        }

        tokio::time::sleep(RATE_DELAY).await;
    }

    println!("done: fetched={ok} media_added={media_added} feat_added={feat_added} skipped_untyped={skipped}");
    Ok(())
}
