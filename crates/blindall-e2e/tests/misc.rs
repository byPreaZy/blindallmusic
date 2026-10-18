mod common;
use common::*;
use reqwest::StatusCode;
use serde_json::json;

// ---- FAQs ------------------------------------------------------------------

#[tokio::test]
async fn faqs_index_shape() {
    let r = Client::anon().get("/api/v1/faqs").await.expect_ok("faqs");
    assert!(r.json("/current_page").is_number(), "{}", r.text);
    assert!(r.json("/per_page").is_number());
    let faqs = r.json("/data").as_array().cloned().unwrap_or_default();
    assert!(!faqs.is_empty(), "seeded faqs expected");
    for k in ["id", "question", "answer", "upvotes", "downvotes"] {
        assert!(faqs[0].get(k).is_some(), "missing {k}");
    }
}

#[tokio::test]
async fn faq_vote_flow() {
    let r = Client::anon().get("/api/v1/faqs").await.expect_ok("faqs");
    let id = r.json("/data/0/id").as_i64().unwrap();
    let before = r.json("/data/0/upvotes").as_i64().unwrap_or(0);

    let r = Client::anon()
        .post_empty(&format!("/api/v1/faqs/{id}/vote/up"))
        .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    let g = Client::guest().await;
    let r = g.post_empty(&format!("/api/v1/faqs/{id}/vote/up")).await;
    assert!(
        matches!(r.status, StatusCode::FORBIDDEN | StatusCode::UNAUTHORIZED),
        "guest: {}",
        r.status
    );

    let (u, _) = Client::register("fq").await;
    let r = u.post_empty(&format!("/api/v1/faqs/{id}/vote/up")).await;
    assert!(r.status.is_success(), "vote: {} {}", r.status, r.text);
    let r = u.get("/api/v1/faqs").await.expect_ok("faqs");
    let after = r.json("/data/0/upvotes").as_i64().unwrap_or(0);
    assert!(after > before, "upvote counted: {before} → {after}");
}

// ---- pages / contact --------------------------------------------------------

#[tokio::test]
async fn page_404_unknown_slug() {
    let r = Client::anon().get("/api/v1/pages/not-a-page-xyz").await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn contact_validation() {
    let r = Client::anon().post("/api/v1/contact", json!({})).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
}

// ---- donations ---------------------------------------------------------------

#[tokio::test]
async fn donations_public_endpoints() {
    let c = Client::anon();
    for ep in [
        "/api/v1/donations/progress",
        "/api/v1/donations/supporters",
        "/api/v1/donations/recent",
        "/api/v1/donations/history",
    ] {
        let r = c.get(ep).await;
        assert!(r.status.is_success(), "{ep}: {} {}", r.status, r.text);
    }
    let r = c
        .get("/api/v1/donations/progress")
        .await
        .expect_ok("progress");
    let t = &r.text;
    assert!(t.contains("goal") || t.contains("percent"), "{t}");
}

#[tokio::test]
async fn stripe_webhook_rejects_bad_signature() {
    let r = Client::anon()
        .post(
            "/api/v1/stripe/webhook",
            json!({"type": "checkout.session.completed"}),
        )
        .await;
    assert!(
        matches!(
            r.status,
            StatusCode::BAD_REQUEST | StatusCode::UNAUTHORIZED | StatusCode::UNPROCESSABLE_ENTITY
        ),
        "unsigned webhook must fail: {} {}",
        r.status,
        r.text
    );
}

// ---- SEO ---------------------------------------------------------------------

#[tokio::test]
async fn seo_endpoints() {
    let c = Client::anon();
    for ep in ["/robots.txt", "/sitemap.xml", "/llms.txt"] {
        let r = c.get(ep).await;
        assert!(r.status.is_success(), "{ep}: {}", r.status);
    }
    let r = c.get("/robots.txt").await;
    assert!(
        r.text.contains("User-agent") || r.text.contains("Sitemap"),
        "{}",
        r.text
    );
}

// ---- minigames -----------------------------------------------------------------

#[tokio::test]
async fn minigames_list_requires_auth() {
    let r = Client::anon().get("/api/v1/minigames").await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn minigames_games_list_shape() {
    let (u, _) = Client::register("mg").await;
    let r = u.get("/api/v1/minigames").await.expect_ok("minigames");
    let games = r.json("/games").as_array().cloned().unwrap_or_default();
    assert!(
        games.len() >= 4,
        "quiz/who_sang/album_cover/first_letter/anagram: {}",
        r.text
    );
}

#[tokio::test]
async fn minigames_next_unknown_game() {
    let (u, _) = Client::register("mg").await;
    let r = u.post_empty("/api/v1/minigames/not-a-game/next").await;
    assert!(
        matches!(
            r.status,
            StatusCode::NOT_FOUND | StatusCode::UNPROCESSABLE_ENTITY | StatusCode::BAD_REQUEST
        ),
        "{} {}",
        r.status,
        r.text
    );
}

// ---- providers -----------------------------------------------------------------

#[tokio::test]
async fn providers_search_requires_auth() {
    let r = Client::anon()
        .get("/api/v1/providers/search?q=test&provider=deezer")
        .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn providers_local_search_shape() {
    let r = Client::anon()
        .get("/api/v1/providers/local/search/track?q=test")
        .await;
    assert!(r.status.is_success(), "{} {}", r.status, r.text);
}

// ---- user/banned ----------------------------------------------------------------

#[tokio::test]
async fn banned_user_endpoint() {
    let r = Client::anon().get("/api/v1/user/banned").await;
    assert!(
        r.status.is_success() || r.status == StatusCode::FORBIDDEN,
        "{} {}",
        r.status,
        r.text
    );
}

#[tokio::test]
async fn local_tracks_store_is_public_moderator_only() {
    let r = Client::anon().post_empty("/api/v1/local-tracks").await;
    assert_eq!(
        r.status,
        StatusCode::UNAUTHORIZED,
        "{} {}",
        r.status,
        r.text
    );
    let (u, _) = Client::register("lt").await;
    let r = u.post_empty("/api/v1/local-tracks").await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{} {}", r.status, r.text);
}
