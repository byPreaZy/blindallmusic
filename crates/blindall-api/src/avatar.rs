//! Local initials-avatar generator.
//!
//! Replaces the ui-avatars.com external dependency: deterministic SVG
//! (initials + hue hashed from the name) so chat/podium/rankings avatars
//! work offline and never break when the third-party service is down.

use axum::extract::Query;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Deserialize)]
pub struct AvatarQuery {
    name: Option<String>,
}

/// `GET /api/v1/avatar?name=<name>` → deterministic initials avatar (SVG).
pub async fn avatar(Query(q): Query<AvatarQuery>) -> Response {
    let name = q.name.unwrap_or_default();
    let svg = avatar_svg(&name);
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "image/svg+xml"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        svg,
    )
        .into_response()
}

/// Fallback URL used in JSON payloads when an entity has no stored photo.
pub fn fallback_url(name: &str) -> String {
    format!(
        "/api/v1/avatar?name={}",
        url::form_urlencoded::byte_serialize(name.as_bytes()).collect::<String>()
    )
}

fn initials(name: &str) -> String {
    let mut it = name
        .split_whitespace()
        .filter_map(|seg| seg.chars().next())
        .filter(|c| c.is_alphanumeric());
    match (it.next(), it.next()) {
        (Some(a), Some(b)) => format!("{a}{b}"),
        (Some(a), None) => a.to_string(),
        _ => "?".to_string(),
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn avatar_svg(name: &str) -> String {
    let initials = xml_escape(&initials(name));
    let mut h = DefaultHasher::new();
    name.hash(&mut h);
    let hue = h.finish() % 360;
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"300\" height=\"300\" viewBox=\"0 0 300 300\">\
         <rect width=\"300\" height=\"300\" fill=\"hsl({hue},45%,32%)\"/>\
         <text x=\"150\" y=\"150\" dy=\"0.35em\" text-anchor=\"middle\" fill=\"#fff\" \
         font-family=\"system-ui,-apple-system,sans-serif\" font-size=\"110\" \
         font-weight=\"700\">{initials}</text></svg>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_two_words() {
        assert_eq!(initials("John Doe"), "JD");
        assert_eq!(initials("yotchii"), "y");
        assert_eq!(initials(""), "?");
        assert_eq!(initials("  "), "?");
    }

    #[test]
    fn svg_escapes_and_is_deterministic() {
        let a = avatar_svg("<script>");
        assert!(!a.contains("<script>"));
        assert_eq!(avatar_svg("preazy"), avatar_svg("preazy"));
        assert!(a.starts_with("<svg"));
    }
}
