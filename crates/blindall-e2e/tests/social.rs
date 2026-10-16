mod common;
use common::*;
use reqwest::StatusCode;
use serde_json::json;

// ---- teams ---------------------------------------------------------------

#[tokio::test]
async fn team_create_and_show() {
    let (u, _) = Client::register("tm").await;
    let name = uniq_name("team");
    let r = u.post("/api/v1/teams", json!({"name": name})).await;
    assert!(r.ok(), "create: {}", r.text);
    let id = r.json("/team_id").as_i64().unwrap();
    let r = u.get(&format!("/api/v1/teams/{id}")).await.expect_ok("show");
    assert_eq!(r.json("/team/name").as_str().unwrap(), name);
}

#[tokio::test]
async fn team_guest_cannot_create() {
    let g = Client::guest().await;
    let r = g.post("/api/v1/teams", json!({"name": uniq_name("t")})).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn team_one_per_user() {
    let (u, _) = Client::register("tm").await;
    u.post("/api/v1/teams", json!({"name": uniq_name("t1")})).await;
    let r = u.post("/api/v1/teams", json!({"name": uniq_name("t2")})).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text);
    assert!(r.json("/errors/team").is_array());
}

#[tokio::test]
async fn team_request_join_flow() {
    let (owner, _) = Client::register("tm").await;
    let (member, _) = Client::register("tm").await;
    let r = owner.post("/api/v1/teams", json!({"name": uniq_name("t")})).await;
    let id = r.json("/team_id").as_i64().unwrap();
    let r = member.post_empty(&format!("/api/v1/teams/{id}/request")).await;
    assert!(r.ok(), "request: {}", r.text);
    let r = member
        .post_empty(&format!("/api/v1/teams/{id}/request/cancel"))
        .await;
    assert!(r.ok(), "cancel: {}", r.text);
}

#[tokio::test]
async fn team_leave() {
    let (owner, _) = Client::register("tm").await;
    let (member, _) = Client::register("tm").await;
    let r = owner.post("/api/v1/teams", json!({"name": uniq_name("t")})).await;
    let id = r.json("/team_id").as_i64().unwrap();
    let r = owner.post_empty(&format!("/api/v1/teams/{id}/leave")).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text);
    assert!(r.json("/errors/team").is_array());
    let r = member.post_empty(&format!("/api/v1/teams/{id}/request")).await;
    assert!(r.ok(), "request: {}", r.text);
    let show = owner.get(&format!("/api/v1/teams/{id}")).await.expect_ok("show");
    let req_id = show.json("/team/requests/0/id").as_i64()
        .or_else(|| show.json("/requests/0/id").as_i64());
    if let Some(req) = req_id {
        let r = owner.post_empty(&format!("/api/v1/teams/requests/{req}/accept")).await;
        assert!(r.ok(), "accept: {}", r.text);
        let r = member.post_empty(&format!("/api/v1/teams/{id}/leave")).await;
        assert!(r.ok(), "member leave: {}", r.text);
    }
}

#[tokio::test]
async fn team_index_search() {
    let (u, _) = Client::register("tm").await;
    let name = uniq_name("findable");
    u.post("/api/v1/teams", json!({"name": name})).await;
    let r = u
        .get(&format!("/api/v1/teams?search={name}"))
        .await
        .expect_ok("index");
    let found = r.json("/teams").as_array().map(|a| {
        a.iter().any(|t| t["name"].as_str().unwrap_or("") == name)
    }).unwrap_or(false);
    assert!(found, "{}", r.text);
}

// ---- users ---------------------------------------------------------------

#[tokio::test]
async fn users_me_shape() {
    let (u, _) = Client::register("um").await;
    let r = u.get("/api/v1/users/me").await.expect_ok("me");
    let acc = &r.body["account"];
    for k in ["id", "name", "email", "stats", "donation_summary", "is_supporter"] {
        assert!(acc.get(k).is_some(), "missing account.{k}: {}", r.text);
    }
}

#[tokio::test]
async fn users_update_self_only() {
    let (u, _) = Client::register("um").await;
    let (other, _) = Client::register("um").await;
    let r = other
        .post(
            &format!("/api/v1/users/{}", u.user_id),
            json!({"name": "hijack", "email": "hijack@x.test"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let new_name = uniq_name("ren");
    let r = u
        .post(
            &format!("/api/v1/users/{}", u.user_id),
            json!({"name": new_name, "email": uniq_email("um")}),
        )
        .await;
    assert!(r.ok(), "update: {}", r.text);
}

#[tokio::test]
async fn users_update_name_validation() {
    let (u, _) = Client::register("um").await;
    let r = u
        .post(
            &format!("/api/v1/users/{}", u.user_id),
            json!({"name": "x", "email": uniq_email("um")}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn users_password_update_flow() {
    let (u, _) = Client::register("um").await;
    let r = u
        .put(
            &format!("/api/v1/users/{}/password", u.user_id),
            json!({"password": "newpassword123", "password_confirmation": "newpassword123", "current_password": "wrong"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text);
    let r = u
        .put(
            &format!("/api/v1/users/{}/password", u.user_id),
            json!({"password": "newpassword123", "password_confirmation": "different", "current_password": "password123"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let r = u
        .put(
            &format!("/api/v1/users/{}/password", u.user_id),
            json!({"password": "newpassword123", "password_confirmation": "newpassword123", "current_password": "password123"}),
        )
        .await;
    assert!(r.ok(), "update: {}", r.text);
    let me = u.get("/api/v1/users/me").await.expect_ok("me");
    let email = me
        .json("/account/email")
        .as_str()
        .expect("email in /me payload")
        .to_string();
    let anon = Client::anon();
    let r = anon.login(&email, "newpassword123").await;
    assert!(r.ok(), "login with new pw: {}", r.text);
}

#[tokio::test]
async fn users_notifications_shape() {
    let (u, _) = Client::register("um").await;
    let r = u.get("/api/v1/users/notifications").await.expect_ok("notifs");
    assert!(r.body.is_object(), "{}", r.text);
}

// ---- profile ---------------------------------------------------------------

#[tokio::test]
async fn profile_show_by_id() {
    let (u, _) = Client::register("pf").await;
    let r = Client::anon()
        .get(&format!("/api/v1/profile/{}", u.user_id))
        .await
        .expect_ok("profile");
    assert_eq!(r.json("/name").as_str().unwrap(), u.name);
    for k in ["id", "elo", "level", "stats", "total_score"] {
        assert!(r.body.get(k).is_some(), "missing {k}: {}", r.text);
    }
}

#[tokio::test]
async fn profile_404_unknown() {
    let r = Client::anon().get("/api/v1/profile/99999999").await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn profile_scores_and_likes() {
    let (u, _) = Client::register("pf").await;
    for ep in ["scores", "likes", "bookmarks", "highlights", "score-evolution"] {
        let r = Client::anon()
            .get(&format!("/api/v1/profile/{}/{ep}", u.user_id))
            .await;
        assert!(r.status.is_success() || r.status == StatusCode::NOT_FOUND,
            "{ep}: {} {}", r.status, r.text);
    }
}

// ---- rankings --------------------------------------------------------------

#[tokio::test]
async fn rankings_players_shape() {
    let r = Client::anon().get("/api/v1/rankings?sort=elo").await.expect_ok("rankings");
    let lb = &r.body["leaderboard"];
    assert!(lb["data"].is_array(), "leaderboard.data");
    assert!(lb["current_page"].is_number(), "paginator");
    if let Some(row) = lb["data"].as_array().and_then(|d| d.first()) {
        assert!(row["user"]["id"].is_i64());
        assert!(row["user"]["name"].is_string());
        assert!(row["stats"].is_object());
    }
}

#[tokio::test]
async fn rankings_teams_and_minigames() {
    let c = Client::anon();
    for ep in ["/api/v1/rankings/teams", "/api/v1/rankings/minigames"] {
        let r = c.get(ep).await;
        assert!(r.status.is_success(), "{ep}: {} {}", r.status, r.text);
    }
}
