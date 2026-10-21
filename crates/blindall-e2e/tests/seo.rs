mod common;
use common::*;
use reqwest::StatusCode;
use serde_json::json;

/// reqwest client that does NOT follow redirects — for 308 assertions.
async fn raw_get(path: &str) -> (StatusCode, String, Option<String>) {
    let c = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let r = c
        .get(format!("{}{}", base(), path))
        .send()
        .await
        .expect("raw get");
    let status = r.status();
    let loc = r
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let text = r.text().await.unwrap_or_default();
    (status, text, loc)
}

fn contains(hay: &str, needle: &str, ctx: &str) {
    assert!(hay.contains(needle), "{ctx}: missing '{needle}'");
}

async fn public_room(owner: &Client) -> (i64, String, String) {
    let name = uniq_name("seoroom");
    let r = owner
        .post(
            "/api/v1/rooms",
            json!({"name": name, "category_id": 4, "is_public": true}),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "room: {}", r.text);
    (
        r.json("/room/id").as_i64().unwrap(),
        name,
        r.json("/room/slug").as_str().unwrap().to_string(),
    )
}

#[tokio::test]
async fn home_serves_seo_head_and_landing() {
    let r = Client::anon().get("/").await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.status);
    contains(&r.text, "<title>Blind test en ligne multijoueur", "home title");
    contains(&r.text, "name=\"robots\" content=\"index, follow\"", "robots");
    contains(&r.text, "rel=\"canonical\"", "canonical");
    contains(&r.text, "hreflang=\"fr\"", "hreflang fr");
    contains(&r.text, "hreflang=\"en\"", "hreflang en");
    contains(&r.text, "id=\"seo-landing\"", "landing block");
    contains(&r.text, "<h1>", "landing h1");
    contains(&r.text, "application/ld+json", "json-ld");
    contains(&r.text, "FAQPage", "faq schema");
}

#[tokio::test]
async fn room_page_serves_seo_landing_and_video_game_schema() {
    let (owner, _) = Client::register("seoroom").await;
    let (id, name, slug) = public_room(&owner).await;
    let r = Client::anon().get(&format!("/rooms/{slug}")).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    contains(&r.text, &format!("<title>{name}"), "room title");
    contains(&r.text, &format!("rel=\"canonical\" href=\"http://localhost:8080/rooms/{slug}\""), "canonical");
    contains(&r.text, "id=\"seo-landing\"", "landing");
    contains(&r.text, &name, "room name in landing");
    contains(&r.text, "VideoGame", "videogame schema");

    // Numeric id URL canonicalizes to the slug URL.
    let (st, _, loc) = raw_get(&format!("/rooms/{id}")).await;
    assert_eq!(st, StatusCode::PERMANENT_REDIRECT);
    assert!(loc.unwrap_or_default().ends_with(&format!("/rooms/{slug}")));
}

#[tokio::test]
async fn private_room_is_noindexed() {
    let (owner, _) = Client::register("seopriv").await;
    let name = uniq_name("privroom");
    let r = owner
        .post(
            "/api/v1/rooms",
            json!({"name": name, "category_id": 4}),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text);
    let room_id = r.json("/room/id").as_i64().unwrap();
    let slug = r.json("/room/slug").as_str().unwrap().to_string();
    let opt = owner
        .put(
            &format!("/api/v1/rooms/{room_id}/options"),
            json!({
                "has_password": true, "password": "secret",
                "tracks_by_round": 10, "track_duration": 15,
                "pause_between_tracks": 5, "pause_between_rounds": 10,
                "is_chat_active": true, "is_autostart": true, "is_random": true,
            }),
        )
        .await;
    assert!(opt.ok(), "options: {}", opt.text);
    let r = Client::anon().get(&format!("/rooms/{slug}")).await;
    assert_eq!(r.status, StatusCode::OK);
    contains(&r.text, "noindex", "private room noindex");
}

#[tokio::test]
async fn unknown_routes_return_real_404() {
    let c = Client::anon();
    for path in [
        "/definitely-not-a-page-xyz",
        "/blind-test-no-such-category",
        "/rooms/99999999",
        "/pages/no-such-page",
        "/minigames/not-a-game",
        "/match/not-a-game",
    ] {
        let r = c.get(path).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{path}");
        contains(&r.text, "noindex", "404 page noindex");
    }
}

#[tokio::test]
async fn private_and_member_routes_are_noindexed() {
    let c = Client::anon();
    for path in ["/login", "/register", "/admin", "/settings", "/me", "/rooms"] {
        let r = c.get(path).await;
        assert_eq!(r.status, StatusCode::OK, "{path}");
        contains(&r.text, "noindex", "{path} robots");
    }
}

#[tokio::test]
async fn legacy_paths_redirect() {
    for (from, to) in [
        ("/faq", "/docs/faq"),
        ("/games", "/minigames"),
        ("/rankings/elo", "/rankings?sort=elo"),
        ("/minigames/", "/minigames"),
    ] {
        let (st, _, loc) = raw_get(from).await;
        assert_eq!(st, StatusCode::PERMANENT_REDIRECT, "{from} expected 308");
        assert!(
            loc.unwrap_or_default().ends_with(to),
            "{from} should redirect to {to}"
        );
    }
}

#[tokio::test]
async fn locale_prefixed_routes_serve_self_canonical() {
    let c = Client::anon();
    let r = c.get("/en").await;
    assert_eq!(r.status, StatusCode::OK);
    contains(&r.text, "canonical\" href=\"http://localhost:8080/en/", "en canonical");
    let r = c.get("/es/rankings").await;
    assert_eq!(r.status, StatusCode::OK);
    contains(&r.text, "canonical\" href=\"http://localhost:8080/es/rankings\"", "es canonical");
}

#[tokio::test]
async fn indexable_content_routes_render_landing() {
    let c = Client::anon();
    for path in [
        "/docs",
        "/docs/faq",
        "/docs/howto",
        "/rankings",
        "/minigames",
        "/minigames/quiz",
        "/match/who-sang",
        "/duels",
        "/daily",
        "/donations",
        "/contact",
    ] {
        let r = c.get(path).await;
        assert_eq!(r.status, StatusCode::OK, "{path}");
        contains(&r.text, "id=\"seo-landing\"", "{path} landing");
        contains(&r.text, "<h1>", "{path} h1");
    }
}

#[tokio::test]
async fn sitemap_lists_public_routes() {
    let r = Client::anon().get("/sitemap.xml").await;
    assert_eq!(r.status, StatusCode::OK);
    for loc in [
        "/minigames",
        "/duels",
        "/daily",
        "/donations",
        "/docs/faq",
        "/blind-test-",
        "/rooms/",
    ] {
        contains(&r.text, loc, "sitemap entry");
    }
}
