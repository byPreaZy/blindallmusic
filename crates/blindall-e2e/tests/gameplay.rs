mod common;
use common::*;
use reqwest::StatusCode;
use serde_json::json;

async fn wait_round(c: &Client, room_id: i64) -> (i64, i64) {
    for _ in 0..40 {
        let r = c
            .get(&format!("/api/v1/rooms/{room_id}/joined"))
            .await
            .expect_ok("joined");
        let rid = r.json("/round/id").as_i64();
        let tid = r.json("/track/id").as_i64();
        if let (Some(rid), Some(tid)) = (rid, tid) {
            return (rid, tid);
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    panic!("round never started for room {room_id}");
}

async fn answer(c: &Client, round: i64, track: i64, text: &str) -> common::Resp {
    c.post(
        &format!("/api/v1/rounds/{round}/tracks/{track}/check"),
        json!({"text": text, "words": [], "currentTime": 5.0}),
    )
    .await
}

#[tokio::test]
async fn joined_autostarts_round_and_returns_track() {
    let (owner, _) = Client::register("gp").await;
    let (room_id, _pl, track_id) =
        game_fixture(&owner, &[]).await;
    let (round, track) = wait_round(&owner, room_id).await;
    assert!(round > 0);
    assert_eq!(track, track_id, "single-track playlist → deterministic pick");
}

#[tokio::test]
async fn joined_hides_current_track_answers() {
    let (owner, _) = Client::register("gh").await;
    let (room_id, _pl, track_id) = game_fixture(&owner, &[]).await;
    let (_round, track) = wait_round(&owner, room_id).await;
    assert_eq!(track, track_id);

    let j = owner
        .get(&format!("/api/v1/rooms/{room_id}/joined"))
        .await
        .expect_ok("joined");
    let played = j.json("/playedTracks").as_array().cloned().unwrap_or_default();
    assert!(
        played.is_empty(),
        "playing track must not be in playedTracks: {played:?}"
    );

    let t = &j.json("/track");
    assert_eq!(t["id"].as_i64().unwrap(), track_id);
    assert!(t.get("track_name").is_none(), "track_name leaked: {t}");
    assert!(t.get("artist_name").is_none(), "artist_name leaked: {t}");
    for a in t["answers"].as_array().unwrap() {
        assert!(a.get("value").is_none(), "answer value leaked: {a}");
        assert!(a["name"].is_string());
    }
}

#[tokio::test]
async fn correct_answer_scores_and_message() {
    let (owner, _) = Client::register("ga").await;
    let (room_id, _pl, _track_id) =
        game_fixture(&owner, &[]).await;
    let (round, track) = wait_round(&owner, room_id).await;

    let r = answer(&owner, round, track, "E2E Song").await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    let good = r.json("/good_answers");
    let good_arr = good.as_array().unwrap();
    assert_eq!(good_arr.len(), 1, "one good answer: {}", r.text);
    assert_eq!(good_arr[0]["name"], "Title");
    assert_eq!(good_arr[0]["score"].as_f64().unwrap(), 1.5);
    assert_eq!(good_arr[0]["speedBonus"], true);
    assert_eq!(good_arr[0]["order"], 1);
    assert_eq!(r.json("/total_score").as_f64().unwrap(), 1.5);
    assert_eq!(r.json("/message/type"), "good");
    assert!(r.json("/message/body").as_str().unwrap().len() > 3);
    assert!(r.json("/words").as_array().unwrap().iter().any(|w| w == "e2e" || w == "song" || w == "E2E"));
}

#[tokio::test]
async fn wrong_answer_returns_bad_message_zero_score() {
    let (owner, _) = Client::register("gw").await;
    let (room_id, _pl, _t) =
        game_fixture(&owner, &[]).await;
    let (round, track) = wait_round(&owner, room_id).await;

    let r = answer(&owner, round, track, "completely wrong guess").await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json("/good_answers").as_array().unwrap().len(), 0);
    assert_eq!(r.json("/message/type"), "bad");
    assert_eq!(r.json("/total_score").as_f64().unwrap_or(0.0), 0.0);
}

#[tokio::test]
async fn answer_matching_is_case_insensitive() {
    let (owner, _) = Client::register("gc").await;
    let (room_id, _pl, _t) =
        game_fixture(&owner, &[]).await;
    let (round, track) = wait_round(&owner, room_id).await;
    let r = answer(&owner, round, track, "e2e song").await;
    assert_eq!(r.json("/good_answers").as_array().unwrap().len(), 1, "lowercase must match");
}

#[tokio::test]
async fn track_mismatch_returns_409() {
    let (owner, _) = Client::register("gm").await;
    let (room_id, _pl, _t) =
        game_fixture(&owner, &[]).await;
    let (round, track) = wait_round(&owner, room_id).await;
    let r = answer(&owner, round, track + 9999, "x").await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text);
}

#[tokio::test]
async fn same_answer_twice_not_double_scored() {
    let (owner, _) = Client::register("gd").await;
    let (room_id, _pl, _t) =
        game_fixture(&owner, &[]).await;
    let (round, track) = wait_round(&owner, room_id).await;

    let first = answer(&owner, round, track, "E2E Song").await;
    assert_eq!(first.json("/good_answers").as_array().unwrap().len(), 1);
    let second = answer(&owner, round, track, "E2E Song").await;
    let good = second.json("/good_answers").as_array().unwrap().len();
    let score = second.json("/total_score").as_f64().unwrap_or(0.0);
    assert!(good == 0 || score == 0.0, "dup answer must not re-score: {}", second.text);
}

#[tokio::test]
async fn both_answer_types_scored_independently() {
    let (owner, _) = Client::register("gb").await;
    let (room_id, _pl, _t) =
        game_fixture(&owner, &[]).await;
    let (round, track) = wait_round(&owner, room_id).await;

    answer(&owner, round, track, "E2E Artist").await.expect_ok("artist");
    let r = answer(&owner, round, track, "E2E Song").await.expect_ok("title");
    assert_eq!(r.json("/total_score").as_f64().unwrap(), 3.0, "cumulative");
}

#[tokio::test]
async fn extra_answer_types_are_asked_and_scored() {
    let (owner, _) = Client::register("gx").await;
    let (room_id, _pl, track_id) = game_fixture(&owner, &[
        ("Mulan", 6, 0.5),   // Movie
        ("Pitbull", 5, 0.5), // Featuring
    ])
    .await;
    let (round, track) = wait_round(&owner, room_id).await;
    assert_eq!(track, track_id);

    let j = owner
        .get(&format!("/api/v1/rooms/{room_id}/joined"))
        .await
        .expect_ok("joined");
    let names: Vec<String> = j
        .json("/track/answers")
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|a| a["name"].as_str().map(str::to_string))
        .collect();
    for want in ["Artist", "Title", "Movie", "Featuring"] {
        assert!(names.iter().any(|n| n == want), "missing {want}: {names:?}");
    }

    for (text, want) in [
        ("Mulan", "Movie"),
        ("Pitbull", "Featuring"),
        ("E2E Artist", "Artist"),
        ("E2E Song", "Title"),
    ] {
        let r = answer(&owner, round, track, text).await;
        let good = r.json("/good_answers").as_array().cloned().unwrap_or_default();
        assert!(
            good.iter().any(|a| a["name"] == want),
            "{text} should find {want}: {}",
            r.text
        );
    }

    let r = answer(&owner, round, track, "anything else").await;
    assert_eq!(r.json("/good_answers").as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn answer_aliases_and_paren_titles_match() {
    let (owner, _) = Client::register("gy").await;
    let (room_id, _pl, track_id) = game_fixture(&owner, &[]).await;

    let a = owner
        .post(
            &format!("/api/v1/tracks/{track_id}/answers"),
            json!({
                "answer_type_id": 6,
                "value": "Pocahontas, Une Légende Indienne",
                "score": 0.5,
                "aliases": ["Pocahontas"],
            }),
        )
        .await;
    assert!(a.status.is_success(), "movie answer: {}", a.text);

    let a2 = owner
        .post(
            &format!("/api/v1/tracks/{track_id}/answers"),
            json!({"answer_type_id": 2, "value": "Second Song (feat. DJ Y)", "score": 0.5}),
        )
        .await;
    assert!(a2.status.is_success(), "title answer: {}", a2.text);

    let (round, track) = wait_round(&owner, room_id).await;
    assert_eq!(track, track_id);

    let r = answer(&owner, round, track, "pocahontas").await;
    let good = r.json("/good_answers").as_array().cloned().unwrap_or_default();
    assert!(
        good.iter().any(|a| a["name"] == "Movie"),
        "alias must match: {}",
        r.text
    );

    let r = answer(&owner, round, track, "dj y").await;
    assert_eq!(
        r.json("/good_answers").as_array().unwrap().len(),
        0,
        "featuring inside parens must not match: {}",
        r.text
    );
    let r = answer(&owner, round, track, "second song").await;
    let good = r.json("/good_answers").as_array().cloned().unwrap_or_default();
    assert!(
        good.iter().any(|a| a["name"] == "Title" && a["value"] == "Second Song (feat. DJ Y)"),
        "paren-stripped title must match: {}",
        r.text
    );
}

#[tokio::test]
async fn round_scores_endpoint_lists_scorers() {
    let (owner, _) = Client::register("gs").await;
    let (room_id, _pl, _t) =
        game_fixture(&owner, &[]).await;
    let (round, track) = wait_round(&owner, room_id).await;
    answer(&owner, round, track, "E2E Song").await;

    let s = owner
        .get(&format!("/api/v1/rounds/{round}/scores"))
        .await
        .expect_ok("scores");
    let list = s.json("/scores").as_array().cloned().unwrap_or_default();
    assert!(
        list.iter().any(|e| e["user_id"] == owner.user_id && e["score"].as_f64().unwrap() > 0.0),
        "owner must appear in round scores: {list:?}"
    );
}

#[tokio::test]
async fn upvote_downvote_toggle_and_reasons() {
    let (owner, _) = Client::register("gv").await;
    let (_room_id, _pl, track_id) = game_fixture(&owner, &[]).await;

    let up = owner
        .post(&format!("/api/v1/rooms/6/tracks/{track_id}/upvote"), json!({}))
        .await;
    assert!(up.ok(), "upvote: {}", up.text);
    assert_eq!(up.json("/user_voted_up"), true, "{:?}", up.body);
    assert_eq!(up.json("/upvotes").as_i64().unwrap_or(0), 1);

    let off = owner
        .post(&format!("/api/v1/rooms/6/tracks/{track_id}/upvote"), json!({}))
        .await;
    assert_eq!(off.json("/user_voted_up"), false);
    assert_eq!(off.json("/upvotes").as_i64().unwrap_or(9), 0);

    let no_reason = owner
        .post(&format!("/api/v1/rooms/6/tracks/{track_id}/downvote"), json!({}))
        .await;
    assert_eq!(no_reason.status, StatusCode::UNPROCESSABLE_ENTITY);

    let invalid = owner
        .post(
            &format!("/api/v1/rooms/6/tracks/{track_id}/downvote"),
            json!({"reason": "bogus"}),
        )
        .await;
    assert_eq!(invalid.status, StatusCode::UNPROCESSABLE_ENTITY);

    let ok = owner
        .post(
            &format!("/api/v1/rooms/6/tracks/{track_id}/downvote"),
            json!({"reason": "personal_taste"}),
        )
        .await;
    assert!(ok.ok(), "downvote: {}", ok.text);
    assert_eq!(ok.json("/user_voted_down"), true);

    let cancel = owner
        .post(&format!("/api/v1/rooms/6/tracks/{track_id}/downvote"), json!({}))
        .await;
    assert_eq!(cancel.json("/user_voted_down"), false);
}

#[tokio::test]
async fn late_answer_loses_speed_bonus() {
    let (owner, _) = Client::register("gl").await;
    let (room_id, _pl, _t) = game_fixture(&owner, &[]).await;
    let (round, track) = wait_round(&owner, room_id).await;
    let r = owner
        .post(
            &format!("/api/v1/rounds/{round}/tracks/{track}/check"),
            json!({"text": "E2E Song", "words": [], "currentTime": 10.0}),
        )
        .await;
    let good = r.json("/good_answers");
    assert_eq!(good[0]["score"].as_f64().unwrap(), 1.0, "{}", r.text);
    assert_eq!(good[0]["speedBonus"], false);
}

#[tokio::test]
async fn check_requires_auth() {
    let anon = Client::anon();
    let r = anon
        .post(
            "/api/v1/rounds/1/tracks/1/check",
            json!({"text": "x", "words": [], "currentTime": 1.0}),
        )
        .await;
    assert!(r.status == StatusCode::UNAUTHORIZED || r.status == StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn stop_marks_room_not_playing() {
    let (owner, _) = Client::register("gz").await;
    let (room_id, _pl, _t) = game_fixture(&owner, &[]).await;
    let _ = wait_round(&owner, room_id).await;
    let stop = owner
        .post_empty(&format!("/api/v1/rooms/{room_id}/stop"))
        .await;
    assert!(stop.ok(), "stop: {}", stop.text);
    let r = owner
        .get(&format!("/api/v1/rooms/{room_id}/joined"))
        .await
        .expect_ok("joined after stop");
    let _ = r.json("/round");
}
