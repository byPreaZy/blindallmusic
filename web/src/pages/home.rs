use crate::api;
use crate::auth::Auth;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::{use_navigate, use_query_map};
use serde_json::Value;
use std::rc::Rc;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;

const USERS_SVG: &str = "M15 19.128a9.38 9.38 0 002.625.372 9.337 9.337 0 004.121-.952 4.125 4.125 0 00-7.533-2.493M15 19.128v-.003c0-1.113-.285-2.16-.786-3.07M15 19.128v.106A12.318 12.318 0 018.624 21c-2.331 0-4.512-.645-6.374-1.766l-.001-.109a6.375 6.375 0 0111.964-3.07M12 6.375a3.375 3.375 0 11-6.75 0 3.375 3.375 0 016.75 0zm8.25 2.25a2.625 2.625 0 11-5.25 0 2.625 2.625 0 015.25 0z";
const NOTE_SVG: &str =
    "M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z";
const ARROW_SVG: &str = "M13.5 4.5L21 12m0 0l-7.5 7.5M21 12H3";
const MIC_SVG: &str = "M12 14c1.66 0 2.99-1.34 2.99-3L15 5c0-1.66-1.34-3-3-3S9 3.34 9 5v6c0 1.66 1.34 3 3 3zm5.3-3c0 3-2.54 5.1-5.3 5.1S6.7 14 6.7 11H5c0 3.41 2.72 6.23 6 6.72V21h2v-3.28c3.28-.48 6-3.3 6-6.72h-1.7z";
const SHUFFLE_SVG: &str = "M10.59 9.17L5.41 4 4 5.41l5.17 5.17 1.42-1.41zM14.5 4l2.04 2.04L4 18.59 5.41 20 17.96 7.46 20 9.5V4h-5.5zm.33 9.41l-1.41 1.41 3.13 3.13L14.5 20H20v-5.5l-2.04 2.04-3.13-3.13z";
const TITLE_SVG: &str = "M5 4v3h5.5v12h3V7H19V4H5z";
const DISC_SVG: &str = "M12 2a10 10 0 100 20 10 10 0 000-20zm0 6a4 4 0 110 8 4 4 0 010-8zm0 3a1 1 0 100 2 1 1 0 000-2z";
const PLAY_SVG: &str = "M8 5v14l11-7z";
const DISCORD_SVG: &str = "M20.317 4.3698a19.7913 19.7913 0 00-4.8851-1.5152.0741.0741 0 00-.0785.0371c-.211.3753-.4447.8648-.6083 1.2495-1.8447-.2762-3.68-.2762-5.4868 0-.1636-.3933-.4058-.8742-.6177-1.2495a.077.077 0 00-.0785-.037 19.7363 19.7363 0 00-4.8852 1.515.0699.0699 0 00-.0321.0277C.5334 9.0458-.319 13.5799.0992 18.0578a.0824.0824 0 00.0312.0561c2.0528 1.5076 4.0413 2.4228 5.9929 3.0294a.0777.0777 0 00.0842-.0276c.4616-.6304.8731-1.2952 1.226-1.9942a.076.076 0 00-.0416-.1057c-.6528-.2476-1.2743-.5495-1.8722-.8923a.077.077 0 01-.0076-.1277c.1258-.0943.2517-.1923.3718-.2914a.0743.0743 0 01.0776-.0105c3.9278 1.7933 8.18 1.7933 12.0614 0a.0739.0739 0 01.0785.0095c.1202.099.246.1981.3728.2924a.077.077 0 01-.0066.1276 12.2986 12.2986 0 01-1.873.8914.0766.0766 0 00-.0407.1067c.3604.698.7719 1.3628 1.225 1.9932a.076.076 0 00.0842.0286c1.961-.6067 3.9495-1.5219 6.0023-3.0294a.077.077 0 00.0313-.0552c.5004-5.177-.8382-9.6739-3.5485-13.6604a.061.061 0 00-.0312-.0286z";

fn users_icon(cls: &'static str) -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class=cls>
            <path stroke-linecap="round" stroke-linejoin="round" d=USERS_SVG />
        </svg>
    }
}

fn note_icon(cls: &'static str) -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class=cls>
            <path d=NOTE_SVG />
        </svg>
    }
}

/// Per-game accent gradient for the minigames catalog icon tiles.
fn minigame_tile(ty: &str) -> &'static str {
    match ty {
        "quiz" => "from-teal-400 to-cyan-500",
        "who_sang" => "from-purple-500 to-fuchsia-600",
        "anagram" => "from-yellow-400 to-amber-600",
        "first_letter" => "from-blue-600 to-blue-800",
        "album_cover" => "from-violet-500 to-purple-800",
        _ => "from-brand-deep to-brand-midnight",
    }
}

/// Per-game icon for the minigames catalog cards — real SVGs (OS emojis
/// render inconsistently and looked cheap).
fn minigame_icon(ty: &str) -> impl IntoView {
    let d = match ty {
        "quiz" => NOTE_SVG,
        "who_sang" => MIC_SVG,
        "anagram" => SHUFFLE_SVG,
        "first_letter" => TITLE_SVG,
        "album_cover" => DISC_SVG,
        _ => PLAY_SVG,
    };
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="h-7 w-7" aria-hidden="true">
            <path d=d />
        </svg>
    }
}

fn arrow_icon() -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2.5" stroke="currentColor" class="h-4 w-4" aria-hidden="true">
            <path stroke-linecap="round" stroke-linejoin="round" d=ARROW_SVG />
        </svg>
    }
}

#[derive(Clone, Debug, Default)]
struct HomeRoom {
    id: i64,
    name: String,
    slug: String,
    photo: Option<String>,
    photo_path: Option<String>,
    is_public: bool,
    is_autostart: bool,
    is_playing: bool,
    is_password_protected: bool,
    tracks_by_round: i64,
    current_track_index: i64,
    subscriptions: i64,
    tracks_count: Option<i64>,
    category_id: Option<i64>,
    owner_name: Option<String>,
    owner_photo: Option<String>,
}

impl HomeRoom {
    fn from(v: &Value) -> Self {
        Self {
            id: v["id"].as_i64().unwrap_or(0),
            name: v["name"].as_str().unwrap_or_default().to_string(),
            slug: v["slug"].as_str().unwrap_or_default().to_string(),
            photo: v["photo"].as_str().map(str::to_string),
            photo_path: v["photo_path"].as_str().map(str::to_string),
            is_public: v["is_public"].as_bool().unwrap_or(true),
            is_autostart: v["is_autostart"].as_bool().unwrap_or(true),
            is_playing: v["is_playing"].as_bool().unwrap_or(false),
            is_password_protected: v["is_password_protected"].as_bool().unwrap_or(false),
            tracks_by_round: v["tracks_by_round"].as_i64().unwrap_or(15),
            current_track_index: v["current_track_index"].as_i64().unwrap_or(0),
            subscriptions: v["subscriptions"].as_i64().unwrap_or(0),
            tracks_count: v["tracks_count"].as_i64(),
            category_id: v["category"]["id"].as_i64(),
            owner_name: v["owner"]["name"].as_str().map(str::to_string),
            owner_photo: v["owner"]["photo"].as_str().map(str::to_string),
        }
    }
}

fn poster_gradient(r: &HomeRoom) -> &'static str {
    const G: [&str; 8] = [
        "from-brand-deep to-brand-midnight",
        "from-brand-midnight to-brand-deep",
        "from-brand-deep via-brand-midnight to-brand-deep",
        "from-brand-midnight via-brand-deep to-brand-midnight",
        "from-brand-deep-hover to-brand-midnight",
        "from-brand-midnight to-brand-deep-hover",
        "from-brand-deep to-brand-deep-hover",
        "from-brand-deep-hover to-brand-deep",
    ];
    let key = r.category_id.unwrap_or(r.id).unsigned_abs() as usize;
    G[key % G.len()]
}

fn poster_initials(name: &str) -> String {
    let words: Vec<&str> = name.split_whitespace().collect();
    if words.len() >= 2 {
        format!(
            "{}{}",
            words[0].chars().next().unwrap_or('?'),
            words[1].chars().next().unwrap_or('?')
        )
        .to_uppercase()
    } else {
        name.chars().take(2).collect::<String>().to_uppercase()
    }
}

#[derive(PartialEq, Clone, Copy)]
enum CardState {
    Hot,
    Live,
    LiveEmpty,
    LobbyManual,
    Open,
}

fn card_state(is_playing: bool, subscriptions: i64, is_autostart: bool) -> CardState {
    if is_playing && subscriptions >= 5 {
        CardState::Hot
    } else if is_playing && subscriptions >= 1 {
        CardState::Live
    } else if is_playing {
        CardState::LiveEmpty
    } else if !is_autostart {
        CardState::LobbyManual
    } else {
        CardState::Open
    }
}

fn status_label(s: CardState) -> &'static str {
    match s {
        CardState::Hot => t("Room in full swing"),
        CardState::Live => t("Game in progress"),
        CardState::LiveEmpty => t("Waiting for players"),
        CardState::LobbyManual => t("Waiting for host"),
        CardState::Open => t("Open room"),
    }
}

fn cta_label(s: CardState) -> &'static str {
    match s {
        CardState::Hot | CardState::Live => t("Join now"),
        CardState::LiveEmpty => t("Be the first to join"),
        CardState::LobbyManual => t("Enter room"),
        CardState::Open => t("Play"),
    }
}

fn cta_class(s: CardState) -> &'static str {
    match s {
        CardState::Hot | CardState::Live => "game-btn-play-hot",
        CardState::LiveEmpty => "game-btn-play-join",
        _ => "game-btn-play-neutral",
    }
}

fn use_room_public_channel(
    room: &HomeRoom,
    card_ref: NodeRef<leptos::html::Article>,
) -> (
    RwSignal<i64>,  // member_count
    RwSignal<bool>, // is_playing
    RwSignal<i64>,  // current_track_index
    RwSignal<bool>, // member_count_bump
) {
    let member_count = RwSignal::new(room.subscriptions);
    let is_playing = RwSignal::new(room.is_playing);
    let track_index = RwSignal::new(room.current_track_index);
    let bump = RwSignal::new(false);

    let room_id = room.id;
    let tab = if room.is_public {
        "official"
    } else {
        "community"
    };
    let channel = StoredValue::new(format!("room.public:{room_id}"));
    let last_reported = StoredValue::new(room.subscriptions);
    let subscribed = StoredValue::new(false);
    let shared_rt = use_context::<crate::app::SharedRealtime>();
    let tab_counts = use_context::<RwSignal<Value>>();
    let disposed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    let report_delta = {
        let disposed = disposed.clone();
        move |next: i64| {
            let delta = next - last_reported.get_value();
            if delta == 0 || disposed.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            last_reported.set_value(next);
            if let Some(counts) = tab_counts {
                counts.update(|v| {
                    let cur = v[tab].as_i64().unwrap_or(0);
                    v[tab] = Value::from((cur + delta).max(0));
                });
            }
        }
    };

    let apply_public_state: Rc<dyn Fn(&Value)> = {
        let disposed = disposed.clone();
        Rc::new(move |p: &Value| {
            if disposed.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            if let Some(mc) = p["memberCount"].as_i64() {
                if mc > member_count.get_untracked() {
                    bump.set(true);
                    let disposed = disposed.clone();
                    set_timeout(
                        move || {
                            if !disposed.load(std::sync::atomic::Ordering::Relaxed) {
                                bump.set(false);
                            }
                        },
                        std::time::Duration::from_millis(700),
                    );
                }
                member_count.set(mc);
                report_delta(mc);
            }
            if let Some(ip) = p["isPlaying"].as_bool() {
                is_playing.set(ip);
            }
            if let Some(i) = p["currentTrackIndex"].as_i64() {
                track_index.set(i);
            }
        })
    };

    let poll: Rc<dyn Fn()> = {
        let apply_public_state = apply_public_state.clone();
        Rc::new(move || {
            let apply_public_state = apply_public_state.clone();
            leptos::task::spawn_local(async move {
                if let Ok(v) =
                    api::get::<Value>(&format!("/api/v1/rooms/{room_id}/public-state")).await
                {
                    apply_public_state(&v);
                }
            });
        })
    };
    let interval_handle = StoredValue::new_local(None::<gloo_timers::callback::Interval>);

    let subscribe: Rc<dyn Fn()> = {
        let disposed = disposed.clone();
        let poll = poll.clone();
        let apply_public_state = apply_public_state.clone();
        Rc::new(move || {
            if subscribed.get_value() || disposed.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            subscribed.set_value(true);
            poll();
            let poll = poll.clone();
            interval_handle.set_value(Some(gloo_timers::callback::Interval::new(
                30_000,
                move || {
                    poll();
                },
            )));
            if let Some(rt) = shared_rt.and_then(|s| s.get()) {
                rt.join(&channel.get_value());
                rt.listen(
                    &channel.get_value(),
                    "RoomPublicState",
                    apply_public_state.clone(),
                );
            }
        })
    };
    let unsubscribe: Rc<dyn Fn()> = {
        let disposed = disposed.clone();
        Rc::new(move || {
            if disposed.load(std::sync::atomic::Ordering::Relaxed) || !subscribed.get_value() {
                return;
            }
            subscribed.set_value(false);
            interval_handle.set_value(None);
            if let Some(rt) = shared_rt.and_then(|s| s.get()) {
                rt.leave(&channel.get_value());
            }
        })
    };

    let observer = StoredValue::new_local(
        None::<(
            web_sys::IntersectionObserver,
            Closure<dyn FnMut(js_sys::Array, web_sys::IntersectionObserver)>,
        )>,
    );
    Effect::new(move |_| {
        let Some(el) = card_ref.get() else {
            return;
        };
        let subscribe = subscribe.clone();
        let unsubscribe = unsubscribe.clone();
        let cb = Closure::wrap(Box::new(
            move |entries: js_sys::Array, _o: web_sys::IntersectionObserver| {
                let Some(entry) = entries
                    .get(0)
                    .dyn_into::<web_sys::IntersectionObserverEntry>()
                    .ok()
                else {
                    return;
                };
                if entry.is_intersecting() {
                    subscribe();
                } else {
                    unsubscribe();
                }
            },
        )
            as Box<dyn FnMut(js_sys::Array, web_sys::IntersectionObserver)>);
        let init = web_sys::IntersectionObserverInit::new();
        init.set_root_margin("120px");
        if let Ok(obs) =
            web_sys::IntersectionObserver::new_with_options(cb.as_ref().unchecked_ref(), &init)
        {
            obs.observe(el.unchecked_ref());
            observer.set_value(Some((obs, cb)));
        }
    });
    on_cleanup(move || {
        if subscribed.get_value() {
            subscribed.set_value(false);
            interval_handle.set_value(None);
            if let Some(rt) = shared_rt.and_then(|s| s.get()) {
                rt.leave(&channel.get_value());
            }
        }
        disposed.store(true, std::sync::atomic::Ordering::Relaxed);
        observer.with_value(|o| {
            if let Some((obs, _cb)) = o {
                obs.disconnect();
            }
        });
    });

    (member_count, is_playing, track_index, bump)
}

#[component]
fn RoomCard(room: HomeRoom) -> impl IntoView {
    let href = format!(
        "/rooms/{}",
        if room.slug.is_empty() {
            room.id.to_string()
        } else {
            room.slug.clone()
        }
    );
    let is_public = room.is_public;
    let is_autostart = room.is_autostart;
    let tracks_by_round = room.tracks_by_round;
    let tracks_count = room.tracks_count;
    let card_ref = NodeRef::<leptos::html::Article>::new();
    let (subs, playing, track_idx, bump) = use_room_public_channel(&room, card_ref);
    let state = Signal::derive(move || card_state(playing.get(), subs.get(), is_autostart));
    let with_players = Signal::derive(move || playing.get() && subs.get() >= 1);
    let near_end = Signal::derive(move || {
        playing.get() && tracks_by_round > 0 && track_idx.get() >= tracks_by_round - 2
    });
    let progress = Signal::derive(move || {
        if playing.get() && tracks_by_round > 0 {
            (100.0 * track_idx.get() as f64 / tracks_by_round as f64).min(100.0)
        } else {
            0.0
        }
    });
    let track_value = Signal::derive(move || {
        if playing.get() {
            format!("{}/{}", track_idx.get(), tracks_by_round)
        } else if !is_public && tracks_count.is_some() {
            tracks_count.unwrap_or(0).to_string()
        } else {
            tracks_by_round.to_string()
        }
    });
    let online_label = Signal::derive(move || format!("{} {}", subs.get(), t("online")));
    let cover = match &room.photo_path {
        Some(_) => {
            let src = room.photo.clone().unwrap_or_default();
            let alt = room.name.clone();
            view! {
                <img src=src alt=alt
                     class="h-full w-full object-cover transition-transform duration-700 group-hover:scale-110" loading="lazy" />
            }.into_any()
        }
        None => {
            let grad = poster_gradient(&room);
            let initials = poster_initials(&room.name);
            view! {
                <div class=format!("flex h-full w-full items-center justify-center bg-gradient-to-br transition-transform duration-700 group-hover:scale-105 {grad}") aria-hidden="true">
                    <span class="game-card__poster-grid" aria-hidden="true" />
                    <span class="select-none text-5xl font-black tracking-tighter text-white/[0.07] sm:text-6xl">
                        {initials}
                    </span>
                </div>
            }.into_any()
        }
    };
    let password_protected = room.is_password_protected;
    let manual_start = !room.is_autostart;
    let name_title = room.name.clone();
    let name_text = room.name;
    let owner = room.owner_name.clone().filter(|_| !is_public);
    let owner_photo = room.owner_photo.clone();
    let owner_block = owner.map(|owner_name| {
        let photo = owner_photo.map(|p| {
            let alt = owner_name.clone();
            view! {
                <img src=p alt=alt class="h-5 w-5 rounded-full ring-2 ring-white/20" loading="lazy" />
            }.into_any()
        });
        view! {
            <div class="flex items-center gap-2">
                {photo}
                <span class="truncate text-xs text-white/75">{owner_name}</span>
            </div>
        }.into_any()
    });

    view! {
        <article class="group flex h-full w-full flex-col" node_ref=card_ref>
            <div class="game-card flex h-full flex-col"
                 class:game-card--live=move || playing.get() && !with_players.get()
                 class:game-card--hot=move || with_players.get()>
                <A href=href.clone() attr:rel=if !is_public { Some("nofollow") } else { None }
                   attr:class="relative block overflow-hidden aspect-[16/9]">
                    {cover}
                    <span class="game-card__scanlines" aria-hidden="true" />
                    <div class="absolute inset-0 bg-gradient-to-t from-brand-midnight via-brand-midnight/50 to-transparent" />

                    {move || playing.get().then(|| view! {
                        <div class=format!("pointer-events-none absolute inset-x-0 top-3 flex justify-center {}", if with_players.get() { "text-brand-secondary" } else { "text-brand-primary" })>
                            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 60 24" class="h-8 w-14 drop-shadow-lg sm:h-10 sm:w-16" fill="currentColor" aria-hidden="true">
                                <rect x="0" y="8" width="4" height="8" rx="1"><animate attributeName="height" values="8;16;8" dur="0.9s" repeatCount="indefinite"/><animate attributeName="y" values="8;4;8" dur="0.9s" repeatCount="indefinite"/></rect>
                                <rect x="8" y="4" width="4" height="16" rx="1"><animate attributeName="height" values="16;6;16" dur="0.7s" repeatCount="indefinite"/><animate attributeName="y" values="4;9;4" dur="0.7s" repeatCount="indefinite"/></rect>
                                <rect x="16" y="10" width="4" height="6" rx="1"><animate attributeName="height" values="6;14;6" dur="0.8s" repeatCount="indefinite"/><animate attributeName="y" values="10;6;10" dur="0.8s" repeatCount="indefinite"/></rect>
                                <rect x="24" y="6" width="4" height="12" rx="1"><animate attributeName="height" values="12;18;12" dur="1s" repeatCount="indefinite"/><animate attributeName="y" values="6;3;6" dur="1s" repeatCount="indefinite"/></rect>
                                <rect x="32" y="9" width="4" height="8" rx="1"><animate attributeName="height" values="8;15;8" dur="0.75s" repeatCount="indefinite"/><animate attributeName="y" values="9;5;9" dur="0.75s" repeatCount="indefinite"/></rect>
                                <rect x="40" y="5" width="4" height="14" rx="1"><animate attributeName="height" values="14;7;14" dur="0.85s" repeatCount="indefinite"/><animate attributeName="y" values="5;9;5" dur="0.85s" repeatCount="indefinite"/></rect>
                                <rect x="48" y="7" width="4" height="10" rx="1"><animate attributeName="height" values="10;17;10" dur="0.95s" repeatCount="indefinite"/><animate attributeName="y" values="7;4;7" dur="0.95s" repeatCount="indefinite"/></rect>
                            </svg>
                        </div>
                    })}

                    <div class="absolute left-2.5 top-2.5 z-20 flex flex-wrap gap-1.5">
                        {move || if playing.get() {
                            view! {
                                <span class="live-pill" class:live-pill--hot=move || with_players.get()>
                                    <span class="live-pill__dot" class:live-pill__dot--hot=move || with_players.get() aria-hidden="true" />
                                    "LIVE"
                                </span>
                            }.into_any()
                        } else {
                            view! { <span class="retro-badge">{status_label(state.get())}</span> }.into_any()
                        }}
                        {move || near_end.get().then(|| view! { <span class="retro-badge">{t("Final tracks")}</span> })}
                    </div>

                    <div class="absolute right-2.5 top-2.5 z-20 flex gap-1">
                        {password_protected.then(|| view! {
                            <span class="retro-badge flex h-7 w-7 items-center justify-center p-0" aria-label=t("Password protected")>
                                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="h-3.5 w-3.5" aria-hidden="true">
                                    <path fill-rule="evenodd" d="M12 1.5a5.25 5.25 0 00-5.25 5.25v3a3 3 0 00-3 3v6.75a3 3 0 003 3h10.5a3 3 0 003-3v-6.75a3 3 0 00-3-3v-3c0-2.9-2.35-5.25-5.25-5.25zm3.75 8.25v-3a3.75 3.75 0 10-7.5 0v3h7.5z" clip-rule="evenodd" />
                                </svg>
                            </span>
                        })}
                        {manual_start.then(|| view! {
                            <span class="retro-badge flex h-7 w-7 items-center justify-center p-0" aria-label=t("Manual start")>
                                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="h-3.5 w-3.5" aria-hidden="true">
                                    <path fill-rule="evenodd" d="M2.25 12c0-5.385 4.365-9.75 9.75-9.75s9.75 4.365 9.75 9.75-4.365 9.75-9.75 9.75S2.25 17.385 2.25 12zM12 8.25a.75.75 0 01.75.75v3.75a.75.75 0 01-1.5 0V9a.75.75 0 01.75-.75zm0 8.25a.75.75 0 100-1.5.75.75 0 000 1.5z" clip-rule="evenodd" />
                                </svg>
                            </span>
                        })}
                    </div>

                    <div class="retro-progress absolute bottom-0 left-0 right-0 z-20">
                        <div class="retro-progress__fill"
                             class:retro-progress__fill--live=move || playing.get() && !with_players.get()
                             class:retro-progress__fill--hot=move || with_players.get()
                             style=move || format!("width:{}%", progress.get()) />
                    </div>
                </A>

                <div class="game-card__deck flex flex-1 flex-col gap-3">
                    <div class="min-w-0">
                        <h3 class="truncate text-sm font-bold uppercase tracking-wide text-white sm:text-base" title=name_title>
                            {name_text}
                        </h3>
                        <div class="game-card__stats">
                            <span class="retro-stat retro-stat--card" class:scale-110=move || bump.get()
                                  aria-label=move || online_label.get() title=move || online_label.get()>
                                {users_icon("h-4 w-4 shrink-0")}
                                <span class="tabular-nums">{move || subs.get()}</span>
                            </span>
                            <span class="retro-stat retro-stat--card">
                                {note_icon("h-4 w-4 shrink-0")}
                                <span class="tabular-nums">{move || track_value.get()}</span>
                            </span>
                        </div>
                    </div>

                    {owner_block}

                    <A href=href attr:class=move || format!("mt-auto {}", cta_class(state.get()))>
                        {move || cta_label(state.get())}
                        {arrow_icon()}
                    </A>
                </div>
            </div>
        </article>
    }
}

#[component]
fn CategoryFilter(
    categories: RwSignal<Vec<Value>>,
    selected: RwSignal<Vec<i64>>,
    disabled: RwSignal<bool>,
) -> impl IntoView {
    let open = RwSignal::new(false);
    let label = move || {
        let sel = selected.get();
        if sel.is_empty() {
            t("All categories").to_string()
        } else if sel.len() == 1 {
            categories
                .get()
                .iter()
                .find(|c| c["id"].as_i64() == Some(sel[0]))
                .and_then(|c| c["name"].as_str().map(str::to_string))
                .unwrap_or_else(|| t("All categories").to_string())
        } else {
            format!("{} {}", sel.len(), t("categories"))
        }
    };

    view! {
        <div class="home-category-filter relative">
            <button type="button"
                class="home-category-filter__trigger"
                class:home-category-filter__trigger--active=move || !selected.get().is_empty()
                aria-label=t("Filter by category")
                on:click=move |_| open.update(|o| *o = !*o)>
                <svg class="home-category-filter__icon" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
                    <path d="M3 4.5a.75.75 0 0 1 .75-.75h12.5a.75.75 0 0 1 .53 1.28l-4.72 4.72v4.19a.75.75 0 0 1-1.085.67L7.5 13.09V9.5L2.72 4.72A.75.75 0 0 1 3 4.5Z" />
                </svg>
                <span class="home-category-filter__value">{label}</span>
                {move || (!selected.get().is_empty()).then(|| view! {
                    <span class="home-category-filter__badge">{selected.get().len()}</span>
                })}
                <svg class="home-category-filter__chevron h-3 w-3" viewBox="0 0 20 20" fill="currentColor" aria-hidden="true">
                    <path fill-rule="evenodd" d="M5.23 7.21a.75.75 0 011.06.02L10 11.168l3.71-3.938a.75.75 0 111.08 1.04l-4.25 4.5a.75.75 0 01-1.08 0l-4.25-4.5a.75.75 0 01.02-1.06z" clip-rule="evenodd" />
                </svg>
            </button>
            <Show when=move || open.get()>
                <div class="home-category-filter__panel retro-dropdown-panel absolute right-0 top-full z-30 mt-2 min-w-[240px]" role="listbox">
                    <button type="button" class="home-category-filter__option"
                        class:home-category-filter__option--selected=move || selected.get().is_empty()
                        on:click=move |_| { selected.set(vec![]); }>
                        <span class="home-category-filter__check" aria-hidden="true">
                            {move || selected.get().is_empty().then(|| view! {
                                <svg class="h-3.5 w-3.5" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" /></svg>
                            })}
                        </span>
                        <span class="home-category-filter__option-label">{t("All categories")}</span>
                    </button>
                    <div class="home-category-filter__divider" />
                    <For
                        each=move || categories.get()
                        key=|c| c["id"].as_i64().unwrap_or(0)
                        children=move |c: Value| {
                            let id = c["id"].as_i64().unwrap_or(0);
                            let name = c["name"].as_str().unwrap_or_default().to_string();
                            let count = c["rooms_count"].as_i64().unwrap_or(0);
                            let is_sel = move || selected.get().contains(&id);
                            view! {
                                <button type="button" class="home-category-filter__option"
                                    class:home-category-filter__option--selected=is_sel
                                    disabled=move || disabled.get()
                                    on:click=move |_| {
                                        selected.update(|s| {
                                            if s.contains(&id) { s.retain(|x| *x != id); } else { s.push(id); }
                                        });
                                    }>
                                    <span class="home-category-filter__check" aria-hidden="true">
                                        {move || is_sel().then(|| view! {
                                            <svg class="h-3.5 w-3.5" viewBox="0 0 20 20" fill="currentColor"><path fill-rule="evenodd" d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z" clip-rule="evenodd" /></svg>
                                        })}
                                    </span>
                                    <span class="home-category-filter__option-label">
                                        {name}
                                        <span class="home-category-filter__count">{format!(" ({count})")}</span>
                                    </span>
                                </button>
                            }
                        }
                    />
                </div>
            </Show>
        </div>
    }
}

#[component]
fn TopPlayers(list: Vec<Value>) -> impl IntoView {
    let podium: Vec<(usize, Value)> = {
        let p: Vec<Value> = list.iter().take(3).cloned().collect();
        if p.len() < 2 {
            p.into_iter().enumerate().map(|(i, v)| (i + 1, v)).collect()
        } else {
            let mut v = vec![(2usize, p[1].clone()), (1, p[0].clone())];
            if let Some(th) = p.get(2) {
                v.push((3, th.clone()));
            }
            v
        }
    };
    let rest: Vec<Value> = list.iter().skip(3).cloned().collect();

    let badge_cls = |rank: usize| match rank {
        1 => "home-weekly-podium__badge home-weekly-podium__badge--1",
        2 => "home-weekly-podium__badge home-weekly-podium__badge--2",
        _ => "home-weekly-podium__badge home-weekly-podium__badge--3",
    };
    let ring_cls = |rank: usize| match rank {
        1 => "ring-brand-secondary/80",
        2 => "ring-brand-accent/70",
        _ => "ring-brand-primary/70",
    };
    let avatar = |u: &Value| {
        let name = u["name"].as_str().unwrap_or("?").to_string();
        let src = u["photo"]
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| format!("/api/v1/avatar?name={}", urlencoding(&name)));
        (src, name)
    };

    view! {
        <article>
            <div class=format!("home-weekly-podium {}", if rest.is_empty() { "mb-0" } else { "mb-4" })>
                {podium.into_iter().map(|(rank, player)| {
                    let (src, name) = avatar(&player["user"]);
                    let uid = player["user"]["id"].as_i64().unwrap_or(0);
                    let score = player["total_score"].as_f64().unwrap_or(0.0);
                    let spot = format!("home-weekly-podium__spot home-weekly-podium__spot--{rank}");
                    let photo_cls = format!("home-weekly-podium__photo {} {}", if rank == 1 { "home-weekly-podium__photo--first" } else { "" }, ring_cls(rank));
                    let name_alt = name.clone();
                    let name_title = name.clone();
                    view! {
                        <div class=spot>
                            <div class="home-weekly-podium__avatar">
                                <span class=badge_cls(rank) aria-hidden="true">{rank}</span>
                                <A href=format!("/profile/{uid}")>
                                    <img src=src alt=name_alt class=photo_cls loading="lazy" />
                                </A>
                            </div>
                            <div class="home-weekly-podium__card">
                                <A href=format!("/profile/{uid}") attr:class="home-weekly-podium__name" attr:title=name_title>
                                    {name}
                                </A>
                                <p class="home-weekly-podium__score">
                                    {format!("{score:.2}")}<sup>{t("PTS")}</sup>
                                </p>
                            </div>
                        </div>
                    }
                }).collect_view()}
            </div>

            <ul class="flex flex-col gap-2">
                {rest.into_iter().enumerate().map(|(i, score)| {
                    let (src, name) = avatar(&score["user"]);
                    let uid = score["user"]["id"].as_i64().unwrap_or(0);
                    let pts = score["total_score"].as_f64().unwrap_or(0.0);
                    let elo = score["user"]["elo"].as_i64().unwrap_or(0);
                    let name_alt = name.clone();
                    view! {
                        <li class="flex items-center gap-3 rounded-lg border border-white/10 bg-brand-deep px-3 py-2 transition hover:border-white/20 hover:bg-brand-deep-hover">
                            <span class="w-5 shrink-0 text-center text-xs font-black text-white/60">{i + 4}</span>
                            <A href=format!("/profile/{uid}") attr:class="shrink-0">
                                <img src=src alt=name_alt class="h-8 w-8 rounded-full ring-1 ring-white/15" loading="lazy" />
                            </A>
                            <div class="min-w-0 flex-1">
                                <A href=format!("/profile/{uid}") attr:class="block truncate text-sm font-medium text-white hover:text-white/80">
                                    {name}
                                </A>
                            </div>
                            <div class="flex shrink-0 flex-col items-end gap-0.5">
                                <span class="text-xs font-bold text-white">{format!("{pts} {}", t("PTS"))}</span>
                                {(elo > 0).then(|| view! {
                                    <crate::components::elo::EloBadge elo=elo size="sm" variant="minimal"/>
                                })}
                            </div>
                        </li>
                    }
                }).collect_view()}
            </ul>
        </article>
    }
}

fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_string()
            } else {
                format!("%{:02X}", c as u32)
            }
        })
        .collect()
}

#[component]
fn DonationGoalCard() -> impl IntoView {
    let goal = RwSignal::new(Option::<Value>::None);
    leptos::task::spawn_local(async move {
        if let Ok(v) = api::get::<Value>("/api/v1/donations/progress").await {
            goal.set(Some(v));
        }
    });

    view! {
        <Show when=move || goal.get().is_some()>
            {move || {
                let g = goal.get().unwrap();
                let reached = g["goal_reached"].as_bool().unwrap_or(false);
                let effective = g["effective_cents"].as_f64().unwrap_or(0.0) / 100.0;
                let percent = g["percent"].as_f64().unwrap_or(0.0).min(100.0);
                let supporters = g["monthly_supporters"].as_array().cloned().unwrap_or_default();
                let days = g["days_remaining"].as_i64().unwrap_or(0);
                view! {
                    <div class="donation-goal-card relative overflow-hidden">
                        <div class="relative z-10 space-y-3">
                            <div class="flex items-start justify-between gap-3">
                                <div class="min-w-0 space-y-1">
                                    <div class="flex flex-wrap items-center gap-2">
                                        <p class="game-section-kicker">{t("Donation card kicker")}</p>
                                        {reached.then(|| view! { <span class="donation-goal-badge">{t("Objectif tenu !")}</span> })}
                                    </div>
                                    <p class="font-semibold leading-snug text-white">
                                        {if reached { t("Merci, objectif atteint !") } else { t("Offrez un café à Blind'All Music") }}
                                    </p>
                                    <p class="text-sm leading-snug text-white/70">
                                        {if reached {
                                            t("Objectif tenu : merci à la communauté qui porte Blind'All Music ce mois-ci.").to_string()
                                        } else {
                                            format!("{} {} {}", t("Jours restants"), days, t("ce mois-ci"))
                                        }}
                                    </p>
                                </div>
                                <div class="shrink-0 text-right">
                                    <p class=format!("text-2xl font-black tabular-nums leading-none {}", if reached { "text-emerald-400" } else { "text-white" })>
                                        {format!("{effective:.0} €")}
                                    </p>
                                    <p class="mt-1 max-w-[5.5rem] text-[10px] font-semibold uppercase leading-tight tracking-wide text-white/45">
                                        {t("ce mois-ci")}
                                    </p>
                                </div>
                            </div>

                            {(!supporters.is_empty()).then(|| view! {
                                <div class="flex -space-x-2">
                                    {supporters.iter().take(10).filter_map(|s| {
                                        s["photo"].as_str().map(|p| view! {
                                            <img src=p.to_string() class="h-6 w-6 rounded-full ring-2 ring-brand-deep" loading="lazy" />
                                        })
                                    }).collect_view()}
                                </div>
                            })}

                            <div class="space-y-1.5">
                                <div class="relative h-1.5 overflow-hidden rounded-full bg-white/10" role="progressbar">
                                    <div class="absolute inset-y-0 left-0 flex overflow-hidden rounded-full transition-all duration-500 ease-out" style=format!("width:{}%", percent)>
                                        <div class="h-full bg-brand-accent" style="width:100%" />
                                    </div>
                                </div>
                            </div>

                            <A href="/donations" attr:class="game-btn-play-primary inline-flex w-full justify-center">
                                {if reached { t("Offrir encore un café") } else { t("Offrir un café") }}
                                <span class="ml-1" aria-hidden="true">"☕"</span>
                            </A>
                            <p class="text-center text-[11px] text-white/40">{t("Donation perks and transparency")}</p>
                        </div>
                    </div>
                }
            }}
        </Show>
    }
}

#[component]
fn HomeSidebar(
    weekly: RwSignal<Vec<Value>>,
    featured: RwSignal<Vec<HomeRoom>>,
    logged_in: Signal<bool>,
) -> impl IntoView {
    let _ = logged_in; // kept for when mini-games are re-enabled
    view! {
        <aside class="w-full shrink-0 lg:w-80 xl:w-96">
            <div class="retro-panel lg:sticky lg:top-6 lg:self-start">
                <div id="home-donation-goal" class="scroll-mt-24 border-b border-white/10 p-4">
                    <DonationGoalCard />
                </div>

                {move || featured.get().into_iter().map(|r| view! {
                    <div class="border-b border-white/10 p-4">
                        <p class="game-section-kicker mb-3">{t("Featured")}</p>
                        <RoomCard room=r />
                    </div>
                }).collect_view()}

                {move || (!weekly.get().is_empty()).then(|| view! {
                    <div class="border-b border-white/10 p-4">
                        <p class="game-section-kicker">{t("Ranking")}</p>
                        <h3 class="retro-title retro-title--secondary mb-4 text-sm">{t("Top 10 of the week")}</h3>
                        <TopPlayers list=weekly.get_untracked() />
                    </div>
                })}

                <div class="p-4">
                    <p class="game-section-kicker">{t("Play")}</p>
                    <h3 class="retro-title retro-title--secondary mb-3 text-sm">{t("Quick Links")}</h3>
                    <div class="grid grid-cols-2 gap-2">
                        <a href=crate::api::DISCORD_INVITE_URL target="_blank" rel="external nofollow" class="sidebar-tile group" title=t("Join the Blind'All Music community on Discord")>
                            <span class="sidebar-tile__icon group-hover:scale-105" aria-hidden="true">
                                <svg class="h-5 w-5" fill="currentColor" viewBox="0 0 24 24"><path d=DISCORD_SVG /></svg>
                            </span>
                            <span class="text-xs font-semibold text-white">"Discord"</span>
                        </a>
                        <A href="/rankings" attr:class="sidebar-tile group">
                            <span class="sidebar-tile__icon group-hover:scale-105" aria-hidden="true">
                                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="h-5 w-5"><path d="M4 20v-7h4v7H4zm6 0V4h4v16h-4zm6 0v-11h4v11h-4z" /></svg>
                            </span>
                            <span class="text-xs font-semibold text-white">{t("Rankings")}</span>
                        </A>
                    </div>
                </div>
            </div>
        </aside>
    }
}

#[component]
pub fn HomePage() -> impl IntoView {
    crate::i18n::set_title_key("Multiplayer music quizzes");
    crate::seo::apply_page("/", t("Home meta description"));
    let auth = expect_context::<Auth>();

    let query = use_query_map();
    let initial_tab = match query
        .get_untracked()
        .get("tab")
        .unwrap_or_default()
        .as_str()
    {
        t @ ("community" | "minigames" | "mine" | "favorites") => t.to_string(),
        _ => "official".to_string(),
    };
    let tab = RwSignal::new(initial_tab);
    let cat_ids = RwSignal::new(Vec::<i64>::new());
    let page = RwSignal::new(1usize);

    let items = RwSignal::new(Vec::<HomeRoom>::new());
    let minigames = RwSignal::new(Vec::<Value>::new());
    let last_page = RwSignal::new(1usize);
    let public_cats = RwSignal::new(Vec::<Value>::new());
    let community_cats = RwSignal::new(Vec::<Value>::new());
    let counts = RwSignal::new(Value::Null);
    provide_context(counts);
    let weekly = RwSignal::new(Vec::<Value>::new());
    let featured = RwSignal::new(Vec::<HomeRoom>::new());
    let loading = RwSignal::new(true);
    let filter_open_disabled = RwSignal::new(false);

    let logged_in = Signal::derive(move || auth.user.get().is_some_and(|u| !u.is_guest));

    let fetch_home = move || {
        let tab_v = tab.get_untracked();
        let cats = cat_ids.get_untracked();
        let pg = page.get_untracked();
        let mut url = format!("/api/v1/home?tab={tab_v}&page={pg}");
        for c in &cats {
            url.push_str(&format!("&category_ids={c}"));
        }
        loading.set(true);
        leptos::task::spawn_local(async move {
            match api::get::<Value>(&url).await {
                Ok(v) => {
                    items.set(
                        v["catalog_items"]["data"]
                            .as_array()
                            .map(|a| a.iter().map(HomeRoom::from).collect())
                            .unwrap_or_default(),
                    );
                    if tab_v == "minigames" {
                        minigames.set(
                            v["catalog_items"]["data"]
                                .as_array()
                                .cloned()
                                .unwrap_or_default(),
                        );
                    }
                    last_page
                        .set(v["catalog_items"]["last_page"].as_i64().unwrap_or(1).max(1) as usize);
                    public_cats.set(
                        v["public_categories"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default(),
                    );
                    community_cats.set(
                        v["community_categories"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default(),
                    );
                    counts.set(v["catalog_tab_player_counts"].clone());
                    weekly.set(
                        v["weekly_top_users"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default(),
                    );
                    featured.set(
                        v["featured_rooms"]
                            .as_array()
                            .map(|a| a.iter().map(HomeRoom::from).collect())
                            .unwrap_or_default(),
                    );
                }
                Err(e) => leptos::logging::warn!("home catalog failed {}", e.message),
            }
            loading.set(false);
        });
    };

    let apply_tab = move |id: &'static str| {
        if tab.get_untracked() == id {
            return;
        }
        let had_cats = !cat_ids.get_untracked().is_empty();
        tab.set(id.to_string());
        cat_ids.set(vec![]);
        page.set(1);
        if !had_cats {
            fetch_home();
        }
    };

    let navigate = StoredValue::new(use_navigate());
    let switch_tab = move |id: &'static str| {
        if tab.get_untracked() == id {
            return;
        }
        // Reflect the tab in the URL so a reload or a shared link lands on
        // the same catalog view (?tab=minigames etc.).
        navigate.with_value(|n| {
            n(
                &format!("/?tab={id}"),
                leptos_router::NavigateOptions {
                    replace: true,
                    ..Default::default()
                },
            )
        });
        apply_tab(id);
    };

    // Deep-link: /?tab=… while the page is already mounted (navbar link)
    // or on a fresh load — select the matching catalog tab reactively.
    Effect::new(move |_| {
        let wanted = query.get().get("tab").unwrap_or_default();
        let id: &'static str = match wanted.as_str() {
            "official" => "official",
            "community" => "community",
            "minigames" => "minigames",
            "mine" if logged_in.get() => "mine",
            "favorites" if logged_in.get() => "favorites",
            _ => return,
        };
        apply_tab(id);
    });

    let refetch = StoredValue::new(fetch_home);
    Effect::new(move |_| {
        cat_ids.track();
        refetch.with_value(|f| f());
    });

    let search_results = RwSignal::new(Vec::<HomeRoom>::new());
    let search_term = Signal::derive(move || query.get().get("search").unwrap_or_default());
    Effect::new(move |_| {
        let s = search_term.get();
        if s.is_empty() {
            search_results.set(vec![]);
            crate::i18n::set_title_key("Multiplayer music quizzes");
            crate::seo::apply_page("/", t("Home meta description"));
            return;
        }
        crate::i18n::set_title_key("Search Results");
        crate::seo::apply_page("/", t("Home search meta description"));
        leptos::task::spawn_local(async move {
            if let Ok(v) = api::get::<Value>(&format!(
                "/api/v1/home?search={}",
                js_sys::encode_uri_component(&s)
            ))
            .await
            {
                search_results.set(
                    v["search_result"]
                        .as_array()
                        .map(|a| a.iter().map(HomeRoom::from).collect())
                        .unwrap_or_default(),
                );
            }
        });
    });
    let load_more = move || {
        page.update(|p| *p += 1);
        let tab_v = tab.get_untracked();
        let cats = cat_ids.get_untracked();
        let pg = page.get_untracked();
        let mut url = format!("/api/v1/home?tab={tab_v}&page={pg}");
        for c in &cats {
            url.push_str(&format!("&category_ids={c}"));
        }
        leptos::task::spawn_local(async move {
            if let Ok(v) = api::get::<Value>(&url).await {
                let more: Vec<HomeRoom> = v["catalog_items"]["data"]
                    .as_array()
                    .map(|a| a.iter().map(HomeRoom::from).collect())
                    .unwrap_or_default();
                items.update(|cur| cur.extend(more));
            }
        });
    };

    let tabs = move || {
        let mut v = vec![
            ("official", t("Official rooms").to_string()),
            ("community", t("Private rooms").to_string()),
            ("minigames", t("Mini-games").to_string()),
        ];
        if logged_in.get() {
            v.push(("mine", t("My rooms").to_string()));
        }
        if logged_in.get() {
            v.push(("favorites", t("Bookmarks").to_string()));
        }
        v
    };

    view! {
        <div class="w-full px-4 py-6 md:px-12">
            <h1 class="sr-only">"Blind'All Music, "{t("Multiplayer music quizzes")}</h1>

            {move || (!search_term.get().is_empty()).then(|| view! {
                <section class="mb-12">
                    <h2 class="sr-only">{t("Search Results")}</h2>
                    <header class="home-catalog-header mb-6">
                        <div class="home-rooms-toolbar">
                            <h2 class="retro-title text-lg sm:text-xl">{t("Search Results")}</h2>
                            <button type="button" class="game-btn-secondary"
                                on:click=move |_| navigate.with_value(|n| n("/", Default::default()))>
                                {t("Clear Search")}
                            </button>
                        </div>
                    </header>
                    {move || if search_results.get().is_empty() {
                        view! { <p class="py-10 text-center text-white/50">{t("No rooms found")}</p> }.into_any()
                    } else {
                        view! {
                            <div class="grid grid-cols-2 gap-4 md:grid-cols-3 xl:grid-cols-4">
                                {search_results.get().into_iter().map(|r| view! { <RoomCard room=r /> }).collect_view()}
                            </div>
                        }.into_any()
                    }}
                </section>
            })}

            <div class="flex flex-col gap-4 lg:gap-8" class:hidden=move || !search_term.get().is_empty()>
                <div class="space-y-2 lg:hidden">
                    <div class="retro-hero flex items-center gap-4">
                        <div class="retro-hero__icon">
                            {note_icon("h-6 w-6")}
                        </div>
                        <p class="text-sm font-bold uppercase leading-snug tracking-wide text-white">
                            {t("Pick a room, listen to the excerpt, be the fastest to answer.")}
                        </p>
                    </div>
                </div>

                <div class="flex flex-col gap-8 lg:flex-row lg:gap-8 xl:gap-10">
                    <section class="min-w-0 flex-1 space-y-8">
                        <section id="home-catalog" class="space-y-4 lg:space-y-6">
                            <header class="home-catalog-header">
                                <div class="home-rooms-toolbar">
                                    <div class="home-rooms-switch home-rooms-toolbar__tabs" role="tablist">
                                        {tabs().into_iter().map(|(id, label)| {
                                            let active = move || tab.get() == id;
                                            let count = move || {
                                                let c = counts.get();
                                                match id {
                                                    "official" => c["official"].as_i64(),
                                                    "community" => c["community"].as_i64(),
                                                    _ => None,
                                                }
                                            };
                                            view! {
                                                <button type="button" role="tab"
                                                    class="home-rooms-switch__tab"
                                                    class:home-rooms-switch__tab--active=active
                                                    aria-selected=move || active().to_string()
                                                    on:click=move |_| switch_tab(id)>
                                                    <span class="home-rooms-switch__tab-inner">
                                                        <span>{label}</span>
                                                        {move || count().map(|n| view! {
                                                            <span class="home-rooms-switch__tab-count">
                                                                {users_icon("h-3.5 w-3.5 shrink-0")}
                                                                <span>{n}</span>
                                                            </span>
                                                        })}
                                                    </span>
                                                </button>
                                            }
                                        }).collect_view()}
                                    </div>

                                    {(move || tab.get() == "official" || tab.get() == "community")().then(|| view! {
                                        <CategoryFilter
                                            categories=if tab.get() == "official" { public_cats } else { community_cats }
                                            selected=cat_ids
                                            disabled=filter_open_disabled
                                        />
                                    })}
                                </div>
                            </header>

                            <div class="home-catalog-panel space-y-6">
                                {move || {
                                    let is_minigames = tab.get() == "minigames";
                                    let list = items.get();
                                    if is_minigames {
                                        view! {
                                            <div>
                                                <div class="mb-4 flex justify-end">
                                                    <A href="/minigames" attr:class="game-link-action">
                                                        {t("View all")}
                                                        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" class="inline-block h-4 w-4"><path stroke-linecap="round" stroke-linejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5" /></svg>
                                                    </A>
                                                </div>
                                                <div class="grid grid-cols-2 gap-3 sm:gap-4 md:grid-cols-3 xl:grid-cols-4">
                                                    {minigames.get().into_iter().map(|g| {
                                                        let ty = g["type"].as_str().unwrap_or_default().to_string();
                                                        let score = g["score"].as_i64().unwrap_or(0);
                                                        let label = match ty.as_str() {
                                                            "quiz" => t("Quiz"),
                                                            "who_sang" => t("Who sang?"),
                                                            "anagram" => t("Anagram"),
                                                            "first_letter" => t("First letter"),
                                                            "album_cover" => t("Album cover"),
                                                            _ => "Mini-game",
                                                        };
                                                        view! {
                                                            <A href=format!("/minigames/{}", ty.replace('_', "-")) attr:class="game-card group flex flex-col">
                                                                <div class="flex aspect-[16/9] items-center justify-center bg-gradient-to-br from-brand-deep to-brand-midnight">
                                                                    <div class=format!("flex h-16 w-16 items-center justify-center rounded-xl bg-gradient-to-br {} text-white shadow-lg ring-1 ring-white/20 transition duration-200 group-hover:scale-110", minigame_tile(&ty)) aria-hidden="true">
                                                                        {minigame_icon(&ty)}
                                                                    </div>
                                                                </div>
                                                                <div class="game-card__deck flex flex-1 flex-col gap-3">
                                                                    <h3 class="truncate text-sm font-bold uppercase tracking-wide text-white sm:text-base">{label}</h3>
                                                                    <span class="retro-stat retro-stat--card">{format!("{} {}", score, t("PTS"))}</span>
                                                                    <span class="game-btn-play-neutral mt-auto inline-flex items-center gap-1">
                                                                        {t("Play")}{arrow_icon()}
                                                                    </span>
                                                                </div>
                                                            </A>
                                                        }
                                                    }).collect_view()}
                                                </div>
                                            </div>
                                        }.into_any()
                                    } else if list.is_empty() && !loading.get() {
                                        view! {
                                            <div class="home-rooms-empty">
                                                <p class="text-sm font-semibold text-white">
                                                    {if tab.get() == "favorites" {
                                                        t("No bookmarks found")
                                                    } else if tab.get() == "mine" {
                                                        t("No rooms yet")
                                                    } else if !cat_ids.get().is_empty() {
                                                        t("No rooms in this category right now.")
                                                    } else {
                                                        t("No rooms available at the moment.")
                                                    }}
                                                </p>
                                                {(!cat_ids.get().is_empty()).then(|| view! {
                                                    <button type="button" class="game-link-action mt-3" on:click=move |_| cat_ids.set(vec![])>
                                                        {t("Show all categories")}
                                                    </button>
                                                })}
                                                {(tab.get() == "mine" && logged_in.get()).then(|| view! {
                                                    <A href="/rooms/create" attr:class="game-btn-secondary mt-4 inline-flex">
                                                        {t("Create my first room")}
                                                    </A>
                                                })}
                                            </div>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <div id="catalog-grid" class="grid grid-cols-2 gap-3 sm:gap-4 md:grid-cols-3 xl:grid-cols-4">
                                                <For
                                                    each=move || items.get()
                                                    key=|r| r.id
                                                    children=|r: HomeRoom| view! { <RoomCard room=r /> }
                                                />
                                            </div>
                                            {(page.get() < last_page.get()).then(|| view! {
                                                <div class="flex justify-center py-4">
                                                    <button type="button" class="game-btn-secondary" on:click=move |_| load_more()>
                                                        {t("Show more rooms")}
                                                    </button>
                                                </div>
                                            })}
                                        }.into_any()
                                    }
                                }}
                            </div>
                        </section>
                    </section>

                    <HomeSidebar weekly=weekly featured=featured logged_in=logged_in />
                </div>
            </div>
        </div>
    }
}
