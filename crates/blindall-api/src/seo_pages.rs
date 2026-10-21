//! Server-side SEO for the SPA shell.
//!
//! The WASM app is CSR-only: without this, crawlers get a generic <head> and an
//! empty <body>. For every public route we splice a route-specific <head>
//! (title/description/robots/canonical/hreflang/OG/JSON-LD) plus a
//! `#seo-landing` HTML block into index.html — the same contract the Laravel
//! app had (`laravel/head` + `$seoLandingHtml`). The frontend removes
//! `#seo-landing` once Leptos mounts.

use crate::state::AppState;
use axum::{
    extract::State,
    http::{HeaderValue, StatusCode, Uri, header},
    response::{IntoResponse, Redirect, Response},
};
use serde_json::{Value, json};
use std::fmt::Write as _;

const DEFAULT_OG_IMAGE: &str = "/images/statics/screenshot.png";
const SITE: &str = "Blind'All Music";
const INDEX: &str = "index, follow";
const NOINDEX: &str = "noindex, nofollow";

// ---------------------------------------------------------------------------
// head model
// ---------------------------------------------------------------------------

struct Head {
    title: String,
    description: String,
    robots: &'static str,
    canonical: String,
    keywords: Option<String>,
    og_image: String,
    og_type: &'static str,
    hreflang: bool,
    json_ld: Vec<Value>,
}

impl Head {
    fn new(state: &AppState, path: &str, title: String, description: String) -> Self {
        Self {
            title,
            description,
            robots: INDEX,
            canonical: format!("{}{}", state.app_url.trim_end_matches('/'), path),
            keywords: None,
            og_image: absolute(state, DEFAULT_OG_IMAGE),
            og_type: "website",
            hreflang: true,
            json_ld: Vec::new(),
        }
    }

    fn noindex(mut self) -> Self {
        self.robots = NOINDEX;
        self
    }
}

fn absolute(state: &AppState, path: &str) -> String {
    if path.starts_with("http") {
        path.to_string()
    } else {
        format!(
            "{}{}",
            state.app_url.trim_end_matches('/'),
            if path.starts_with('/') {
                path.to_string()
            } else {
                format!("/{path}")
            }
        )
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let cut: String = s.chars().take(max - 1).collect();
    format!("{}…", cut.trim_end())
}

fn set_title(html: &mut String, title: &str) {
    if let Some(start) = html.find("<title>") {
        if let Some(end_rel) = html[start..].find("</title>") {
            html.replace_range(start + 7..start + end_rel, &esc(title));
        }
    }
}

fn set_meta(html: &mut String, attr: &str, key: &str, value: &str) {
    let needle = format!("{attr}=\"{key}\"");
    let Some(pos) = html.find(&needle) else {
        return;
    };
    let Some(cpos_rel) = html[pos..].find("content=\"") else {
        return;
    };
    let vstart = pos + cpos_rel + "content=\"".len();
    let Some(vend_rel) = html[vstart..].find('"') else {
        return;
    };
    html.replace_range(vstart..vstart + vend_rel, &esc(value));
}

fn inject_head(html: &mut String, block: &str) {
    if let Some(pos) = html.find("</head>") {
        html.insert_str(pos, block);
    }
}

fn inject_body(html: &mut String, block: &str) {
    if let Some(bpos) = html.find("<body") {
        if let Some(gpos_rel) = html[bpos..].find('>') {
            html.insert_str(bpos + gpos_rel + 1, block);
        }
    }
}

fn render(state: &AppState, shell: &str, head: &Head, landing: Option<&str>) -> String {
    let mut html = shell.to_string();
    set_title(&mut html, &head.title);
    set_meta(&mut html, "name", "description", &head.description);
    set_meta(&mut html, "name", "robots", head.robots);
    if let Some(kw) = &head.keywords {
        set_meta(&mut html, "name", "keywords", kw);
    }
    set_meta(&mut html, "property", "og:title", &head.title);
    set_meta(&mut html, "property", "og:description", &head.description);
    set_meta(&mut html, "property", "og:image", &head.og_image);
    set_meta(&mut html, "property", "og:type", head.og_type);
    set_meta(&mut html, "name", "twitter:title", &head.title);
    set_meta(&mut html, "name", "twitter:description", &head.description);
    set_meta(&mut html, "name", "twitter:image", &head.og_image);

    let mut extra = String::new();
    let _ = writeln!(
        extra,
        "    <link rel=\"canonical\" href=\"{}\" />",
        esc(&head.canonical)
    );
    let _ = writeln!(
        extra,
        "    <meta property=\"og:url\" content=\"{}\" />",
        esc(&head.canonical)
    );
    if head.hreflang {
        let base = state.app_url.trim_end_matches('/');
        let inner = head.canonical.strip_prefix(base).unwrap_or("/");
        let inner = inner
            .strip_prefix("/en")
            .or_else(|| inner.strip_prefix("/es"))
            .unwrap_or(inner);
        for locale in ["fr", "en", "es"] {
            let loc_path = if locale == "fr" {
                inner.to_string()
            } else {
                format!("/{locale}{inner}")
            };
            let _ = writeln!(
                extra,
                "    <link rel=\"alternate\" hreflang=\"{locale}\" href=\"{base}{loc_path}\" />"
            );
        }
        let _ = writeln!(
            extra,
            "    <link rel=\"alternate\" hreflang=\"x-default\" href=\"{base}{inner}\" />"
        );
    }
    for block in &head.json_ld {
        // "</" inside a <script> would close the tag — escape it as <\/.
        let json = serde_json::to_string(block)
            .unwrap_or_default()
            .replace("</", "<\\/");
        let _ = writeln!(
            extra,
            "    <script type=\"application/ld+json\">{json}</script>"
        );
    }
    inject_head(&mut html, &extra);

    if let Some(content) = landing {
        let wrapped = format!(
            "<div id=\"seo-landing\">\
             <style>#seo-landing{{font-family:system-ui,sans-serif;background:#161220;color:#e6e1ef;padding:24px;max-width:900px;margin:0 auto;line-height:1.6}}\
             #seo-landing a{{color:#a78bfa}}\
             #seo-landing h1{{font-size:1.9rem;font-weight:800;margin:0 0 12px}}\
             #seo-landing h2{{font-size:1.25rem;font-weight:700;margin:22px 0 8px}}\
             #seo-landing nav a{{margin-right:8px}}\
             #seo-landing ul{{list-style:none;padding:0}}\
             #seo-landing li{{margin:6px 0}}\
             #seo-landing .sl-links{{display:flex;flex-wrap:wrap;gap:8px}}\
             #seo-landing .sl-links a{{background:#251f38;border-radius:8px;padding:8px 12px;text-decoration:none}}\
             #seo-landing details{{margin:6px 0}}\
             #seo-landing .muted{{color:#9c93b8}}\
             #seo-landing .sl-nav{{font-size:.85rem;margin-bottom:14px}}\
             #seo-landing .sl-stats{{display:flex;gap:18px;margin:12px 0}}\
             #seo-landing .sl-stats b{{display:block;font-size:1.3rem}}</style>{content}</div>"
        );
        inject_body(&mut html, &wrapped);
    }
    html
}

// ---------------------------------------------------------------------------
// route classification
// ---------------------------------------------------------------------------

const MINIGAME_TYPES: [&str; 5] = ["quiz", "who-sang", "anagram", "first-letter", "album-cover"];

const NOINDEX_PREFIXES: [&str; 16] = [
    "/login",
    "/register",
    "/forgot-password",
    "/reset-password",
    "/verify-email",
    "/oauth",
    "/playlists",
    "/profile",
    "/me",
    "/settings",
    "/teams",
    "/admin",
    "/moderation",
    "/banned",
    "/user/banned",
    "/language",
];

fn noindex_head(state: &AppState, path: &str, title: &str) -> (StatusCode, Head, Option<String>) {
    let head = Head::new(state, path, format!("{title} | {SITE}"), String::new()).noindex();
    (StatusCode::OK, head, None)
}

fn redirect(location: &str) -> Response {
    Redirect::permanent(location).into_response()
}

fn not_found(state: &AppState, shell: &str) -> Response {
    let head = Head::new(
        state,
        "/404",
        format!("Page introuvable | {SITE}"),
        "Cette page n'existe pas.".into(),
    )
    .noindex();
    let html = render(state, shell, &head, None);
    (StatusCode::NOT_FOUND, axum::response::Html(html)).into_response()
}

// ---------------------------------------------------------------------------
// landing renderers (all FR — primary market)
// ---------------------------------------------------------------------------

fn breadcrumb(items: &[(&str, Option<&str>)]) -> String {
    let mut s = String::from("<nav class=\"sl-nav\">");
    for (i, (label, href)) in items.iter().enumerate() {
        if i > 0 {
            s.push_str(" / ");
        }
        match href {
            Some(h) => {
                let _ = write!(s, "<a href=\"{}\">{}</a>", esc(h), esc(label));
            }
            None => {
                let _ = write!(s, "<span>{}</span>", esc(label));
            }
        }
    }
    s.push_str("</nav>");
    s
}

fn links_list(items: &[(String, String)]) -> String {
    let mut s = String::from("<ul class=\"sl-links\">");
    for (href, label) in items {
        let _ = write!(s, "<li><a href=\"{}\">{}</a></li>", esc(href), esc(label));
    }
    s.push_str("</ul>");
    s
}

fn faq_dl(faq: &[(String, String)]) -> String {
    let mut s = String::new();
    for (q, a) in faq {
        let _ = write!(
            s,
            "<details><summary>{}</summary><p>{}</p></details>",
            esc(q),
            esc(a)
        );
    }
    s
}

fn faq_json_ld(faq: &[(String, String)]) -> Value {
    json!({
        "@context": "https://schema.org",
        "@type": "FAQPage",
        "mainEntity": faq.iter().map(|(q, a)| json!({
            "@type": "Question",
            "name": q,
            "acceptedAnswer": {"@type": "Answer", "text": a},
        })).collect::<Vec<_>>(),
    })
}

fn breadcrumb_json_ld(state: &AppState, items: &[(&str, String)]) -> Value {
    let base = state.app_url.trim_end_matches('/');
    json!({
        "@context": "https://schema.org",
        "@type": "BreadcrumbList",
        "itemListElement": items.iter().enumerate().map(|(i, (label, path))| json!({
            "@type": "ListItem",
            "position": i + 1,
            "name": label,
            "item": format!("{base}{path}"),
        })).collect::<Vec<_>>(),
    })
}

fn website_json_ld(state: &AppState) -> Value {
    let base = state.app_url.trim_end_matches('/');
    json!({
        "@context": "https://schema.org",
        "@type": "WebSite",
        "name": SITE,
        "url": format!("{base}/"),
        "inLanguage": ["fr", "en", "es"],
    })
}

const HOME_FAQ: [(&str, &str); 4] = [
    (
        "C'est quoi un blind test en ligne ?",
        "Un blind test (ou quiz musical) est un jeu où l'on écoute un extrait et où il faut retrouver le titre ou l'artiste le plus vite possible. En ligne, plusieurs joueurs répondent en même temps dans la même partie.",
    ),
    (
        "Est-ce que Blind'All Music est gratuit ?",
        "Oui. Toutes les salles officielles, les mini-jeux, les duels et le matchmaking sont gratuits et jouables directement dans le navigateur, sans téléchargement.",
    ),
    (
        "Faut-il créer un compte pour jouer ?",
        "Non, tu peux rejoindre une partie en invité. Un compte gratuit permet de sauvegarder ton score, ton XP, ton classement et de créer tes propres playlists.",
    ),
    (
        "Peut-on jouer entre amis ?",
        "Oui : crée une room privée, partage le lien et jouez en temps réel — ou lancez un duel asynchrone où chacun joue quand il veut.",
    ),
];

const GAME_COPY: [(&str, &str, &str); 5] = [
    (
        "quiz",
        "Quiz musical",
        "Écoute un extrait et retrouve le titre parmi 4 propositions. Dix questions, des points de rapidité, un classement en direct.",
    ),
    (
        "who-sang",
        "Qui a chanté ?",
        "Un extrait, 4 artistes : retrouve qui interprète le morceau le plus vite possible.",
    ),
    (
        "anagram",
        "Anagramme",
        "Les lettres du titre sont mélangées : reconstitue-le avant la fin du chrono.",
    ),
    (
        "first-letter",
        "Première lettre",
        "Un indice et la première lettre de chaque mot du titre : à toi de le deviner en entier.",
    ),
    (
        "album-cover",
        "Pochette d'album",
        "Une pochette floutée se dévoile progressivement : identifie l'album ou l'artiste le plus tôt possible.",
    ),
];

fn game_label(t: &str) -> (&'static str, &'static str) {
    GAME_COPY
        .iter()
        .find(|(slug, _, _)| *slug == t)
        .map(|(_, label, desc)| (*label, *desc))
        .unwrap_or(("Mini-jeu", "Mini-jeu musical Blind'All Music."))
}

async fn landing_home(state: &AppState) -> String {
    let rooms = state.rooms.public_room_cards().await.unwrap_or_default();
    let categories = state
        .seo
        .indexable_categories(&state.hidden_category_ids)
        .await
        .unwrap_or_default();
    let mut s = String::from("<main>");
    s.push_str("<h1>Blind test en ligne multijoueur gratuit</h1>");
    s.push_str(
        "<p>Blind'All Music est un <strong>blind test en ligne</strong> gratuit et multijoueur : \
         écoute un extrait, devine le titre ou l'artiste avant les autres joueurs et grimpe au \
         classement. Aucun téléchargement — tout se joue dans le navigateur, sur mobile comme \
         sur ordinateur.</p>",
    );
    s.push_str("<h2>Comment jouer ?</h2><ol>\
         <li>Choisis une salle par thème (Années 80, Disney, Rap FR, Rock, Chanson française…)</li>\
         <li>Écoute l'extrait audio et tape ta réponse le plus vite possible</li>\
         <li>Marque des points, monte de niveau et défie tes amis en duel ou en room privée</li></ol>");
    if !rooms.is_empty() {
        s.push_str("<h2>Salles de blind test par thème</h2>");
        let items: Vec<(String, String)> = rooms
            .iter()
            .filter_map(|(slug, name, _)| {
                slug.as_ref().map(|s| (format!("/rooms/{s}"), name.clone()))
            })
            .collect();
        s.push_str(&links_list(&items));
    }
    if !categories.is_empty() {
        s.push_str("<h2>Catégories de blind tests</h2>");
        let items: Vec<(String, String)> = categories
            .iter()
            .map(|c| {
                (
                    format!("/blind-test-{}", c.slug),
                    format!("Blind test {}", c.slug.replace('-', " ")),
                )
            })
            .collect();
        s.push_str(&links_list(&items));
    }
    s.push_str("<h2>Aussi sur Blind'All Music</h2>");
    s.push_str(&links_list(&[
        ("/minigames".into(), "Mini-jeux musicaux".into()),
        ("/duels".into(), "Duels entre joueurs".into()),
        ("/daily".into(), "Défi du jour".into()),
        ("/rankings".into(), "Classements".into()),
        ("/docs/howto".into(), "Comment jouer".into()),
        ("/docs/faq".into(), "FAQ".into()),
    ]));
    s.push_str("<h2>Questions fréquentes</h2>");
    s.push_str(&faq_dl(
        &HOME_FAQ
            .iter()
            .map(|(q, a)| (q.to_string(), a.to_string()))
            .collect::<Vec<_>>(),
    ));
    s.push_str("</main>");
    s
}

async fn landing_room(state: &AppState, row: &blindall_db::RoomRow) -> String {
    let slug = row.slug.clone().unwrap_or_else(|| row.id.to_string());
    let mut s = String::from("<main>");
    let category = match row.category_id {
        Some(cid) => state.rooms.category_name_slug(cid).await.ok().flatten(),
        None => None,
    };
    let mut crumbs: Vec<(&str, Option<String>)> = vec![("Accueil", Some("/".into()))];
    if let Some((cname, cslug)) = &category {
        crumbs.push(("Blind tests", None));
        let _ = cname;
        crumbs.push(("Catégorie", Some(format!("/blind-test-{cslug}"))));
    }
    crumbs.push((&row.name, None));
    s.push_str(&breadcrumb(
        &crumbs
            .iter()
            .map(|(l, h)| (*l, h.as_deref()))
            .collect::<Vec<_>>(),
    ));
    let _ = write!(s, "<h1>{} — Blind test en ligne</h1>", esc(&row.name));

    let intro = crate::official_seo::configured_intro(&slug, "fr")
        .and_then(|i| i.intro)
        .or_else(|| row.description.clone())
        .unwrap_or_else(|| {
            format!(
                "Rejoins la salle {} et joue au blind test en direct contre d'autres joueurs.",
                row.name
            )
        });
    let _ = write!(s, "<p>{}</p>", esc(&intro));
    if let Some(sec) =
        crate::official_seo::configured_intro(&slug, "fr").and_then(|i| i.intro_secondary)
    {
        let _ = write!(s, "<p>{}</p>", esc(&sec));
    }

    let tracks = state.rooms.tracks_count(row.id).await.unwrap_or(0);
    let rounds = state.rooms.rounds_count(row.id).await.unwrap_or(0);
    let members = state
        .presence
        .member_count(row.id, chrono::Utc::now().timestamp())
        .await
        .unwrap_or(0);
    let _ = write!(
        s,
        "<div class=\"sl-stats\"><div><b>{tracks}</b><span class=\"muted\">titres</span></div>\
         <div><b>{rounds}</b><span class=\"muted\">manches jouées</span></div>\
         <div><b>{members}</b><span class=\"muted\">en ligne</span></div></div>"
    );
    s.push_str(&format!(
        "<p><a href=\"/rooms/{slug}\">▶ Rejoindre la salle et jouer maintenant</a></p>"
    ));

    if let Some(cid) = row.category_id {
        let similar = state
            .rooms
            .official_similar_rooms(cid, row.id, crate::official_seo::slugs(), 6)
            .await
            .unwrap_or_default();
        if !similar.is_empty() {
            s.push_str("<h2>Autres salles de blind test</h2>");
            let items: Vec<(String, String)> = similar
                .iter()
                .filter_map(|(rslug, name, _)| {
                    rslug
                        .as_ref()
                        .map(|s| (format!("/rooms/{s}"), name.clone()))
                })
                .collect();
            s.push_str(&links_list(&items));
        }
    }
    s.push_str("<h2>Questions fréquentes</h2>");
    let faq = vec![
        (
            format!("Comment jouer au blind test {} ?", row.name),
            "Rejoins la salle, écoute l'extrait et tape le titre ou l'artiste le plus vite \
             possible. Chaque manche joue plusieurs morceaux, les réponses les plus rapides \
             rapportent le plus de points."
                .to_string(),
        ),
        (
            "Faut-il un compte ?".to_string(),
            "Non, tu peux jouer en invité. Le compte gratuit conserve ton score et ton classement."
                .to_string(),
        ),
    ];
    s.push_str(&faq_dl(&faq));
    s.push_str("<h2>Voir aussi</h2>");
    s.push_str(&links_list(&[
        ("/".into(), "Toutes les salles".into()),
        ("/minigames".into(), "Mini-jeux".into()),
        ("/rankings".into(), "Classements".into()),
    ]));
    s.push_str("</main>");
    s
}

fn doc_head(path: &str) -> Option<(&'static str, &'static str, &'static str, &'static str)> {
    // (title, description, h1, intro) — FR
    let v = match path {
        "docs" => (
            "Niveau, score, ELO et XP — les systèmes de progression | Blind'All Music",
            "Comprends les systèmes de progression de Blind'All Music : XP, niveaux, score par manche et classement ELO.",
            "Les systèmes de progression",
            "Blind'All Music mesure ta progression sur quatre systèmes complémentaires : l'XP cumulée, le niveau, le score par manche et le classement ELO compétitif.",
        ),
        "docs/level" => (
            "Système de niveaux et XP | Blind'All Music",
            "Comment fonctionnent l'XP et les niveaux sur Blind'All Music : gains par partie, paliers et récompenses.",
            "Niveaux et XP",
            "Chaque partie rapporte de l'XP qui fait progresser ton niveau. Plus tu réponds vite et juste, plus tu gagnes d'expérience.",
        ),
        "docs/elo" => (
            "Système ELO — classement compétitif | Blind'All Music",
            "Le classement ELO de Blind'All Music expliqué : calcul, gains, pertes et comment progresser.",
            "Système ELO",
            "L'ELO mesure ton niveau relatif face aux autres joueurs : chaque victoire fait monter ton score, chaque défaite le fait baisser.",
        ),
        "docs/glossary" => (
            "Glossaire du blind test | Blind'All Music",
            "Définitions des termes de Blind'All Music : blind test, room, manche, extrait, score, ELO, XP, playlist.",
            "Glossaire Blind'All Music",
            "Toutes les définitions des termes utilisés sur Blind'All Music : salles, manches, extraits, scores et classements.",
        ),
        "docs/howto" => (
            "Comment jouer au blind test en ligne | Blind'All Music",
            "Guide pas à pas pour jouer à un blind test en ligne : rejoindre une salle, écouter les extraits et répondre.",
            "Comment jouer ?",
            "Rejoins une salle publique, écoute chaque extrait et tape ta réponse avant la fin du chrono. Voici le guide complet, pas à pas.",
        ),
        "docs/create-content" => (
            "Créer une room et une playlist | Blind'All Music",
            "Guide pour créer tes propres rooms et playlists de blind test : import de titres, réponses acceptées et partage.",
            "Créer du contenu",
            "Crée ta propre salle de blind test, importe ta playlist et partage le lien à tes amis pour jouer ensemble.",
        ),
        "docs/support" => (
            "Support et aide | Blind'All Music",
            "Besoin d'aide sur Blind'All Music ? Support, signalement de bug et contact de l'équipe.",
            "Support",
            "Une question ou un bug ? Retrouve ici comment contacter l'équipe Blind'All Music et obtenir de l'aide.",
        ),
        _ => return None,
    };
    Some(v)
}

// ---------------------------------------------------------------------------
// main handler
// ---------------------------------------------------------------------------

pub async fn spa_page(State(state): State<AppState>, uri: Uri) -> Response {
    let path = uri.path().trim_end_matches('/');
    let path = if path.is_empty() { "/" } else { path };

    // Trailing-slash normalization before classification.
    if uri.path() != path {
        let loc = uri
            .path_and_query()
            .map(|pq| {
                let q = pq.query().unwrap_or_default();
                if q.is_empty() {
                    path.to_string()
                } else {
                    format!("{path}?{q}")
                }
            })
            .unwrap_or_else(|| path.to_string());
        return redirect(&loc);
    }

    let shell = match tokio::fs::read_to_string(state.spa_dir.join("index.html")).await {
        Ok(s) => s,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };

    // Locale-prefixed URLs (/en/*, /es/*) serve localized head; the inner path
    // classifies the same way. Canonical stays self-prefixed (PHP parity).
    let (locale, inner) = match path.split('/').nth(1) {
        Some("en") => ("en", path.strip_prefix("/en").unwrap_or("/")),
        Some("es") => ("es", path.strip_prefix("/es").unwrap_or("/")),
        _ => ("fr", path),
    };
    let inner = if inner.is_empty() { "/" } else { inner };

    let decision = decide(&state, inner, locale).await;

    match decision {
        Decision::Redirect(loc) => redirect(&loc),
        Decision::NotFound => not_found(&state, &shell),
        Decision::Page(status, mut head, landing) => {
            // Locale-prefixed URLs keep a self-referencing canonical (PHP
            // parity): /en/rooms/x canonicalizes to /en/rooms/x, not /rooms/x.
            if locale != "fr" {
                let base = state.app_url.trim_end_matches('/');
                if let Some(rest) = head.canonical.strip_prefix(base) {
                    head.canonical = format!("{base}/{locale}{rest}");
                }
            }
            let html = render(&state, &shell, &head, landing.as_deref());
            let mut res = (status, axum::response::Html(html)).into_response();
            if head.robots == NOINDEX {
                res.headers_mut().insert(
                    header::HeaderName::from_static("x-robots-tag"),
                    HeaderValue::from_static("noindex, nofollow"),
                );
            }
            res
        }
    }
}

enum Decision {
    Page(StatusCode, Head, Option<String>),
    Redirect(String),
    NotFound,
}

async fn decide(state: &AppState, path: &str, _locale: &str) -> Decision {
    // Fast path: explicit noindex prefixes (member/admin/utility pages).
    for p in NOINDEX_PREFIXES {
        if path == p || path.starts_with(&format!("{p}/")) {
            let (st, h, l) = noindex_head(state, path, SITE);
            return Decision::Page(st, h, l);
        }
    }

    // Static redirects.
    match path {
        "/faq" => return Decision::Redirect("/docs/faq".into()),
        "/games" => return Decision::Redirect("/minigames".into()),
        "/language" => return Decision::Redirect("/".into()),
        _ => {}
    }
    if let Some(rest) = path.strip_prefix("/games/") {
        return Decision::Redirect(format!("/minigames/{rest}"));
    }
    if let Some(sort) = path.strip_prefix("/rankings/") {
        if ["level", "score", "elo", "week"].contains(&sort) {
            return Decision::Redirect(format!("/rankings?sort={sort}"));
        }
    }

    let segs: Vec<&str> = path.trim_matches('/').split('/').collect();

    // Rooms — before the single-segment catch-all.
    if segs[0] == "rooms" && segs.len() >= 2 {
        let key = segs[1];
        if segs.len() == 2 {
            return room_decision(state, key).await;
        }
        // /rooms/{key}/live|edit → thin member/utility pages.
        let (st, h, l) = noindex_head(state, path, SITE);
        return Decision::Page(st, h, l);
    }
    if segs[0] == "rooms" {
        let (st, h, l) = noindex_head(state, path, SITE);
        return Decision::Page(st, h, l);
    }

    match (segs[0], segs.len()) {
        ("", 1) => home_decision(state, path).await,
        ("docs", 1) => docs_decision(state, path, "docs").unwrap_or(Decision::NotFound),
        ("docs", 2) => {
            if segs[1] == "faq" {
                faq_decision(state, path).await
            } else {
                docs_decision(state, path, &format!("docs/{}", segs[1]))
                    .unwrap_or(Decision::NotFound)
            }
        }
        ("contact", 1) => simple_decision(
            state,
            path,
            "Contact | Blind'All Music",
            "Contacte l'équipe Blind'All Music : bug, suggestion, playlist ou question de modération.",
            "Contact",
            "<p>Une question, un bug à signaler ou une suggestion de titre ? Envoie un message à l'équipe — pour ajouter ou modifier des titres dans les playlists publiques, contacte directement les modérateurs.</p>",
        ),
        ("rankings", 1) => rankings_decision(state, path).await,
        ("rankings", 2) => match segs[1] {
            "season" | "teams" | "minigames" => rankings_decision(state, path).await,
            _ => Decision::NotFound,
        },
        ("minigames", 1) => minigames_decision(state, path),
        ("minigames", 2) => {
            if MINIGAME_TYPES.contains(&segs[1]) {
                minigame_play_decision(state, path, segs[1])
            } else {
                Decision::NotFound
            }
        }
        ("match", 2) => {
            if MINIGAME_TYPES.contains(&segs[1]) {
                match_decision(state, path, segs[1])
            } else {
                Decision::NotFound
            }
        }
        ("duels", 1) => simple_decision(
            state,
            path,
            "Duels de blind test asynchrones | Blind'All Music",
            "Défie un joueur en duel de blind test : les mêmes 10 questions pour les deux, chacun joue quand il veut. Classement au score.",
            "Duels",
            "<p>Crée un défi ouvert ou cible un joueur : vous répondez chacun aux mêmes 10 questions, à votre rythme. Le meilleur score gagne le duel — revanche possible en un clic.</p>",
        ),
        ("daily", 1) => simple_decision(
            state,
            path,
            "Défi du jour — blind test quotidien | Blind'All Music",
            "Le défi du jour de Blind'All Music : les mêmes 10 questions pour tout le monde, un classement quotidien. Joue chaque jour !",
            "Défi du jour",
            "<p>Chaque jour, les mêmes 10 questions pour tous les joueurs. Une seule partie par jour, un classement quotidien — reviens demain pour défendre ton rang.</p>",
        ),
        ("donations", 1) => simple_decision(
            state,
            path,
            "Soutenir Blind'All Music | Dons",
            "Blind'All Music est un projet indépendant et gratuit. Les dons financent les serveurs et le développement du jeu.",
            "Soutenir Blind'All Music",
            "<p>Blind'All Music est gratuit et indépendant : les dons couvrent l'hébergement, la bande passante audio et le développement. Merci pour ton soutien !</p>",
        ),
        ("pages", 2) => static_page_decision(state, segs[1]).await,
        ("duels", 2) => {
            // /duels/:id — member-gated play page.
            let (st, h, l) = noindex_head(state, path, "Duel | Blind'All Music");
            Decision::Page(st, h, l)
        }
        _ => {
            // Single-segment catch-all: category landings /blind-test-{slug}.
            if segs.len() == 1 && segs[0].starts_with("blind-test-") {
                return category_decision(state, &segs[0]["blind-test-".len()..]).await;
            }
            Decision::NotFound
        }
    }
}

// ---------------------------------------------------------------------------
// per-route decisions
// ---------------------------------------------------------------------------

async fn home_decision(state: &AppState, path: &str) -> Decision {
    let mut head = Head::new(
        state,
        path,
        format!("Blind test en ligne multijoueur gratuit | {SITE}"),
        "Joue au blind test en ligne gratuit contre d'autres joueurs : Années 80, 90, 2000, Disney, Rap FR, Rock, Pop, Chanson française. Multijoueur, sans téléchargement, dans le navigateur.".into(),
    );
    head.keywords = Some("blind test, blindtest, blind test en ligne, blind test gratuit, quiz musical, quiz musique, blind test multijoueur, blind test entre amis, jeu musical en ligne".into());
    let faq: Vec<(String, String)> = HOME_FAQ
        .iter()
        .map(|(q, a)| (q.to_string(), a.to_string()))
        .collect();
    head.json_ld.push(website_json_ld(state));
    head.json_ld.push(json!({
        "@context": "https://schema.org",
        "@type": "Organization",
        "name": SITE,
        "url": format!("{}/", state.app_url.trim_end_matches('/')),
        "logo": absolute(state, "/images/statics/blindall-music-logo.png"),
    }));
    head.json_ld.push(faq_json_ld(&faq));
    Decision::Page(StatusCode::OK, head, Some(landing_home(state).await))
}

async fn room_decision(state: &AppState, key: &str) -> Decision {
    let row = match key.parse::<i64>() {
        Ok(id) => state.rooms.find(id).await,
        Err(_) => state.rooms.find_by_slug(key).await,
    };
    let Ok(Some(row)) = row else {
        return Decision::NotFound;
    };
    if row.deleted_at.is_some() || !row.is_active {
        return Decision::NotFound;
    }
    let slug = row.slug.clone().unwrap_or_else(|| row.id.to_string());

    // Numeric URL → canonical slug URL (dedupe, 301).
    if key.parse::<i64>().is_ok() && !slug.is_empty() && slug != key {
        return Decision::Redirect(format!("/rooms/{slug}"));
    }

    let intro = crate::official_seo::configured_intro(&slug, "fr")
        .and_then(|i| i.intro)
        .or_else(|| row.description.clone());
    let description = truncate(
        &intro.unwrap_or_else(|| {
            format!(
                "Joue au blind test {} en ligne : écoute les extraits, trouve le titre et l'artiste avant les autres joueurs. Gratuit, multijoueur, dans le navigateur.",
                row.name
            )
        }),
        160,
    );
    let mut head = Head::new(
        state,
        &format!("/rooms/{slug}"),
        format!("{} : Blind test en ligne | {SITE}", row.name),
        description,
    );
    if let Some(p) = row.photo_path.as_deref() {
        head.og_image = absolute(state, &state.storage.url(p));
    }
    if row.has_password {
        head.robots = NOINDEX;
        return Decision::Page(StatusCode::OK, head, None);
    }

    // JSON-LD: VideoGame + Breadcrumb + FAQ (official rooms get rich copy).
    let mut crumbs = vec![("Accueil".to_string(), "/".to_string())];
    if let Some(cid) = row.category_id {
        if let Ok(Some((cname, cslug))) = state.rooms.category_name_slug(cid).await {
            crumbs.push((cname, format!("/blind-test-{cslug}")));
        }
    }
    crumbs.push((row.name.clone(), format!("/rooms/{slug}")));
    let faq = vec![
        (
            format!("Comment jouer au blind test {} ?", row.name),
            "Rejoins la salle, écoute l'extrait et tape le titre ou l'artiste le plus vite possible."
                .to_string(),
        ),
        (
            "Faut-il un compte ?".to_string(),
            "Non, tu peux jouer en invité. Le compte gratuit conserve ton score.".to_string(),
        ),
    ];
    head.json_ld.push(json!({
        "@context": "https://schema.org",
        "@type": "VideoGame",
        "name": format!("{} — Blind'All Music", row.name),
        "description": head.description,
        "url": head.canonical,
        "image": head.og_image,
        "playMode": "MultiPlayer",
        "applicationCategory": "Game",
        "operatingSystem": "Web",
        "isAccessibleForFree": true,
        "inLanguage": ["fr", "en", "es"],
        "offers": {"@type": "Offer", "price": 0, "priceCurrency": "EUR"},
        "publisher": {"@type": "Organization", "name": SITE},
    }));
    head.json_ld.push(breadcrumb_json_ld(
        state,
        &crumbs
            .iter()
            .map(|(l, p)| (l.as_str(), p.clone()))
            .collect::<Vec<_>>(),
    ));
    head.json_ld.push(faq_json_ld(&faq));
    Decision::Page(StatusCode::OK, head, Some(landing_room(state, &row).await))
}

async fn category_decision(state: &AppState, slug: &str) -> Decision {
    let Some((cat_id, cat_name, cat_slug)) =
        state.rooms.category_by_slug(slug).await.ok().flatten()
    else {
        return Decision::NotFound;
    };
    let rooms = state
        .rooms
        .public_for_category(cat_id)
        .await
        .unwrap_or_default();
    if rooms.is_empty() {
        return Decision::NotFound;
    }
    let names: Vec<String> = rooms.iter().map(|r| r.name.clone()).collect();
    let c = crate::category_content::content(
        &cat_slug,
        &cat_name,
        rooms.len() as i64,
        &names.join(", "),
    );

    let path = format!("/blind-test-{cat_slug}");
    let mut head = Head::new(
        state,
        &path,
        c.meta_title.clone(),
        c.meta_description.clone(),
    );
    let faq: Vec<(String, String)> = c.faq.clone();
    head.json_ld.push(breadcrumb_json_ld(
        state,
        &[
            ("Accueil", "/".to_string()),
            (cat_name.as_str(), path.clone()),
        ],
    ));
    head.json_ld.push(faq_json_ld(&faq));
    head.json_ld.push(json!({
        "@context": "https://schema.org",
        "@type": "CollectionPage",
        "name": c.heading,
        "description": c.meta_description,
        "url": head.canonical,
        "mainEntity": {
            "@type": "ItemList",
            "itemListElement": rooms.iter().enumerate().filter_map(|(i, r)| {
                r.slug.as_ref().map(|s| json!({
                    "@type": "ListItem",
                    "position": i + 1,
                    "name": r.name,
                    "url": format!("{}/rooms/{}", state.app_url.trim_end_matches('/'), s),
                }))
            }).collect::<Vec<_>>(),
        },
    }));

    let mut landing = String::from("<main>");
    landing.push_str(&breadcrumb(&[
        ("Accueil", Some("/")),
        ("Blind tests", None),
        (&cat_name, None),
    ]));
    let _ = write!(landing, "<h1>{}</h1>", esc(&c.heading));
    let _ = write!(landing, "<p>{}</p>", esc(&c.intro));
    let _ = write!(landing, "<p>{}</p>", esc(&c.intro_secondary));
    let _ = write!(landing, "<h2>{}</h2>", esc(&c.rooms_heading));
    let items: Vec<(String, String)> = rooms
        .iter()
        .filter_map(|r| {
            r.slug
                .as_ref()
                .map(|s| (format!("/rooms/{s}"), r.name.clone()))
        })
        .collect();
    landing.push_str(&links_list(&items));
    let _ = write!(landing, "<h2>{}</h2>", esc(&c.faq_title));
    landing.push_str(&faq_dl(&faq));
    landing.push_str("</main>");
    Decision::Page(StatusCode::OK, head, Some(landing))
}

fn docs_decision(state: &AppState, path: &str, key: &str) -> Option<Decision> {
    let (title, desc, h1, intro) = doc_head(key)?;
    let head = Head::new(state, path, title.to_string(), desc.to_string());
    let mut landing = String::from("<main>");
    landing.push_str(&breadcrumb(&[
        ("Accueil", Some("/")),
        ("Documentation", Some("/docs")),
        (h1, None),
    ]));
    let _ = write!(landing, "<h1>{h1}</h1><p>{intro}</p>");
    landing.push_str(&links_list(&[
        ("/docs".into(), "Progression".into()),
        ("/docs/level".into(), "Niveaux et XP".into()),
        ("/docs/elo".into(), "Système ELO".into()),
        ("/docs/glossary".into(), "Glossaire".into()),
        ("/docs/howto".into(), "Comment jouer".into()),
        ("/docs/create-content".into(), "Créer une room".into()),
        ("/docs/support".into(), "Support".into()),
    ]));
    landing.push_str("</main>");
    Some(Decision::Page(StatusCode::OK, head, Some(landing)))
}

async fn faq_decision(state: &AppState, path: &str) -> Decision {
    let faqs = state
        .faqs
        .list(None, 1, 25)
        .await
        .map(|(rows, _)| rows)
        .unwrap_or_default();
    let faq: Vec<(String, String)> = faqs
        .iter()
        .map(|f| {
            (
                f["question"].as_str().unwrap_or_default().to_string(),
                f["answer"].as_str().unwrap_or_default().to_string(),
            )
        })
        .filter(|(q, _)| !q.is_empty())
        .collect();
    let mut head = Head::new(
        state,
        path,
        format!("FAQ — Questions fréquentes | {SITE}"),
        "Questions fréquentes sur Blind'All Music : compte, salles, scores, règles, playlists et fonctionnement du blind test en ligne.".into(),
    );
    if !faq.is_empty() {
        head.json_ld.push(faq_json_ld(&faq));
    }
    let mut landing = String::from(
        "<main><h1>FAQ — Questions fréquentes</h1>\
         <p>Tout ce qu'il faut savoir sur Blind'All Music : compte, salles, scores, règles et fonctionnement du jeu.</p>",
    );
    landing.push_str(&faq_dl(&faq));
    landing.push_str("</main>");
    Decision::Page(StatusCode::OK, head, Some(landing))
}

async fn rankings_decision(state: &AppState, _path: &str) -> Decision {
    let head = Head::new(
        state,
        "/rankings",
        format!("Classements des joueurs | {SITE}"),
        "Le classement public des joueurs de Blind'All Music : tri par ELO, score, niveau ou top hebdo dans les salles officielles.".into(),
    );
    let landing = "<main><h1>Classements</h1>\
        <p>Le classement public des joueurs de Blind'All Music : ELO, score cumulé, niveau et \
        top de la semaine dans les salles officielles. Joue aux blind tests en ligne pour \
        grimper au classement.</p></main>"
        .to_string();
    Decision::Page(StatusCode::OK, head, Some(landing))
}

fn minigames_decision(state: &AppState, path: &str) -> Decision {
    let head = Head::new(
        state,
        path,
        format!("Mini-jeux musicaux en ligne | {SITE}"),
        "Cinq mini-jeux musicaux gratuits : quiz à 4 choix, qui a chanté, anagramme, première lettre et pochette d'album. En solo, en duel ou en matchmaking en ligne.".into(),
    );
    let mut landing = String::from(
        "<main><h1>Mini-jeux musicaux</h1>\
         <p>Cinq mini-jeux de blind test gratuits, jouables en solo, en duel entre amis ou en \
         matchmaking en ligne contre d'autres joueurs.</p><ul>",
    );
    for (slug, label, desc) in GAME_COPY {
        let _ = write!(
            landing,
            "<li><a href=\"/minigames/{slug}\"><strong>{label}</strong></a> — {} \
             <a href=\"/match/{slug}\">Jouer en ligne</a></li>",
            esc(desc)
        );
    }
    landing.push_str("</ul>");
    landing.push_str(&links_list(&[
        ("/duels".into(), "Duels entre joueurs".into()),
        ("/daily".into(), "Défi du jour".into()),
        ("/rankings/minigames".into(), "Classement mini-jeux".into()),
    ]));
    landing.push_str("</main>");
    Decision::Page(StatusCode::OK, head, Some(landing))
}

fn minigame_play_decision(state: &AppState, path: &str, gtype: &str) -> Decision {
    let (label, desc) = game_label(gtype);
    let head = Head::new(
        state,
        path,
        format!("{label} — mini-jeu musical en ligne | {SITE}"),
        format!("{desc} Joue gratuitement dans le navigateur, en solo ou en multijoueur en ligne."),
    );
    let landing = format!(
        "<main><h1>{label}</h1><p>{}</p>\
         <p><a href=\"/match/{gtype}\">▶ Jouer en ligne contre d'autres joueurs</a> · \
         <a href=\"/minigames/{gtype}\">Jouer en solo</a></p>\
         <p><a href=\"/minigames\">Tous les mini-jeux</a></p></main>",
        esc(desc)
    );
    Decision::Page(StatusCode::OK, head, Some(landing))
}

fn match_decision(state: &AppState, path: &str, gtype: &str) -> Decision {
    let (label, _) = game_label(gtype);
    let head = Head::new(
        state,
        path,
        format!("{label} en ligne multijoueur — matchmaking | {SITE}"),
        format!(
            "Rejoins la file de matchmaking {label} : un match en direct se lance dès qu'un autre joueur est trouvé. 10 questions, scores en temps réel, revanche en un clic."
        ),
    );
    let landing = format!(
        "<main><h1>{label} — match en ligne</h1>\
         <p>Le matchmaking Blind'All Music trouve un adversaire en quelques secondes : 10 questions \
         en direct, le score de chacun s'affiche en temps réel, puis la revanche se lance en un clic.</p>\
         <p><a href=\"/match/{gtype}\">▶ Rejoindre la file d'attente</a></p>\
         <p><a href=\"/minigames\">Tous les mini-jeux</a></p></main>"
    );
    Decision::Page(StatusCode::OK, head, Some(landing))
}

fn simple_decision(
    state: &AppState,
    path: &str,
    title: &str,
    desc: &str,
    h1: &str,
    body: &str,
) -> Decision {
    let head = Head::new(state, path, title.to_string(), desc.to_string());
    let landing = format!("<main><h1>{h1}</h1>{body}</main>");
    Decision::Page(StatusCode::OK, head, Some(landing))
}

async fn static_page_decision(state: &AppState, slug: &str) -> Decision {
    let Ok(Some(page)) = state.admin.page_by_slug(slug).await else {
        return Decision::NotFound;
    };
    let title = page["title"].as_str().unwrap_or(SITE).to_string();
    let content = page["content"].as_str().unwrap_or_default();
    let desc = truncate(
        &content
            .replace(|c| c == '<' || c == '>', " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
        160,
    );
    let head = Head::new(
        state,
        &format!("/pages/{slug}"),
        format!("{title} | {SITE}"),
        if desc.is_empty() {
            format!("{title} — Blind'All Music")
        } else {
            desc
        },
    );
    // Page content is admin-authored HTML — inject raw (parity with the SPA's
    // inner_html rendering).
    let landing = format!("<main><h1>{}</h1>{content}</main>", esc(&title));
    Decision::Page(StatusCode::OK, head, Some(landing))
}
