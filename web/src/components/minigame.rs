//! Shared minigame visuals: per-game icon, accent gradient tile and
//! display name. Used by the home tab cards, the /minigames index and
//! the room party panel.

use crate::i18n::t;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

const NOTE_SVG: &str =
    "M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z";
const MIC_SVG: &str = "M12 14c1.66 0 2.99-1.34 2.99-3L15 5c0-1.66-1.34-3-3-3S9 3.34 9 5v6c0 1.66 1.34 3 3 3zm5.3-3c0 3-2.54 5.1-5.3 5.1S6.7 14 6.7 11H5c0 3.41 2.72 6.23 6 6.72V21h2v-3.28c3.28-.48 6-3.3 6-6.72h-1.7z";
const SHUFFLE_SVG: &str = "M10.59 9.17L5.41 4 4 5.41l5.17 5.17 1.42-1.41zM14.5 4l2.04 2.04L4 18.59 5.41 20 17.96 7.46 20 9.5V4h-5.5zm.33 9.41l-1.41 1.41 3.13 3.13L14.5 20H20v-5.5l-2.04 2.04-3.13-3.13z";
const TITLE_SVG: &str = "M5 4v3h5.5v12h3V7H19V4H5z";
const DISC_SVG: &str = "M12 2a10 10 0 100 20 10 10 0 000-20zm0 6a4 4 0 110 8 4 4 0 010-8zm0 3a1 1 0 100 2 1 1 0 000-2z";
const PLAY_SVG: &str = "M8 5v14l11-7z";

pub const MINIGAME_TYPES: [&str; 5] =
    ["quiz", "who_sang", "anagram", "first_letter", "album_cover"];

/// Short display name (i18n key) per game type.
pub fn game_name(ty: &str) -> &'static str {
    match ty {
        "quiz" => "Quiz",
        "who_sang" => "Who sang?",
        "album_cover" => "Album cover",
        "first_letter" => "First letter",
        "anagram" => "Anagram",
        _ => "Mini-game",
    }
}

/// Per-game accent gradient for the catalog icon tiles.
pub fn minigame_tile(ty: &str) -> &'static str {
    match ty {
        "quiz" => "from-teal-400 to-cyan-500",
        "who_sang" => "from-purple-500 to-fuchsia-600",
        "anagram" => "from-yellow-400 to-amber-600",
        "first_letter" => "from-blue-600 to-indigo-800",
        "album_cover" => "from-violet-500 to-purple-800",
        _ => "from-brand-deep to-brand-midnight",
    }
}

/// Per-game icon — real SVGs (OS emojis render inconsistently and looked
/// cheap).
pub fn minigame_icon(ty: &str) -> impl IntoView {
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

/// Audio player for minigame questions (duels, daily, party, live matches).
/// Two behaviors the bare `<audio autoplay>` lacked:
///  - volume persists across questions/games via the shared `volume`
///    localStorage key (same as the room player),
///  - when the autoplay policy rejects playback (no user gesture yet —
///    e.g. a match entered through a websocket push), a tap-to-play
///    overlay surfaces instead of silently staying paused.
#[component]
pub fn MinigameAudio(
    #[prop(into)] src: String,
    #[prop(optional)] on_ended: Option<Callback<()>>,
) -> impl IntoView {
    let node = NodeRef::<leptos::html::Audio>::new();
    let needs_gesture = RwSignal::new(false);

    // Fallback: whatever the block path (autoplay attr denied, deferred
    // metadata on iOS, play() Err), if nothing is audible ~1.2s after
    // mount the gesture button must surface — silence is the bug.
    {
        let node = node.clone();
        gloo_timers::callback::Timeout::new(1_200, move || {
            let paused = node
                .get_untracked()
                .map(|a| {
                    let el: &web_sys::HtmlAudioElement = &a;
                    el.paused()
                })
                .unwrap_or(false);
            if paused {
                let _ = needs_gesture.try_set(true);
            }
        })
        .forget();
    }

    view! {
        <div class="relative w-full">
            <audio
                node_ref=node
                controls
                autoplay
                class="w-full"
                src=src
                on:loadedmetadata=move |ev| {
                    let a: web_sys::HtmlAudioElement =
                        ev.target().unwrap().unchecked_into();
                    a.set_volume(crate::components::player::saved_volume());
                    if let Ok(p) = a.play() {
                        let needs_gesture = needs_gesture;
                        wasm_bindgen_futures::spawn_local(async move {
                            if let Err(e) =
                                wasm_bindgen_futures::JsFuture::from(p).await
                            {
                                let name = js_sys::Reflect::get(&e, &"name".into())
                                    .ok()
                                    .and_then(|n| n.as_string())
                                    .unwrap_or_default();
                                if name == "NotAllowedError" {
                                    needs_gesture.set(true);
                                }
                            }
                        });
                    }
                }
                on:volumechange=move |ev| {
                    let a: web_sys::HtmlAudioElement =
                        ev.target().unwrap().unchecked_into();
                    if let Some(s) = web_sys::window()
                        .and_then(|w| w.local_storage().ok().flatten())
                    {
                        let _ = s.set_item("volume", &a.volume().to_string());
                    }
                }
                on:ended=move |_| {
                    if let Some(cb) = on_ended {
                        cb.run(());
                    }
                }
            />
            <Show when=move || needs_gesture.get()>
                <button
                    type="button"
                    class="absolute inset-0 flex items-center justify-center gap-2 rounded-md bg-neutral-900/95 font-semibold text-white"
                    on:click=move |_| {
                        if let Some(a) = node.get_untracked() {
                            let el: &web_sys::HtmlAudioElement = &a;
                            let _ = el.play();
                        }
                        needs_gesture.set(false);
                    }
                >
                    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="h-5 w-5" aria-hidden="true">
                        <path d="M8 5v14l11-7z"/>
                    </svg>
                    {t("Enable sound")}
                </button>
            </Show>
        </div>
    }
}
