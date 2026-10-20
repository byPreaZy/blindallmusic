//! Black-box coverage of the online matchmaking flow: queue → match →
//! live answers → scoring → guards. Each test pins ONE game type so the
//! process-wide in-memory queues don't cross-match between parallel tests.

mod common;
use common::Client;
use reqwest::StatusCode;
use serde_json::{Value, json};

async fn join(c: &Client, ty: &str) -> Value {
    c.post("/api/v1/matchmaking/join", json!({"game_type": ty}))
        .await
        .expect_ok("join")
        .body
}

/// Poll the match snapshot until `phase` (or timeout).
async fn wait_phase(c: &Client, match_id: i64, phase: &str, tries: usize) -> Value {
    for _ in 0..tries {
        let m = c
            .get(&format!("/api/v1/matches/{match_id}"))
            .await
            .expect_ok("match show");
        if m.json("/phase").as_str() == Some(phase) {
            return m.body;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    panic!("match {match_id} never reached phase {phase}");
}

/// Quiz oracle: ask the solo check endpoint which of the 4 choices is
/// correct, so the test can answer the *shared* match question correctly.
async fn quiz_oracle(c: &Client, q: &Value) -> String {
    let track_id = q["question"]["track"]["id"].as_i64().expect("track id");
    for choice in q["question"]["choices"].as_array().expect("choices") {
        let v = choice.as_str().unwrap();
        let r = c
            .post(
                "/api/v1/minigames/quiz/check",
                json!({"track_id": track_id, "chosen_value": v}),
            )
            .await
            .expect_ok("oracle check");
        if r.json("/correct").as_bool() == Some(true) {
            return v.to_string();
        }
    }
    panic!("no correct choice among {:?}", q);
}

#[tokio::test]
async fn match_queue_forms_match_answers_and_scores() {
    let (p1, _) = Client::register("mma").await;
    let (p2, _) = Client::register("mmb").await;
    let (outsider, _) = Client::register("mmo").await;

    // p1 waits in the quiz queue, p2 completes it → both get the same match.
    let j1 = join(&p1, "quiz").await;
    assert_eq!(j1["status"], "queued", "{j1}");
    let j2 = join(&p2, "quiz").await;
    assert_eq!(j2["status"], "matched", "{j2}");
    let match_id = j2["match_id"].as_i64().expect("match id");

    let s1 = p1
        .get("/api/v1/matchmaking/status")
        .await
        .expect_ok("status");
    assert_eq!(s1.json("/status"), "matched", "{}", s1.text);
    assert_eq!(s1.json("/match_id"), match_id);

    // Outsider can neither view nor answer the match.
    let r = outsider.get(&format!("/api/v1/matches/{match_id}")).await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.text);
    let r = outsider
        .post(
            &format!("/api/v1/matches/{match_id}/answer"),
            json!({"idx": 0, "value": "x"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.text);

    // Wait for the first live question (4s countdown), both players see
    // the same stripped question.
    let m1 = wait_phase(&p1, match_id, "question", 20).await;
    let m2 = p2
        .get(&format!("/api/v1/matches/{match_id}"))
        .await
        .expect_ok("p2 show");
    assert_eq!(m2.json("/phase"), "question");
    assert_eq!(m1["players"].as_array().unwrap().len(), 2);
    assert!(m1["question"]["question"].get("correct_value").is_none());
    assert_eq!(
        m1["question"]["question"]["track"]["id"], m2.body["question"]["question"]["track"]["id"],
        "players must share the same track"
    );
    let idx = m1["idx"].as_i64().unwrap();

    // p1 answers wrong → 200 correct:false, no double answer.
    let wrong = p1
        .post(
            &format!("/api/v1/matches/{match_id}/answer"),
            json!({"idx": idx, "value": "zzz wrong zzz"}),
        )
        .await
        .expect_ok("p1 answer");
    assert_eq!(wrong.json("/correct"), false);
    let again = p1
        .post(
            &format!("/api/v1/matches/{match_id}/answer"),
            json!({"idx": idx, "value": "x"}),
        )
        .await;
    assert_eq!(again.status, StatusCode::UNPROCESSABLE_ENTITY);
    let bad_idx = p1
        .post(
            &format!("/api/v1/matches/{match_id}/answer"),
            json!({"idx": idx + 9, "value": "x"}),
        )
        .await;
    assert_eq!(bad_idx.status, StatusCode::UNPROCESSABLE_ENTITY);

    // p2 answers correctly via the oracle → score broadcast in snapshot.
    let good = quiz_oracle(&p2, &m1["question"]).await;
    let ok = p2
        .post(
            &format!("/api/v1/matches/{match_id}/answer"),
            json!({"idx": idx, "value": good}),
        )
        .await
        .expect_ok("p2 answer");
    assert_eq!(ok.json("/correct"), true);

    let snap = p2
        .get(&format!("/api/v1/matches/{match_id}"))
        .await
        .expect_ok("snapshot");
    let p2_row = snap.body["scores"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["user_id"].as_i64() == Some(p2.user_id))
        .expect("p2 row");
    assert_eq!(p2_row["score"], 1, "{}", snap.text);
    let p1_row = snap.body["scores"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["user_id"].as_i64() == Some(p1.user_id))
        .expect("p1 row");
    assert_eq!(p1_row["score"], 0);

    // Match point counts toward the personal total (same rule as duels).
    let idx_games = p2.get("/api/v1/minigames").await.expect_ok("index");
    let quiz_total = idx_games.body["games"]
        .as_array()
        .unwrap()
        .iter()
        .find(|g| g["type"] == "quiz")
        .and_then(|g| g["score"].as_i64())
        .unwrap_or(0);
    assert!(
        quiz_total >= 2,
        "oracle solo point + match point: {quiz_total}"
    );

    // A live match blocks re-queuing (join routes back to it instead).
    let re = p1
        .post("/api/v1/matchmaking/join", json!({"game_type": "quiz"}))
        .await
        .expect_ok("rejoin");
    assert_eq!(re.json("/status"), "matched");
    assert_eq!(re.json("/match_id"), match_id);
}

#[tokio::test]
async fn match_leave_clears_queue() {
    let (u, _) = Client::register("mml").await;
    let j = join(&u, "who_sang").await;
    assert_eq!(j["status"], "queued", "{j}");
    u.post("/api/v1/matchmaking/leave", json!({}))
        .await
        .expect_ok("leave");
    let s = u
        .get("/api/v1/matchmaking/status")
        .await
        .expect_ok("status");
    assert_eq!(s.json("/status"), "idle", "{}", s.text);
}

#[tokio::test]
async fn match_no_self_match_rejoin_is_idempotent() {
    let (u, _) = Client::register("mms").await;
    for _ in 0..2 {
        let j = join(&u, "anagram").await;
        assert_eq!(j["status"], "queued", "{j}");
    }
    let s = u
        .get("/api/v1/matchmaking/status")
        .await
        .expect_ok("status");
    assert_eq!(s.json("/status"), "queued");
    u.post("/api/v1/matchmaking/leave", json!({}))
        .await
        .expect_ok("leave");
}

#[tokio::test]
async fn match_guards() {
    // Guests and anonymous visitors can't queue (playing is members-only).
    let guest = Client::guest().await;
    let r = guest
        .post(
            "/api/v1/matchmaking/join",
            json!({"game_type": "first_letter"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.text);
    let anon = Client::anon();
    let r = anon
        .post(
            "/api/v1/matchmaking/join",
            json!({"game_type": "first_letter"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNAUTHORIZED, "{}", r.text);

    let (u, _) = Client::register("mmg").await;
    let r = u
        .post(
            "/api/v1/matchmaking/join",
            json!({"game_type": "not_a_game"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::UNPROCESSABLE_ENTITY, "{}", r.text);
    let r = u.get("/api/v1/matches/99999999").await;
    assert_eq!(r.status, StatusCode::FORBIDDEN, "{}", r.text);
}

/// A real full match: countdown → 10 questions → reveal → finished, all
/// driven server-side. ~3.5 min wall clock — proves the whole cycle.
#[tokio::test]
async fn match_completes_full_cycle() {
    let (p1, _) = Client::register("mmc").await;
    let (p2, _) = Client::register("mmd").await;

    join(&p1, "album_cover").await;
    let j2 = join(&p2, "album_cover").await;
    assert_eq!(j2["status"], "matched", "{j2}");
    let match_id = j2["match_id"].as_i64().unwrap();

    // Wait for finish (10×19s + countdown ≈ 200s, allow slack for gen).
    let mut last = Value::Null;
    let mut finished = None;
    for _ in 0..110 {
        let m = p1
            .get(&format!("/api/v1/matches/{match_id}"))
            .await
            .expect_ok("poll");
        last = m.body.clone();
        if m.json("/phase").as_str() == Some("finished") {
            finished = Some(m.body);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
    let fin = finished.unwrap_or_else(|| panic!("match never finished: {last}"));
    assert_eq!(fin["total"], 10, "{fin}");
    assert_eq!(fin["scores"].as_array().unwrap().len(), 2);

    // Both players are free to queue again — finished matches don't
    // trap them (status must be idle/queued, not the stale match).
    let s = p2
        .get("/api/v1/matchmaking/status")
        .await
        .expect_ok("status");
    assert_ne!(s.json("/match_id"), match_id, "{}", s.text);
    let st = s.json("/status").as_str().unwrap_or_default().to_string();
    assert!(["idle", "queued", "matched"].contains(&st.as_str()));
}
