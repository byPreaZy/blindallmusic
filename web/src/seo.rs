use wasm_bindgen::JsCast;

fn document() -> Option<web_sys::Document> {
    web_sys::window()?.document()
}

// Site-wide default head values — identical to the static index.html shell.
// These are constants (not a DOM snapshot) because by the time a page has
// called set_meta once, the live DOM no longer holds the defaults.
const DEFAULT_META: &[(&str, &str, &str)] = &[
    ("name", "description", "Des quiz musicaux multijoueurs en ligne. Teste ta culture sonore face à d'autres joueurs : Années 80, Disney, Rap, Rock, Pop, Chanson française et bien d'autres salles."),
    ("name", "robots", "index, follow"),
    ("property", "og:title", "Blind'All Music - Quiz musicaux multijoueurs en ligne"),
    ("property", "og:description", "Des quiz musicaux multijoueurs en ligne. Teste ta culture sonore face à d'autres joueurs : Années 80, Disney, Rap, Rock, Pop, Chanson française et bien d'autres salles."),
    ("property", "og:image", "/images/statics/screenshot.png"),
    ("property", "og:url", ""),
    ("property", "og:locale", "fr_FR"),
    ("name", "twitter:title", "Blind'All Music - Quiz musicaux multijoueurs en ligne"),
    ("name", "twitter:description", "Des quiz musicaux multijoueurs en ligne. Teste ta culture sonore face à d'autres joueurs : Années 80, Disney, Rap, Rock, Pop et bien plus."),
    ("name", "twitter:image", "/images/statics/screenshot.png"),
];

fn defaults() -> Vec<(String, String)> {
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    DEFAULT_META
        .iter()
        .map(|(attr, key, value)| {
            let sel = format!("meta[{attr}=\"{key}\"]");
            let v = if *value == "" {
                format!("{origin}/")
            } else if value.starts_with('/') {
                format!("{origin}{value}")
            } else {
                value.to_string()
            };
            (sel, v)
        })
        .collect()
}

fn head_el(doc: &web_sys::Document) -> Option<web_sys::Element> {
    doc.head().map(|h| h.unchecked_into::<web_sys::Element>())
}

pub fn set_meta(attr: &str, key: &str, content: &str) {
    let Some(doc) = document() else { return };
    let sel = format!("meta[{attr}=\"{key}\"]");
    match doc.query_selector(&sel).ok().flatten() {
        Some(el) => {
            let _ = el.set_attribute("data-seo-managed", "1");
            let _ = el.set_attribute("content", content);
        }
        None => {
            let Ok(el) = doc.create_element("meta") else {
                return;
            };
            let _ = el.set_attribute(attr, key);
            let _ = el.set_attribute("content", content);
            let _ = el.set_attribute("data-seo-injected", "1");
            if let Some(h) = head_el(&doc) {
                let _ = h.append_child(&el);
            }
        }
    }
}

pub fn set_canonical(href: &str) {
    let Some(doc) = document() else { return };
    match doc.query_selector("link[rel=\"canonical\"]").ok().flatten() {
        Some(el) => {
            let _ = el.set_attribute("data-seo-managed", "1");
            let _ = el.set_attribute("href", href);
        }
        None => {
            let Ok(el) = doc.create_element("link") else {
                return;
            };
            let _ = el.set_attribute("rel", "canonical");
            let _ = el.set_attribute("href", href);
            let _ = el.set_attribute("data-seo-injected", "1");
            if let Some(h) = head_el(&doc) {
                let _ = h.append_child(&el);
            }
        }
    }
}

pub fn set_hreflang(lang: &str, href: &str) {
    let Some(doc) = document() else { return };
    let sel = format!("link[rel=\"alternate\"][hreflang=\"{lang}\"]");
    if doc.query_selector(&sel).ok().flatten().is_none() {
        let Ok(el) = doc.create_element("link") else {
            return;
        };
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

pub fn set_json_ld(id: &str, json: &serde_json::Value) {
    let Some(doc) = document() else { return };
    let sel = format!("script[data-seo-ld=\"{id}\"]");
    let text = json.to_string();
    match doc.query_selector(&sel).ok().flatten() {
        Some(el) => el.set_text_content(Some(&text)),
        None => {
            let Ok(el) = doc.create_element("script") else {
                return;
            };
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

pub fn set_noindex() {
    set_meta("name", "robots", "noindex");
}

pub fn reset() {
    let Some(doc) = document() else { return };
    if let Ok(nodes) = doc.query_selector_all("[data-seo-injected]") {
        for i in 0..nodes.length() {
            if let Some(el) = nodes
                .item(i)
                .and_then(|n| n.dyn_into::<web_sys::Element>().ok())
            {
                el.remove();
            }
        }
    }
    for (sel, content) in defaults() {
        if let Ok(Some(el)) = doc.query_selector(&sel) {
            let _ = el.set_attribute("content", &content);
            let _ = el.remove_attribute("data-seo-managed");
        }
    }
}

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

pub fn apply_alternates(path: &str) {
    let origin = web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default();
    set_hreflang("fr", &format!("{origin}{path}"));
    set_hreflang("en", &format!("{origin}/en{path}"));
    set_hreflang("es", &format!("{origin}/es{path}"));
    set_hreflang("x-default", &format!("{origin}{path}"));
}

pub fn apply_page(path: &str, description: &str) {
    set_meta("name", "description", description);
    set_canonical(&canonical_url(path));
    apply_alternates(path);
    set_meta("property", "og:description", description);
    set_meta("name", "twitter:description", description);
    set_meta("property", "og:url", &canonical_url(path));
    set_meta("property", "og:locale", og_locale());
}

pub fn og_locale() -> &'static str {
    match crate::i18n::locale() {
        "en" => "en_US",
        "es" => "es_ES",
        _ => "fr_FR",
    }
}

pub fn truncate160(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() > 160 {
        format!("{}...", chars[..157].iter().collect::<String>())
    } else {
        s.to_string()
    }
}
