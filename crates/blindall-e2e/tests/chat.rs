mod common;
use common::*;
use reqwest::StatusCode;
use serde_json::json;

async fn room_for(owner: &Client) -> i64 {
    let r = owner
        .post("/api/v1/rooms", json!({"name": uniq_name("ch"), "category_id": 4}))
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "room: {}", r.text);
    r.json("/room/id").as_i64().unwrap()
}

#[tokio::test]
async fn chat_history_empty_shape() {
    let (owner, _) = Client::register("ch").await;
    let room_id = room_for(&owner).await;
    let r = owner
        .get(&format!("/api/v1/rooms/{room_id}/messages"))
        .await
        .expect_ok("messages");
    assert!(r.json("/messages").is_array(), "shape: {}", r.text);
}

#[tokio::test]
async fn chat_store_requires_auth_and_member() {
    let (owner, _) = Client::register("ch").await;
    let room_id = room_for(&owner).await;
    let r = Client::anon()
        .post(
            &format!("/api/v1/rooms/{room_id}/messages"),
            json!({"body": "hi"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
    let g = Client::guest().await;
    let r = g
        .post(
            &format!("/api/v1/rooms/{room_id}/messages"),
            json!({"body": "hi"}),
        )
        .await;
    assert!(
        matches!(r.status, StatusCode::FORBIDDEN | StatusCode::UNAUTHORIZED),
        "guest chat blocked: {} {}",
        r.status,
        r.text
    );
}

#[tokio::test]
async fn chat_store_and_history_roundtrip() {
    let (owner, _) = Client::register("ch").await;
    let room_id = room_for(&owner).await;
    let r = owner
        .post(
            &format!("/api/v1/rooms/{room_id}/messages"),
            json!({"body": "hello room"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "store: {}", r.text);

    let r = owner
        .get(&format!("/api/v1/rooms/{room_id}/messages"))
        .await
        .expect_ok("history");
    let msgs = r.json("/messages");
    let found = msgs
        .as_array()
        .unwrap()
        .iter()
        .any(|m| m["body"] == "hello room");
    assert!(found, "message in history: {}", r.text);
    let m = &msgs[msgs.as_array().unwrap().len() - 1];
    for k in ["body", "created_at", "id", "time", "user", "user_id"] {
        assert!(!m[k].is_null(), "missing {k}");
    }
    for k in ["id", "name", "is_supporter", "donor_perks"] {
        assert!(!m["user"][k].is_null(), "missing user.{k}");
    }
    assert!(m["user"].get("team_id").is_some(), "missing user.team_id");
}

#[tokio::test]
async fn chat_body_validation() {
    let (owner, _) = Client::register("ch").await;
    let room_id = room_for(&owner).await;
    let r = owner
        .post(&format!("/api/v1/rooms/{room_id}/messages"), json!({}))
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text);
    let r = owner
        .post(
            &format!("/api/v1/rooms/{room_id}/messages"),
            json!({"body": ""}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn chat_delete_own_message() {
    let (owner, _) = Client::register("ch").await;
    let room_id = room_for(&owner).await;
    owner
        .post(
            &format!("/api/v1/rooms/{room_id}/messages"),
            json!({"body": "to delete"}),
        )
        .await;
    let r = owner
        .get(&format!("/api/v1/rooms/{room_id}/messages"))
        .await
        .expect_ok("history");
    let id = r.json("/messages/0/id").as_i64().unwrap();
    let r = owner
        .delete(&format!("/api/v1/messages/{id}"))
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "delete: {}", r.text);
    let r = owner
        .get(&format!("/api/v1/rooms/{room_id}/messages"))
        .await
        .expect_ok("history");
    assert!(r.json("/messages")
        .as_array()
        .unwrap()
        .iter()
        .all(|m| m["body"] != "to delete"));
}

#[tokio::test]
async fn chat_cannot_delete_others_message() {
    let (owner, _) = Client::register("ch").await;
    let (other, _) = Client::register("ch").await;
    let room_id = room_for(&owner).await;
    owner
        .post(
            &format!("/api/v1/rooms/{room_id}/messages"),
            json!({"body": "mine"}),
        )
        .await;
    let r = owner
        .get(&format!("/api/v1/rooms/{room_id}/messages"))
        .await
        .expect_ok("history");
    let id = r.json("/messages/0/id").as_i64().unwrap();
    let r = other.delete(&format!("/api/v1/messages/{id}")).await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{}", r.text);
    let r = owner
        .get(&format!("/api/v1/rooms/{room_id}/messages"))
        .await
        .expect_ok("history");
    assert!(
        r.json("/messages").as_array().unwrap().iter().any(|m| m["body"] == "mine"),
        "message must survive unauthorized delete: {}",
        r.text
    );
}

#[tokio::test]
async fn chat_report_flow() {
    let (owner, _) = Client::register("ch").await;
    let (reporter, _) = Client::register("ch").await;
    let room_id = room_for(&owner).await;
    owner
        .post(
            &format!("/api/v1/rooms/{room_id}/messages"),
            json!({"body": "report me"}),
        )
        .await;
    let r = owner
        .get(&format!("/api/v1/rooms/{room_id}/messages"))
        .await
        .expect_ok("history");
    let id = r.json("/messages/0/id").as_i64().unwrap();
    let r = reporter
        .post(
            &format!("/api/v1/messages/{id}/report"),
            json!({}),
        )
        .await;
    assert!(
        r.status.is_success() || r.status == StatusCode::NO_CONTENT,
        "report: {} {}",
        r.status,
        r.text
    );
    let r = owner
        .get(&format!("/api/v1/rooms/{room_id}/messages"))
        .await
        .expect_ok("history");
    assert_eq!(r.json("/messages/0/reports").as_i64().unwrap_or(0), -1);
}
