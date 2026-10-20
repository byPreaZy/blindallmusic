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
    let r = admin
        .get("/api/v1/admin/dashboard")
        .await
        .expect_ok("dashboard");
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
    for ep in [
        "/api/v1/admin/rooms",
        "/api/v1/admin/playlists",
        "/api/v1/admin/teams",
    ] {
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
    let id = r
        .json("/id")
        .as_i64()
        .unwrap_or_else(|| r.json("/answer_type/id").as_i64().unwrap());
    let r = Client::anon()
        .get("/api/v1/answer-types")
        .await
        .expect_ok("types");
    let names: Vec<String> = r
        .json("/answer_types")
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str().map(String::from))
        .collect();
    assert!(names.iter().any(|n| *n == name), "{names:?}");
    let r = admin
        .delete(&format!("/api/v1/admin/answer-types/{id}"))
        .await;
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
    let r = Client::anon()
        .get(&format!("/api/v1/pages/{}", title.to_lowercase()))
        .await;
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
    assert!(
        r.status.is_success(),
        "modo dashboard: {} {}",
        r.status,
        r.text
    );
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
    assert!(
        r.status.is_success() || r.status == StatusCode::NO_CONTENT,
        "ban: {} {}",
        r.status,
        r.text
    );
    let r = admin
        .get("/api/v1/moderation/banned-users")
        .await
        .expect_ok("banned");
    let found = r.text.contains(&victim.name);
    let r = admin
        .post(
            &format!("/api/v1/moderation/banned-users/{}/unban", victim.user_id),
            json!({}),
        )
        .await;
    assert!(
        r.status.is_success() || r.status == StatusCode::NO_CONTENT,
        "unban: {} {}",
        r.status,
        r.text
    );
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
    assert!(
        r.text.contains(&victim.name),
        "{}",
        &r.text[..r.text.len().min(400)]
    );
}

#[tokio::test]
async fn admin_set_user_role() {
    let admin = Client::admin().await;
    let (target, _) = Client::register("rl").await;
    let profile_role = |id: i64| async move {
        let r = Client::anon()
            .get(&format!("/api/v1/profile/{id}"))
            .await
            .expect_ok("profile");
        r.json("/role").as_str().unwrap_or("").to_string()
    };

    // non-admin cannot set roles
    let (plain, _) = Client::register("rl2").await;
    let r = plain
        .put(
            &format!("/api/v1/admin/users/{}/role", target.user_id),
            json!({"role": "admin"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.text);

    // invalid role rejected
    let r = admin
        .put(
            &format!("/api/v1/admin/users/{}/role", target.user_id),
            json!({"role": "superadmin"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text);

    // self-role-change rejected (lockout guard)
    let r = admin
        .put(
            &format!("/api/v1/admin/users/{}/role", admin.user_id),
            json!({"role": "user"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text);

    // promote to moderator -> profile role reflects it
    admin
        .put(
            &format!("/api/v1/admin/users/{}/role", target.user_id),
            json!({"role": "moderator"}),
        )
        .await
        .expect_ok("role=moderator");
    assert_eq!(profile_role(target.user_id).await, "moderator");

    // admin users list exposes the same role + filters on it
    let r = admin
        .get(&format!(
            "/api/v1/admin/users?search={}&role=moderator",
            target.name
        ))
        .await
        .expect_ok("users role=moderator");
    let entry = r.body["users"]["data"]
        .as_array()
        .and_then(|a| a.iter().find(|u| u["id"].as_i64() == Some(target.user_id)));
    let entry = entry.expect("moderator should appear in role=moderator filter");
    assert_eq!(entry["role"].as_str(), Some("moderator"), "{}", r.text);
    assert_eq!(entry["is_moderator"].as_bool(), Some(true));

    // ... and is gone from the plain-player filter
    let r = admin
        .get(&format!(
            "/api/v1/admin/users?search={}&role=user",
            target.name
        ))
        .await
        .expect_ok("users role=user");
    assert!(
        r.body["users"]["data"]
            .as_array()
            .map(|a| a.iter().all(|u| u["id"].as_i64() != Some(target.user_id)))
            .unwrap_or(true),
        "moderator must not appear in role=user filter: {}",
        r.text
    );

    // promote to admin
    admin
        .put(
            &format!("/api/v1/admin/users/{}/role", target.user_id),
            json!({"role": "admin"}),
        )
        .await
        .expect_ok("role=admin");
    assert_eq!(profile_role(target.user_id).await, "admin");

    // demote back to player
    admin
        .put(
            &format!("/api/v1/admin/users/{}/role", target.user_id),
            json!({"role": "user"}),
        )
        .await
        .expect_ok("role=user");
    assert_eq!(profile_role(target.user_id).await, "");
}

// ---- global bans are admin-only; moderators keep room-level powers ---------

#[tokio::test]
async fn moderator_cannot_ban_but_keeps_room_powers() {
    let admin = Client::admin().await;
    let (room_id, _, _) = game_fixture(&admin, &[("Mod Room", 12, 1.0)]).await;

    // Public room -> attached moderator becomes a public moderator.
    admin
        .put(
            &format!("/api/v1/admin/rooms/{room_id}"),
            json!({"name": uniq_name("modroom"), "is_public": true}),
        )
        .await
        .expect_ok("publish room");
    let (mod_u, _) = Client::register("pmod").await;
    admin
        .post(
            &format!("/api/v1/rooms/{room_id}/moderators/attach"),
            json!({"user_id": mod_u.user_id}),
        )
        .await
        .expect_ok("attach moderator");

    let (victim, _) = Client::register("vban").await;

    // Moderator cannot globally ban.
    let r = mod_u
        .post(
            &format!("/api/v1/moderation/users/{}/ban", victim.user_id),
            json!({"reason": "mod attempt"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "mod ban: {}", r.text);

    // Moderator cannot globally unban.
    let r = mod_u
        .post(
            &format!("/api/v1/moderation/banned-users/{}/unban", victim.user_id),
            json!({}),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "mod unban: {}", r.text);

    // Read/investigation tools stay available to moderators.
    mod_u
        .get("/api/v1/moderation/banned-users")
        .await
        .expect_ok("banned list");
    mod_u
        .get("/api/v1/moderation/users")
        .await
        .expect_ok("users list");

    // Room-level power retained: public moderator deletes a chat message.
    admin
        .post(
            &format!("/api/v1/rooms/{room_id}/messages"),
            json!({"body": "spam msg"}),
        )
        .await;
    let hist = admin
        .get(&format!("/api/v1/rooms/{room_id}/messages"))
        .await
        .expect_ok("history");
    let msg_id = hist.body["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["body"] == "spam msg")
        .and_then(|m| m["id"].as_i64())
        .expect("message stored");
    let r = mod_u.delete(&format!("/api/v1/messages/{msg_id}")).await;
    assert!(
        r.status.is_success() || r.status == StatusCode::NO_CONTENT,
        "mod delete msg: {} {}",
        r.status,
        r.text
    );

    // Room moderator can still manage moderators (detach is room-scoped).
    let (other_mod, _) = Client::register("omod").await;
    admin
        .post(
            &format!("/api/v1/rooms/{room_id}/moderators/attach"),
            json!({"user_id": other_mod.user_id}),
        )
        .await
        .expect_ok("attach other");
    let r = mod_u
        .delete_body(
            &format!("/api/v1/rooms/{room_id}/moderators/detach"),
            json!({"user_id": other_mod.user_id}),
        )
        .await;
    assert!(
        r.status.is_success(),
        "mod detach moderator: {} {}",
        r.status,
        r.text
    );

    // Admin ban + unban still work end to end.
    admin
        .post(
            &format!("/api/v1/moderation/users/{}/ban", victim.user_id),
            json!({"reason": "admin ban"}),
        )
        .await
        .expect_ok("admin ban");
    admin
        .post(
            &format!("/api/v1/moderation/banned-users/{}/unban", victim.user_id),
            json!({}),
        )
        .await
        .expect_ok("admin unban");
}
