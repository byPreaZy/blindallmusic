mod common;
use common::*;
use reqwest::StatusCode;
use serde_json::json;

const PNG_1PX: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4,
    0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

async fn wait_round(c: &Client, room_id: i64) -> (i64, i64) {
    for _ in 0..40 {
        let r = c
            .get(&format!("/api/v1/rooms/{room_id}/joined"))
            .await
            .expect_ok("joined");
        if let (Some(rid), Some(tid)) = (r.json("/round/id").as_i64(), r.json("/track/id").as_i64())
        {
            return (rid, tid);
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    panic!("round never started for room {room_id}")
}

async fn create_room(c: &Client) -> i64 {
    let r = c
        .post(
            "/api/v1/rooms",
            json!({"name": uniq_name("par"), "category_id": 4}),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "create room: {}", r.text);
    r.json("/room/id").as_i64().unwrap()
}

async fn post_message(c: &Client, room_id: i64, body: &str) -> common::Resp {
    c.post(
        &format!("/api/v1/rooms/{room_id}/messages"),
        json!({"body": body}),
    )
    .await
}

async fn message_ids(c: &Client, room_id: i64) -> Vec<i64> {
    let r = c
        .get(&format!("/api/v1/rooms/{room_id}/messages"))
        .await
        .expect_ok("history");
    r.json("/messages")
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|m| m["id"].as_i64())
        .collect()
}

// ---- BannedUserMessageCleanupTest -----------------------------------------

#[tokio::test]
async fn ban_soft_deletes_all_user_messages() {
    let (user, _) = Client::register("ban").await;
    let admin = Client::admin().await;

    let r1 = create_room(&user).await;
    let r2 = create_room(&user).await;
    post_message(&user, r1, "Spam message one")
        .await
        .expect_ok("msg1");
    post_message(&user, r2, "Spam message two")
        .await
        .expect_ok("msg2");
    assert_eq!(message_ids(&user, r1).await.len(), 1);
    assert_eq!(message_ids(&user, r2).await.len(), 1);

    let ban = admin
        .post(
            &format!("/api/v1/moderation/users/{}/ban", user.user_id),
            json!({"reason": "Spam", "duration": 60}),
        )
        .await;
    assert!(ban.ok(), "ban: {}", ban.text);

    assert!(
        message_ids(&user, r1).await.is_empty(),
        "messages must be soft-deleted on ban"
    );
    assert!(
        message_ids(&user, r2).await.is_empty(),
        "messages in every room must go"
    );
}

#[tokio::test]
async fn cross_room_autoban_soft_deletes_existing_messages() {
    let (user, _) = Client::register("spam").await;
    let r1 = create_room(&user).await;
    let r2 = create_room(&user).await;
    let r3 = create_room(&user).await;
    let spam = "Join my promo channel now";

    post_message(&user, r1, spam).await.expect_ok("spam1");
    post_message(&user, r2, spam).await.expect_ok("spam2");
    let third = post_message(&user, r3, spam).await;
    assert_eq!(
        third.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "3rd room → validation error: {}",
        third.text
    );

    let follow = post_message(&user, r1, "not spam anymore").await;
    assert_eq!(
        follow.status,
        StatusCode::FORBIDDEN,
        "banned → 403: {}",
        follow.text
    );
    assert!(message_ids(&user, r1).await.is_empty());
    assert!(message_ids(&user, r2).await.is_empty());
}

// ---- RoomJoinedIncludesPlayedTracksTest ------------------------------------

#[tokio::test]
async fn played_tracks_listed_for_mid_round_joiner() {
    let (owner, _) = Client::register("pj").await;
    let (room_id, playlist_id, _t1) = game_fixture(&owner, &[]).await;

    let t2_body = json!({
        "provider": "test",
        "provider_id": format!("e2e-pj-{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()),
        "provider_url": "https://example.com/track/2",
        "artist_name": "E2E Artist 2",
        "track_name": "E2E Song 2",
        "preview_url": format!("{}/healthz", base()),
        "artwork_url": "https://example.com/cover2.jpg",
    });
    owner
        .post(&format!("/api/v1/playlists/{playlist_id}/tracks"), t2_body)
        .await
        .expect_ok("track2");
    let options = json!({
        "tracks_by_round": 10, "track_duration": 5,
        "pause_between_tracks": 1, "pause_between_rounds": 30,
        "is_chat_active": true, "is_autostart": true, "is_random": true,
    });
    owner
        .put(&format!("/api/v1/rooms/{room_id}/options"), options)
        .await
        .expect_ok("options");

    for _ in 0..40 {
        let r = owner
            .get(&format!("/api/v1/rooms/{room_id}/joined"))
            .await
            .expect_ok("owner joined");
        if r.json("/track/id").as_i64().is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }

    let mut played = vec![];
    for _ in 0..60 {
        let r = owner
            .get(&format!("/api/v1/rooms/{room_id}/joined"))
            .await
            .expect_ok("poll");
        played = r
            .json("/playedTracks")
            .as_array()
            .cloned()
            .unwrap_or_default();
        if !played.is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    }
    assert!(
        !played.is_empty(),
        "track 1 should be in playedTracks by now"
    );

    let first = &played[0];
    assert!(first["id"].as_i64().unwrap() > 0);
    assert!(first.get("audio").is_none(), "no audio on played tracks");
    assert!(
        first["answers"].as_array().map_or(false, |a| !a.is_empty()),
        "answers present: {first}"
    );
    assert!(first["answers"][0]["type"]["name"].is_string());
    assert_eq!(first["user_voted_down"], false);

    let (joiner, _) = Client::register("pj2").await;
    visit_room(&joiner, room_id).await;
    let jr = joiner
        .get(&format!("/api/v1/rooms/{room_id}/joined"))
        .await
        .expect_ok("joiner joined");
    let jplayed = jr.json("/playedTracks").as_array().cloned().unwrap();
    assert_eq!(
        jplayed.len(),
        played.len(),
        "joiner sees the same played tracks"
    );
}

// ---- MessageReactionTest ---------------------------------------------------

#[tokio::test]
async fn message_reactions_toggle_and_supporter_gate() {
    let (user, _) = Client::register("react").await;
    let room_id = create_room(&user).await;
    post_message(&user, room_id, "react to me")
        .await
        .expect_ok("msg");
    let mid = message_ids(&user, room_id).await[0];

    let r = user
        .post(
            &format!("/api/v1/messages/{mid}/reactions"),
            json!({"emoji": "👍"}),
        )
        .await;
    assert!(r.ok(), "react: {}", r.text);
    let idx = user
        .get(&format!("/api/v1/messages/{mid}/reactions"))
        .await
        .expect_ok("reactions index");
    assert_eq!(idx.json("/userReaction"), json!("👍"));
    let grouped = idx.json("/reactions");
    assert!(
        grouped.to_string().contains("👍"),
        "grouped contains emoji: {grouped}"
    );
    user.post(
        &format!("/api/v1/messages/{mid}/reactions"),
        json!({"emoji": "👍"}),
    )
    .await;
    let idx2 = user.get(&format!("/api/v1/messages/{mid}/reactions")).await;
    assert!(idx2.json("/userReaction").is_null(), "toggled off");

    let locked = user
        .post(
            &format!("/api/v1/messages/{mid}/reactions"),
            json!({"emoji": "💎"}),
        )
        .await;
    assert_eq!(
        locked.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        locked.text
    );

    let bad = user
        .post(
            &format!("/api/v1/messages/{mid}/reactions"),
            json!({"emoji": "🐍"}),
        )
        .await;
    assert_eq!(bad.status, StatusCode::UNPROCESSABLE_ENTITY);

    let guest = Client::guest().await;
    let g = guest
        .post(
            &format!("/api/v1/messages/{mid}/reactions"),
            json!({"emoji": "👍"}),
        )
        .await;
    assert_eq!(g.status, StatusCode::FORBIDDEN);
}

// ---- UserPhotoUpdateTest ---------------------------------------------------

#[tokio::test]
async fn avatar_upload_remove_self_only() {
    let (user, _) = Client::register("av").await;
    let (other, _) = Client::register("av2").await;

    let up = user
        .post_file(
            &format!("/api/v1/users/{}/photo", user.user_id),
            "photo",
            "avatar.png",
            PNG_1PX,
        )
        .await;
    assert!(up.ok(), "upload: {}", up.text);
    let path = up.json("/photo_path").as_str().unwrap().to_string();
    assert!(path.starts_with("users/"), "stored under users/: {path}");

    let denied_up = other
        .post_file(
            &format!("/api/v1/users/{}/photo", user.user_id),
            "photo",
            "avatar.png",
            PNG_1PX,
        )
        .await;
    assert_eq!(denied_up.status, StatusCode::FORBIDDEN);
    let denied_del = other
        .delete(&format!("/api/v1/users/{}/photo", user.user_id))
        .await;
    assert_eq!(denied_del.status, StatusCode::FORBIDDEN);

    let del = user
        .delete(&format!("/api/v1/users/{}/photo", user.user_id))
        .await;
    assert!(del.ok(), "remove: {}", del.text);
}

// ---- RoundParticipationAuthorizationTest -----------------------------------

#[tokio::test]
async fn outsider_cannot_join_check_or_read_scores() {
    let (owner, _) = Client::register("pa").await;
    let (outsider, _) = Client::register("pa2").await;
    let (room_id, _pl, _t) = game_fixture(&owner, &[]).await;

    let j = outsider
        .get(&format!("/api/v1/rooms/{room_id}/joined"))
        .await;
    assert_eq!(j.status, StatusCode::FORBIDDEN, "{}", j.text);

    let (round, track) = wait_round(&owner, room_id).await;

    let sc = outsider
        .get(&format!("/api/v1/rounds/{round}/scores"))
        .await;
    assert_eq!(sc.status, StatusCode::FORBIDDEN);
    let ck = outsider
        .post(
            &format!("/api/v1/rounds/{round}/tracks/{track}/check"),
            json!({"text": "x", "words": [], "currentTime": 1.0}),
        )
        .await;
    assert_eq!(ck.status, StatusCode::FORBIDDEN);
    let listened = outsider
        .post_empty(&format!("/api/v1/rounds/{round}/tracks/{track}/listened"))
        .await;
    assert_eq!(listened.status, StatusCode::FORBIDDEN);

    visit_room(&outsider, room_id).await;
    let j2 = outsider
        .get(&format!("/api/v1/rooms/{room_id}/joined"))
        .await;
    assert_eq!(j2.status, StatusCode::OK, "post-show joined: {}", j2.text);
    let sc2 = outsider
        .get(&format!("/api/v1/rounds/{round}/scores"))
        .await;
    assert_eq!(sc2.status, StatusCode::OK);
}

#[tokio::test]
async fn password_room_requires_password() {
    let (owner, _) = Client::register("pw").await;
    let room_id = create_room(&owner).await;
    let options = json!({
        "password": "s3cret", "has_password": true,
        "tracks_by_round": 10, "track_duration": 20,
        "pause_between_tracks": 3, "pause_between_rounds": 10,
        "is_chat_active": true, "is_autostart": false, "is_random": true,
    });
    owner
        .put(&format!("/api/v1/rooms/{room_id}/options"), options)
        .await
        .expect_ok("options");

    let (user, _) = Client::register("pw2").await;
    let no_pw = user.get(&format!("/api/v1/rooms/{room_id}")).await;
    assert_eq!(no_pw.status, StatusCode::FORBIDDEN);
    let wrong = user
        .get(&format!("/api/v1/rooms/{room_id}?password=nope"))
        .await;
    assert_eq!(wrong.status, StatusCode::FORBIDDEN);
    let j = user.get(&format!("/api/v1/rooms/{room_id}/joined")).await;
    assert_eq!(j.status, StatusCode::FORBIDDEN);

    let ok = user
        .get(&format!("/api/v1/rooms/{room_id}?password=s3cret"))
        .await;
    assert_eq!(ok.status, StatusCode::OK, "{}", ok.text);
    let j2 = user.get(&format!("/api/v1/rooms/{room_id}/joined")).await;
    assert_eq!(j2.status, StatusCode::OK);
}

// ---- GuestPlayerTest restrictions ------------------------------------------

#[tokio::test]
async fn guest_cannot_chat_or_have_profile() {
    let (owner, _) = Client::register("g").await;
    let room_id = create_room(&owner).await;
    let guest = Client::guest().await;

    let msg = post_message(&guest, room_id, "hello from guest").await;
    assert_eq!(msg.status, StatusCode::FORBIDDEN, "guest can't chat");

    let me = guest.get("/api/v1/users/me").await;
    assert!(
        me.status == StatusCode::FORBIDDEN || me.status == StatusCode::UNAUTHORIZED,
        "guest /me → denied, got {}",
        me.status
    );

    let profile = guest
        .get(&format!("/api/v1/profile/{}", guest.user_id))
        .await;
    assert_eq!(
        profile.status,
        StatusCode::NOT_FOUND,
        "guest profile 404 (PHP scopeRealUsers)"
    );
}

// ---- ProcessDeletedTrack ---------------------------------------------------

#[tokio::test]
async fn track_delete_process_deleted_track_parity() {
    let (owner, _) = Client::register("td").await;
    let pl = owner
        .post("/api/v1/playlists", json!({"name": uniq_name("td")}))
        .await;
    let pl_id = pl.json("/id").as_i64().unwrap();
    let t = owner
        .post(
            &format!("/api/v1/playlists/{pl_id}/tracks"),
            json!({
                "provider": "test",
                "provider_id": uniq_name("td"),
                "provider_url": "https://example.com/track/0",
                "artist_name": "E2E Artist",
                "track_name": "E2E Song",
                "preview_url": "https://example.com/p.mp3",
                "artwork_url": "https://example.com/c.jpg",
            }),
        )
        .await;
    let track_id = t.json("/id").as_i64().unwrap();

    let del = owner
        .delete(&format!("/api/v1/playlists/{pl_id}/tracks/{track_id}"))
        .await;
    assert!(del.ok(), "delete: {}", del.text);

    let show = owner
        .get(&format!("/api/v1/playlists/{pl_id}"))
        .await
        .expect_ok("show");
    assert_eq!(show.json("/playlist/total_tracks"), json!(0));

    let notifs = owner
        .get("/api/v1/users/notifications")
        .await
        .expect_ok("notifications");
    let found = notifs
        .json("/notifications")
        .as_array()
        .unwrap()
        .iter()
        .any(|n| {
            n["type"] == "App\\Notifications\\TrackDeleted"
                && n["data"]["playlist"]["id"] == pl_id
                && n["data"]["message"]
                    .as_str()
                    .unwrap_or("")
                    .contains("E2E Song")
                && n["data"]["message"]
                    .as_str()
                    .unwrap_or("")
                    .contains(&format!("supprimé par {}", owner.name))
        });
    assert!(found, "TrackDeleted notif missing: {}", notifs.text);
}

#[tokio::test]
async fn playlist_clear_process_deleted_track_per_track() {
    let (owner, _) = Client::register("clr").await;
    let pl = owner
        .post("/api/v1/playlists", json!({"name": uniq_name("clr")}))
        .await;
    let pl_id = pl.json("/id").as_i64().unwrap();
    for i in 0..2 {
        owner
            .post(
                &format!("/api/v1/playlists/{pl_id}/tracks"),
                json!({
                    "provider": "test",
                    "provider_id": uniq_name("clr"),
                    "provider_url": format!("https://example.com/t/{i}"),
                    "artist_name": "A", "track_name": format!("T{i}"),
                    "preview_url": "https://example.com/p.mp3",
                    "artwork_url": "https://example.com/c.jpg",
                }),
            )
            .await
            .expect_ok("track");
    }

    let (other, _) = Client::register("clr2").await;
    let denied = other
        .delete(&format!("/api/v1/playlists/{pl_id}/tracks/clear"))
        .await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);

    let clear = owner
        .delete(&format!("/api/v1/playlists/{pl_id}/tracks/clear"))
        .await;
    assert!(clear.ok(), "clear: {}", clear.text);
    let show = owner
        .get(&format!("/api/v1/playlists/{pl_id}"))
        .await
        .expect_ok("show");
    assert_eq!(show.json("/playlist/total_tracks"), json!(0));

    let notifs = owner
        .get("/api/v1/users/notifications")
        .await
        .expect_ok("notifications");
    let count = notifs
        .json("/notifications")
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| {
            n["type"] == "App\\Notifications\\TrackDeleted" && n["data"]["playlist"]["id"] == pl_id
        })
        .count();
    assert_eq!(count, 2, "one TrackDeleted per track: {}", notifs.text);
}

#[tokio::test]
async fn track_delete_public_playlist_sends_no_notification() {
    let (owner, _) = Client::register("tdp").await;
    let pl = owner
        .post("/api/v1/playlists", json!({"name": uniq_name("tdp")}))
        .await;
    let pl_id = pl.json("/id").as_i64().unwrap();
    Client::admin()
        .await
        .put(
            &format!("/api/v1/admin/playlists/{pl_id}"),
            json!({"name": "pub", "is_public": true}),
        )
        .await
        .expect_ok("publish");
    let t = owner
        .post(
            &format!("/api/v1/playlists/{pl_id}/tracks"),
            json!({
                "provider": "test",
                "provider_id": uniq_name("tdp"),
                "provider_url": "https://example.com/track/0",
                "artist_name": "A", "track_name": "T",
                "preview_url": "https://example.com/p.mp3",
                "artwork_url": "https://example.com/c.jpg",
            }),
        )
        .await;
    let track_id = t.json("/id").as_i64().unwrap();
    owner
        .delete(&format!("/api/v1/playlists/{pl_id}/tracks/{track_id}"))
        .await
        .expect_ok("delete");

    let notifs = owner
        .get("/api/v1/users/notifications")
        .await
        .expect_ok("notifications");
    let found = notifs
        .json("/notifications")
        .as_array()
        .unwrap()
        .iter()
        .any(|n| {
            n["type"] == "App\\Notifications\\TrackDeleted" && n["data"]["playlist"]["id"] == pl_id
        });
    assert!(!found, "public playlist must not notify: {}", notifs.text);
}

#[tokio::test]
async fn rooms_mine_filters_and_extras() {
    let (owner, _) = Client::register("mine").await;
    let room_id = {
        let r = owner
            .post(
                "/api/v1/rooms",
                json!({"name": uniq_name("mymine"), "category_id": 4}),
            )
            .await
            .expect_ok("create room");
        r.json("/room/id").as_i64().unwrap()
    };
    let resp = owner.get("/api/v1/rooms/mine").await.expect_ok("mine");
    let room = resp
        .json("/rooms")
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == room_id)
        .cloned()
        .expect("owned room must be listed");
    assert!(room["rounds_count"].is_number(), "rounds_count: {}", room);
    assert!(room["moderators"].is_array(), "moderators: {}", room);
    assert!(room["playlists"].is_array(), "playlists: {}", room);
    assert!(room["category"].is_object(), "category: {}", room);

    let name = room["name"].as_str().unwrap().to_string();
    let hit = owner
        .get(&format!("/api/v1/rooms/mine?search={name}"))
        .await
        .expect_ok("search hit");
    assert!(
        hit.json("/rooms")
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == room_id),
        "search must match by name: {}",
        hit.text
    );
    let miss = owner
        .get("/api/v1/rooms/mine?search=zz-no-such-room-zz")
        .await
        .expect_ok("search miss");
    assert!(
        miss.json("/rooms")
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["id"] != room_id),
        "absent term must filter the room out"
    );

    let trashed = owner
        .get("/api/v1/rooms/mine?trashed=only")
        .await
        .expect_ok("trashed only");
    assert!(
        trashed
            .json("/rooms")
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["id"] != room_id),
        "non-deleted room must not appear in trashed=only"
    );

    owner
        .delete(&format!("/api/v1/rooms/{room_id}"))
        .await
        .expect_ok("delete room");
    let active = owner
        .get("/api/v1/rooms/mine")
        .await
        .expect_ok("mine post-delete");
    assert!(
        active
            .json("/rooms")
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["id"] != room_id),
        "deleted room must be hidden by default"
    );
    let only = owner
        .get("/api/v1/rooms/mine?trashed=only")
        .await
        .expect_ok("trashed only post-delete");
    assert!(
        only.json("/rooms")
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["id"] != room_id),
        "moderator link is gone on delete, trashed=only stays empty"
    );
}

#[tokio::test]
async fn account_delete_removes_user() {
    let email = uniq_email("del");
    let c = Client::anon();
    let r = c
        .post(
            "/api/v1/auth/register",
            json!({
                "name": uniq_name("del"),
                "email": email,
                "password": "password123",
                "password_confirmation": "password123",
            }),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "register: {}", r.text);
    let uid = r.json("/user/id").as_i64().unwrap();
    let mut c = c;
    c.token = Some(r.json("/token").as_str().unwrap().to_string());

    c.delete(&format!("/api/v1/users/{uid}"))
        .await
        .expect_ok("delete account");
    let login = Client::anon().login(&email, "password123").await;
    assert!(
        login.status != StatusCode::OK,
        "deleted account must not log in: {}",
        login.text
    );
    let me = c.get("/api/v1/users/me").await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED, "stale token must 401");
}
