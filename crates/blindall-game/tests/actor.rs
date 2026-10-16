use blindall_core::scoring::CheckOutcome;
use blindall_game::actor::{RoomActor, RoomCommand, RoomSnapshot};
use blindall_game::events;
use blindall_game::store::{RoundRow, ScoreStore};
use blindall_game::testkit::{fake_room, fake_track, FakeGameStore, FakePresenceStore, FakeScoreStore};
use blindall_realtime::{Hub, ServerOp};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{advance, timeout, Instant};

struct Fixture {
    tx: mpsc::Sender<RoomCommand>,
    store: Arc<FakeGameStore>,
    scores: Arc<FakeScoreStore>,
    presence: Arc<FakePresenceStore>,
    _hub: Hub,
    rx: mpsc::Receiver<ServerOp>,
}

async fn spawn_fixture() -> Fixture {
    let store = Arc::new(FakeGameStore::new(fake_room(1)));
    {
        let mut tracks = store.tracks.lock().unwrap();
        tracks.insert(1, fake_track(1, "deezer"));
        tracks.insert(2, fake_track(2, "deezer"));
        *store.picked.lock().unwrap() = vec![1, 2];
    }
    let scores = Arc::new(FakeScoreStore::default());
    let presence = Arc::new(FakePresenceStore::default());
    presence.counts.lock().unwrap().insert(1, 2);
    let hub = Hub::new();
    let (mut conn, rx) = hub.connect(100);
    hub.subscribe(&mut conn, &events::room_channel(1), Some(json!({"user_id": 100, "name": "observer"})))
        .await;
    std::mem::forget(conn); // keep the socket alive for the test duration

    let tx = RoomActor::spawn(
        fake_room(1),
        store.clone(),
        scores.clone(),
        presence.clone(),
        hub.clone(),
        "/storage".to_string(),
    );
    Fixture {
        tx,
        store,
        scores,
        presence,
        _hub: hub,
        rx,
    }
}

async fn next_event(rx: &mut mpsc::Receiver<ServerOp>, name: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match timeout(Duration::from_secs(30), rx.recv()).await {
            Ok(Some(ServerOp::Event { event, data, .. })) if event == name => return data,
            Ok(Some(_)) => continue,
            Ok(None) => panic!("channel closed while waiting for {name}"),
            Err(_) if Instant::now() >= deadline => panic!("timeout waiting for {name}"),
            Err(_) => continue,
        }
    }
}

async fn drain_events(rx: &mut mpsc::Receiver<ServerOp>) -> Vec<String> {
    tokio::task::yield_now().await;
    let mut names = vec![];
    while let Ok(op) = rx.try_recv() {
        if let ServerOp::Event { event, .. } = op {
            names.push(event);
        }
    }
    names
}

async fn snapshot(tx: &mpsc::Sender<RoomCommand>) -> RoomSnapshot {
    let (reply, rx) = oneshot::channel();
    tx.send(RoomCommand::Snapshot { reply }).await.unwrap();
    timeout(Duration::from_secs(30), rx).await.unwrap().unwrap()
}

async fn start(fx: &Fixture) {
    fx.tx
        .send(RoomCommand::Start { user_id: Some(9) })
        .await
        .unwrap();
}

#[tokio::test(start_paused = true)]
async fn start_picks_tracks_and_plays_first() {
    let mut fx = spawn_fixture().await;
    start(&fx).await;

    let started = next_event(&mut fx.rx, "RoundStarted").await;
    assert_eq!(started["round"]["tracks"], json!([1, 2]));

    let played = next_event(&mut fx.rx, "TrackPlayed").await;
    assert_eq!(played["track"]["id"], json!(1));
    assert_eq!(played["track"]["audio"], json!("https://cdn.test/1.mp3"));

    let snap = snapshot(&fx.tx).await;
    assert!(snap.is_playing);
    assert_eq!(snap.current_track.map(|t| t.id), Some(1));
    assert_eq!(snap.played_track_ids, Vec::<i64>::new());
    assert_eq!(fx.store.room_playing.lock().unwrap()[&1], true);
}

#[tokio::test(start_paused = true)]
async fn track_end_waits_pause_then_plays_next() {
    let mut fx = spawn_fixture().await;
    start(&fx).await;
    next_event(&mut fx.rx, "TrackPlayed").await;

    advance(Duration::from_secs(29)).await;
    assert!(!drain_events(&mut fx.rx).await.contains(&"TrackEnded".to_string()));

    advance(Duration::from_secs(2)).await; // t=31 > 30
    let ended = next_event(&mut fx.rx, "TrackEnded").await;
    assert_eq!(ended["next_track_id"], json!(2));

    advance(Duration::from_secs(4)).await;
    let played = next_event(&mut fx.rx, "TrackPlayed").await;
    assert_eq!(played["track"]["id"], json!(2));

    let snap = snapshot(&fx.tx).await;
    assert_eq!(snap.played_track_ids, vec![1]);
}

#[tokio::test(start_paused = true)]
async fn full_round_finalizes_and_autostarts() {
    let mut fx = spawn_fixture().await;
    start(&fx).await;
    next_event(&mut fx.rx, "TrackPlayed").await;

    advance(Duration::from_secs(31)).await;
    next_event(&mut fx.rx, "TrackEnded").await;
    advance(Duration::from_secs(4)).await;
    next_event(&mut fx.rx, "TrackPlayed").await;
    advance(Duration::from_secs(31)).await;
    next_event(&mut fx.rx, "TrackEnded").await;

    next_event(&mut fx.rx, "RoundFinalizing").await;
    advance(Duration::from_secs(11)).await;
    next_event(&mut fx.rx, "RoundStarted").await;
    let played = next_event(&mut fx.rx, "TrackPlayed").await;
    assert_eq!(played["track"]["id"], json!(1));
}

#[tokio::test(start_paused = true)]
async fn no_subscriber_means_no_autostart() {
    let store = Arc::new(FakeGameStore::new(fake_room(1)));
    {
        let mut tracks = store.tracks.lock().unwrap();
        tracks.insert(1, fake_track(1, "deezer"));
        *store.picked.lock().unwrap() = vec![1]; // single-track round
    }
    let scores = Arc::new(FakeScoreStore::default());
    let presence = Arc::new(FakePresenceStore::default());
    presence.counts.lock().unwrap().insert(1, 2);
    let hub = Hub::new();
    let tx = RoomActor::spawn(
        fake_room(1),
        store.clone(),
        scores,
        presence,
        hub.clone(),
        "/storage".to_string(),
    );

    let (mut conn, mut rx) = hub.connect(55);
    hub.subscribe(&mut conn, &events::room_channel(1), None).await;

    tx.send(RoomCommand::Start { user_id: None }).await.unwrap();
    next_event(&mut rx, "TrackPlayed").await;
    advance(Duration::from_secs(31)).await;
    next_event(&mut rx, "TrackEnded").await;
    next_event(&mut rx, "RoundFinalizing").await;

    hub.unsubscribe(&mut conn, &events::room_channel(1)).await;
    drop(conn);
    advance(Duration::from_secs(60)).await;
    tokio::task::yield_now().await;

    let snap = snapshot(&tx).await;
    assert!(!snap.is_playing);
    assert_ne!(fx_room_playing(&store), Some(true));
}

fn fx_room_playing(store: &FakeGameStore) -> Option<bool> {
    store.room_playing.lock().unwrap().get(&1).copied()
}

#[tokio::test(start_paused = true)]
async fn pause_freezes_and_resume_re_arms() {
    let mut fx = spawn_fixture().await;
    start(&fx).await;
    next_event(&mut fx.rx, "TrackPlayed").await;

    advance(Duration::from_secs(10)).await; // 20s remaining
    fx.tx.send(RoomCommand::Pause).await.unwrap();
    next_event(&mut fx.rx, "TrackPaused").await;

    advance(Duration::from_secs(60)).await;
    assert!(!drain_events(&mut fx.rx).await.contains(&"TrackEnded".to_string()));

    fx.tx.send(RoomCommand::Resume).await.unwrap();
    next_event(&mut fx.rx, "TrackResumed").await;

    advance(Duration::from_secs(19)).await;
    assert!(!drain_events(&mut fx.rx).await.contains(&"TrackEnded".to_string()));
    advance(Duration::from_secs(2)).await;
    next_event(&mut fx.rx, "TrackEnded").await;
}

#[tokio::test(start_paused = true)]
async fn check_answer_scores_and_broadcasts() {
    let mut fx = spawn_fixture().await;
    start(&fx).await;
    next_event(&mut fx.rx, "TrackPlayed").await;

    let (reply, out_rx) = oneshot::channel();
    fx.tx
        .send(RoomCommand::CheckAnswer {
            user_id: 5,
            track_id: 1,
            text: "song 1".into(),
            client_words: vec![],
            current_time: 2.0,
            reply,
        })
        .await
        .unwrap();
    let outcome = timeout(Duration::from_secs(30), out_rx).await.unwrap().unwrap();
    match outcome {
        CheckOutcome::Good { answers, found_all, total_score } => {
            assert_eq!(answers.len(), 1);
            assert_eq!(answers[0].answer_id, 10);
            assert!(found_all); // only answer of the track
            assert!(total_score > 0.0);
        }
        other => panic!("expected Good, got {other:?}"),
    }

    next_event(&mut fx.rx, "NewScore").await;
    next_event(&mut fx.rx, "UserHasFoundAllTheAnswers").await;
    assert!(fx.scores.user_score(1, 5).await > 0.0);

    let (reply2, out_rx2) = oneshot::channel();
    fx.tx
        .send(RoomCommand::CheckAnswer {
            user_id: 5,
            track_id: 1,
            text: "song 1".into(),
            client_words: vec![],
            current_time: 3.0,
            reply: reply2,
        })
        .await
        .unwrap();
    let dup = timeout(Duration::from_secs(30), out_rx2).await.unwrap().unwrap();
    assert!(matches!(dup, CheckOutcome::AlreadyFound | CheckOutcome::Bad));
}

#[tokio::test(start_paused = true)]
async fn stop_writes_standings_and_cleans_scores() {
    let mut fx = spawn_fixture().await;
    start(&fx).await;
    next_event(&mut fx.rx, "TrackPlayed").await;

    let (reply, out_rx) = oneshot::channel();
    fx.tx
        .send(RoomCommand::CheckAnswer {
            user_id: 5,
            track_id: 1,
            text: "song 1".into(),
            client_words: vec![],
            current_time: 2.0,
            reply,
        })
        .await
        .unwrap();
    let _ = timeout(Duration::from_secs(30), out_rx).await.unwrap();

    fx.tx.send(RoomCommand::Stop).await.unwrap();
    next_event(&mut fx.rx, "RoundFinished").await;

    for _ in 0..50 {
        tokio::task::yield_now().await;
        if !fx.store.standings_written.lock().unwrap().is_empty() {
            break;
        }
        advance(Duration::from_millis(10)).await;
    }
    let standings = fx.store.standings_written.lock().unwrap();
    assert!(standings.iter().any(|s| s.user_id == 5), "standings written");
    drop(standings);
    assert!(fx
        .store
        .total_scores
        .lock()
        .unwrap()
        .iter()
        .any(|(_, u, _)| *u == 5));
    assert!(fx.scores.cleaned.lock().unwrap().contains(&1));
}

#[tokio::test(start_paused = true)]
async fn empty_pick_aborts_like_php() {
    let mut fx = spawn_fixture().await;
    *fx.store.picked.lock().unwrap() = vec![];
    start(&fx).await;

    next_event(&mut fx.rx, "RoundFinished").await;
    let snap = snapshot(&fx.tx).await;
    assert!(!snap.is_playing);
    assert_eq!(fx.store.room_playing.lock().unwrap()[&1], false);
}

#[tokio::test(start_paused = true)]
async fn recover_resumes_at_next_boundary() {
    let mut fx = spawn_fixture().await;
    fx.store.insert_round(RoundRow {
        id: 7,
        room_id: 1,
        tracks: vec![1, 2],
        current: 1,
        is_playing: true,
        finished_at: None,
        current_track_started_at: Some(chrono::Utc::now()),
    });
    fx.tx
        .send(RoomCommand::Recover {
            round: RoundRow {
                id: 7,
                room_id: 1,
                tracks: vec![1, 2],
                current: 1,
                is_playing: true,
                finished_at: None,
                current_track_started_at: Some(chrono::Utc::now()),
            },
        })
        .await
        .unwrap();

    advance(Duration::from_secs(3)).await; // recovery delay is 2s
    let played = next_event(&mut fx.rx, "TrackPlayed").await;
    assert_eq!(played["track"]["id"], json!(2));
}

// Regression: a server restart drops all WS conns and lets presence scores go
// stale; the recovered round must keep playing while clients reconnect (grace
// window), not stop on the first empty member_count.
#[tokio::test(start_paused = true)]
async fn recover_survives_empty_presence_during_grace() {
    let mut fx = spawn_fixture().await;
    fx.presence.counts.lock().unwrap().insert(1, 0); // everyone "gone" as far as redis scores say
    fx.store.insert_round(RoundRow {
        id: 7,
        room_id: 1,
        tracks: vec![1, 2],
        current: 1,
        is_playing: true,
        finished_at: None,
        current_track_started_at: Some(chrono::Utc::now()),
    });
    fx.tx
        .send(RoomCommand::Recover {
            round: RoundRow {
                id: 7,
                room_id: 1,
                tracks: vec![1, 2],
                current: 1,
                is_playing: true,
                finished_at: None,
                current_track_started_at: Some(chrono::Utc::now()),
            },
        })
        .await
        .unwrap();

    advance(Duration::from_secs(3)).await;
    let played = next_event(&mut fx.rx, "TrackPlayed").await;
    assert_eq!(played["track"]["id"], json!(2));
}

// Regression: presence member_count collapses to 0 on any Redis error. A
// connected player (WS subscribed) must keep the round alive even when the
// presence store reports nobody — a Redis blip must never kill a game.
#[tokio::test(start_paused = true)]
async fn empty_presence_with_subscriber_keeps_playing() {
    let mut fx = spawn_fixture().await;
    fx.presence.counts.lock().unwrap().insert(1, 0);
    start(&fx).await;
    next_event(&mut fx.rx, "TrackPlayed").await; // track 1 playing

    advance(Duration::from_secs(31)).await; // past the 30s track
    next_event(&mut fx.rx, "TrackEnded").await;
    advance(Duration::from_secs(4)).await; // past the 3s pause
    let played = next_event(&mut fx.rx, "TrackPlayed").await;
    assert_eq!(played["track"]["id"], json!(2));
}

#[tokio::test(start_paused = true)]
async fn track_mismatch_is_rejected() {
    let fx = spawn_fixture().await;
    start(&fx).await;

    let (reply, out_rx) = oneshot::channel();
    fx.tx
        .send(RoomCommand::CheckAnswer {
            user_id: 5,
            track_id: 999, // not the current track
            text: "song 1".into(),
            client_words: vec![],
            current_time: 1.0,
            reply,
        })
        .await
        .unwrap();
    let outcome = timeout(Duration::from_secs(30), out_rx).await.unwrap().unwrap();
    assert_eq!(outcome, CheckOutcome::Bad);
}
