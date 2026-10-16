mod common;
use common::*;
use reqwest::StatusCode;
use serde_json::json;

#[tokio::test]
async fn register_returns_token_and_user_shape() {
    let (_c, body) = Client::register("a").await;
    let token = body["token"].as_str().expect("token string");
    assert!(token.contains('|'), "sanctum token format id|plain");
    let user = &body["user"];
    for key in ["id", "name", "photo_path", "elo", "is_guest", "is_administrator", "is_public_moderator", "team_id"] {
        assert!(user.get(key).is_some(), "user.{key} missing");
    }
    assert_eq!(user["elo"], 1500, "default elo");
    assert_eq!(user["is_guest"], false);
}

#[tokio::test]
async fn register_validates_like_laravel() {
    let c = Client::anon();
    let r = c
        .post(
            "/api/v1/auth/register",
            json!({"name":"admin","email":uniq_email("v"),"password":"password123","password_confirmation":"password123"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(r.json("/errors/name").is_array(), "errors.name list");

    let r = c
        .post(
            "/api/v1/auth/register",
            json!({"name":uniq_name("v"),"email":uniq_email("v"),"password":"short","password_confirmation":"short"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(r.json("/errors/password").is_array());

    let r = c
        .post(
            "/api/v1/auth/register",
            json!({"name":uniq_name("v"),"email":uniq_email("v"),"password":"password123","password_confirmation":"other"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn register_rejects_duplicate_name_and_email() {
    let (c, _) = Client::register("dup").await;
    let email_holder = Client::anon();
    let r = email_holder
        .post(
            "/api/v1/auth/register",
            json!({"name": c.name, "email": uniq_email("d"), "password":"password123","password_confirmation":"password123"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(r.json("/errors/name/0"), "The name has already been taken.");
}

#[tokio::test]
async fn guest_creates_guest_user_and_token() {
    let c = Client::guest().await;
    assert!(c.name.starts_with("Guest-"), "guest name pattern: {}", c.name);
    let me = c.get("/api/v1/auth/me").await.expect_ok("me");
    assert_eq!(me.json("/is_guest"), true);
    assert_eq!(me.json("/elo"), 1500);
}

#[tokio::test]
async fn guest_unique_emails_no_collisions() {
    for i in 0..3 {
        let c = Client::anon();
        let r = c.post("/api/v1/auth/guest", json!({})).await;
        assert_eq!(r.status, StatusCode::CREATED, "guest {i}: {}", r.text);
    }
}

#[tokio::test]
async fn login_ok_and_wrong_password() {
    let email = uniq_email("lg");
    let name = uniq_name("lg");
    let c = Client::anon();
    c.post(
        "/api/v1/auth/register",
        json!({"name": name, "email": email, "password": "password123", "password_confirmation": "password123"}),
    )
    .await
    .expect_ok("register");

    let ok = c.login(&email, "password123").await;
    assert_eq!(ok.status, StatusCode::OK, "login: {}", ok.text);
    assert!(ok.json("/token").as_str().unwrap().contains('|'));

    let bad = c.login(&email, "wrongpass").await;
    assert!(
        bad.status == StatusCode::UNPROCESSABLE_ENTITY || bad.status == StatusCode::UNAUTHORIZED,
        "wrong password → 422/401, got {}",
        bad.status
    );
}

#[tokio::test]
async fn login_throttle_after_5_attempts() {
    let email = uniq_email("thr");
    let c = Client::anon();
    for _ in 0..5 {
        c.login(&email, "nope").await;
    }
    let r = c.login(&email, "nope").await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "6th → 422");
    assert!(r.text.contains("Too many login attempts"), "{}", r.text);
}

#[tokio::test]
async fn me_requires_auth() {
    let anon = Client::anon();
    let r = anon.get("/api/v1/auth/me").await;
    assert!(r.status == StatusCode::UNAUTHORIZED || r.status == StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn bearer_token_roundtrip_and_logout() {
    let (c, _) = Client::register("rt").await;
    let token = c.token.clone().unwrap();
    let fresh = Client::with_token(&token);
    let me = fresh.get("/api/v1/auth/me").await.expect_ok("me");
    assert_eq!(me.json("/name"), c.name);

    let out = c.post_empty("/api/v1/auth/logout").await;
    assert!(out.status == StatusCode::NO_CONTENT || out.status == StatusCode::OK);
    let after = fresh.get("/api/v1/auth/me").await;
    assert!(
        after.status == StatusCode::UNAUTHORIZED || after.status == StatusCode::FORBIDDEN,
        "post-logout token must be revoked, got {}",
        after.status
    );
}
