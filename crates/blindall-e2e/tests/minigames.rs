mod common;
use common::{Client, game_fixture};
use reqwest::StatusCode;
use serde_json::json;

// ---- solo flow --------------------------------------------------------------

#[tokio::test]
async fn solo_quiz_next_and_check() {
    let (u, _) = Client::register("mgq").await;
    let q = u
        .post_empty("/api/v1/minigames/quiz/next")
        .await
        .expect_ok("next");
    let track_id = q.json("/track/id").as_i64().expect("track id");
    let correct = q.json("/correct_value").as_str().unwrap().to_string();
    assert!(q.json("/choices").as_array().map(|c| c.len()).unwrap_or(0) >= 2);

    let good = u
        .post(
            "/api/v1/minigames/quiz/check",
            json!({"track_id": track_id, "chosen_value": correct}),
        )
        .await
        .expect_ok("check correct");
    assert_eq!(good.json("/correct"), true);
    assert_eq!(good.json("/points"), 1);

    let bad = u
        .post(
            "/api/v1/minigames/quiz/check",
            json!({"track_id": track_id, "chosen_value": "zzz wrong zzz"}),
        )
        .await
        .expect_ok("check wrong");
    assert_eq!(bad.json("/correct"), false);
    assert_eq!(bad.json("/points"), 0);
}

// ---- duels ------------------------------------------------------------------

fn duel_track_ids(d: &serde_json::Value) -> Vec<i64> {
    d["questions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| q["question"]["track"]["id"].as_i64().unwrap_or(-1))
        .collect()
}

/// Play a full 3-question duel: p1 answers wrong, p2 sweeps with the revealed
/// correct_value → p2 wins 3-0.
async fn duel_p2_sweeps(p1: &Client, p2: &Client) -> i64 {
    let created = p1
        .post(
            "/api/v1/duels",
            json!({"game_type": "quiz", "opponent_id": p2.user_id, "count": 3}),
        )
        .await
        .expect_ok("create duel");
    let id = created.json("/id").as_i64().unwrap();
    p2.post_empty(&format!("/api/v1/duels/{id}/accept"))
        .await
        .expect_ok("accept");
    for i in 0..3 {
        let r1 = p1
            .post(
                &format!("/api/v1/duels/{id}/answer"),
                json!({"idx": i, "value": "zzz wrong zzz"}),
            )
            .await
            .expect_ok("p1 answer");
        let cv = r1.json("/correct_value").as_str().unwrap().to_string();
        p2.post(
            &format!("/api/v1/duels/{id}/answer"),
            json!({"idx": i, "value": cv}),
        )
        .await
        .expect_ok("p2 answer");
    }
    let fin = p2
        .get(&format!("/api/v1/duels/{id}"))
        .await
        .expect_ok("final");
    assert_eq!(fin.json("/status"), "finished");
    assert_eq!(fin.json("/creator_score"), 0);
    assert_eq!(fin.json("/opponent_score"), 3);
    id
}

#[tokio::test]
async fn duel_lifecycle_frozen_questions_and_result() {
    let (p1, _) = Client::register("dla").await;
    let (p2, _) = Client::register("dlb").await;

    let created = p1
        .post(
            "/api/v1/duels",
            json!({"game_type": "quiz", "opponent_id": p2.user_id, "count": 3}),
        )
        .await
        .expect_ok("create duel");
    let id = created.json("/id").as_i64().unwrap();

    // opponent accepts → active
    let acc = p2
        .post_empty(&format!("/api/v1/duels/{id}/accept"))
        .await
        .expect_ok("accept");
    assert_eq!(acc.json("/accepted"), true);

    // frozen sequence: both players see the same track order
    let v1 = p1
        .get(&format!("/api/v1/duels/{id}"))
        .await
        .expect_ok("show p1");
    let v2 = p2
        .get(&format!("/api/v1/duels/{id}"))
        .await
        .expect_ok("show p2");
    assert_eq!(duel_track_ids(&v1.body), duel_track_ids(&v2.body));
    // correct_value must NOT leak to clients
    assert!(
        v1.body["questions"][0]["question"]
            .get("correct_value")
            .is_none()
    );

    // p1 answers wrong everywhere; p2 reuses the correct_value revealed by
    // p1's response (same frozen idx → same answer) → p2 sweeps 3-0.
    for i in 0..3 {
        let r1 = p1
            .post(
                &format!("/api/v1/duels/{id}/answer"),
                json!({"idx": i, "value": "zzz wrong zzz"}),
            )
            .await
            .expect_ok("p1 answer");
        assert_eq!(r1.json("/correct"), false);
        let cv = r1.json("/correct_value").as_str().unwrap().to_string();
        let r2 = p2
            .post(
                &format!("/api/v1/duels/{id}/answer"),
                json!({"idx": i, "value": cv}),
            )
            .await
            .expect_ok("p2 answer");
        assert_eq!(r2.json("/correct"), true);
    }

    let fin = p2
        .get(&format!("/api/v1/duels/{id}"))
        .await
        .expect_ok("final");
    assert_eq!(fin.json("/status"), "finished");
    assert_eq!(fin.json("/creator_score"), 0);
    assert_eq!(fin.json("/opponent_score"), 3);

    // p2 appears on the wins leaderboard — the route serves a fixed top-N
    // window and the test DB persists across runs, so p2 must strictly out-win
    // the current board maximum (1 win already secured above).
    let lb = p1
        .get("/api/v1/duels/leaderboard")
        .await
        .expect_ok("leaderboard");
    let max_wins = lb
        .json("/leaderboard")
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|r| r["wins"].as_i64())
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);
    for _ in 0..max_wins {
        duel_p2_sweeps(&p1, &p2).await;
    }
    let lb = p1
        .get("/api/v1/duels/leaderboard")
        .await
        .expect_ok("leaderboard");
    let names: Vec<i64> = lb
        .json("/leaderboard")
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_i64().unwrap())
        .collect();
    assert!(
        names.contains(&p2.user_id),
        "winner on leaderboard: {}",
        lb.text
    );

    // p1 got the finish notification
    let n = p1
        .get("/api/v1/users/notifications")
        .await
        .expect_ok("notifications");
    assert!(
        n.text.contains("DuelFinished") || n.text.contains("Duel"),
        "finish notification present: {}",
        n.text
    );
}

#[tokio::test]
async fn duel_authorization_and_replay_guards() {
    let (p1, _) = Client::register("dga").await;
    let (p2, _) = Client::register("dgb").await;
    let (p3, _) = Client::register("dgc").await;

    let id = p1
        .post(
            "/api/v1/duels",
            json!({"game_type": "quiz", "opponent_id": p2.user_id, "count": 3}),
        )
        .await
        .expect_ok("create")
        .json("/id")
        .as_i64()
        .unwrap();

    // outsider cannot view or answer
    let r = p3.get(&format!("/api/v1/duels/{id}")).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    let r = p3
        .post(
            &format!("/api/v1/duels/{id}/answer"),
            json!({"idx": 0, "value": "x"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    // outsider cannot accept a targeted duel
    let r = p3.post_empty(&format!("/api/v1/duels/{id}/accept")).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);
    // creator cannot accept their own duel
    let r = p1.post_empty(&format!("/api/v1/duels/{id}/accept")).await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);

    p2.post_empty(&format!("/api/v1/duels/{id}/accept"))
        .await
        .expect_ok("accept");

    // replaying the same index is rejected
    p1.post(
        &format!("/api/v1/duels/{id}/answer"),
        json!({"idx": 0, "value": "x"}),
    )
    .await
    .expect_ok("first answer");
    let r = p1
        .post(
            &format!("/api/v1/duels/{id}/answer"),
            json!({"idx": 0, "value": "y"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text);
}

#[tokio::test]
async fn duel_open_challenge_accepted_by_anyone() {
    let (p1, _) = Client::register("doa").await;
    let (p2, _) = Client::register("dob").await;

    let id = p1
        .post(
            "/api/v1/duels",
            json!({"game_type": "who_sang", "count": 3}),
        )
        .await
        .expect_ok("open duel")
        .json("/id")
        .as_i64()
        .unwrap();

    // visible in p2's lobby and acceptable
    let list = p2.get("/api/v1/duels").await.expect_ok("list");
    assert!(list.text.contains(&format!("\"id\":{id}")));
    p2.post_empty(&format!("/api/v1/duels/{id}/accept"))
        .await
        .expect_ok("accept open");
    let d = p2
        .get(&format!("/api/v1/duels/{id}"))
        .await
        .expect_ok("show");
    assert_eq!(d.json("/status"), "active");
    assert_eq!(d.json("/opponent_id"), p2.user_id);
}

// ---- daily challenge ----------------------------------------------------------

#[tokio::test]
async fn daily_challenge_shared_questions_and_leaderboard() {
    let (p1, _) = Client::register("dya").await;
    let (p2, _) = Client::register("dyb").await;

    let d1 = p1
        .get("/api/v1/daily-challenge?type=quiz")
        .await
        .expect_ok("daily p1");
    let d2 = p2
        .get("/api/v1/daily-challenge?type=quiz")
        .await
        .expect_ok("daily p2");

    // same day, same frozen questions
    assert_eq!(d1.json("/date"), d2.json("/date"));
    assert_eq!(duel_track_ids(&d1.body), duel_track_ids(&d2.body));

    // p1 answers idx 0 wrong → reveals correct_value → p2 answers it right
    let r1 = p1
        .post(
            "/api/v1/daily-challenge/answer",
            json!({"game_type": "quiz", "idx": 0, "value": "zzz wrong zzz"}),
        )
        .await
        .expect_ok("p1 answer");
    assert_eq!(r1.json("/correct"), false);
    let cv = r1.json("/correct_value").as_str().unwrap().to_string();

    let r2 = p2
        .post(
            "/api/v1/daily-challenge/answer",
            json!({"game_type": "quiz", "idx": 0, "value": cv}),
        )
        .await
        .expect_ok("p2 answer");
    assert_eq!(r2.json("/correct"), true);
    assert_eq!(r2.json("/score"), 1);

    // replay guard
    let r = p2
        .post(
            "/api/v1/daily-challenge/answer",
            json!({"game_type": "quiz", "idx": 0, "value": "x"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);

    // p1 reveals every remaining idx, p2 sweeps → p2 hits the maximum
    // score for the day, which guarantees a top-20 leaderboard slot no
    // matter how many players the persistent test DB already has today.
    for idx in 1..10 {
        let reveal = p1
            .post(
                "/api/v1/daily-challenge/answer",
                json!({"game_type": "quiz", "idx": idx, "value": "zzz wrong zzz"}),
            )
            .await
            .expect_ok("p1 reveal");
        assert_eq!(reveal.json("/correct"), false);
        let cv = reveal.json("/correct_value").as_str().unwrap().to_string();
        let ok = p2
            .post(
                "/api/v1/daily-challenge/answer",
                json!({"game_type": "quiz", "idx": idx, "value": cv}),
            )
            .await
            .expect_ok("p2 sweep");
        assert_eq!(ok.json("/correct"), true);
    }

    // p1's own progress came back in the show payload (score 0, all 10
    // answered) — its row exists even though a 0-score entry can
    // legitimately fall outside the top-20 on a populated day.
    let lb = p1
        .get("/api/v1/daily-challenge?type=quiz")
        .await
        .expect_ok("daily lb");
    assert_eq!(lb.json("/score"), 0);
    assert_eq!(lb.json("/answered"), 10);
    // p2 scored the daily maximum → must be on the board.
    let lbtext = lb.body["leaderboard"].to_string();
    assert!(
        lbtext.contains(&p2.user_id.to_string()),
        "top scorer missing from daily leaderboard: {lbtext}"
    );
}

// ---- room party (realtime quiz) ------------------------------------------------

#[tokio::test]
async fn party_lifecycle_and_guards() {
    let (owner, _) = Client::register("pty").await;
    let (room_id, _pl, _t) = game_fixture(&owner, &[]).await;
    let (guest, _) = Client::register("ptg").await;

    // not joined → no party access
    let r = guest.get(&format!("/api/v1/rooms/{room_id}/party")).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);

    // Participation is granted by viewing the room, required by /joined.
    for c in [&owner, &guest] {
        c.get(&format!("/api/v1/rooms/{room_id}"))
            .await
            .expect_ok("visit");
        c.get(&format!("/api/v1/rooms/{room_id}/joined"))
            .await
            .expect_ok("join");
    }

    // guest is a participant but not a host → cannot start
    let r = guest
        .post(
            &format!("/api/v1/rooms/{room_id}/party/start"),
            json!({"game_type": "quiz", "count": 3}),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.text);

    // no party yet → show 404
    let r = guest.get(&format!("/api/v1/rooms/{room_id}/party")).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);

    // owner starts
    owner
        .post(
            &format!("/api/v1/rooms/{room_id}/party/start"),
            json!({"game_type": "quiz", "count": 3}),
        )
        .await
        .expect_ok("start");

    // second start → conflict
    let r = owner
        .post(
            &format!("/api/v1/rooms/{room_id}/party/start"),
            json!({"game_type": "quiz"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY);

    // snapshot: question phase, questions stripped of correct_value
    let s = guest
        .get(&format!("/api/v1/rooms/{room_id}/party"))
        .await
        .expect_ok("show");
    assert_eq!(s.json("/phase"), "question", "{}", s.text);
    assert_eq!(s.json("/total"), 3);
    assert!(
        s.body["question"]["question"]
            .get("correct_value")
            .is_none()
    );
    let idx = s.json("/idx").as_i64().unwrap();

    // guest answers (wrong is fine — determinism not needed for guards)
    let a = guest
        .post(
            &format!("/api/v1/rooms/{room_id}/party/answer"),
            json!({"idx": idx, "value": "zzz wrong zzz"}),
        )
        .await
        .expect_ok("answer");
    assert_eq!(a.json("/correct"), false);

    // double answer → rejected
    let a2 = guest
        .post(
            &format!("/api/v1/rooms/{room_id}/party/answer"),
            json!({"idx": idx, "value": "x"}),
        )
        .await;
    assert_eq!(a2.status, StatusCode::UNPROCESSABLE_ENTITY);

    // guest cannot stop; owner stops → party removed
    let r = guest
        .post(&format!("/api/v1/rooms/{room_id}/party/stop"), json!({}))
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN);
    owner
        .post(&format!("/api/v1/rooms/{room_id}/party/stop"), json!({}))
        .await
        .expect_ok("stop");
    let r = guest.get(&format!("/api/v1/rooms/{room_id}/party")).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

/// Every minigame type is a valid realtime party mode: the host picks the
/// type at start and all participants get that game's questions.
#[tokio::test]
async fn party_supports_all_game_types() {
    let (owner, _) = Client::register("pty5").await;
    let (room_id, _pl, _t) = game_fixture(&owner, &[]).await;
    owner
        .get(&format!("/api/v1/rooms/{room_id}"))
        .await
        .expect_ok("visit");
    owner
        .get(&format!("/api/v1/rooms/{room_id}/joined"))
        .await
        .expect_ok("join");

    for ty in ["quiz", "who_sang", "album_cover", "first_letter", "anagram"] {
        owner
            .post(
                &format!("/api/v1/rooms/{room_id}/party/start"),
                json!({"game_type": ty, "count": 3}),
            )
            .await
            .expect_ok(&format!("start {ty}"));
        let s = owner
            .get(&format!("/api/v1/rooms/{room_id}/party"))
            .await
            .expect_ok("show");
        assert_eq!(s.json("/game_type"), ty, "{ty}");
        owner
            .post(&format!("/api/v1/rooms/{room_id}/party/stop"), json!({}))
            .await
            .expect_ok("stop");
    }
}

// ---- media validity & distractor quality (regressions) ------------------------

/// Regression for "question N has no music" + "album cover shows no cover":
/// every frozen question must ship a playable http(s) audio URL, and
/// album_cover questions a usable artwork URL. A pick whose provider
/// resolution fails must be skipped, never served.
#[tokio::test]
async fn duel_questions_always_have_media() {
    let (p1, _) = Client::register("mda").await;
    for game_type in ["quiz", "who_sang", "album_cover", "first_letter"] {
        let created = p1
            .post("/api/v1/duels", json!({"game_type": game_type, "count": 5}))
            .await
            .expect_ok("create duel");
        let id = created.json("/id").as_i64().unwrap();
        let d = p1
            .get(&format!("/api/v1/duels/{id}"))
            .await
            .expect_ok("show");
        let questions = d.body["questions"].as_array().unwrap();
        assert_eq!(questions.len(), 5, "{game_type}: {}", d.text);
        for (i, q) in questions.iter().enumerate() {
            let track = &q["question"]["track"];
            let audio = track["preview_url"].as_str().unwrap_or("");
            assert!(
                audio.starts_with("http"),
                "{game_type} q{i}: preview_url missing/invalid: {audio:?}"
            );
            if game_type == "album_cover" {
                let art = track["artwork_url"].as_str().unwrap_or("");
                assert!(
                    art.starts_with("http") && !art.contains("/cover//"),
                    "album_cover q{i}: artwork_url missing/invalid: {art:?}"
                );
            }
        }
    }
}

/// Solo /next must also never serve a question without playable audio.
#[tokio::test]
async fn solo_next_always_has_audio() {
    let (u, _) = Client::register("mna").await;
    for game_type in ["quiz", "who_sang", "album_cover", "first_letter"] {
        let q = u
            .post_empty(&format!("/api/v1/minigames/{game_type}/next"))
            .await
            .expect_ok("next");
        let audio = q.body["track"]["preview_url"].as_str().unwrap_or("");
        assert!(
            audio.starts_with("http"),
            "{game_type}: preview_url missing/invalid: {audio:?}"
        );
        if game_type == "album_cover" {
            let art = q.body["track"]["artwork_url"].as_str().unwrap_or("");
            assert!(
                art.starts_with("http") && !art.contains("/cover//"),
                "album_cover: artwork_url missing/invalid: {art:?}"
            );
        }
    }
}

/// Regression for "French distractors on an English title": when the
/// correct title has a detectable language, every distractor must share
/// it. Draws with undetectable language are skipped (playlist coherence
/// covers them).
#[tokio::test]
async fn quiz_distractors_match_correct_language() {
    use blindall_core::lang::{Lang, compatible, detect};
    let (u, _) = Client::register("mql").await;
    let mut sampled = 0;
    for _ in 0..40 {
        if sampled >= 8 {
            break;
        }
        let q = u
            .post_empty("/api/v1/minigames/quiz/next")
            .await
            .expect_ok("next");
        let correct = q.body["correct_value"].as_str().unwrap_or("");
        let lang = detect(correct);
        if lang == Lang::Other {
            continue;
        }
        sampled += 1;
        let choices = q.body["choices"].as_array().unwrap();
        assert_eq!(choices.len(), 4);
        for c in choices {
            let v = c.as_str().unwrap_or("");
            if v == correct {
                continue;
            }
            assert!(
                compatible(lang, v),
                "distractor {v:?} is not {lang:?} (correct: {correct:?})"
            );
        }
    }
    assert!(
        sampled >= 3,
        "too few language-detectable draws ({sampled})"
    );
}

/// Mirror of `norm_choice` in the repo: strip bracketed segments, slug.
fn norm_choice(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut depth = 0usize;
    for c in s.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    blindall_core::sanitize::str_slug(&out, ' ')
}

/// Choices must be 4 distinct, normalized-different values — no
/// "Imagine"/"Imagine (Remastered)" ambiguity.
#[tokio::test]
async fn quiz_choices_are_distinct() {
    let (u, _) = Client::register("mqd").await;
    for _ in 0..6 {
        let q = u
            .post_empty("/api/v1/minigames/quiz/next")
            .await
            .expect_ok("next");
        let choices: Vec<String> = q.body["choices"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|c| c.as_str().map(norm_choice))
            .collect();
        let mut dedup = choices.clone();
        dedup.sort();
        dedup.dedup();
        assert_eq!(
            dedup.len(),
            4,
            "duplicate/near-duplicate choices: {:?} (raw: {})",
            choices,
            q.text
        );
    }
}
