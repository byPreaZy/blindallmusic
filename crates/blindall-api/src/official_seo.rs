use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Raw {
    slugs: Vec<String>,
    intros: HashMap<String, HashMap<String, IntroTexts>>,
}

#[derive(Deserialize, Clone)]
pub struct IntroTexts {
    pub intro: Option<String>,
    pub intro_secondary: Option<String>,
}

struct OfficialSeo {
    slugs: Vec<String>,
    intros: HashMap<String, HashMap<String, IntroTexts>>,
}

fn seo() -> &'static OfficialSeo {
    static SEO: OnceLock<OfficialSeo> = OnceLock::new();
    SEO.get_or_init(|| {
        let raw: Raw = serde_json::from_str(include_str!("official_room_seo.json"))
            .expect("official_room_seo.json is valid");
        OfficialSeo {
            slugs: raw.slugs,
            intros: raw.intros,
        }
    })
}

pub fn slugs() -> &'static [String] {
    &seo().slugs
}

pub fn is_official_slug(slug: &str) -> bool {
    seo().slugs.iter().any(|s| s == slug)
}

pub fn configured_intro(slug: &str, locale: &str) -> Option<IntroTexts> {
    seo().intros.get(slug)?.get(locale).cloned()
}
