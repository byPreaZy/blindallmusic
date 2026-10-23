//! Online matchmaking for minigames: `/match/{type}` joins the game queue,
//! waits for an opponent (status poll + MatchFound events), then plays a
//! live match driven by the server on the `match.{id}` channel — same
//! question for every player, scores race in real time.

use crate::api;
use crate::auth::{on_member, use_auth};
use crate::components::minigame::{game_name, minigame_icon, minigame_tile, MINIGAME_TYPES};
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use send_wrapper::SendWrapper;
use serde_json::{json, Value};
use std::rc::Rc;

#[component]
pub fn MatchPage() -> impl IntoView {
    let params = use_params_map();
    let auth = use_auth();
    let game_type = move || {
        params
            .get()
            .get("type")
            .unwrap_or_default()
            .replace('-', "_")
    };
    // Resolve to a 'static str so it can be captured in Fn closures.
    let gtype = MINIGAME_TYPES.iter().copied().find(|t| *t == game_type());
    let valid = gtype.is_some();
    let gtype = gtype.unwrap_or("quiz");

    // "boot" (resolving status) | "queue" | "playing" | "results"
    let stage = RwSignal::new("boot");
    let position = RwSignal::new(0i64);
    let match_id = RwSignal::new(Option::<i64>::None);
    let match_state = RwSignal::new(Option::<Value>::None);
    let feedback = RwSignal::new(Option::<Value>::None);
    let answering = RwSignal::new(false);
    let tick = RwSignal::new(0u32);
    let error = RwSignal::new(Option::<String>::None);

    crate::i18n::set_title_key("Online match");

    // Shared realtime socket — reuse the app-wide one (the user channel
    // carries MatchFound), connect it on demand.
    let shared_rt = use_context::<crate::app::SharedRealtime>();
    let rt = StoredValue::new_local(None::<crate::ws::Realtime>);
    if let Some(shared) = shared_rt {
        if let Some(existing) = shared.get_untracked() {
            rt.set_value(Some(existing));
        } else if let Some(token) = api::stored_token() {
            let conn = crate::ws::Realtime::connect(&token);
            conn.start_heartbeat().forget();
            shared.set(Some(conn.clone()));
            rt.set_value(Some(conn));
        }
    }

    let joined_channel = RwSignal::new(Option::<String>::None);
    let still_queued = RwSignal::new(false);
    // Stale-callback guard for hooks parked on the shared realtime socket
    // (they outlive this component → touching disposed signals panics).
    let alive = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    {
        let alive = alive.clone();
        on_cleanup(move || alive.store(false, std::sync::atomic::Ordering::Relaxed));
    }

    // ---- enter a formed match: subscribe to its channel + load snapshot --
    let enter_alive = alive.clone();
    let enter_match = Callback::new(move |id: i64| {
        match_id.set(Some(id));
        stage.set("playing");
        feedback.set(None);
        let channel = format!("match.{id}");
        if let Some(realtime) = rt.get_value() {
            realtime.join(&channel);
            macro_rules! on {
                ($event:literal, $body:expr) => {{
                    let cb: Rc<dyn Fn(&Value)> = Rc::new($body);
                    realtime.listen(&channel, $event, cb);
                }};
            }
            on!("MatchQuestion", move |e| {
                feedback.set(None);
                match_state.set(Some(e.clone()));
            });
            on!("MatchReveal", move |e| match_state.set(Some(e.clone())));
            on!("MatchFinished", move |e| {
                match_state.set(Some(e.clone()));
                stage.set("results");
            });
            on!("MatchScore", move |e| {
                match_state.update(|m| {
                    if let Some(rows) = m.as_mut().and_then(|m| m["scores"].as_array_mut()) {
                        let uid = e["user_id"].as_i64();
                        if let Some(row) = rows.iter_mut().find(|r| r["user_id"].as_i64() == uid) {
                            row["score"] = e["score"].clone();
                        } else {
                            rows.push(json!({
                                "user_id": uid,
                                "name": e["name"],
                                "score": e["score"],
                            }));
                        }
                    }
                });
            });
            // Re-fetch the snapshot after a reconnect — question events
            // fired while the socket was down are not replayed. The hook
            // outlives this component on the shared socket: bail out once
            // the page is disposed or its signals panic on access.
            let alive = enter_alive.clone();
            realtime.on_reconnect(Rc::new(move || {
                if !alive.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                leptos::task::spawn_local(async move {
                    if let Ok(v) = api::get::<Value>(&format!("/api/v1/matches/{id}")).await {
                        let _ = match_state.try_set(Some(v));
                    }
                });
            }));
            joined_channel.set(Some(channel));
        }
        leptos::task::spawn_local(async move {
            match api::get::<Value>(&format!("/api/v1/matches/{id}")).await {
                Ok(v) => {
                    if v["phase"].as_str() == Some("finished") {
                        stage.set("results");
                    }
                    match_state.set(Some(v));
                }
                Err(_) => {
                    // Match swept (finished >10min ago) — back to queue.
                    match_id.set(None);
                    stage.set("queue");
                }
            }
        });
    });

    // ---- status poll while queued --------------------------------------
    let poll = StoredValue::new_local(None::<SendWrapper<gloo_timers::callback::Interval>>);
    let stop_poll = Callback::new(move |_: ()| poll.set_value(None));

    // Instant pickup: the server pings MatchFound on the user channel the
    // moment a match forms (the 2.5s status poll below is the fallback).
    let instant = RwSignal::new(false);
    Effect::new(move |_| {
        if instant.get() {
            return;
        }
        let Some(uid) = auth.user.get().map(|u| u.id) else {
            return;
        };
        let Some(realtime) = rt.get_value() else {
            return;
        };
        instant.set(true);
        let alive = alive.clone();
        realtime.listen(
            &format!("user:{uid}"),
            "MatchFound",
            Rc::new(move |e| {
                if !alive.load(std::sync::atomic::Ordering::Relaxed) {
                    return;
                }
                if let Some(id) = e["match_id"].as_i64() {
                    still_queued.set(false);
                    stop_poll.run(());
                    enter_match.run(id);
                }
            }),
        );
    });

    let start_poll = Callback::new(move |_: ()| {
        poll.set_value(None);
        let iv = SendWrapper::new(gloo_timers::callback::Interval::new(2_500, move || {
            leptos::task::spawn_local(async move {
                if let Ok(v) = api::get::<Value>("/api/v1/matchmaking/status").await {
                    match v["status"].as_str() {
                        Some("matched") => {
                            if let Some(id) = v["match_id"].as_i64() {
                                stop_poll.run(());
                                enter_match.run(id);
                            }
                        }
                        Some("queued") => {
                            position.set(v["position"].as_i64().unwrap_or(0));
                        }
                        _ => {
                            // dropped from the queue (stale sweep) — rejoin
                            let g = game_type();
                            let _ = api::post::<Value, _>(
                                "/api/v1/matchmaking/join",
                                &json!({"game_type": g}),
                            )
                            .await;
                        }
                    }
                }
            });
        }));
        poll.set_value(Some(iv));
    });

    // ---- boot: resume an active match, keep queueing, or join ----------
    if valid {
        on_member(auth, move || {
            leptos::task::spawn_local(async move {
                match api::get::<Value>("/api/v1/matchmaking/status").await {
                    Ok(v) => match v["status"].as_str() {
                        Some("matched") => {
                            if let Some(id) = v["match_id"].as_i64() {
                                enter_match.run(id);
                                return;
                            }
                        }
                        Some("queued") => {
                            position.set(v["position"].as_i64().unwrap_or(0));
                            stage.set("queue");
                            still_queued.set(true);
                            start_poll.run(());
                            return;
                        }
                        _ => {}
                    },
                    Err(_) => {}
                }
                let g = game_type();
                match api::post::<Value, _>("/api/v1/matchmaking/join", &json!({"game_type": g}))
                    .await
                {
                    Ok(v) => {
                        if v["status"].as_str() == Some("matched") {
                            if let Some(id) = v["match_id"].as_i64() {
                                enter_match.run(id);
                                return;
                            }
                        }
                        position.set(v["position"].as_i64().unwrap_or(1));
                        stage.set("queue");
                        still_queued.set(true);
                        start_poll.run(());
                    }
                    Err(e) => {
                        error.set(Some(e.message));
                        stage.set("queue");
                    }
                }
            });
        });
    }

    // countdown re-render every second — dropped on unmount: a forgotten
    // interval keeps updating a disposed signal → reactive panic.
    let tick_iv = SendWrapper::new(gloo_timers::callback::Interval::new(1_000, move || {
        let _ = tick.try_update(|t| *t += 1);
    }));
    on_cleanup(move || drop(tick_iv));

    on_cleanup(move || {
        if let Some(ch) = joined_channel.get_untracked() {
            if let Some(realtime) = rt.get_value() {
                realtime.leave(&ch);
            }
        }
        if still_queued.get_untracked() {
            leptos::task::spawn_local(async move {
                let _ = api::post::<Value, _>("/api/v1/matchmaking/leave", &json!({})).await;
            });
        }
    });

    let navigate = StoredValue::new(leptos_router::hooks::use_navigate());
    let cancel_queue = move |_| {
        still_queued.set(false);
        stop_poll.run(());
        leptos::task::spawn_local(async move {
            let _ = api::post::<Value, _>("/api/v1/matchmaking/leave", &json!({})).await;
        });
        navigate.get_value()("/minigames", Default::default());
    };

    let rematch = move |_| {
        still_queued.set(true);
        stage.set("queue");
        feedback.set(None);
        match_state.set(None);
        match_id.set(None);
        start_poll.run(());
        let g = game_type();
        leptos::task::spawn_local(async move {
            match api::post::<Value, _>("/api/v1/matchmaking/join", &json!({"game_type": g})).await
            {
                Ok(v) => {
                    if v["status"].as_str() == Some("matched") {
                        if let Some(id) = v["match_id"].as_i64() {
                            still_queued.set(false);
                            stop_poll.run(());
                            enter_match.run(id);
                        }
                    }
                }
                Err(e) => error.set(Some(e.message)),
            }
        });
    };

    let submit = Callback::new(move |value: String| {
        let Some(mid) = match_id.get_untracked() else {
            return;
        };
        let Some(m) = match_state.get_untracked() else {
            return;
        };
        let idx = m["idx"].as_i64().unwrap_or(-1);
        answering.set(true);
        leptos::task::spawn_local(async move {
            match api::post::<Value, _>(
                &format!("/api/v1/matches/{mid}/answer"),
                &json!({"idx": idx, "value": value}),
            )
            .await
            {
                Ok(v) => feedback.set(Some(v)),
                Err(e) => feedback.set(Some(json!({"correct": false, "error": e.message}))),
            }
            answering.set(false);
        });
    });

    let name = t(game_name(gtype));

    view! {
        <crate::components::member_gate::MemberGate title="Online match">
        <div class="mx-auto max-w-2xl px-4 py-8">
            <div class="mb-6 flex items-center justify-between">
                <A href="/minigames" attr:class="text-sm text-white/60 hover:text-white">
                    "← " {t("Back")}
                </A>
                <h1 class="retro-title retro-title--white text-xl">
                    {name}" · "{t("Online")}
                </h1>
                <span class="w-10"/>
            </div>

            {move || error.get().map(|e| view! {
                <div class="mb-4 rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">{e}</div>
            })}

            // ---------- queue ----------
            <Show when=move || stage.get() == "queue" || stage.get() == "boot">
                <div class="rounded-2xl border-2 border-teal-500/20 bg-neutral-900/80 p-8 text-center shadow-2xl">
                    <div class=format!("mx-auto mb-5 flex h-20 w-20 items-center justify-center rounded-xl bg-gradient-to-br {} text-white shadow-lg animate-pulse", minigame_tile(gtype))>
                        {minigame_icon(gtype)}
                    </div>
                    <p class="text-xl font-black text-white">{t("Searching for players…")}</p>
                    <p class="mt-2 text-sm text-white/50">
                        {t("You'll be matched as soon as another player joins the queue.")}
                    </p>
                    {move || (position.get() > 1).then(|| view! {
                        <p class="mt-3 text-xs text-white/40">
                            {t("Position in queue")}": "{position.get()}
                        </p>
                    })}
                    <div class="mt-6 flex items-center justify-center gap-3">
                        <button type="button"
                            class="rounded-lg border border-white/15 px-5 py-2 text-sm text-white/70 hover:bg-white/5"
                            on:click=cancel_queue>
                            {t("Cancel")}
                        </button>
                    </div>
                </div>
            </Show>

            // ---------- live match ----------
            {move || {
                if stage.get() != "playing" && stage.get() != "results" {
                    return view! { <div/> }.into_any();
                }
                let Some(m) = match_state.get() else {
                    return view! {
                        <div class="py-24 text-center text-white/60">{t("Loading")}"…"</div>
                    }.into_any();
                };
                let phase = m["phase"].as_str().unwrap_or("").to_string();
                let mtype = m["game_type"].as_str().unwrap_or(gtype).to_string();
                let idx = m["idx"].as_i64().unwrap_or(0);
                let total = m["total"].as_i64().unwrap_or(0);
                let ends = m["ends_at"].as_i64().unwrap_or(0);
                let _ = tick.get();
                let remaining = (ends - js_sys::Date::now() as i64 / 1000).max(0);
                let mut scores = m["scores"].as_array().cloned().unwrap_or_default();
                scores.sort_by_key(|s| -(s["score"].as_i64().unwrap_or(0)));
                let players = m["players"].as_array().cloned().unwrap_or_default();
                let finished = stage.get() == "results" || phase == "finished";
                let my_id = auth.user.get().map(|u| u.id).unwrap_or(0);
                let _ = players;

                let scores_table = view! {
                    <div class="mt-4 border-t border-white/10 pt-3">
                        <p class="mb-2 text-[10px] font-bold uppercase tracking-wider text-white/40">
                            {t("Scores")}
                        </p>
                        <div class="space-y-1">
                            {scores.iter().enumerate().map(|(i, s)| {
                                let me = s["user_id"].as_i64() == Some(my_id);
                                view! {
                                    <div class=move || format!(
                                        "flex items-center gap-2 rounded-lg px-3 py-1.5 text-sm {}",
                                        if me { "bg-teal-500/15 text-teal-200" } else { "text-white/80" }
                                    )>
                                        <span class="w-5 text-center text-xs text-white/40">{i + 1}</span>
                                        <span class="min-w-0 flex-1 truncate">
                                            {s["name"].as_str().map(str::to_string)
                                                .unwrap_or_else(|| format!("#{}", s["user_id"].as_i64().unwrap_or(0)))}
                                            {me.then(|| view! { <span class="ml-1 text-[10px] text-teal-400/70">"· "{t("you")}</span> })}
                                        </span>
                                        <span class="font-bold text-teal-400">
                                            {s["score"].as_i64().unwrap_or(0)}
                                        </span>
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    </div>
                };

                view! {
                    <div class="rounded-2xl border-2 border-purple-500/30 bg-neutral-900/80 p-5 shadow-2xl">
                        <div class="mb-3 flex items-center justify-between">
                            <p class="text-sm font-black uppercase tracking-wider text-purple-300">
                                {t("Match")}" · "{t(game_name(&mtype))}
                                {(!finished).then(|| view! {
                                    <span class="ml-2 text-white/40 normal-case">{idx + 1}"/"{total}</span>
                                })}
                            </p>
                            {(!finished).then(|| view! {
                                <span class="font-mono text-xs text-white/40">
                                    {format!("{}s", remaining)}
                                </span>
                            })}
                        </div>

                        {if finished {
                            let best = scores.first()
                                .and_then(|s| s["score"].as_i64()).unwrap_or(0);
                            let i_win = scores.first()
                                .and_then(|s| s["user_id"].as_i64()) == Some(my_id)
                                && best > 0
                                && scores.iter().filter(|s| s["score"].as_i64() == Some(best)).count() == 1;
                            view! {
                                <div class="py-6 text-center">
                                    <p class="text-2xl font-black text-white">
                                        {if i_win { t("You win!").to_string() }
                                         else if best == 0 { t("Match over!").to_string() }
                                         else { t("Match over!").to_string() }}
                                    </p>
                                    <p class="mt-1 text-sm text-white/50">
                                        {t("Final standings below.")}
                                    </p>
                                    <div class="mt-6 flex items-center justify-center gap-3">
                                        <button type="button" class="btn-primary"
                                            on:click=rematch>
                                            {t("Rematch")}
                                        </button>
                                        <A href="/minigames" attr:class="rounded-lg border border-white/15 px-4 py-2 text-sm text-white/70 hover:bg-white/5">
                                            {t("Back to games")}
                                        </A>
                                    </div>
                                </div>
                            }.into_any()
                        } else if phase == "countdown" {
                            view! {
                                <div class="py-10 text-center">
                                    <p class="text-xs font-bold uppercase tracking-widest text-teal-300">{t("Match found!")}</p>
                                    <p class="mt-3 text-5xl font-black text-white">{remaining}</p>
                                    <p class="mt-2 text-sm text-white/50">{t("Get ready…")}</p>
                                </div>
                            }.into_any()
                        } else if phase == "reveal" {
                            let cv = m["reveal_value"].as_str().unwrap_or("").to_string();
                            view! {
                                <div class="py-6 text-center">
                                    <p class="text-xs uppercase tracking-wider text-white/40">{t("Answer was")}</p>
                                    <p class="mt-1 text-xl font-black text-teal-400">{cv}</p>
                                </div>
                            }.into_any()
                        } else {
                            let q = m["question"]["question"].clone();
                            if let Some(fb) = feedback.get() {
                                let correct = fb["correct"].as_bool().unwrap_or(false);
                                let cv = fb["correct_value"].as_str().unwrap_or("").to_string();
                                view! {
                                    <div class="py-6 text-center">
                                        <p class=move || if correct { "text-lg font-bold text-teal-400" } else { "text-lg font-bold text-amber-500" }>
                                            {if correct { t("Correct!").to_string() } else { t("Not quite!").to_string() }}
                                        </p>
                                        {(!correct).then(|| view! {
                                            <p class="text-sm text-white/50">
                                                {t("Correct answer")}": "<span class="font-semibold text-white">{cv}</span>
                                            </p>
                                        })}
                                        <p class="mt-2 text-xs text-white/40">{t("Waiting for the next question…")}</p>
                                    </div>
                                }.into_any()
                            } else {
                                view! {
                                    <crate::pages::duels::QuestionView
                                        q=q
                                        game_type=mtype.clone()
                                        disabled=answering.get()
                                        on_answer=submit/>
                                }.into_any()
                            }
                        }}

                        {scores_table}
                    </div>
                }.into_any()
            }}
        </div>
        </crate::components::member_gate::MemberGate>
    }
}
