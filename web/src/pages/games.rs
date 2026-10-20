use crate::api;
use crate::auth::use_auth;
use crate::components::minigame::{game_name, minigame_icon, minigame_tile};
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use serde_json::{json, Value};

const QUESTIONS_PER_ROUND: usize = 5;

const WRONG_MESSAGES: &[&str] = &[
    "Not quite!",
    "Almost!",
    "Good try!",
    "Next time!",
    "Keep going!",
    "No worries!",
    "Close one!",
    "Not this time!",
    "You'll get the next one!",
];

fn game_meta(ty: &str) -> (&'static str, &'static str) {
    match ty {
        "quiz" => ("Quiz à 4 choix", "Guess the track title from 4 choices."),
        "who_sang" => (
            "Qui a chanté ?",
            "Guess who sang this track from 4 choices.",
        ),
        "album_cover" => ("Pochette d'album", "Find the artist from the album cover."),
        "first_letter" => (
            "Première lettre",
            "Listen and complete the title from the first letter of each word.",
        ),
        "anagram" => (
            "Anagramme",
            "Unscramble the letters to find the artist name.",
        ),
        _ => ("Mini-jeu", ""),
    }
}

// ---- index -----------------------------------------------------------------------

#[component]
pub fn GamesIndexPage() -> impl IntoView {
    crate::i18n::set_title_key("Mini-games");
    crate::seo::apply_page("/minigames", t("Mini-games meta description"));
    crate::seo::set_meta("name", "keywords", t("Mini-games meta keywords"));
    let auth = use_auth();
    let games = RwSignal::new(Vec::<Value>::new());
    let posters = RwSignal::new(Vec::<String>::new());
    let loading = RwSignal::new(true);

    // Public catalog — the game list renders for everyone (visitors land
    // here from the navbar; only actually *playing* requires an account).
    leptos::task::spawn_local(async move {
        match api::get::<Value>("/api/v1/minigames").await {
            Ok(v) => {
                games.set(v["games"].as_array().cloned().unwrap_or_default());
                posters.set(
                    v["posters"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|p| p.as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default(),
                );
            }
            Err(_) => {}
        }
        loading.set(false);
    });

    view! {
        <div class="mx-auto max-w-3xl px-4 py-8">
            <A href="/" attr:class="mb-6 inline-flex items-center gap-2 text-sm text-neutral-400 transition hover:text-white">
                <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4 rotate-180" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                    <path stroke-linecap="round" stroke-linejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5"/>
                </svg>
                {t("Back")}
            </A>

            <header class="relative mb-10 overflow-hidden rounded-2xl border-2 border-teal-500/20 bg-gradient-to-br from-teal-600/25 via-neutral-800/90 to-amber-600/15 px-8 py-10 shadow-2xl ring-1 ring-white/5">
                <h1 class="text-3xl font-black uppercase tracking-widest text-white drop-shadow-lg sm:text-4xl">
                    {t("Mini-games")}
                </h1>
                <p class="mt-3 max-w-xl text-neutral-300">
                    {t("Solo games to train your music knowledge.")}
                </p>
            </header>

            <div class="mb-4 rounded-2xl border-2 border-purple-500/30 bg-gradient-to-r from-purple-600/20 to-neutral-800/80 p-5 shadow-xl">
                <p class="text-lg font-black uppercase tracking-wider text-purple-300">{t("Play online")}</p>
                <p class="mb-4 text-sm text-neutral-400">{t("Live matchmaking — face real players right now.")}</p>
                <div class="grid grid-cols-2 gap-2 sm:grid-cols-5">
                    {crate::components::minigame::MINIGAME_TYPES.into_iter().map(|ty| view! {
                        <A href=format!("/match/{}", ty.replace('_', "-"))
                            attr:class="group flex flex-col items-center gap-2 rounded-xl border border-white/10 bg-neutral-900/60 px-2 py-3 text-center transition hover:border-purple-400/60 hover:bg-purple-500/10">
                            <div class=format!("flex h-10 w-10 items-center justify-center rounded-lg bg-gradient-to-br {} text-white", minigame_tile(ty))>
                                {minigame_icon(ty)}
                            </div>
                            <span class="text-[11px] font-bold uppercase tracking-wider text-white/70 group-hover:text-white">
                                {t(game_name(ty))}
                            </span>
                        </A>
                    }).collect::<Vec<_>>()}
                </div>
            </div>
            <A href="/duels" attr:class="group mb-4 flex items-center justify-between rounded-2xl border-2 border-purple-500/30 bg-gradient-to-r from-purple-600/20 to-neutral-800/80 p-5 shadow-xl transition hover:border-purple-400/60">
                <div>
                    <p class="text-lg font-black uppercase tracking-wider text-purple-300">{t("Multiplayer")}</p>
                    <p class="text-sm text-neutral-400">{t("All 5 games vs other players: async duels + the daily challenge.")}</p>
                </div>
                <span class="rounded-full bg-purple-500/20 px-4 py-2 text-sm font-bold text-purple-300 group-hover:bg-purple-500/30">"→"</span>
            </A>
            <div class="mb-6 flex items-center gap-3 rounded-xl border border-teal-500/20 bg-teal-500/5 px-4 py-3 text-sm text-teal-200/80">
                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="h-5 w-5 flex-shrink-0 text-teal-300" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M15 19.128a9.38 9.38 0 002.625.372 9.337 9.337 0 004.121-.952 4.125 4.125 0 00-7.533-2.493M15 19.128v-.003c0-1.113-.285-2.16-.786-3.07M15 19.128v.106A12.318 12.318 0 018.624 21c-2.331 0-4.512-.645-6.374-1.766l-.001-.109a6.375 6.375 0 0111.964-3.07M12 6.375a3.375 3.375 0 11-6.75 0 3.375 3.375 0 016.75 0zm8.25 2.25a2.625 2.625 0 11-5.25 0 2.625 2.625 0 015.25 0z"/></svg>
                <p>
                    {t("Live multiplayer: inside any room, the owner or a moderator can start a real-time party of any of these games for everyone present.")}
                </p>
            </div>

            <Show when=move || auth.ready.get() && auth.user.get().map(|u| u.is_guest).unwrap_or(true) && !loading.get()>
                <div class="mb-6 rounded-xl border border-amber-400/30 bg-amber-400/10 px-4 py-3 text-sm text-amber-200">
                    {t("Log in to save your scores.")}
                </div>
            </Show>

            <ul class="flex flex-col gap-5">
                {["quiz", "who_sang", "anagram", "first_letter", "album_cover"].into_iter().enumerate().map(|(i, ty)| {
                    let ty_s = ty.to_string();
                    let (name, desc) = game_meta(ty);
                    view! {
                        <li>
                            <A href=format!("/minigames/{}", ty.replace('_', "-"))
                                attr:class="group relative block overflow-hidden rounded-2xl border-2 border-neutral-700/60 bg-neutral-800/70 shadow-xl ring-1 ring-neutral-600/30 transition-all duration-200 hover:-translate-y-1 hover:border-teal-500/50 hover:ring-teal-500/30">
                                <div class="pointer-events-none absolute inset-y-0 right-0 grid w-1/3 grid-cols-3 opacity-15" aria-hidden="true">
                                    {move || {
                                        let all = posters.get();
                                        if all.is_empty() { return Vec::new(); }
                                        (0..3).map(|k| view! {
                                            <img src=all[(i * 3 + k) % all.len()].clone() class="h-full w-full object-cover" alt="" loading="lazy"/>
                                        }).collect()
                                    }}
                                </div>
                                <div class="relative flex flex-row items-center gap-5 p-6">
                                    <div class=format!("flex h-20 w-20 flex-shrink-0 items-center justify-center rounded-xl bg-gradient-to-br {} text-white shadow-lg" , minigame_tile(ty)) aria-hidden="true">
                                        {minigame_icon(ty)}
                                    </div>
                                    <div class="min-w-0 flex-1">
                                        <h2 class="text-xl font-black text-neutral-100 transition-colors group-hover:text-teal-400">
                                            {t(name)}
                                        </h2>
                                        <p class="mt-1 text-sm text-neutral-400">{t(desc)}</p>
                                        {move || {
                                            let score = games.get().iter()
                                                .find(|g| g["type"].as_str() == Some(ty_s.as_str()))
                                                .and_then(|g| g["score"].as_i64())
                                                .unwrap_or(0);
                                            (score > 0).then(|| view! {
                                                <p class="mt-2 inline-flex items-center rounded-full border border-teal-500/40 bg-teal-500/20 px-3 py-1 text-sm font-bold text-teal-400">
                                                    {t("Score")}": "{score}" "{t("points")}
                                                </p>
                                            })
                                        }}
                                    </div>
                                    <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6 flex-shrink-0 text-neutral-500 transition-colors group-hover:text-teal-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M8.25 4.5l7.5 7.5-7.5 7.5"/>
                                    </svg>
                                </div>
                            </A>
                        </li>
                    }
                }).collect::<Vec<_>>()}
            </ul>
        </div>
    }
}

// ---- play -------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct Question {
    track_id: Option<i64>,
    preview_url: Option<String>,
    artwork_url: Option<String>,
    choices: Vec<String>,
    question_id: Option<String>,
    scrambled_artist: Option<String>,
    correct_value: Option<String>,
}

#[derive(Clone, Debug)]
struct RoundResult {
    artwork_url: Option<String>,
    correct_value: Option<String>,
    chosen_value: Option<String>,
    correct: bool,
}

#[component]
pub fn GamePlayPage() -> impl IntoView {
    let params = use_params_map();
    let auth = use_auth();
    let game = move || {
        params
            .get()
            .get("type")
            .unwrap_or_default()
            .replace('-', "_")
    };
    let questions = RwSignal::new(Vec::<Question>::new());
    let index = RwSignal::new(0usize);
    let result = RwSignal::new(Option::<Value>::None);
    let results = RwSignal::new(Vec::<RoundResult>::new());
    let session_score = RwSignal::new(0i64);
    let checking = RwSignal::new(false);
    let time_up = RwSignal::new(false);
    let wrong_msg = RwSignal::new(String::new());
    let input = RwSignal::new(String::new());
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);
    let summary = RwSignal::new(false);
    let game_lb = RwSignal::new(Vec::<Value>::new());

    let game_type = RwSignal::new(game());
    let (name, _desc) = game_meta(&game_type.get_untracked());
    let _ = name;
    let (title_key, desc_key) = match game_type.get_untracked().as_str() {
        "quiz" => ("Quiz : 4 choices", "Quiz 4 choices meta description"),
        "who_sang" => ("Who sang?", "Who sang meta description"),
        "anagram" => ("Anagram", "Anagram meta description"),
        "first_letter" => ("First letter", "First letter meta description"),
        "album_cover" => ("Album cover", "Album cover meta description"),
        _ => ("Mini-games", "Mini-games meta description"),
    };
    crate::i18n::set_title_key(title_key);
    crate::seo::set_meta("name", "description", t(desc_key));
    let valid_type = ["quiz", "who_sang", "album_cover", "first_letter", "anagram"]
        .contains(&game_type.get_untracked().as_str());

    let pick_wrong = move || {
        let n = (js_sys::Math::random() * WRONG_MESSAGES.len() as f64) as usize;
        WRONG_MESSAGES[n.min(WRONG_MESSAGES.len() - 1)].to_string()
    };

    let preload = move || {
        let g = game_type.get_untracked();
        loading.set(true);
        error.set(None);
        questions.set(vec![]);
        results.set(vec![]);
        session_score.set(0);
        summary.set(false);
        index.set(0);
        result.set(None);
        input.set(String::new());
        leptos::task::spawn_local(async move {
            let mut qs = Vec::with_capacity(QUESTIONS_PER_ROUND);
            for _ in 0..QUESTIONS_PER_ROUND {
                match api::post_empty::<Value>(&format!("/api/v1/minigames/{g}/next")).await {
                    Ok(v) => qs.push(Question {
                        track_id: v["track"]["id"].as_i64(),
                        preview_url: v["track"]["preview_url"].as_str().map(str::to_string),
                        artwork_url: v["track"]["artwork_url"].as_str().map(str::to_string),
                        choices: v["choices"]
                            .as_array()
                            .cloned()
                            .unwrap_or_default()
                            .iter()
                            .filter_map(|c| c.as_str().map(str::to_string))
                            .collect(),
                        question_id: v["question_id"].as_str().map(str::to_string),
                        scrambled_artist: v["scrambled_artist"].as_str().map(str::to_string),
                        correct_value: v["correct_value"].as_str().map(str::to_string),
                    }),
                    Err(e) => {
                        error.set(Some(e.message));
                        break;
                    }
                }
            }
            questions.set(qs);
            loading.set(false);
        });
    };
    if valid_type {
        crate::auth::on_member(auth, preload);
    }

    let finish_wrong = move |correct_value: Option<String>| {
        wrong_msg.set(pick_wrong());
        result.set(Some(
            json!({"correct": false, "correct_value": correct_value, "points": 0}),
        ));
        results.update(|r| {
            r.push(RoundResult {
                artwork_url: current(questions, index).and_then(|q| q.artwork_url.clone()),
                correct_value,
                chosen_value: None,
                correct: false,
            })
        });
    };

    let submit_choice = move |chosen: String| {
        if checking.get() || result.get().is_some() {
            return;
        }
        let g = game_type.get_untracked();
        let Some(q) = current(questions, index) else {
            return;
        };
        checking.set(true);
        leptos::task::spawn_local(async move {
            let body = json!({
                "track_id": q.track_id,
                "question_id": q.question_id,
                "chosen_value": chosen,
            });
            let data = api::post::<Value, _>(&format!("/api/v1/minigames/{g}/check"), &body)
                .await
                .unwrap_or_else(|_| json!({"correct": false, "points": 0}));
            let correct = data["correct"].as_bool().unwrap_or(false);
            if !correct {
                wrong_msg.set(pick_wrong());
            } else {
                session_score.update(|s| *s += data["points"].as_i64().unwrap_or(0));
            }
            results.update(|r| {
                r.push(RoundResult {
                    artwork_url: q.artwork_url.clone(),
                    correct_value: data["correct_value"].as_str().map(str::to_string),
                    chosen_value: Some(chosen.clone()),
                    correct,
                })
            });
            result.set(Some(data));
            checking.set(false);
        });
    };

    let next_question = move || {
        result.set(None);
        time_up.set(false);
        input.set(String::new());
        if results.get().len() >= QUESTIONS_PER_ROUND {
            summary.set(true);
            let g = game_type.get_untracked();
            leptos::task::spawn_local(async move {
                if let Ok(v) =
                    api::get::<Value>(&format!("/api/v1/minigames/{g}/leaderboard")).await
                {
                    game_lb.set(v["leaderboard"].as_array().cloned().unwrap_or_default());
                }
            });
        } else {
            index.update(|i| *i += 1);
        }
    };

    let on_ended = move |_| {
        if result.get().is_none() {
            time_up.set(true);
            let cv = current(questions, index).and_then(|q| q.correct_value.clone());
            finish_wrong(cv);
        }
    };

    let hint_text = move || {
        let cv = current(questions, index)
            .and_then(|q| q.correct_value)
            .unwrap_or_default();
        cv.trim()
            .split_whitespace()
            .map(|w| {
                let mut chars = w.chars();
                let first = chars.next().map(|c| c.to_string()).unwrap_or_default();
                format!("{}{}", first, "_".repeat(chars.count()))
            })
            .collect::<Vec<_>>()
            .join(" ")
    };

    view! {
        <crate::components::member_gate::MemberGate title="Mini-games">
        <div class="mx-auto max-w-2xl px-4 py-8">
            <div class="mb-6 flex items-center justify-between">
                <A href="/minigames" attr:class="text-sm text-white/60 hover:text-white">
                    "← " {t("Back")}
                </A>
                <h1 class="retro-title retro-title--white text-xl">{t(name)}</h1>
                <span class="rounded-full border border-teal-500/40 bg-teal-500/20 px-3 py-1 text-sm font-bold text-teal-400">
                    {move || session_score.get()} " pts"
                </span>
            </div>

            <Show when=move || !valid_type>
                <div class="py-24 text-center text-white/60">{t("Game not found")}</div>
            </Show>
            <Show when=move || valid_type && loading.get()>
                <div class="py-24 text-center text-white/60">{t("Loading")}"…"</div>
            </Show>
            {move || error.get().map(|e| view! {
                <div class="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">{e}</div>
            })}

            <Show when=move || summary.get()>
                <div class="rounded-2xl border-2 border-teal-500/20 bg-neutral-900/80 p-6">
                    <h2 class="mb-2 text-center text-2xl font-black text-white">{t("Round over!")}</h2>
                    <p class="mb-6 text-center text-lg text-teal-400">
                        {move || session_score.get()} " / " {QUESTIONS_PER_ROUND} " " {t("points")}
                    </p>
                    <div class="mb-6 space-y-2">
                        <For
                            each=move || results.get()
                            key=|r| r.artwork_url.clone().unwrap_or_default() + &r.correct_value.clone().unwrap_or_default()
                            children=move |r: RoundResult| view! {
                                <div class="flex items-center gap-3 rounded-lg border border-white/10 px-3 py-2">
                                    {r.artwork_url.clone().map(|a| view! { <img src=a class="h-8 w-8 rounded" alt=""/> })}
                                    <span class=move || if r.correct { "text-teal-400" } else { "text-amber-500" }>
                                        {if r.correct { "✓" } else { "✗" }}
                                    </span>
                                    <span class="min-w-0 flex-1 truncate text-sm text-white/80">
                                        {r.correct_value.clone().unwrap_or_else(|| "—".into())}
                                    </span>
                                    {(!r.correct).then(|| view! {
                                        <span class="truncate text-xs text-white/40">
                                            {r.chosen_value.clone().map(|c| format!("({c})")).unwrap_or_else(|| t("Time's up!").to_string())}
                                        </span>
                                    })}
                                </div>
                            }
                        />
                    </div>
                    <div class="flex gap-3">
                        <button type="button" class="btn-primary flex-1" on:click=move |_| preload()>
                            {t("Play again")}
                        </button>
                        <A href="/minigames" attr:class="rounded-lg border border-white/10 px-4 py-2 text-white/70">
                            {t("Back")}
                        </A>
                    </div>
                    <div class="mt-4 grid gap-2 sm:grid-cols-2">
                        <A href=format!("/match/{}", game_type.get_untracked().replace('_', "-")) attr:class="group flex items-center justify-between rounded-xl border border-purple-500/30 bg-purple-600/15 px-4 py-3 transition hover:border-purple-400/60">
                            <span class="text-sm font-bold text-purple-300">{t("Play online")}</span>
                            <span class="text-purple-300 transition group-hover:translate-x-0.5">"→"</span>
                        </A>
                        <A href=format!("/duels?type={}", game_type.get_untracked()) attr:class="group flex items-center justify-between rounded-xl border border-purple-500/30 bg-purple-600/10 px-4 py-3 transition hover:border-purple-400/60">
                            <span class="text-sm font-bold text-purple-300">{t("Play this game against a friend")}</span>
                            <span class="text-purple-300 transition group-hover:translate-x-0.5">"→"</span>
                        </A>
                    </div>
                    {move || (!game_lb.get().is_empty()).then(|| view! {
                        <div class="mt-4">
                            <h3 class="mb-2 text-sm font-bold text-white/60">{t("Leaderboard")}</h3>
                            <div class="space-y-1">
                                {move || game_lb.get().iter().take(5).enumerate().map(|(i, r)| {
                                    let medal = ["🥇", "🥈", "🥉"].get(i).copied().unwrap_or("");
                                    view! {
                                        <div class="flex items-center gap-2 rounded-lg px-2 py-1 text-sm">
                                            <span class="w-6 text-center">{if medal.is_empty() { format!("{}", i + 1) } else { medal.to_string() }}</span>
                                            <span class="min-w-0 flex-1 truncate text-white/80">{r["name"].as_str().unwrap_or("—").to_string()}</span>
                                            <span class="font-bold text-teal-400">{r["total"].as_i64().unwrap_or(0)} {t("pts")}</span>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        </div>
                    })}
                </div>
            </Show>

            <Show when=move || valid_type && !loading.get() && !summary.get() && current(questions, index).is_some()>
                <div class="overflow-hidden rounded-2xl border-2 border-teal-500/20 bg-neutral-900/80 shadow-2xl">
                    <div class="flex flex-col gap-6 p-6">
                        <div class="flex items-center justify-between text-xs text-white/50">
                            <span>{move || format!("{} {}/{}", t("Question"), index.get() + 1, QUESTIONS_PER_ROUND)}</span>
                            <div class="flex gap-1">
                                {(0..QUESTIONS_PER_ROUND).map(|i| view! {
                                    <span class=move || {
                                        if i < results.get().len() { "h-2 w-2 rounded-full bg-teal-400" }
                                        else if i == index.get() { "h-2 w-2 rounded-full bg-white" }
                                        else { "h-2 w-2 rounded-full bg-white/20" }
                                    }/>
                                }).collect::<Vec<_>>()}
                            </div>
                        </div>

                        {move || {
                            let Some(q) = current(questions, index) else {
                                return view! { <div/> }.into_any();
                            };
                            view! {
                                <div class="flex flex-col items-center gap-4">
                                    {(game() == "album_cover").then(|| {
                                        // PHP parity: cover is the puzzle — blurred or
                                        // zoomed until the answer, never readable.
                                        let cls = match index.get() % 3 {
                                            0 => "h-32 w-32 rounded-xl object-cover shadow-lg blur-md scale-110",
                                            1 => "h-32 w-32 rounded-xl object-cover shadow-lg scale-[2.2]",
                                            _ => "h-32 w-32 rounded-xl object-cover shadow-lg blur-md scale-[2.2]",
                                        };
                                        q.artwork_url.clone().map(|a| view! {
                                            <div class="overflow-hidden rounded-xl"><img src=a class=cls alt=""/></div>
                                        })
                                    })}
                                    {q.preview_url.clone().map(|url| view! {
                                        <audio controls autoplay class="w-full" src=url
                                            on:ended=on_ended />
                                    })}
                                    {q.scrambled_artist.clone().map(|s| view! {
                                        <div class="text-center">
                                            <p class="text-sm uppercase tracking-wider text-white/50">{t("Unscramble the artist")}</p>
                                            <p class="mt-2 font-mono text-2xl font-black tracking-[0.3em] text-teal-400">{s}</p>
                                        </div>
                                    })}
                                </div>
                            }.into_any()
                        }}

                        {move || {
                            if let Some(res) = result.get() {
                                let correct = res["correct"].as_bool().unwrap_or(false);
                                view! {
                                    <div class="space-y-4">
                                        <Show when=move || time_up.get()>
                                            <div class="rounded-xl border-2 border-amber-500/40 bg-amber-500/10 px-4 py-3 text-center font-bold uppercase tracking-wider text-amber-400">
                                                {t("Time's up!")}
                                            </div>
                                        </Show>
                                        <p class=move || if correct { "text-center text-lg font-bold text-teal-400" } else { "text-center text-lg font-bold text-amber-500" }>
                                            {if correct {
                                                t("Correct!").to_string()
                                            } else {
                                                t(&wrong_msg.get()).to_string()
                                            }}
                                            {res["points"].as_i64().filter(|p| *p > 0).map(|p| format!(" +{p} pts")).unwrap_or_default()}
                                        </p>
                                        {(!correct).then(|| view! {
                                            <p class="text-center text-white/60">
                                                {t("Correct answer")}": "
                                                <span class="font-semibold text-white">
                                                    {res["correct_value"].as_str().unwrap_or("").to_string()}
                                                </span>
                                            </p>
                                        })}
                                        <button type="button" class="btn-primary w-full"
                                            on:click=move |_| next_question()>
                                            {move || if index.get() >= QUESTIONS_PER_ROUND - 1 { t("See results") } else { t("Next question") }}
                                        </button>
                                    </div>
                                }.into_any()
                            } else {
                                let g = game_type.get();
                                match current(questions, index) {
                                    Some(q) if !q.choices.is_empty() => view! {
                                        <div class="grid gap-3 sm:grid-cols-2">
                                            {q.choices.iter().map(|c| {
                                                let c2 = c.clone();
                                                view! {
                                                    <button type="button"
                                                        class="rounded-xl border-2 border-neutral-600 bg-neutral-800/80 px-4 py-3.5 font-semibold text-white transition hover:border-teal-500 disabled:opacity-50"
                                                        disabled=move || checking.get()
                                                        on:click=move |_| submit_choice(c2.clone())>
                                                        {c.clone()}
                                                    </button>
                                                }
                                            }).collect::<Vec<_>>()}
                                        </div>
                                    }.into_any(),
                                    Some(_) => view! {
                                        <div class="flex flex-col gap-3">
                                            {if g == "first_letter" {
                                                view! {
                                                    <div class="text-center">
                                                        <p class="text-sm font-bold uppercase tracking-wider text-white/50">
                                                            {t("Complete the title (first letter of each word)")}
                                                        </p>
                                                        <p class="mt-2 font-mono text-xl font-bold tracking-widest text-teal-400">
                                                            {hint_text()}
                                                        </p>
                                                    </div>
                                                }.into_any()
                                            } else { view! { <div/> }.into_any() }}
                                            <form class="flex flex-col gap-3" on:submit=move |ev| {
                                                ev.prevent_default();
                                                submit_choice(input.get_untracked());
                                            }>
                                                <input type="text"
                                                    class="w-full rounded-xl border-2 border-neutral-600 bg-neutral-800/80 px-4 py-3.5 font-semibold text-white outline-none focus:border-teal-500"
                                                    placeholder={t("Your answer…")}
                                                    prop:value=input
                                                    on:input=move |ev| input.set(event_target_value(&ev))/>
                                                <button type="submit" class="btn-primary w-full"
                                                    disabled=move || checking.get() || input.get().trim().is_empty()>
                                                    {t("Validate")}
                                                </button>
                                            </form>
                                        </div>
                                    }.into_any(),
                                    None => view! { <div/> }.into_any(),
                                }
                            }
                        }}
                    </div>
                </div>
            </Show>
        </div>
        </crate::components::member_gate::MemberGate>
    }
}

fn current(questions: RwSignal<Vec<Question>>, index: RwSignal<usize>) -> Option<Question> {
    questions.get().get(index.get()).cloned()
}
