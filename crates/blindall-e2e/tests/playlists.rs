mod common;
use common::*;
use reqwest::StatusCode;
use serde_json::json;

async fn playlist(owner: &Client) -> i64 {
    let r = owner
        .post("/api/v1/playlists", json!({"name": uniq_name("pl")}))
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "playlist: {}", r.text);
    r.json("/id").as_i64().unwrap()
}

async fn track(owner: &Client, playlist_id: i64) -> i64 {
    let r = owner
        .post(
            &format!("/api/v1/playlists/{playlist_id}/tracks"),
            json!({
                "provider": "test",
                "provider_id": format!("e2e-{}", uniq_name("t")),
                "provider_url": "https://example.com/track/0",
                "artist_name": "E2E Artist",
                "track_name": "E2E Song",
                "preview_url": "https://example.com/p.mp3",
                "artwork_url": "https://example.com/c.jpg",
            }),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "track: {}", r.text);
    r.json("/id").as_i64().unwrap()
}

// ---- playlists ------------------------------------------------------------

#[tokio::test]
async fn playlist_store_validation() {
    let (owner, _) = Client::register("pl").await;
    for name in ["", &"x".repeat(51)] {
        let r = owner
            .post("/api/v1/playlists", json!({"name": name}))
            .await;
        assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "name={name}");
    }
    let r = owner
        .post("/api/v1/playlists", json!({"name": uniq_name("ok")}))
        .await;
    assert_eq!(r.status, StatusCode::CREATED);
    assert!(r.json("/id").is_i64());
}

#[tokio::test]
async fn playlist_index_lists_mine() {
    let (owner, _) = Client::register("pl").await;
    let name = uniq_name("mine");
    owner.post("/api/v1/playlists", json!({"name": name})).await;
    let r = owner.get("/api/v1/playlists").await.expect_ok("index");
    let found = r.json("/playlists")
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["name"] == name);
    assert!(found, "{}", r.text);
}

#[tokio::test]
async fn playlist_show_shape_and_auth() {
    let (owner, _) = Client::register("pl").await;
    let id = playlist(&owner).await;
    let r = owner
        .get(&format!("/api/v1/playlists/{id}"))
        .await
        .expect_ok("show");
    for k in ["id", "name", "user_id", "moderators", "total_tracks", "difficulties"] {
        assert!(r.body["playlist"].get(k).is_some(),
            "missing playlist.{k}: {}", r.text);
    }
    assert!(r.body.get("can_edit").is_some() && r.body.get("owned").is_some());
    let r = owner
        .get(&format!("/api/v1/playlists/{id}/tracks"))
        .await
        .expect_ok("tracks");
    assert!(r.body.is_object() || r.body.is_array(), "{}", r.text);
}

#[tokio::test]
async fn playlist_update_owner_only() {
    let (owner, _) = Client::register("pl").await;
    let (other, _) = Client::register("pl").await;
    let id = playlist(&owner).await;
    let r = other
        .put(
            &format!("/api/v1/playlists/{id}"),
            json!({"name": "hijack", "user_id": other.user_id}),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = owner
        .put(
            &format!("/api/v1/playlists/{id}"),
            json!({"name": uniq_name("ren"), "user_id": owner.user_id}),
        )
        .await;
    assert!(r.ok(), "owner update: {}", r.text);
}

#[tokio::test]
async fn playlist_destroy_owner_only() {
    let (owner, _) = Client::register("pl").await;
    let (other, _) = Client::register("pl").await;
    let id = playlist(&owner).await;
    let r = other.delete(&format!("/api/v1/playlists/{id}")).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = owner.delete(&format!("/api/v1/playlists/{id}")).await;
    assert!(r.ok(), "{}", r.text);
    let r = owner.get(&format!("/api/v1/playlists/{id}")).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

// ---- tracks ----------------------------------------------------------------

#[tokio::test]
async fn track_store_autocreates_artist_and_title_answers() {
    let (owner, _) = Client::register("pl").await;
    let pl = playlist(&owner).await;
    track(&owner, pl).await;
    let r = owner
        .get(&format!("/api/v1/playlists/{pl}/tracks"))
        .await
        .expect_ok("tracks");
    assert!(r.text.contains("E2E Artist") && r.text.contains("E2E Song"), "{}", r.text);
    let r = owner
        .get("/api/v1/answer-types")
        .await
        .expect_ok("types");
    assert!(r.json("/answer_types").as_array().unwrap().len() >= 2);
}

#[tokio::test]
async fn track_update_dificulty_and_hint() {
    let (owner, _) = Client::register("pl").await;
    let pl = playlist(&owner).await;
    let t = track(&owner, pl).await;
    let r = owner
        .put(
            &format!("/api/v1/playlists/{pl}/tracks/{t}"),
            json!({"dificulty": 3, "hint": "starts with E"}),
        )
        .await;
    assert!(r.ok(), "{}", r.text);
    let r = owner
        .put(
            &format!("/api/v1/playlists/{pl}/tracks/{t}"),
            json!({"dificulty": 9}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn track_delete_and_404() {
    let (owner, _) = Client::register("pl").await;
    let pl = playlist(&owner).await;
    let t = track(&owner, pl).await;
    let r = owner
        .delete(&format!("/api/v1/playlists/{pl}/tracks/{t}"))
        .await;
    assert!(r.ok());
    let r = owner
        .put(
            &format!("/api/v1/playlists/{pl}/tracks/{t}"),
            json!({"dificulty": 1}),
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn track_wrong_playlist_scope_404() {
    let (owner, _) = Client::register("pl").await;
    let pl1 = playlist(&owner).await;
    let pl2 = playlist(&owner).await;
    let t = track(&owner, pl1).await;
    let r = owner
        .put(
            &format!("/api/v1/playlists/{pl2}/tracks/{t}"),
            json!({"dificulty": 1}),
        )
        .await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

// ---- answers ---------------------------------------------------------------

#[tokio::test]
async fn answer_crud_and_validation() {
    let (owner, _) = Client::register("pl").await;
    let pl = playlist(&owner).await;
    let t = track(&owner, pl).await;

    let r = owner
        .post(
            &format!("/api/v1/tracks/{t}/answers"),
            json!({"answer_type_id": 999, "value": "X", "score": 0.5}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);

    let r = owner
        .post(
            &format!("/api/v1/tracks/{t}/answers"),
            json!({"answer_type_id": 1, "value": "", "score": 0.5}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);

    let r = owner
        .post(
            &format!("/api/v1/tracks/{t}/answers"),
            json!({"answer_type_id": 1, "value": "Alt", "score": 100.0}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);

    let r = owner
        .post(
            &format!("/api/v1/tracks/{t}/answers"),
            json!({"answer_type_id": 1, "value": "Alt Artist", "score": 1.0, "aliases": ["alt"]}),
        )
        .await;
    assert!(r.ok(), "create answer: {}", r.text);
    let aid = r.json("/id").as_i64().unwrap_or_else(|| r.json("/answer/id").as_i64().unwrap());

    let r = owner
        .put(
            &format!("/api/v1/tracks/{t}/answers/{aid}"),
            json!({"answer_type_id": 1, "value": "Alt Artist 2", "score": 1.5}),
        )
        .await;
    assert!(r.ok(), "update: {}", r.text);

    let r = owner
        .delete(&format!("/api/v1/tracks/{t}/answers/{aid}"))
        .await;
    assert!(r.ok(), "delete: {}", r.text);
}

#[tokio::test]
async fn answer_not_owner_forbidden() {
    let (owner, _) = Client::register("pl").await;
    let (other, _) = Client::register("pl").await;
    let pl = playlist(&owner).await;
    let t = track(&owner, pl).await;
    let r = other
        .post(
            &format!("/api/v1/tracks/{t}/answers"),
            json!({"answer_type_id": 1, "value": "X", "score": 0.5}),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
}

// ---- moderators / export ---------------------------------------------------

#[tokio::test]
async fn playlist_moderator_attach_detach() {
    let (owner, _) = Client::register("pl").await;
    let (modo, _) = Client::register("pl").await;
    let pl = playlist(&owner).await;
    let r = owner
        .post(
            &format!("/api/v1/playlists/{pl}/moderators"),
            json!({"user_id": 999999}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    let r = owner
        .post(
            &format!("/api/v1/playlists/{pl}/moderators"),
            json!({"user_id": modo.user_id}),
        )
        .await;
    assert!(r.ok(), "attach: {}", r.text);
    let r = modo
        .put(
            &format!("/api/v1/playlists/{pl}"),
            json!({"name": uniq_name("mod"), "user_id": owner.user_id}),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "moderator rename must 403");
    let r = modo
        .post(
            &format!("/api/v1/playlists/{pl}/tracks"),
            json!({
                "provider": "test", "provider_id": uniq_name("mt"),
                "provider_url": "https://example.com/t", "artist_name": "Mod Artist",
                "track_name": "Mod Song", "preview_url": "https://example.com/p.mp3",
                "artwork_url": "https://example.com/c.jpg",
            }),
        )
        .await;
    assert!(r.ok(), "moderator add track: {}", r.text);
    let r = owner
        .delete_body(
            &format!("/api/v1/playlists/{pl}/moderators"),
            json!({"user_id": modo.user_id}),
        )
        .await;
    assert!(r.ok(), "detach: {}", r.text);
    let r = modo
        .post(
            &format!("/api/v1/playlists/{pl}/tracks"),
            json!({
                "provider": "test", "provider_id": uniq_name("mt"),
                "provider_url": "https://example.com/t", "artist_name": "No",
                "track_name": "No", "preview_url": "https://example.com/p.mp3",
                "artwork_url": "https://example.com/c.jpg",
            }),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn playlist_export_csv() {
    let (owner, _) = Client::register("pl").await;
    let pl = playlist(&owner).await;
    track(&owner, pl).await;
    let r = owner
        .get(&format!("/api/v1/playlists/{pl}/export"))
        .await
        .expect_ok("export");
    assert!(r.text.contains("E2E") || r.text.contains(';'), "csv: {}", &r.text[..r.text.len().min(300)]);
}

// ---- answer types -----------------------------------------------------------

#[tokio::test]
async fn answer_types_list() {
    let r = Client::anon()
        .get("/api/v1/answer-types")
        .await
        .expect_ok("types");
    let types = r.json("/answer_types").as_array().cloned().unwrap_or_default();
    let names: Vec<String> = types.iter().filter_map(|t| t["name"].as_str().map(String::from)).collect();
    assert!(names.iter().any(|n| n.contains("Artist")), "{names:?}");
    assert!(names.iter().any(|n| n.contains("Title")), "{names:?}");
}
