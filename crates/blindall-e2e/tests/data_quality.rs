mod common;

use common::{Client, game_fixture, uniq_name};
use reqwest::StatusCode;
use serde_json::json;

async fn track(owner: &Client, playlist_id: i64) -> i64 {
    let r = owner
        .post(
            &format!("/api/v1/playlists/{playlist_id}/tracks"),
            json!({
                "provider": "test",
                "provider_id": format!("dq-{}", uniq_name("t")),
                "provider_url": "https://example.com/track/0",
                "artist_name": "DQ Artist",
                "track_name": "DQ Song (From Test Game)",
                "preview_url": "https://example.com/p.mp3",
                "artwork_url": "https://example.com/c.jpg",
            }),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "track: {}", r.text);
    r.json("/id").as_i64().unwrap()
}

async fn missing_ids(admin: &Client, room_id: i64) -> Vec<i64> {
    admin
        .get(&format!(
            "/api/v1/admin/data-quality/rooms/{room_id}/missing?page=1&per_page=100"
        ))
        .await
        .expect_ok("missing")
        .json("/data")
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|t| t["id"].as_i64())
        .collect()
}

#[tokio::test]
async fn data_quality_requires_admin() {
    let (user, _) = Client::register("dq").await;
    let r = user.get("/api/v1/admin/data-quality/rooms").await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = Client::anon().get("/api/v1/admin/data-quality/rooms").await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn data_quality_review_flow() {
    let admin = Client::admin().await;
    // Two typed tracks out of four make "Video game" (12) the expected type.
    let (room_id, playlist_id, t1) = game_fixture(&admin, &[("Test Game", 12, 1.0)]).await;
    let t4 = track(&admin, playlist_id).await;
    admin
        .post(
            &format!("/api/v1/tracks/{t4}/answers"),
            json!({"answer_type_id": 12, "value": "Test Game", "score": 1.0}),
        )
        .await
        .expect_ok("extra typed answer");
    let t2 = track(&admin, playlist_id).await;
    let t3 = track(&admin, playlist_id).await;

    // Rooms summary reports the expected type and missing count.
    let rooms = admin
        .get("/api/v1/admin/data-quality/rooms")
        .await
        .expect_ok("rooms");
    let room = rooms
        .json("/rooms")
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["room_id"].as_i64() == Some(room_id))
        .expect("room listed")
        .clone();
    assert_eq!(room["answer_type_id"].as_i64(), Some(12));
    assert_eq!(room["missing"].as_i64(), Some(2));

    // Queue lists the untyped tracks only.
    let ids = missing_ids(&admin, room_id).await;
    assert!(ids.contains(&t2) && ids.contains(&t3) && !ids.contains(&t1));

    // Apply an answer: audited + leaves the queue, idempotent on retry.
    let apply = |tid: i64| {
        let admin = &admin;
        async move {
            admin
                .post(
                    &format!("/api/v1/admin/data-quality/tracks/{tid}/answer"),
                    json!({"room_id": room_id, "playlist_id": playlist_id, "value": "Test Game"}),
                )
                .await
        }
    };
    apply(t2).await.expect_ok("apply");
    apply(t2).await.expect_ok("apply idempotent");
    assert!(!missing_ids(&admin, room_id).await.contains(&t2));

    // Apply keeps a single answer row (no duplicate).
    let show = admin
        .get(&format!("/api/v1/playlists/{playlist_id}/tracks"))
        .await
        .expect_ok("playlist tracks");
    let t2_answers = show
        .json("/data")
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"].as_i64() == Some(t2))
        .map(|t| t["answers"].as_array().cloned().unwrap_or_default())
        .unwrap_or_default();
    assert_eq!(
        t2_answers
            .iter()
            .filter(|a| a["answer_type_id"].as_str() == Some("12"))
            .count(),
        1
    );

    // Dismiss removes from the queue, idempotently; restore puts it back.
    let dismiss = admin
        .post(
            &format!("/api/v1/admin/data-quality/tracks/{t3}/dismiss"),
            json!({"room_id": room_id, "playlist_id": playlist_id}),
        )
        .await
        .expect_ok("dismiss");
    assert!(dismiss.ok());
    admin
        .post(
            &format!("/api/v1/admin/data-quality/tracks/{t3}/dismiss"),
            json!({"room_id": room_id, "playlist_id": playlist_id}),
        )
        .await
        .expect_ok("dismiss idempotent");
    assert!(!missing_ids(&admin, room_id).await.contains(&t3));

    let actions = admin
        .get("/api/v1/admin/data-quality/actions")
        .await
        .expect_ok("actions")
        .json("/actions")
        .as_array()
        .cloned()
        .unwrap_or_default();
    let dismiss_action = actions
        .iter()
        .find(|a| a["action"].as_str() == Some("dismiss") && a["track_id"].as_i64() == Some(t3))
        .expect("dismiss logged")
        .clone();
    admin
        .post_empty(&format!(
            "/api/v1/admin/data-quality/actions/{}/restore",
            dismiss_action["id"].as_i64().unwrap()
        ))
        .await
        .expect_ok("undismiss");
    assert!(missing_ids(&admin, room_id).await.contains(&t3));

    // Remove purges the track from its playlist with an audited snapshot.
    admin
        .post(
            &format!("/api/v1/admin/data-quality/tracks/{t3}/remove"),
            json!({"room_id": room_id, "playlist_id": playlist_id}),
        )
        .await
        .expect_ok("remove");
    let show = admin
        .get(&format!("/api/v1/playlists/{playlist_id}/tracks"))
        .await
        .expect_ok("tracks after remove");
    assert!(
        !show
            .json("/data")
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["id"].as_i64() == Some(t3)),
        "removed track gone from playlist"
    );

    // Remove on a foreign playlist is rejected.
    let r = admin
        .post(
            &format!("/api/v1/admin/data-quality/tracks/{t2}/remove"),
            json!({"room_id": room_id, "playlist_id": 999999}),
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}
