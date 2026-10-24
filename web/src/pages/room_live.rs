use crate::api;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use send_wrapper::SendWrapper;
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;
use web_sys::HtmlAudioElement;

/// Salon / TV view of a room: full-screen live scoreboard that plays the
/// current extract and reveals answers between tracks. Read-only — joining
/// happens on the phone via the QR code / link. Pure polling (no account,
/// no websocket needed) so it works on any browser left unattended.
#[component]
pub fn RoomLivePage() -> impl IntoView {
    crate::i18n::set_title_key("Live");

    let params = use_params_map();
    let room_id = Memo::new(move |_| {
        params
            .with(|p| p.get("id").and_then(|v| v.parse::<i64>().ok()))
            .unwrap_or(0)
    });

    let state = RwSignal::new(Option::<Value>::None);
    let not_found = RwSignal::new(false);
    let forbidden = RwSignal::new(false);
    let sound_on = RwSignal::new(false);
    let progress = RwSignal::new(0f64);
    let last_key = RwSignal::new(String::new());
    let last_started = RwSignal::new(0f64); // epoch seconds (server clock)
    let track_dur = RwSignal::new(30f64);

    let audio = SendWrapper::new(Rc::new(RefCell::new(Option::<HtmlAudioElement>::None)));
    {
        let audio = audio.clone();
        if audio.borrow().is_none() {
            if let Ok(a) = HtmlAudioElement::new() {
                a.set_volume(1.0);
                *audio.borrow_mut() = Some(a);
            }
        }
    }

    // Poll the live state — the whole view is driven by it.
    {
        let audio = audio.clone();
        let tick = move || {
            let audio = audio.clone();
            let rid = room_id.get();
            if rid == 0 {
                return;
            }
            leptos::task::spawn_local(async move {
                match api::get::<Value>(&format!("/api/v1/rooms/{rid}/live-state")).await {
                    Ok(v) => {
                        forbidden.set(false);
                        not_found.set(false);
                        // New extract started → (re)sync audio to server clock.
                        if let Some(ct) = v.get("current_track") {
                            let idx = ct["index"].as_i64().unwrap_or(0);
                            let started = ct["started_at"].as_str().unwrap_or("");
                            let key = format!("{idx}:{started}");
                            if key != last_key.get_untracked() {
                                last_key.set(key);
                                track_dur.set(ct["duration"].as_f64().unwrap_or(30.0).max(1.0));
                                let url = ct["audio"]
                                    .as_str()
                                    .or_else(|| ct["preview_url"].as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let started_epoch = js_sys::Date::parse(started) / 1000.0;
                                last_started.set(started_epoch);
                                if sound_on.get_untracked() && !url.is_empty() {
                                    if let Some(a) = audio.borrow().as_ref() {
                                        a.set_src(&url);
                                        let offset =
                                            (js_sys::Date::now() / 1000.0 - started_epoch).max(0.0);
                                        a.set_current_time(offset);
                                        let _ = a.play();
                                    }
                                }
                            }
                        } else if let Some(a) = audio.borrow().as_ref() {
                            if v["phase"].as_str() != Some("playing") && !a.paused() {
                                let _ = a.pause();
                            }
                        }
                        state.set(Some(v));
                    }
                    Err(e) => {
                        if e.status == 404 {
                            not_found.set(true);
                        } else if e.status == 403 {
                            forbidden.set(true);
                        }
                    }
                }
            });
        };
        tick();
        let iv = SendWrapper::new(gloo_timers::callback::Interval::new(3_000, tick));
        on_cleanup(move || drop(iv));
    }

    // Progress bar driven by the audio element clock.
    {
        let audio = audio.clone();
        let iv = SendWrapper::new(gloo_timers::callback::Interval::new(250, move || {
            if let Some(a) = audio.borrow().as_ref() {
                if !a.paused() {
                    let d = track_dur.get_untracked();
                    progress.set((a.current_time() / d * 100.0).clamp(0.0, 100.0));
                }
            }
        }));
        on_cleanup(move || drop(iv));
    }

    let host_cmd = {
        move |action: &'static str| {
            let rid = room_id.get_untracked();
            leptos::task::spawn_local(async move {
                let _ = api::post::<Value, _>(
                    &format!("/api/v1/rooms/{rid}/{action}"),
                    &serde_json::json!({}),
                )
                .await;
            });
        }
    };

    let enable_sound = {
        let audio = audio.clone();
        move |_| {
            sound_on.set(true);
            if let Some(a) = audio.borrow().as_ref() {
                if let Some(v) = state.get_untracked() {
                    if let Some(ct) = v.get("current_track") {
                        let url = ct["audio"]
                            .as_str()
                            .or_else(|| ct["preview_url"].as_str())
                            .unwrap_or("");
                        if !url.is_empty() {
                            a.set_src(url);
                            a.set_current_time(
                                (js_sys::Date::now() / 1000.0 - last_started.get_untracked())
                                    .max(0.0),
                            );
                            let _ = a.play();
                        }
                    }
                }
            }
        }
    };

    view! {
        <div class="min-h-[calc(100vh-4rem)] bg-neutral-950 px-4 py-6 sm:px-8">
            {move || {
                if not_found.get() {
                    return view! {
                        <div class="mx-auto max-w-lg py-24 text-center">
                            <p class="text-xl font-bold text-white">{t("Room not found")}</p>
                            <A href="/" attr:class="btn-primary mt-6 inline-block">{t("Back")}</A>
                        </div>
                    }
                    .into_any();
                }
                if forbidden.get() {
                    return view! {
                        <div class="mx-auto max-w-lg py-24 text-center">
                            <p class="text-xl font-bold text-white">{t("This room is private")}</p>
                            <p class="mt-2 text-sm text-white/50">{t("Join the room on your phone to watch it live.")}</p>
                            <A href=format!("/rooms/{}", room_id.get()) attr:class="btn-primary mt-6 inline-block">{t("Join the room")}</A>
                        </div>
                    }
                    .into_any();
                }
                match state.get() {
                    None => view! {
                        <div class="py-24 text-center text-white/50">{t("Loading")}"…"</div>
                    }
                    .into_any(),
                    Some(v) => {
                        let phase = v["phase"].as_str().unwrap_or("idle").to_string();
                        let idx = v["current_index"].as_i64().unwrap_or(0);
                        let total = v["tracks_total"].as_i64().unwrap_or(0);
                        let members = v["member_count"].as_i64().unwrap_or(0);
                        let standings = v["standings"].as_array().cloned().unwrap_or_default();
                        let reveal = v.get("reveal_track").cloned();
                        let venue = v["venue_name"].as_str().unwrap_or("").to_string();
                        let can_control = v["can_control"].as_bool().unwrap_or(false);
                        let rid = room_id.get();
                        view! {
                            <div class="mx-auto max-w-6xl">
                                // header : room name + track counter + members
                                <div class="mb-6 flex flex-wrap items-center justify-between gap-4">
                                    <div>
                                        {(!venue.is_empty()).then(|| view! {
                                            <p class="mb-1 text-sm font-bold uppercase tracking-[0.25em] text-amber-300">
                                                "📍 "{venue.clone()}
                                            </p>
                                        })}
                                        <h1 class="text-4xl font-black text-white sm:text-5xl lg:text-6xl">
                                            {v["name"].as_str().unwrap_or("").to_string()}
                                        </h1>
                                        <p class="mt-1 text-base text-white/50 lg:text-lg">
                                            {t("Live")}" · "{members}" "{t("players")}
                                        </p>
                                    </div>
                                    <div class="flex items-center gap-3">
                                        {if total > 0 {
                                            view! {
                                                <span class="rounded-full border border-white/15 bg-white/5 px-4 py-2 text-sm font-bold text-white">
                                                    {format!("{idx}/{total}")}
                                                </span>
                                            }.into_any()
                                        } else { view! { <div/> }.into_any() }}
                                        <button type="button"
                                            class=move || if sound_on.get() {
                                                "rounded-full border border-teal-500/50 bg-teal-500/15 px-4 py-2 text-sm font-semibold text-teal-300"
                                            } else {
                                                "rounded-full border border-amber-500/50 bg-amber-500/15 px-4 py-2 text-sm font-semibold text-amber-300"
                                            }
                                            on:click=enable_sound.clone()>
                                            {move || if sound_on.get() { "🔊 ".to_string() + &t("Sound on") } else { "🔇 ".to_string() + &t("Enable sound") }}
                                        </button>
                                    </div>
                                </div>

                                // center zone : playing / reveal / idle
                                <div class="mb-6 rounded-2xl border border-white/10 bg-neutral-900/80 p-6 text-center sm:p-10">
                                    {match phase.as_str() {
                                        "playing" => {
                                            let hint = v["current_track"]["hint"].as_str().unwrap_or("").to_string();
                                            view! {
                                                <div>
                                                    <p class="text-xs font-bold uppercase tracking-[0.3em] text-purple-300">
                                                        {t("Now playing")}
                                                    </p>
                                                    <div class="mx-auto mt-6 h-3 w-full max-w-md overflow-hidden rounded-full bg-white/10">
                                                        <div class="h-full rounded-full bg-gradient-to-r from-purple-500 to-teal-400 transition-[width] duration-200"
                                                            style=move || format!("width:{}%", progress.get())></div>
                                                    </div>
                                                    {(!hint.is_empty()).then(|| view! {
                                                        <p class="mt-4 text-sm text-white/40">"💡 "{hint}</p>
                                                    })}
                                                </div>
                                            }.into_any()
                                        }
                                        "paused" => view! {
                                            <div class="py-6">
                                                <p class="text-xs font-bold uppercase tracking-[0.3em] text-amber-300">
                                                    {t("Paused")}
                                                </p>
                                                <p class="mt-4 text-lg font-semibold text-white/70">{t("The game is paused by the host.")}</p>
                                            </div>
                                        }.into_any(),
                                        "reveal" => {
                                            let rt = reveal.clone().unwrap_or_default();
                                            let art = rt["artwork_url"].as_str().unwrap_or("").to_string();
                                            let answers = rt["answers"].as_array().cloned().unwrap_or_default();
                                            view! {
                                                <div>
                                                    <p class="text-xs font-bold uppercase tracking-[0.3em] text-amber-300">
                                                        {t("It was")}
                                                    </p>
                                                    {(!art.is_empty()).then(|| view! {
                                                        <img src=art alt="" class="mx-auto mt-4 h-32 w-32 rounded-xl object-cover shadow-2xl sm:h-40 sm:w-40"/>
                                                    })}
                                                    <div class="mt-4 flex flex-wrap justify-center gap-2">
                                                        {answers.iter().map(|a| view! {
                                                            <span class="rounded-lg border border-white/15 bg-white/5 px-3 py-1.5 text-sm font-semibold text-white">
                                                                {a["value"].as_str().unwrap_or("").to_string()}
                                                            </span>
                                                        }).collect::<Vec<_>>()}
                                                    </div>
                                                </div>
                                            }.into_any()
                                        }
                                        _ => view! {
                                            <div class="py-6">
                                                <p class="text-lg font-semibold text-white/70">{t("Waiting for the next round…")}</p>
                                            </div>
                                        }.into_any(),
                                    }}
                                </div>

                                {can_control.then(|| {
                                    let host = host_cmd.clone();
                                    view! {
                                        <div class="mb-6 flex flex-wrap items-center gap-3 rounded-2xl border border-amber-500/20 bg-amber-500/5 px-5 py-3">
                                            <span class="text-xs font-bold uppercase tracking-widest text-amber-300/80">{t("Host controls")}</span>
                                            {if phase == "paused" {
                                                view! {
                                                    <button type="button" class="rounded-lg bg-teal-500/20 px-4 py-1.5 text-sm font-semibold text-teal-200 hover:bg-teal-500/30"
                                                        on:click=move |_| host("resume")>
                                                        "▶ "{t("Resume")}
                                                    </button>
                                                }.into_any()
                                            } else {
                                                view! {
                                                    <button type="button" class="rounded-lg bg-white/10 px-4 py-1.5 text-sm font-semibold text-white hover:bg-white/20"
                                                        on:click=move |_| host("pause")>
                                                        "⏸ "{t("Pause")}
                                                    </button>
                                                }.into_any()
                                            }}
                                            {(phase == "playing").then(|| view! {
                                                <button type="button" class="rounded-lg bg-white/10 px-4 py-1.5 text-sm font-semibold text-white hover:bg-white/20"
                                                    on:click=move |_| host("next")>
                                                    "⏭ "{t("Next track")}
                                                </button>
                                            })}
                                        </div>
                                    }
                                })}
                                <div class="grid gap-6 lg:grid-cols-[1fr_260px]">
                                    // podium live
                                    <div class="rounded-2xl border border-white/10 bg-neutral-900/80 p-6">
                                        <h2 class="mb-4 text-sm font-bold uppercase tracking-widest text-white/50">{t("Scoreboard")}</h2>
                                        {if standings.is_empty() {
                                            view! { <p class="text-sm text-white/40">{t("No scores yet.")}</p> }.into_any()
                                        } else {
                                            view! {
                                                <ol class="space-y-2 lg:space-y-3">
                                                    {standings.iter().take(10).enumerate().map(|(i, s)| {
                                                        let medal = match i { 0 => "🥇", 1 => "🥈", 2 => "🥉", _ => "" };
                                                        view! {
                                                            <li class=format!("flex items-center gap-4 rounded-xl px-5 py-3 lg:py-4 {}",
                                                                if i < 3 { "bg-white/10" } else { "bg-white/[0.03]" })>
                                                                <span class="w-10 text-center text-xl font-black text-white/70 lg:text-2xl">
                                                                    {if medal.is_empty() { (i + 1).to_string() } else { medal.to_string() }}
                                                                </span>
                                                                <span class="flex-1 truncate text-lg font-bold text-white sm:text-xl lg:text-2xl">
                                                                    {s["name"].as_str().unwrap_or("").to_string()}
                                                                </span>
                                                                <span class="text-xl font-black text-teal-300 lg:text-3xl">
                                                                    {s["score"].as_f64().map(|x| format!("{x:.0}")).unwrap_or_default()}
                                                                </span>
                                                            </li>
                                                        }
                                                    }).collect::<Vec<_>>()}
                                                </ol>
                                            }.into_any()
                                        }}
                                    </div>

                                    // join card : QR + link — bar CTA
                                    <div class="rounded-2xl border border-white/10 bg-neutral-900/80 p-6 text-center">
                                        <h2 class="mb-4 text-sm font-bold uppercase tracking-widest text-white/50">{t("Scan to buzz!")}</h2>
                                        <div class="mx-auto w-fit rounded-xl bg-white p-3">
                                            <img src=format!("/api/v1/rooms/{rid}/qr.svg")
                                                alt="QR" class="h-52 w-52 lg:h-60 lg:w-60"/>
                                        </div>
                                        <p class="mt-3 text-xs font-semibold text-white/50">
                                            {web_sys::window()
                                                .and_then(|w| w.location().origin().ok())
                                                .map(|o| o.trim_start_matches("https://").trim_start_matches("http://").to_string())
                                                .unwrap_or_default()}
                                        </p>
                                        <A href=format!("/rooms/{rid}/pad") attr:class="btn-primary mt-4 inline-block w-full">
                                            {t("Join the game")}
                                        </A>
                                    </div>
                                </div>
                            </div>
                        }.into_any()
                    }
                }
            }}
        </div>
    }
}
