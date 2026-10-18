mod common;
use common::*;
use reqwest::StatusCode;
use serde_json::json;

// ---- home -----------------------------------------------------------------

#[tokio::test]
async fn home_payload_shape() {
    let c = Client::anon();
    let r = c.get("/api/v1/home").await.expect_ok("home");
    let items = &r.body["catalog_items"];
    assert!(items["data"].is_array(), "catalog_items.data array");
    assert!(items["current_page"].is_number(), "paginator current_page");
    assert!(items["last_page"].is_number(), "paginator last_page");
    if let Some(room) = items["data"].as_array().and_then(|d| d.first()) {
        for key in [
            "id",
            "name",
            "slug",
            "photo",
            "is_playing",
            "is_public",
            "owner",
            "user_count",
        ] {
            assert!(room.get(key).is_some(), "room card .{key} missing: {room}");
        }
    }
}

async fn searchable_room(owner: &Client) -> (i64, String, String) {
    let pl = owner
        .post("/api/v1/playlists", json!({"name": uniq_name("pl")}))
        .await;
    assert_eq!(pl.status, StatusCode::CREATED, "playlist: {}", pl.text);
    let playlist_id = pl.json("/id").as_i64().unwrap();
    let name = uniq_name("srch");
    let r = owner
        .post("/api/v1/rooms", json!({"name": name, "category_id": 4}))
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "room: {}", r.text);
    let room_id = r.json("/room/id").as_i64().unwrap();
    let slug = r.json("/room/slug").as_str().unwrap().to_string();
    let attach = owner
        .post(
            &format!("/api/v1/rooms/{room_id}/playlists/attach"),
            json!({"playlist_id": playlist_id}),
        )
        .await;
    assert!(attach.ok(), "attach: {}", attach.text);
    (room_id, name, slug)
}

#[tokio::test]
async fn home_search_filters_by_room_name() {
    let (owner, _) = Client::register("sr").await;
    let (_, name, _) = searchable_room(&owner).await;
    let c = Client::anon();
    let r = c
        .get(&format!("/api/v1/home?search={name}"))
        .await
        .expect_ok("search");
    let res = &r.body["search_result"];
    let names: Vec<String> = res
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|r| r["name"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        names.iter().any(|n| n == &name),
        "{name} room in results: {names:?}"
    );
}

#[tokio::test]
async fn home_search_by_owner_name() {
    let (owner, _) = Client::register("ownr").await;
    let (_, name_a, _) = searchable_room(&owner).await;
    let (_, name_b, _) = searchable_room(&owner).await;
    let c = Client::anon();
    let r = c
        .get(&format!("/api/v1/home?search={}", owner.name))
        .await
        .expect_ok("search");
    let names: Vec<String> = r.body["search_result"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|r| r["name"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        names.contains(&name_a) && names.contains(&name_b),
        "owner rooms in results: {names:?}"
    );
}

#[tokio::test]
async fn categories_endpoint_lists_seeded_categories() {
    let c = Client::anon();
    let r = c.get("/api/v1/categories").await.expect_ok("categories");
    let cats = r.body["categories"].as_array().cloned().unwrap_or_default();
    assert!(cats.len() >= 4, "4 seeded categories, got {}", cats.len());
}

// ---- room CRUD -------------------------------------------------------------

#[tokio::test]
async fn room_create_validation_and_shape() {
    let (owner, _) = Client::register("rc").await;
    let bad = owner
        .post("/api/v1/rooms", json!({"name": "", "category_id": 4}))
        .await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);

    let bad_cat = owner
        .post(
            "/api/v1/rooms",
            json!({"name": uniq_name("r"), "category_id": 9999}),
        )
        .await;
    assert_eq!(bad_cat.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(bad_cat.json("/errors/category_id").is_array());

    let ok = owner
        .post(
            "/api/v1/rooms",
            json!({"name": uniq_name("r"), "category_id": 4}),
        )
        .await;
    assert_eq!(ok.status, StatusCode::CREATED, "{}", ok.text);
    let room = &ok.body["room"];
    assert!(room["id"].is_i64());
    assert_eq!(room["is_public"], false, "new rooms are private by default");
}

#[tokio::test]
async fn room_guest_cannot_create() {
    let g = Client::guest().await;
    let r = g
        .post(
            "/api/v1/rooms",
            json!({"name": uniq_name("g"), "category_id": 4}),
        )
        .await;
    assert!(
        r.status == StatusCode::FORBIDDEN || r.status == StatusCode::UNAUTHORIZED,
        "guest create room → 401/403, got {}",
        r.status
    );
}

#[tokio::test]
async fn room_show_payload_complete() {
    let (owner, _) = Client::register("rs").await;
    let (_, _, slug) = searchable_room(&owner).await;
    let c = Client::anon();
    let r = c
        .get(&format!("/api/v1/rooms/{slug}"))
        .await
        .expect_ok("room show");
    let room = &r.body["room"];
    for key in [
        "id",
        "name",
        "slug",
        "description",
        "category",
        "owner",
        "moderators",
        "photo",
        "is_playing",
        "is_public",
        "is_chat_active",
        "is_bookmarked",
        "track_duration",
        "pause_between_tracks",
        "pause_between_rounds",
        "tracks_by_round",
        "rounds_count",
        "can_edit",
    ] {
        assert!(room.get(key).is_some(), "room.{key} missing");
    }
    assert!(
        r.body["public_rooms"]
            .as_array()
            .is_some_and(|a| !a.is_empty())
    );
    assert!(r.body.get("game").is_some());
}

#[tokio::test]
async fn room_show_404_for_unknown_slug() {
    let c = Client::anon();
    let r = c.get("/api/v1/rooms/definitely-not-a-room-xyz").await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn room_bookmark_toggle() {
    let (u, _) = Client::register("bm").await;
    let on = u.post_empty("/api/v1/rooms/6/bookmark").await;
    assert!(on.ok(), "bookmark on: {}", on.text);
    let off = u.delete("/api/v1/rooms/6/bookmark").await;
    assert!(off.ok(), "bookmark off: {}", off.text);
}

#[tokio::test]
async fn room_private_not_in_public_index() {
    let (owner, _) = Client::register("pv").await;
    let name = uniq_name("priv");
    owner
        .post("/api/v1/rooms", json!({"name": name, "category_id": 4}))
        .await
        .expect_ok("create");
    let list = Client::anon()
        .get("/api/v1/rooms?per_page=100")
        .await
        .expect_ok("list");
    let found = list.body["rooms"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["name"] == name);
    assert!(!found, "private room must not appear in public index");
}

#[tokio::test]
async fn room_update_options_owner_only() {
    let (owner, _) = Client::register("uo").await;
    let (other, _) = Client::register("uo2").await;
    let r = owner
        .post(
            "/api/v1/rooms",
            json!({"name": uniq_name("opt"), "category_id": 4}),
        )
        .await
        .expect_ok("create");
    let id = r.json("/room/id").as_i64().unwrap();

    let full = json!({
        "tracks_by_round": 10, "track_duration": 20,
        "pause_between_tracks": 3, "pause_between_rounds": 10,
        "is_chat_active": true, "is_autostart": true, "is_random": true,
    });
    let denied = other
        .put(&format!("/api/v1/rooms/{id}/options"), full.clone())
        .await;
    assert_eq!(
        denied.status,
        StatusCode::FORBIDDEN,
        "non-owner options → 403"
    );

    let ok = owner
        .put(&format!("/api/v1/rooms/{id}/options"), full)
        .await;
    assert!(ok.ok(), "owner options: {}", ok.text);
}

#[tokio::test]
async fn room_edit_non_owner_forbidden() {
    let (owner, _) = Client::register("ed").await;
    let (other, _) = Client::register("ed2").await;
    let r = owner
        .post(
            "/api/v1/rooms",
            json!({"name": uniq_name("e"), "category_id": 4}),
        )
        .await
        .expect_ok("create");
    let id = r.json("/room/id").as_i64().unwrap();
    let denied = other.get(&format!("/api/v1/rooms/{id}/edit")).await;
    assert!(denied.status == StatusCode::FORBIDDEN || denied.status == StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn public_state_returns_game_snapshot() {
    let c = Client::anon();
    let r = c
        .get("/api/v1/rooms/6/public-state")
        .await
        .expect_ok("public-state");
    assert!(r.body.is_object());
}

// ---- seasonal visibility (is_active) ---------------------------------------

#[tokio::test]
async fn room_is_active_toggles_search_visibility() {
    let (owner, _) = Client::register("vis").await;
    let (id, name, _slug) = searchable_room(&owner).await;
    let c = Client::anon();

    let found = |body: &serde_json::Value| {
        body["search_result"]
            .as_array()
            .is_some_and(|a| a.iter().any(|r| r["name"] == name))
    };
    let search = format!("/api/v1/home?search={name}");

    let before = c.get(&search).await.expect_ok("search before");
    assert!(found(&before.body), "new room searchable: {}", before.text);

    let edit = owner
        .get(&format!("/api/v1/rooms/{id}/edit"))
        .await
        .expect_ok("edit payload");
    assert_eq!(edit.body["room"]["is_active"], true, "new room is_active");

    let opts = |active: bool| {
        json!({
            "tracks_by_round": 10, "track_duration": 20,
            "pause_between_tracks": 3, "pause_between_rounds": 10,
            "is_chat_active": true, "is_autostart": true, "is_random": true,
            "is_active": active,
        })
    };
    let off = owner
        .put(&format!("/api/v1/rooms/{id}/options"), opts(false))
        .await;
    assert!(off.ok(), "hide room: {}", off.text);

    let hidden = c.get(&search).await.expect_ok("search hidden");
    assert!(!found(&hidden.body), "inactive room hidden from search");

    let on = owner
        .put(&format!("/api/v1/rooms/{id}/options"), opts(true))
        .await;
    assert!(on.ok(), "show room: {}", on.text);
    let back = c.get(&search).await.expect_ok("search restored");
    assert!(found(&back.body), "reactivated room back in search");
}

#[tokio::test]
async fn room_is_active_toggle_forbidden_for_outsider() {
    let (owner, _) = Client::register("vo").await;
    let (other, _) = Client::register("vo2").await;
    let r = owner
        .post(
            "/api/v1/rooms",
            json!({"name": uniq_name("visf"), "category_id": 4}),
        )
        .await
        .expect_ok("create");
    let id = r.json("/room/id").as_i64().unwrap();
    let denied = other
        .put(
            &format!("/api/v1/rooms/{id}/options"),
            json!({
                "tracks_by_round": 10, "track_duration": 20,
                "pause_between_tracks": 3, "pause_between_rounds": 10,
                "is_chat_active": true, "is_autostart": true, "is_random": true,
                "is_active": false,
            }),
        )
        .await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
}

// ---- round control auth ----------------------------------------------------

#[tokio::test]
async fn room_start_requires_control() {
    let (owner, _) = Client::register("so").await;
    let (other, _) = Client::register("so2").await;
    let (mod_u, _) = Client::register("so3").await;
    let r = owner
        .post(
            "/api/v1/rooms",
            json!({"name": uniq_name("st"), "category_id": 4}),
        )
        .await
        .expect_ok("create");
    let id = r.json("/room/id").as_i64().unwrap();
    let mod_id = mod_u.user_id;

    // outsider → 403
    let denied = other.post_empty(&format!("/api/v1/rooms/{id}/start")).await;
    assert_eq!(
        denied.status,
        StatusCode::FORBIDDEN,
        "outsider start → 403: {}",
        denied.text
    );

    // guest → 403
    let guest = Client::guest().await;
    let g = guest.post_empty(&format!("/api/v1/rooms/{id}/start")).await;
    assert_eq!(g.status, StatusCode::FORBIDDEN, "guest start → 403");

    // attach moderator, then moderator can start
    let attach = owner
        .post(
            &format!("/api/v1/rooms/{id}/moderators/attach"),
            json!({"user_id": mod_id}),
        )
        .await;
    assert!(attach.ok(), "attach moderator: {}", attach.text);
    let ok = mod_u.post_empty(&format!("/api/v1/rooms/{id}/start")).await;
    assert!(ok.ok(), "moderator start: {}", ok.text);

    // owner can also start (room not playing after empty round auto-finish)
    let own = owner.post_empty(&format!("/api/v1/rooms/{id}/start")).await;
    assert!(own.ok(), "owner start: {}", own.text);
}
