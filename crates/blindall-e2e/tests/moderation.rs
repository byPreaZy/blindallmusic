mod common;
use common::*;
use reqwest::StatusCode;
use serde_json::json;

// ---- gates ---------------------------------------------------------------

#[tokio::test]
async fn moderation_requires_moderator() {
    let (u, _) = Client::register("mo").await;
    for ep in [
        "/api/v1/moderation",
        "/api/v1/moderation/trashed-messages",
        "/api/v1/moderation/banned-users",
        "/api/v1/moderation/users",
        "/api/v1/moderation/moderators",
    ] {
        let r = u.get(ep).await;
        assert!(
            matches!(r.status, StatusCode::FORBIDDEN | StatusCode::UNAUTHORIZED),
            "{ep} should be protected: {} {}",
            r.status,
            r.text
        );
    }
}

#[tokio::test]
async fn admin_requires_administrator() {
    let (u, _) = Client::register("ad").await;
    for ep in [
        "/api/v1/admin/dashboard",
        "/api/v1/admin/users",
        "/api/v1/admin/rooms",
        "/api/v1/admin/teams",
        "/api/v1/admin/playlists",
        "/api/v1/admin/pages",
        "/api/v1/admin/faqs",
        "/api/v1/admin/categories",
        "/api/v1/admin/answer-types",
    ] {
        let r = u.get(ep).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{ep}: {}", r.text);
    }
}

#[tokio::test]
async fn admin_anonymous_401() {
    let r = Client::anon().get("/api/v1/admin/dashboard").await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
}

// ---- admin surface (seeded admin) -----------------------------------------

#[tokio::test]
async fn admin_dashboard_shape() {
    let admin = Client::admin().await;
    let r = admin.get("/api/v1/admin/dashboard").await.expect_ok("dashboard");
    assert!(r.body.is_object());
    assert!(
        r.body.get("cacheDriver").is_some() || r.body.get("stats").is_some(),
        "dashboard props: {}",
        &r.text[..r.text.len().min(400)]
    );
}

#[tokio::test]
async fn admin_users_list_and_search() {
    let admin = Client::admin().await;
    let r = admin.get("/api/v1/admin/users").await.expect_ok("users");
    let users = &r.body["users"];
    assert!(users["data"].is_array() || users.is_array(), "{}", r.text);
}

#[tokio::test]
async fn admin_rooms_playlists_teams() {
    let admin = Client::admin().await;
    for ep in ["/api/v1/admin/rooms", "/api/v1/admin/playlists", "/api/v1/admin/teams"] {
        let r = admin.get(ep).await;
        assert!(r.status.is_success(), "{ep}: {} {}", r.status, r.text);
    }
}

#[tokio::test]
async fn admin_answer_types_crud() {
    let admin = Client::admin().await;
    let name = uniq_name("type");
    let r = admin
        .post("/api/v1/admin/answer-types", json!({"name": name}))
        .await;
    assert!(r.ok(), "create: {} {}", r.status, r.text);
    let id = r.json("/id").as_i64().unwrap_or_else(|| r.json("/answer_type/id").as_i64().unwrap());
    let r = Client::anon().get("/api/v1/answer-types").await.expect_ok("types");
    let names: Vec<String> = r.json("/answer_types").as_array().unwrap().iter()
        .filter_map(|t| t["name"].as_str().map(String::from)).collect();
    assert!(names.iter().any(|n| *n == name), "{names:?}");
    let r = admin.delete(&format!("/api/v1/admin/answer-types/{id}")).await;
    assert!(r.ok(), "delete: {}", r.text);
}

#[tokio::test]
async fn admin_pages_crud() {
    let admin = Client::admin().await;
    let title = uniq_name("pg");
    let r = admin
        .post(
            "/api/v1/admin/pages",
            json!({"title": title, "content": "Hello body"}),
        )
        .await;
    assert!(r.ok(), "create page: {} {}", r.status, r.text);
    let r = Client::anon().get(&format!("/api/v1/pages/{}", title.to_lowercase())).await;
    assert!(r.status.is_success(), "public page: {}", r.text);
    assert!(r.text.contains(&title), "{}", r.text);
}

#[tokio::test]
async fn admin_clear_cache_and_force_clear_rounds() {
    let admin = Client::admin().await;
    let r = admin.post_empty("/api/v1/admin/clear-cache").await;
    assert!(r.status.is_success(), "clear-cache: {}", r.text);
    let r = admin.post_empty("/api/v1/admin/force-clear-rounds").await;
    assert!(r.status.is_success(), "force-clear: {}", r.text);
}

// ---- moderation as admin (admin is a moderator too in Laravel) --------------

#[tokio::test]
async fn moderation_dashboard_as_admin() {
    let admin = Client::admin().await;
    let r = admin.get("/api/v1/moderation").await;
    assert!(r.status.is_success(), "modo dashboard: {} {}", r.status, r.text);
}

#[tokio::test]
async fn moderation_ban_unban_flow() {
    let admin = Client::admin().await;
    let (victim, _) = Client::register("ba").await;
    let r = admin
        .post(
            &format!("/api/v1/moderation/users/{}/ban", victim.user_id),
            json!({"reason": "e2e ban test"}),
        )
        .await;
    assert!(r.status.is_success() || r.status == StatusCode::NO_CONTENT,
        "ban: {} {}", r.status, r.text);
    let r = admin.get("/api/v1/moderation/banned-users").await.expect_ok("banned");
    let found = r.text.contains(&victim.name);
    let r = admin
        .post(
            &format!("/api/v1/moderation/banned-users/{}/unban", victim.user_id),
            json!({}),
        )
        .await;
    assert!(r.status.is_success() || r.status == StatusCode::NO_CONTENT,
        "unban: {} {}", r.status, r.text);
    assert!(found, "victim was in banned list");
}

#[tokio::test]
async fn moderation_users_search() {
    let admin = Client::admin().await;
    let (victim, _) = Client::register("ms").await;
    let r = admin
        .get(&format!("/api/v1/moderation/users?search={}", victim.name))
        .await
        .expect_ok("users");
    assert!(r.text.contains(&victim.name), "{}", &r.text[..r.text.len().min(400)]);
}
