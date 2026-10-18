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

#[tokio::test]
async fn data_quality_moderator_scope() {
    let admin = Client::admin().await;
    let (room_id, playlist_id, _) = game_fixture(&admin, &[("Test Game", 12, 1.0)]).await;
    let t4 = track(&admin, playlist_id).await;
    admin
        .post(
            &format!("/api/v1/tracks/{t4}/answers"),
            json!({"answer_type_id": 12, "value": "Test Game", "score": 1.0}),
        )
        .await
        .expect_ok("typed answer");
    let t2 = track(&admin, playlist_id).await;

    // Make the room public so attached moderators become public moderators.
    admin
        .put(
            &format!("/api/v1/admin/rooms/{room_id}"),
            json!({"name": format!("DQ{}", uniq_name("room")), "is_public": true}),
        )
        .await
        .expect_ok("publish room");

    let (mod_u, _) = Client::register("dqmod").await;
    admin
        .post(
            &format!("/api/v1/rooms/{room_id}/moderators/attach"),
            json!({"user_id": mod_u.user_id}),
        )
        .await
        .expect_ok("attach moderator");

    // Moderator can review: list, queue, apply, dismiss, history.
    mod_u
        .get("/api/v1/admin/data-quality/rooms")
        .await
        .expect_ok("mod rooms");
    mod_u
        .get(&format!(
            "/api/v1/admin/data-quality/rooms/{room_id}/missing"
        ))
        .await
        .expect_ok("mod missing");
    mod_u
        .post(
            &format!("/api/v1/admin/data-quality/tracks/{t2}/answer"),
            json!({"room_id": room_id, "playlist_id": playlist_id, "value": "Test Game"}),
        )
        .await
        .expect_ok("mod apply");
    let t3 = track(&admin, playlist_id).await;
    mod_u
        .post(
            &format!("/api/v1/admin/data-quality/tracks/{t3}/dismiss"),
            json!({"room_id": room_id, "playlist_id": playlist_id}),
        )
        .await
        .expect_ok("mod dismiss");

    // But structural ops stay admin-only.
    for (path, body) in [
        (
            format!("/api/v1/admin/data-quality/tracks/{t3}/remove"),
            json!({"room_id": room_id, "playlist_id": playlist_id}),
        ),
        (
            format!("/api/v1/admin/data-quality/tracks/{t3}/move"),
            json!({"room_id": room_id, "playlist_id": playlist_id, "to_playlist_id": playlist_id}),
        ),
        (
            "/api/v1/admin/data-quality/enrichment".to_string(),
            json!({"rows": [{"track_id": t3}]}),
        ),
    ] {
        let r = mod_u.post(&path, body).await;
        assert_eq!(r.status, StatusCode::FORBIDDEN, "{path}");
    }
}

#[tokio::test]
async fn data_quality_enrichment_move_undo() {
    let admin = Client::admin().await;
    let (room_id, playlist_id, _) = game_fixture(&admin, &[("Test Game", 12, 1.0)]).await;
    let t4 = track(&admin, playlist_id).await;
    admin
        .post(
            &format!("/api/v1/tracks/{t4}/answers"),
            json!({"answer_type_id": 12, "value": "Test Game", "score": 1.0}),
        )
        .await
        .expect_ok("typed answer");
    let t2 = track(&admin, playlist_id).await;

    // Enrichment import: upserts and is surfaced on the queue row.
    let rows = json!({"rows": [{
        "track_id": t2, "album": "OST Vol 1", "mb_work": "Test Game",
        "mb_releases": "OST Vol 1 | OST Vol 2", "confidence": "high",
    }]});
    let r = admin
        .post("/api/v1/admin/data-quality/enrichment", rows.clone())
        .await
        .expect_ok("enrichment");
    assert_eq!(r.json("/imported").as_i64(), Some(1));
    admin
        .post("/api/v1/admin/data-quality/enrichment", rows)
        .await
        .expect_ok("enrichment idempotent");

    let data = admin
        .get(&format!(
            "/api/v1/admin/data-quality/rooms/{room_id}/missing?page=1&per_page=100"
        ))
        .await
        .expect_ok("missing")
        .json("/data")
        .as_array()
        .cloned()
        .unwrap_or_default();
    let row = data
        .iter()
        .find(|r| r["id"].as_i64() == Some(t2))
        .expect("t2 in queue")
        .clone();
    assert_eq!(row["album"].as_str(), Some("OST Vol 1"));
    assert_eq!(row["mb_work"].as_str(), Some("Test Game"));
    assert_eq!(row["confidence"].as_str(), Some("high"));

    // Playlist options for the move picker.
    let pl2 = admin
        .post("/api/v1/playlists", json!({"name": uniq_name("dq-target")}))
        .await
        .expect_ok("target playlist")
        .json("/id")
        .as_i64()
        .unwrap();
    let options = admin
        .get("/api/v1/admin/data-quality/playlists")
        .await
        .expect_ok("playlist options")
        .json("/playlists")
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(options.iter().any(|p| p["id"].as_i64() == Some(pl2)));

    // Move the track: audited, lands in the target playlist.
    admin
        .post(
            &format!("/api/v1/admin/data-quality/tracks/{t2}/move"),
            json!({"room_id": room_id, "playlist_id": playlist_id, "to_playlist_id": pl2}),
        )
        .await
        .expect_ok("move");
    let moved = admin
        .get(&format!("/api/v1/playlists/{pl2}/tracks"))
        .await
        .expect_ok("target tracks")
        .json("/data")
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(moved.iter().any(|t| t["id"].as_i64() == Some(t2)));
    assert!(!missing_ids(&admin, room_id).await.contains(&t2));

    // Remove + undo restores the snapshot (track + answers, original ids).
    let t3 = track(&admin, playlist_id).await;
    admin
        .post(
            &format!("/api/v1/admin/data-quality/tracks/{t3}/remove"),
            json!({"room_id": room_id, "playlist_id": playlist_id}),
        )
        .await
        .expect_ok("remove");
    let actions = admin
        .get("/api/v1/admin/data-quality/actions")
        .await
        .expect_ok("actions")
        .json("/actions")
        .as_array()
        .cloned()
        .unwrap_or_default();
    let remove_action = actions
        .iter()
        .find(|a| {
            a["action"].as_str() == Some("remove_track") && a["track_id"].as_i64() == Some(t3)
        })
        .expect("remove logged")
        .clone();
    admin
        .post_empty(&format!(
            "/api/v1/admin/data-quality/actions/{}/undo",
            remove_action["id"].as_i64().unwrap()
        ))
        .await
        .expect_ok("undo");
    let restored = admin
        .get(&format!("/api/v1/playlists/{playlist_id}/tracks"))
        .await
        .expect_ok("tracks after undo")
        .json("/data")
        .as_array()
        .cloned()
        .unwrap_or_default();
    let t3r = restored
        .iter()
        .find(|t| t["id"].as_i64() == Some(t3))
        .expect("t3 restored");
    assert!(
        t3r["answers"].as_array().unwrap().len() >= 2,
        "restored answers: {}",
        t3r["answers"]
    );
}
