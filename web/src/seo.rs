//! Client-side `Laravel\Head` parity — the PHP app renders title/description/
//! canonical/OG/Twitter/JSON-LD server-side per page; the SPA does the same by
//! patching `<head>` at runtime. Injected tags carry `data-seo-injected` and
//! static tags temporarily overridden carry `data-seo-managed`, so `reset()`
//! restores the `index.html` defaults on route change.

use std::sync::OnceLock;
use wasm_bindgen::JsCast;

fn document() -> Option<web_sys::Document> {
    web_sys::window()?.document()
}

/// Static `<head>` defaults captured at boot (selector → content); `reset()`
/// replays them. Covers every static meta index.html ships.
const STATIC_META: &[(&str, &str)] = &[
    ("name", "description"),
    ("name", "robots"),
    ("property", "og:title"),
    ("property", "og:description"),
    ("property", "og:image"),
    ("property", "og:url"),
    ("property", "og:locale"),
    ("name", "twitter:title"),
    ("name", "twitter:description"),
    ("name", "twitter:image"),
];

fn defaults() -> &'static Vec<(String, String)> {
    static D: OnceLock<Vec<(String, String)>> = OnceLock::new();
    D.get_or_init(|| {
        let Some(doc) = document() else {
            return Vec::new();
        };
        STATIC_META
            .iter()
            .filter_map(|(attr, key)| {
                let sel = format!("meta[{attr}=\"{key}\"]");
                let el = doc.query_selector(&sel).ok()??;
                Some((sel, el.get_attribute("content").unwrap_or_default()))
            })
            .collect()
    })
}

fn head_el(doc: &web_sys::Document) -> Option<web_sys::Element> {
    doc.head().map(|h| h.unchecked_into::<web_sys::Element>())
}

/// Upsert `<meta attr="key" content="…">`. Static tags are flagged managed,
/// missing ones are injected (removed by `reset`).
pub fn set_meta(attr: &str, key: &str, content: &str) {
    let Some(doc) = document() else { return };
    let sel = format!("meta[{attr}=\"{key}\"]");
    match doc.query_selector(&sel).ok().flatten() {
        Some(el) => {
            let _ = el.set_attribute("data-seo-managed", "1");
            let _ = el.set_attribute("content", content);
        }
        None => {
            let Ok(el) = doc.create_element("meta") else { return };
            let _ = el.set_attribute(attr, key);
            let _ = el.set_attribute("content", content);
            let _ = el.set_attribute("data-seo-injected", "1");
            if let Some(h) = head_el(&doc) {
                let _ = h.append_child(&el);
            }
        }
    }
}

/// Upsert `<link rel="canonical">` (`LocaleUrl::canonical`).
pub fn set_canonical(href: &str) {
    let Some(doc) = document() else { return };
    match doc.query_selector("link[rel=\"canonical\"]").ok().flatten() {
        Some(el) => {
            let _ = el.set_attribute("data-seo-managed", "1");
            let _ = el.set_attribute("href", href);
        }
        None => {
            let Ok(el) = doc.create_element("link") else { return };
            let _ = el.set_attribute("rel", "canonical");
            let _ = el.set_attribute("href", href);
            let _ = el.set_attribute("data-seo-injected", "1");
            if let Some(h) = head_el(&doc) {
                let _ = h.append_child(&el);
            }
        }
    }
}

/// Upsert `<link rel="alternate" hreflang="…">` (`LocaleUrl::alternates`).
pub fn set_hreflang(lang: &str, href: &str) {
    let Some(doc) = document() else { return };
    let sel = format!("link[rel=\"alternate\"][hreflang=\"{lang}\"]");
    if doc.query_selector(&sel).ok().flatten().is_none() {
        let Ok(el) = doc.create_element("link") else { return };
        let _ = el.set_attribute("rel", "alternate");
        let _ = el.set_attribute("hreflang", lang);
        let _ = el.set_attribute("data-seo-injected", "1");
        if let Some(h) = head_el(&doc) {
            let _ = h.append_child(&el);
        }
    }
    if let Some(el) = doc.query_selector(&sel).ok().flatten() {
        let _ = el.set_attribute("href", href);
    }
}

/// Upsert a JSON-LD `<script>` keyed by `data-seo-ld="{id}"`.
pub fn set_json_ld(id: &str, json: &serde_json::Value) {
    let Some(doc) = document() else { return };
    let sel = format!("script[data-seo-ld=\"{id}\"]");
    let text = json.to_string();
    match doc.query_selector(&sel).ok().flatten() {
        Some(el) => el.set_text_content(Some(&text)),
        None => {
            let Ok(el) = doc.create_element("script") else { return };
            let _ = el.set_attribute("type", "application/ld+json");
            let _ = el.set_attribute("data-seo-ld", id);
            let _ = el.set_attribute("data-seo-injected", "1");
            el.set_text_content(Some(&text));
            if let Some(h) = head_el(&doc) {
                let _ = h.append_child(&el);
            }
        }
    }
}

/// `Head::hiddenFromRobots` for password-protected rooms.
pub fn set_noindex() {
    set_meta("name", "robots", "noindex");
}

/// Restore `index.html` head state — remove injected tags, reset managed
/// ones to their boot defaults. Runs on every route change.
pub fn reset() {
    let Some(doc) = document() else { return };
    if let Ok(nodes) = doc.query_selector_all("[data-seo-injected]") {
        for i in 0..nodes.length() {
            if let Some(el) = nodes.item(i).and_then(|n| n.dyn_into::<web_sys::Element>().ok()) {
                el.remove();
            }
        }
    }
    for (sel, content) in defaults() {
        if let Ok(Some(el)) = doc.query_selector(sel) {
            let _ = el.set_attribute("content", content);
            let _ = el.remove_attribute("data-seo-managed");
        }
    }
}

/// `LocaleUrl` — canonical + hreflang alternates for the current locale.
/// `path` is the locale-free path (`/rooms/x`); canonical gets the active
/// locale prefix (`/en`, `/es`, `""` for fr).
pub fn canonical_url(path: &str) -> String {
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    let prefix = match crate::i18n::locale() {
        "en" => "/en",
        "es" => "/es",
        _ => "",
    };
    format!("{origin}{prefix}{path}")
}

/// `LocaleUrl::alternates` — fr/en/es + x-default (fr path, `x-default`
/// points at the unprefixed URL).
pub fn apply_alternates(path: &str) {
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    set_hreflang("fr", &format!("{origin}{path}"));
    set_hreflang("en", &format!("{origin}/en{path}"));
    set_hreflang("es", &format!("{origin}/es{path}"));
    set_hreflang("x-default", &format!("{origin}{path}"));
}

/// Generic page head — every PHP `*Head` service does title + description +
/// canonical; call after `i18n::set_title`.
pub fn apply_page(path: &str, description: &str) {
    set_meta("name", "description", description);
    set_canonical(&canonical_url(path));
    apply_alternates(path);
    set_meta("property", "og:description", description);
    set_meta("name", "twitter:description", description);
    set_meta("property", "og:url", &canonical_url(path));
    set_meta("property", "og:locale", og_locale());
}

/// `RoomHead::ogLocale`.
pub fn og_locale() -> &'static str {
    match crate::i18n::locale() {
        "en" => "en_US",
        "es" => "es_ES",
        _ => "fr_FR",
    }
}

/// Truncate like PHP `mb_substr($s, 0, 157).'...'` (160-char meta cap).
pub fn truncate160(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() > 160 {
        format!("{}...", chars[..157].iter().collect::<String>())
    } else {
        s.to_string()
    }
}
