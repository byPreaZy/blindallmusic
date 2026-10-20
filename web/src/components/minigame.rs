//! Shared minigame visuals: per-game icon, accent gradient tile and
//! display name. Used by the home tab cards, the /minigames index and
//! the room party panel.

use leptos::prelude::*;

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
