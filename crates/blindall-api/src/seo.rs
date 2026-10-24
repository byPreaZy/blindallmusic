use crate::state::AppState;
use axum::{
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use std::fmt::Write as _;

const LOCALES: [&str; 3] = ["fr", "en", "es"];

fn localized_path(base: &str, path: &str, locale: &str) -> String {
    let prefix = match locale {
        "en" | "es" => format!("/{locale}"),
        _ => String::new(),
    };
    let path = path.trim_matches('/');
    if path.is_empty() {
        return format!(
            "{base}{}",
            if prefix.is_empty() {
                "/".into()
            } else {
                prefix
            }
        );
    }
    format!("{base}{prefix}/{path}")
}

fn text_response(content: String) -> Response {
    let mut res = content.into_response();
    res.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=UTF-8"),
    );
    res
}

pub async fn robots(State(state): State<AppState>) -> Response {
    let base = state.app_url.trim_end_matches('/');
    text_response(format!(
        "User-agent: *\nAllow: /\n\n\
         # AI crawlers (inference & answer engines)\n\
         User-agent: GPTBot\nAllow: /\n\n\
         User-agent: ChatGPT-User\nAllow: /\n\n\
         User-agent: ClaudeBot\nAllow: /\n\n\
         User-agent: anthropic-ai\nAllow: /\n\n\
         User-agent: PerplexityBot\nAllow: /\n\n\
         User-agent: Google-Extended\nAllow: /\n\n\
         Sitemap: {base}/sitemap.xml\n\n\
         # LLM content index: {base}/llms.txt\n"
    ))
}

// ads.txt — required by AdSense site verification. Served only when an
// ADSENSE_CLIENT_ID (ca-pub-…) is configured; 404 otherwise.
pub async fn ads_txt(State(state): State<AppState>) -> Response {
    let pub_id = state
        .adsense_client
        .strip_prefix("ca-pub-")
        .unwrap_or(&state.adsense_client)
        .trim();
    if pub_id.is_empty() {
        return StatusCode::NOT_FOUND.into_response();
    }
    text_response(format!(
        "google.com, pub-{pub_id}, DIRECT, f08c47fec0942fa0\n"
    ))
}

pub async fn llms(State(state): State<AppState>) -> Response {
    let base = state.app_url.trim_end_matches('/');
    text_response(format!(
        "# Blind'All Music\n\n\
         > Blind'All Music is an online multiplayer music quiz platform (blind tests). \
         Players join themed rooms, listen to audio clips and identify songs in \
         real time against other players.\n\n\
         Blind'All Music runs official public rooms (80s, 2000s, Disney, French songs, \
         rock, pop, rap…) and community-created private rooms. Playable in \
         the browser, no download. Available in French, English and Spanish. \
         Primary audience: French-speaking music fans in France and worldwide.\n\n\
         ## Core pages\n\n\
         - [Home]({base}/): Public and private rooms, mini-games, room search.\n\
         - [How to play]({base}/docs/howto): Step-by-step guide to join a room and play a blind test.\n\
         - [FAQ]({base}/docs/faq): Frequently asked questions about accounts, rooms, scores and rules.\n\
         - [Glossary]({base}/docs/glossary): Definitions of blind test, room, round, track, score, level, ELO, XP, playlist.\n\n\
         ## Progression & docs\n\n\
         - [Progression overview]({base}/docs): Level, score, ELO and XP systems.\n\
         - [Player rankings]({base}/rankings): Public leaderboard with ELO, score, level and weekly stats.\n\
         - [Level system]({base}/docs/level): How XP and levels work.\n\
         - [ELO system]({base}/docs/elo): Competitive ranking explained.\n\
         - [Create rooms & playlists]({base}/docs/create-content): Guide for hosts and playlist creators.\n\n\
         ## Optional\n\n\
         - [Mini-games]({base}/minigames): Five free music mini-games — quiz, who sang, anagram, first letter, album cover.\n\
         - [Contact]({base}/contact): Contact the Blind'All Music team.\n\
         - [Legal notice]({base}/pages/mentions-legales): Legal information (French).\n"
    ))
}

const DOC_PATHS: [(&str, &str); 18] = [
    ("docs", "0.60"),
    ("docs/howto", "0.65"),
    ("docs/glossary", "0.55"),
    ("docs/create-content", "0.55"),
    ("docs/support", "0.55"),
    ("docs/faq", "0.50"),
    ("docs/level", "0.45"),
    ("docs/elo", "0.45"),
    ("minigames", "0.70"),
    ("minigames/quiz", "0.60"),
    ("minigames/who-sang", "0.60"),
    ("minigames/anagram", "0.60"),
    ("minigames/first-letter", "0.60"),
    ("minigames/album-cover", "0.60"),
    ("duels", "0.50"),
    ("daily", "0.50"),
    ("donations", "0.30"),
    ("contact", "0.25"),
];

fn atom(dt: chrono::DateTime<chrono::Utc>) -> String {
    dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub async fn sitemap(State(state): State<AppState>) -> Response {
    let base = state.app_url.trim_end_matches('/').to_string();
    let now = atom(chrono::Utc::now());
    let mut xml = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<urlset xmlns=\"http://www.sitemaps.org/schemas/sitemap/0.9\">\n",
    );

    let mut entry = |loc: &str, lastmod: &str, freq: &str, priority: &str| {
        let _ = writeln!(
            xml,
            "  <url>\n    <loc>{loc}</loc>\n    <lastmod>{lastmod}</lastmod>\n    <changefreq>{freq}</changefreq>\n    <priority>{priority}</priority>\n  </url>"
        );
    };

    for locale in LOCALES {
        entry(&localized_path(&base, "", locale), &now, "daily", "1.00");
    }

    if let Ok(rooms) = state.seo.public_rooms().await {
        for room in rooms {
            for locale in LOCALES {
                entry(
                    &localized_path(&base, &format!("rooms/{}", room.slug), locale),
                    &atom(room.updated_at),
                    "weekly",
                    "0.90",
                );
            }
        }
    }

    for locale in LOCALES {
        entry(
            &localized_path(&base, "rankings", locale),
            &now,
            "daily",
            "0.75",
        );
    }

    if let Ok(categories) = state
        .seo
        .indexable_categories(&state.hidden_category_ids)
        .await
    {
        for cat in categories {
            for locale in LOCALES {
                entry(
                    &localized_path(&base, &format!("blind-test-{}", cat.slug), locale),
                    &atom(cat.updated_at),
                    "weekly",
                    "0.80",
                );
            }
        }
    }

    if let Ok(pages) = state.seo.pages().await {
        for page in pages {
            for locale in LOCALES {
                entry(
                    &localized_path(&base, &format!("pages/{}", page.slug), locale),
                    &atom(page.updated_at),
                    "yearly",
                    "0.30",
                );
            }
        }
    }

    for locale in LOCALES {
        for (path, priority) in DOC_PATHS {
            entry(
                &localized_path(&base, path, locale),
                &now,
                "monthly",
                priority,
            );
        }
    }

    xml.push_str("</urlset>\n");
    let mut res = xml.into_response();
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/xml; charset=utf-8"),
    );
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=300"),
    );
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localized_paths() {
        let base = "https://blindall.test";
        assert_eq!(localized_path(base, "", "fr"), "https://blindall.test/");
        assert_eq!(localized_path(base, "", "en"), "https://blindall.test/en");
        assert_eq!(
            localized_path(base, "rankings", "es"),
            "https://blindall.test/es/rankings"
        );
        assert_eq!(
            localized_path(base, "/docs/faq/", "fr"),
            "https://blindall.test/docs/faq"
        );
    }
}
