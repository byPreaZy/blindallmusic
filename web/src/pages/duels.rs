use crate::api;
use crate::auth::use_auth;
use crate::components::minigame::game_name;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::{use_params_map, use_query_map};
use serde_json::{json, Value};

/// Wordle-style 🟩🟥 grid from a player's stored answers ({idx, correct}).
fn answers_grid(answers: &[Value]) -> String {
    let mut v: Vec<(i64, bool)> = answers
        .iter()
        .map(|a| {
            (
                a["idx"].as_i64().unwrap_or(0),
                a["correct"].as_bool().unwrap_or(false),
            )
        })
        .collect();
    v.sort_by_key(|(i, _)| *i);
    v.iter()
        .map(|(_, c)| if *c { "🟩" } else { "🟥" })
        .collect()
}

fn copy_clipboard(text: String, copied: RwSignal<bool>) {
    if let Some(w) = web_sys::window() {
        let _ = w.navigator().clipboard().write_text(&text);
    }
    copied.set(true);
    gloo_timers::callback::Timeout::new(1_500, move || {
        let _ = copied.try_set(false);
    })
    .forget();
}

fn site_origin() -> String {
    web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default()
}

// ---- question card (shared by duel + daily) -----------------------------------

#[component]
pub fn QuestionView(
    q: Value,
    #[prop(into)] on_answer: Callback<String>,
    #[prop(default = false)] disabled: bool,
    #[prop(into, default = String::new())] game_type: String,
) -> impl IntoView {
    let input = RwSignal::new(String::new());
    let choices: Vec<String> = q["choices"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|c| c.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    // Cover art only for the album_cover game (and it stays blurred/zoomed —
    // it IS the puzzle). Elsewhere it can spoil the artist/title printed on it.
    let artwork = (game_type == "album_cover")
        .then(|| q["track"]["artwork_url"].as_str().map(str::to_string))
        .flatten();
    let preview = q["track"]["preview_url"].as_str().map(str::to_string);
    let scrambled = q["scrambled_artist"].as_str().map(str::to_string);
    let hint = q["hint"].as_str().map(str::to_string);
    let audio_ended = RwSignal::new(false);

    view! {
        <div class="flex flex-col gap-5">
            <div class="flex flex-col items-center gap-4">
                {artwork.map(|a| view! {
                    <div class="overflow-hidden rounded-xl">
                        <img src=a class="h-32 w-32 rounded-xl object-cover blur-md scale-110 shadow-lg" alt=""/>
                    </div>
                })}
                {preview.map(|url| view! {
                    <audio controls autoplay class="w-full" src=url
                        on:ended=move |_| audio_ended.set(true)/>
                })}
                {scrambled.map(|s| view! {
                    <div class="text-center">
                        <p class="text-sm uppercase tracking-wider text-white/50">{t("Unscramble the artist")}</p>
                        <p class="mt-2 font-mono text-2xl font-black tracking-[0.3em] text-teal-400">{s}</p>
                    </div>
                })}
                {hint.map(|h| view! {
                    <div class="text-center">
                        <p class="text-sm font-bold uppercase tracking-wider text-white/50">
                            {t("Complete the title (first letter of each word)")}
                        </p>
                        <p class="mt-2 font-mono text-xl font-bold tracking-widest text-teal-400">{h}</p>
                    </div>
                })}
            </div>
            {if !choices.is_empty() {
                view! {
                    <div class="grid gap-3 sm:grid-cols-2">
                        {choices.iter().map(|c| {
                            let c2 = c.clone();
                            view! {
                                <button type="button"
                                    class="rounded-xl border-2 border-neutral-600 bg-neutral-800/80 px-4 py-3.5 font-semibold text-white transition hover:border-teal-500 disabled:opacity-50"
                                    disabled=move || disabled
                                    on:click=move |_| on_answer.run(c2.clone())>
                                    {c.clone()}
                                </button>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                }.into_any()
            } else {
                view! {
                    <form class="flex flex-col gap-3" on:submit=move |ev| {
                        ev.prevent_default();
                        on_answer.run(input.get_untracked());
                    }>
                        <input type="text"
                            class="w-full rounded-xl border-2 border-neutral-600 bg-neutral-800/80 px-4 py-3.5 font-semibold text-white outline-none focus:border-teal-500"
                            placeholder={t("Your answer…")}
                            prop:value=input
                            on:input=move |ev| input.set(event_target_value(&ev))/>
                        <button type="submit" class="btn-primary w-full"
                            disabled=move || disabled || input.get().trim().is_empty()>
                            {t("Validate")}
                        </button>
                    </form>
                }.into_any()
            }}
        </div>
    }
}

// ---- duels lobby ---------------------------------------------------------------

#[component]
pub fn DuelsPage() -> impl IntoView {
    crate::i18n::set_title_key("Duels");
    let auth = use_auth();
    let duels = RwSignal::new(Vec::<Value>::new());
    let leaderboard = RwSignal::new(Vec::<Value>::new());
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);

    // create form — ?type= pre-selects the game (linked from each game page)
    let initial_type = use_query_map()
        .get_untracked()
        .get("type")
        .unwrap_or_default()
        .replace('-', "_");
    let new_type = RwSignal::new(
        if crate::components::minigame::MINIGAME_TYPES.contains(&initial_type.as_str()) {
            initial_type
        } else {
            "quiz".to_string()
        },
    );
    let opponent_name = RwSignal::new(String::new());
    let suggestions = RwSignal::new(Vec::<Value>::new());
    let creating = RwSignal::new(false);

    let reload = move |silent: bool| {
        if !crate::auth::is_member(auth) {
            return;
        }
        leptos::task::spawn_local(async move {
            match api::get::<Value>("/api/v1/duels").await {
                Ok(v) => duels.set(v["duels"].as_array().cloned().unwrap_or_default()),
                Err(e) if !silent => error.set(Some(e.message)),
                _ => {}
            }
            match api::get::<Value>("/api/v1/duels/leaderboard").await {
                Ok(v) => leaderboard.set(v["leaderboard"].as_array().cloned().unwrap_or_default()),
                Err(_) => {}
            }
            loading.set(false);
        });
    };
    crate::auth::on_member(auth, move || reload(false));
    // Keep the lobby fresh: new challenges / completed duels appear without reload.
    {
        let iv = send_wrapper::SendWrapper::new(gloo_timers::callback::Interval::new(
            15_000,
            move || reload(true),
        ));
        on_cleanup(move || drop(iv));
    }

    let search_users = move |ev| {
        let v = event_target_value(&ev);
        opponent_name.set(v.clone());
        if v.len() >= 2 {
            leptos::task::spawn_local(async move {
                if let Ok(r) = api::get::<Value>(&format!("/api/v1/users?search={v}")).await {
                    suggestions.set(r["users"]["data"].as_array().cloned().unwrap_or_default());
                }
            });
        }
    };

    let create_duel = move |opponent: Option<i64>| {
        creating.set(true);
        error.set(None);
        let gt = new_type.get_untracked();
        leptos::task::spawn_local(async move {
            let body = json!({"game_type": gt, "opponent_id": opponent, "count": 10});
            match api::post::<Value, _>("/api/v1/duels", &body).await {
                Ok(v) => {
                    if let Some(id) = v["id"].as_i64() {
                        leptos_router::hooks::use_navigate()(
                            &format!("/duels/{id}"),
                            Default::default(),
                        );
                    }
                }
                Err(e) => error.set(Some(e.message)),
            }
            creating.set(false);
        });
    };

    let accept = move |id: i64| {
        leptos::task::spawn_local(async move {
            let _ = api::post_empty::<Value>(&format!("/api/v1/duels/{id}/accept")).await;
            leptos_router::hooks::use_navigate()(&format!("/duels/{id}"), Default::default());
        });
    };

    let my_id = move || auth.user.get().map(|u| u.id).unwrap_or(0);

    view! {
        <crate::components::member_gate::MemberGate title="Duels">
        <div class="mx-auto max-w-3xl px-4 py-8">
            <A href="/minigames" attr:class="mb-6 inline-flex items-center gap-2 text-sm text-neutral-400 transition hover:text-white">
                "← " {t("Back")}
            </A>

            <header class="relative mb-8 overflow-hidden rounded-2xl border-2 border-purple-500/20 bg-gradient-to-br from-purple-600/25 via-neutral-800/90 to-teal-600/15 px-8 py-8 shadow-2xl">
                <h1 class="text-3xl font-black uppercase tracking-widest text-white">{t("Duels")}</h1>
                <p class="mt-2 max-w-xl text-neutral-300">
                    {t("Challenge other players asynchronously: same questions, play whenever you want.")}
                </p>
            </header>

            // daily challenge banner
            <A href="/daily" attr:class="group mb-8 flex items-center justify-between rounded-2xl border-2 border-amber-500/30 bg-gradient-to-r from-amber-600/20 to-neutral-800/80 p-5 shadow-xl transition hover:border-amber-400/60">
                <div>
                    <p class="text-lg font-black uppercase tracking-wider text-amber-300">{t("Daily challenge")}</p>
                    <p class="text-sm text-neutral-400">{t("Same 10 questions for everyone. Today's leaderboard is live.")}</p>
                </div>
                <span class="rounded-full bg-amber-500/20 px-4 py-2 text-sm font-bold text-amber-300 group-hover:bg-amber-500/30">"→"</span>
            </A>

            // create duel
            <div class="mb-8 rounded-2xl border-2 border-neutral-700/60 bg-neutral-800/70 p-6">
                <h2 class="mb-4 text-lg font-black text-white">{t("New duel")}</h2>
                <div class="flex flex-col gap-4 sm:flex-row">
                    <select class="rounded-lg border border-white/10 bg-neutral-900 px-4 py-2.5 text-sm text-white"
                        prop:value=new_type
                        on:change=move |ev| new_type.set(event_target_value(&ev))>
                        {crate::components::minigame::MINIGAME_TYPES.into_iter().map(|ty| view! {
                            <option value=ty>{t(game_name(ty))}</option>
                        }).collect::<Vec<_>>()}
                    </select>
                    <div class="relative flex-1">
                        <input type="text" list="duel-opponents"
                            class="w-full rounded-lg border border-white/10 bg-neutral-900 px-4 py-2.5 text-sm text-white"
                            placeholder={t("Opponent (optional — leave empty for open challenge)")}
                            prop:value=opponent_name
                            on:input=search_users/>
                        <datalist id="duel-opponents">
                            {move || suggestions.get().iter().map(|u| view! {
                                <option value=u["name"].as_str().unwrap_or("").to_string()/>
                            }).collect::<Vec<_>>()}
                        </datalist>
                    </div>
                    <button type="button" class="btn-primary whitespace-nowrap"
                        disabled=move || creating.get()
                        on:click=move |_| {
                            let name = opponent_name.get_untracked();
                            let opp = suggestions.get().iter()
                                .find(|u| u["name"].as_str() == Some(name.as_str()))
                                .and_then(|u| u["id"].as_i64());
                            create_duel(opp);
                        }>
                        {t("Create duel")}
                    </button>
                </div>
                {move || error.get().map(|e| view! {
                    <p class="mt-3 text-sm text-red-400">{e}</p>
                })}
            </div>

            // duel list
            <Show when=move || loading.get()>
                <div class="py-12 text-center text-white/50">{t("Loading")}"…"</div>
            </Show>
            <Show when=move || !loading.get() && duels.get().is_empty()>
                <p class="py-8 text-center text-white/50">{t("No duels yet — create the first one!")}</p>
            </Show>
            <ul class="flex flex-col gap-3">
                <For
                    each=move || duels.get()
                    key=|d| d["id"].as_i64().unwrap_or(0)
                    children=move |d: Value| {
                        let id = d["id"].as_i64().unwrap_or_default();
                        let status = d["status"].as_str().unwrap_or("").to_string();
                        let mine = d["creator_id"].as_i64() == Some(my_id())
                            || d["opponent_id"].as_i64() == Some(my_id());
                        let open_for_me = status == "open" && d["creator_id"].as_i64() != Some(my_id());
                        let vs = format!("{} vs {}",
                            d["creator_name"].as_str().unwrap_or("?"),
                            d["opponent_name"].as_str().unwrap_or("…"));
                        let score = format!("{} - {}",
                            d["creator_score"].as_i64().unwrap_or(0),
                            d["opponent_score"].as_i64().unwrap_or(0));
                        let status_badge = match status.as_str() {
                            "open" => ("border-sky-500/40 bg-sky-500/15 text-sky-300", t("Open")),
                            "active" => ("border-amber-500/40 bg-amber-500/15 text-amber-300", t("In progress")),
                            _ => ("border-neutral-500/40 bg-neutral-500/15 text-neutral-300", t("Finished")),
                        };
                        view! {
                            <li class="flex items-center gap-4 rounded-xl border border-white/10 bg-neutral-800/60 p-4">
                                <div class="min-w-0 flex-1">
                                    <div class="flex flex-wrap items-center gap-2">
                                        <span class="font-bold text-white">{vs}</span>
                                        <span class=format!("rounded-full border px-2 py-0.5 text-[10px] font-bold uppercase {}", status_badge.0)>
                                            {status_badge.1}
                                        </span>
                                        <span class="text-xs text-neutral-400">{t(game_name(d["game_type"].as_str().unwrap_or("")))}</span>
                                    </div>
                                    <p class="mt-1 text-sm text-neutral-400">
                                        {t("Score")}": "{score}" · "{d["question_count"].as_i64().unwrap_or(0)}" "{t("questions")}
                                    </p>
                                </div>
                                {if open_for_me {
                                    view! {
                                        <button type="button" class="btn-primary text-sm"
                                            on:click=move |_| accept(id)>{t("Accept")}</button>
                                    }.into_any()
                                } else if mine && status != "finished" {
                                    view! {
                                        <A href=format!("/duels/{id}") attr:class="btn-primary text-sm">{t("Play")}</A>
                                    }.into_any()
                                } else if mine {
                                    view! {
                                        <A href=format!("/duels/{id}") attr:class="rounded-lg border border-white/10 px-3 py-2 text-sm text-white/70">{t("Results")}</A>
                                    }.into_any()
                                } else {
                                    view! { <div/> }.into_any()
                                }}
                            </li>
                        }
                    }
                />
            </ul>

            // wins leaderboard
            <Show when=move || !leaderboard.get().is_empty()>
                <div class="mt-10 rounded-2xl border border-white/10 bg-neutral-800/50 p-6">
                    <h2 class="mb-4 text-lg font-black text-white">{t("Duel leaderboard")}</h2>
                    <ul class="space-y-2">
                        {move || leaderboard.get().iter().enumerate().map(|(i, r)| {
                            view! {
                                <li class="flex items-center gap-3 text-sm">
                                    <span class="w-6 text-right font-bold text-neutral-500">{i + 1}</span>
                                    <span class="flex-1 font-semibold text-white">{r["name"].as_str().unwrap_or("").to_string()}</span>
                                    <span class="text-teal-400">{r["wins"].as_i64().unwrap_or(0)}" "{t("wins")}</span>
                                    {r["draws"].as_i64().filter(|d| *d > 0).map(|d| view! {
                                        <span class="text-neutral-500">{d}" "{t("draws")}</span>
                                    })}
                                </li>
                            }
                        }).collect::<Vec<_>>()}
                    </ul>
                </div>
            </Show>
        </div>
        </crate::components::member_gate::MemberGate>
    }
}

// ---- duel play -----------------------------------------------------------------

#[component]
pub fn DuelPlayPage() -> impl IntoView {
    let params = use_params_map();
    let auth = use_auth();
    let duel_id = move || params.get().get("id").unwrap_or_default();
    let duel = RwSignal::new(Option::<Value>::None);
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);
    let idx = RwSignal::new(0usize);
    let result = RwSignal::new(Option::<Value>::None);
    let checking = RwSignal::new(false);
    let wrong = RwSignal::new(false);
    let copied = RwSignal::new(false);

    let fetch_duel = move |advance: bool| {
        leptos::task::spawn_local(async move {
            match api::get::<Value>(&format!("/api/v1/duels/{}", duel_id())).await {
                Ok(v) => {
                    if advance {
                        let answered: std::collections::HashSet<i64> = v["my_answers"]
                            .as_array()
                            .map(|a| a.iter().filter_map(|x| x["idx"].as_i64()).collect())
                            .unwrap_or_default();
                        let total = v["questions"].as_array().map(|q| q.len()).unwrap_or(0);
                        let next = (0..total as i64)
                            .find(|i| !answered.contains(i))
                            .unwrap_or(total as i64);
                        idx.set(next as usize);
                    }
                    duel.set(Some(v));
                }
                Err(e) => error.set(Some(e.message)),
            }
            loading.set(false);
        });
    };
    crate::auth::on_member(auth, move || fetch_duel(true));
    // While waiting for the opponent (my side done), poll so the final
    // result flips on its own instead of needing a manual reload.
    {
        let iv = send_wrapper::SendWrapper::new(gloo_timers::callback::Interval::new(
            4_000,
            move || {
                let waiting = duel.get_untracked().is_some_and(|d| {
                    d["status"].as_str() != Some("finished")
                        && idx.get_untracked() >= d["question_count"].as_i64().unwrap_or(0) as usize
                });
                if waiting {
                    fetch_duel(false);
                }
            },
        ));
        on_cleanup(move || drop(iv));
    }

    let submit = move |value: String| {
        if checking.get() || result.get().is_some() {
            return;
        }
        let id = duel_id();
        let i = idx.get();
        checking.set(true);
        leptos::task::spawn_local(async move {
            let body = json!({"idx": i, "value": value});
            match api::post::<Value, _>(&format!("/api/v1/duels/{id}/answer"), &body).await {
                Ok(data) => {
                    wrong.set(!data["correct"].as_bool().unwrap_or(false));
                    result.set(Some(data.clone()));
                    duel.update(|d| {
                        if let Some(d) = d.as_mut() {
                            d["creator_score"] = data["creator_score"].clone();
                            d["opponent_score"] = data["opponent_score"].clone();
                            d["creator_done"] = data["creator_done"].clone();
                            d["opponent_done"] = data["opponent_done"].clone();
                            d["status"] = data["status"].clone();
                        }
                    });
                }
                Err(e) => error.set(Some(e.message)),
            }
            checking.set(false);
        });
    };

    let next = move || {
        result.set(None);
        wrong.set(false);
        idx.update(|i| *i += 1);
    };

    view! {
        <crate::components::member_gate::MemberGate title="Duels">
        <div class="mx-auto max-w-2xl px-4 py-8">
            <A href="/duels" attr:class="mb-6 inline-block text-sm text-white/60 hover:text-white">
                "← " {t("Back")}
            </A>
            <Show when=move || loading.get()>
                <div class="py-24 text-center text-white/50">{t("Loading")}"…"</div>
            </Show>
            {move || error.get().map(|e| view! {
                <div class="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">{e}</div>
            })}
            {move || duel.get().map(|d| {
                let total = d["question_count"].as_i64().unwrap_or(0) as usize;
                let finished = d["status"].as_str() == Some("finished");
                let my_done = idx.get() >= total;
                let vs = format!("{} vs {}",
                    d["creator_name"].as_str().unwrap_or("?"),
                    d["opponent_name"].as_str().unwrap_or("…"));
                let score = format!("{} - {}",
                    d["creator_score"].as_i64().unwrap_or(0),
                    d["opponent_score"].as_i64().unwrap_or(0));
                let header = view! {
                    <div class="mb-6 flex items-center justify-between">
                        <h1 class="retro-title retro-title--white text-xl">{vs}</h1>
                        <span class="rounded-full border border-purple-500/40 bg-purple-500/20 px-3 py-1 text-sm font-bold text-purple-300">
                            {score}
                        </span>
                    </div>
                };
                if finished || my_done {
                    let me_won = {
                        let uid = crate::auth::use_auth().user.get().map(|u| u.id).unwrap_or(0);
                        let my = if d["creator_id"].as_i64() == Some(uid) {
                            d["creator_score"].as_i64().unwrap_or(0)
                        } else {
                            d["opponent_score"].as_i64().unwrap_or(0)
                        };
                        let their = if d["creator_id"].as_i64() == Some(uid) {
                            d["opponent_score"].as_i64().unwrap_or(0)
                        } else {
                            d["creator_score"].as_i64().unwrap_or(0)
                        };
                        my.cmp(&their)
                    };
                    view! {
                        <div>
                            {header}
                            <div class="rounded-2xl border-2 border-purple-500/20 bg-neutral-900/80 p-8 text-center">
                                <h2 class="text-2xl font-black text-white">
                                    {if finished {
                                        match me_won {
                                            std::cmp::Ordering::Greater => t("Victory!").to_string(),
                                            std::cmp::Ordering::Less => t("Defeat").to_string(),
                                            std::cmp::Ordering::Equal => t("Draw!").to_string(),
                                        }
                                    } else {
                                        t("Waiting for your opponent…").to_string()
                                    }}
                                </h2>
                                <p class="mt-2 text-neutral-400">
                                    {if finished { t("Final score").to_string() } else { t("Your side is done — check back later.").to_string() }}
                                </p>
                                <div class="mt-6 flex flex-wrap justify-center gap-3">
                                    <A href="/duels" attr:class="btn-primary">{t("Back to duels")}</A>
                                    <button type="button"
                                        class="rounded-lg border border-white/15 bg-white/5 px-4 py-2 text-sm font-semibold text-white/80 transition hover:bg-white/10"
                                        on:click={
                                            let gt = d["game_type"].as_str().unwrap_or("quiz").to_string();
                                            let line = format!("{} {} – {} {}",
                                                d["creator_name"].as_str().unwrap_or("?"),
                                                d["creator_score"].as_i64().unwrap_or(0),
                                                d["opponent_score"].as_i64().unwrap_or(0),
                                                d["opponent_name"].as_str().unwrap_or("…"));
                                            let grid = answers_grid(
                                                d["my_answers"].as_array().map(Vec::as_slice).unwrap_or(&[]));
                                            move |_| {
                                                copy_clipboard(
                                                    format!("Blind'All : {} {}\n{}\n{}\n{}/duels",
                                                        t("Duel"), t(game_name(&gt)), line, grid, site_origin()),
                                                    copied);
                                            }
                                        }>
                                        {move || if copied.get() { t("Copied!") } else { t("Share result") }}
                                    </button>
                                    {if finished {
                                        let uid = crate::auth::use_auth().user.get().map(|u| u.id).unwrap_or(0);
                                        let other = if d["creator_id"].as_i64() == Some(uid) {
                                            d["opponent_id"].as_i64()
                                        } else {
                                            Some(d["creator_id"].as_i64().unwrap_or(0))
                                        };
                                        let gt = d["game_type"].as_str().unwrap_or("quiz").to_string();
                                        let qc = d["question_count"].as_i64().unwrap_or(10);
                                        other.map(|oid| view! {
                                            <button type="button" class="btn-primary"
                                                on:click=move |_| {
                                                    let gt = gt.clone();
                                                    leptos::task::spawn_local(async move {
                                                        if let Ok(v) = api::post::<Value, _>("/api/v1/duels",
                                                            &json!({"game_type": gt, "count": qc, "opponent_id": oid})).await {
                                                            if let Some(nid) = v["id"].as_i64() {
                                                                leptos_router::hooks::use_navigate()(
                                                                    &format!("/duels/{nid}"), Default::default());
                                                            }
                                                        }
                                                    });
                                                }>{t("Rematch")}</button>
                                        }.into_any()).unwrap_or_else(|| view! { <div/> }.into_any())
                                    } else {
                                        view! { <div/> }.into_any()
                                    }}
                                </div>
                            </div>
                        </div>
                    }.into_any()
                } else {
                    let qs = d["questions"].as_array().cloned().unwrap_or_default();
                    let cur = qs.get(idx.get()).cloned().unwrap_or_default();
                    view! {
                        <div>
                            {header}
                            <div class="overflow-hidden rounded-2xl border-2 border-purple-500/20 bg-neutral-900/80 p-6">
                                <p class="mb-4 text-xs text-white/50">
                                    {format!("{} {}/{}", t("Question"), idx.get() + 1, total)}
                                </p>
                                {if let Some(res) = result.get() {
                                    let cv = res["correct_value"].as_str().unwrap_or("").to_string();
                                    view! {
                                        <div class="space-y-4">
                                            <p class=move || if wrong.get() { "text-center text-lg font-bold text-amber-500" } else { "text-center text-lg font-bold text-teal-400" }>
                                                {if wrong.get() { t("Not quite!").to_string() } else { t("Correct!").to_string() }}
                                            </p>
                                            {wrong.get().then(|| view! {
                                                <p class="text-center text-white/60">
                                                    {t("Correct answer")}": "<span class="font-semibold text-white">{cv}</span>
                                                </p>
                                            })}
                                            <button type="button" class="btn-primary w-full" on:click=move |_| next()>
                                                {move || if idx.get() + 1 >= total { t("See results") } else { t("Next question") }}
                                            </button>
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {
                                        <QuestionView q=cur["question"].clone()
                                            game_type=d["game_type"].as_str().unwrap_or("").to_string()
                                            disabled=checking.get()
                                            on_answer=submit/>
                                    }.into_any()
                                }}
                            </div>
                        </div>
                    }.into_any()
                }
            })}
        </div>
        </crate::components::member_gate::MemberGate>
    }
}

// ---- daily challenge ------------------------------------------------------------

#[component]
pub fn DailyPage() -> impl IntoView {
    crate::i18n::set_title_key("Daily challenge");
    let auth = use_auth();
    let data = RwSignal::new(Option::<Value>::None);
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);
    let idx = RwSignal::new(0usize);
    let score = RwSignal::new(0i64);
    let total = RwSignal::new(0i64);
    let result = RwSignal::new(Option::<Value>::None);
    let checking = RwSignal::new(false);
    let wrong = RwSignal::new(false);
    let daily_type = RwSignal::new("quiz");
    let copied = RwSignal::new(false);
    let marks = RwSignal::new(Vec::<Value>::new());

    let load = move |ty: &'static str| {
        daily_type.set(ty);
        loading.set(true);
        data.set(None);
        result.set(None);
        wrong.set(false);
        idx.set(0);
        score.set(0);
        total.set(0);
        marks.set(vec![]);
        leptos::task::spawn_local(async move {
            match api::get::<Value>(&format!("/api/v1/daily-challenge?type={ty}")).await {
                Ok(v) => {
                    idx.set(v["answered"].as_i64().unwrap_or(0) as usize);
                    score.set(v["score"].as_i64().unwrap_or(0));
                    total.set(v["total"].as_i64().unwrap_or(0));
                    marks.set(v["my_answers"].as_array().cloned().unwrap_or_default());
                    data.set(Some(v));
                }
                Err(e) => error.set(Some(e.message)),
            }
            loading.set(false);
        });
    };
    crate::auth::on_member(auth, move || load("quiz"));

    let submit = move |value: String| {
        if checking.get() || result.get().is_some() {
            return;
        }
        let i = idx.get();
        let gt = daily_type.get();
        checking.set(true);
        leptos::task::spawn_local(async move {
            let body = json!({"game_type": gt, "idx": i, "value": value});
            match api::post::<Value, _>("/api/v1/daily-challenge/answer", &body).await {
                Ok(d) => {
                    wrong.set(!d["correct"].as_bool().unwrap_or(false));
                    score.set(d["score"].as_i64().unwrap_or(0));
                    total.set(d["total"].as_i64().unwrap_or(0));
                    marks.update(|m| {
                        m.push(json!({
                            "idx": i as i64,
                            "correct": d["correct"].as_bool().unwrap_or(false),
                        }))
                    });
                    result.set(Some(d));
                }
                Err(e) => error.set(Some(e.message)),
            }
            checking.set(false);
        });
    };

    let next = move || {
        result.set(None);
        wrong.set(false);
        idx.update(|i| *i += 1);
    };

    view! {
        <crate::components::member_gate::MemberGate title="Daily challenge">
        <div class="mx-auto max-w-2xl px-4 py-8">
            <A href="/duels" attr:class="mb-6 inline-block text-sm text-white/60 hover:text-white">
                "← " {t("Back")}
            </A>
            <div class="mb-5 flex flex-wrap gap-2">
                {["quiz", "who_sang", "album_cover", "first_letter", "anagram"].into_iter().map(|ty| {
                    view! {
                        <button type="button"
                            class=move || if daily_type.get() == ty {
                                "rounded-full border border-amber-500/50 bg-amber-500/20 px-3 py-1 text-xs font-semibold text-amber-300"
                            } else {
                                "rounded-full border border-white/10 px-3 py-1 text-xs text-white/50 transition hover:border-white/30 hover:text-white"
                            }
                            on:click=move |_| {
                                if daily_type.get() != ty {
                                    load(ty);
                                }
                            }>
                            {game_name(ty)}
                        </button>
                    }
                }).collect::<Vec<_>>()}
            </div>
            <Show when=move || loading.get()>
                <div class="py-24 text-center text-white/50">{t("Loading")}"…"</div>
            </Show>
            {move || error.get().map(|e| view! {
                <div class="rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">{e}</div>
            })}
            {move || data.get().map(|d| {
                let date = d["date"].as_str().unwrap_or("").to_string();
                let lb = d["leaderboard"].as_array().cloned().unwrap_or_default();
                let done = idx.get() >= total.get() as usize;
                let streak_h = d["streak"].as_i64().unwrap_or(0);
                let header = view! {
                    <div class="mb-6 flex items-center justify-between">
                        <div>
                            <h1 class="retro-title retro-title--white text-xl">{t("Daily challenge")}</h1>
                            <p class="text-xs text-amber-300/80">{date}</p>
                        </div>
                        <div class="flex items-center gap-2">
                            {if streak_h > 1 {
                                view! {
                                    <span class="rounded-full border border-orange-500/40 bg-orange-500/15 px-3 py-1 text-sm font-bold text-orange-300"
                                        title=t("days in a row")>
                                        "🔥 "{streak_h}
                                    </span>
                                }.into_any()
                            } else { view! { <div/> }.into_any() }}
                            <span class="rounded-full border border-amber-500/40 bg-amber-500/20 px-3 py-1 text-sm font-bold text-amber-300">
                                {move || score.get()} " pts"
                        </span>
                        </div>
                    </div>
                };
                let leaderboard = view! {
                    {(!lb.is_empty()).then(|| view! {
                        <div class="mt-8 rounded-2xl border border-white/10 bg-neutral-800/50 p-6">
                            <h2 class="mb-4 text-lg font-black text-white">{t("Today's leaderboard")}</h2>
                            <ul class="space-y-2">
                                {lb.iter().enumerate().map(|(i, r)| view! {
                                    <li class="flex items-center gap-3 text-sm">
                                        <span class="w-6 text-right font-bold text-neutral-500">{i + 1}</span>
                                        <span class="flex-1 font-semibold text-white">{r["name"].as_str().unwrap_or("").to_string()}</span>
                                        <span class="text-amber-300">{r["score"].as_i64().unwrap_or(0)}"/"{r["answered"].as_i64().unwrap_or(0)}</span>
                                    </li>
                                }).collect::<Vec<_>>()}
                            </ul>
                        </div>
                    })}
                };
                if done {
                    let streak = d["streak"].as_i64().unwrap_or(0);
                    let date = d["date"].as_str().unwrap_or("").to_string();
                    let gt = daily_type.get().to_string();
                    view! {
                        <div>
                            {header}
                            <div class="rounded-2xl border-2 border-amber-500/20 bg-neutral-900/80 p-8 text-center">
                                <h2 class="text-2xl font-black text-white">{t("Daily complete!")}</h2>
                                <p class="mt-2 text-neutral-400">{t("Come back tomorrow for a new set.")}</p>
                                {if streak > 0 {
                                    view! {
                                        <p class="mt-3 text-lg font-bold text-amber-400">
                                            "🔥 "{streak}" "{t("days in a row")}
                                        </p>
                                    }.into_any()
                                } else { view! { <div/> }.into_any() }}
                                <div class="mt-6 flex justify-center gap-3">
                                    <button type="button"
                                        class="rounded-lg border border-white/15 bg-white/5 px-4 py-2 text-sm font-semibold text-white/80 transition hover:bg-white/10"
                                        on:click=move |_| {
                                            copy_clipboard(
                                                format!("Blind'All — {} {} {}\n{} {}\n{}\n{}/daily",
                                                    t("Daily challenge"), t(game_name(&gt)), date,
                                                    score.get_untracked(), t("pts"),
                                                    answers_grid(&marks.get_untracked()),
                                                    site_origin()),
                                                copied);
                                        }>
                                        {move || if copied.get() { t("Copied!") } else { t("Share result") }}
                                    </button>
                                </div>
                            </div>
                            {leaderboard}
                        </div>
                    }.into_any()
                } else {
                    let qs = d["questions"].as_array().cloned().unwrap_or_default();
                    let cur = qs.get(idx.get()).cloned().unwrap_or_default();
                    view! {
                        <div>
                            {header}
                            <div class="overflow-hidden rounded-2xl border-2 border-amber-500/20 bg-neutral-900/80 p-6">
                                <p class="mb-4 text-xs text-white/50">
                                    {format!("{} {}/{}", t("Question"), idx.get() + 1, total.get())}
                                </p>
                                {if let Some(res) = result.get() {
                                    let cv = res["correct_value"].as_str().unwrap_or("").to_string();
                                    view! {
                                        <div class="space-y-4">
                                            <p class=move || if wrong.get() { "text-center text-lg font-bold text-amber-500" } else { "text-center text-lg font-bold text-teal-400" }>
                                                {if wrong.get() { t("Not quite!").to_string() } else { t("Correct!").to_string() }}
                                            </p>
                                            {wrong.get().then(|| view! {
                                                <p class="text-center text-white/60">
                                                    {t("Correct answer")}": "<span class="font-semibold text-white">{cv}</span>
                                                </p>
                                            })}
                                            <button type="button" class="btn-primary w-full" on:click=move |_| next()>
                                                {move || if idx.get() + 1 >= total.get() as usize { t("See results") } else { t("Next question") }}
                                            </button>
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {
                                        <QuestionView q=cur["question"].clone()
                                            game_type=daily_type.get().to_string()
                                            disabled=checking.get()
                                            on_answer=submit/>
                                    }.into_any()
                                }}
                            </div>
                            {leaderboard}
                        </div>
                    }.into_any()
                }
            })}
        </div>
        </crate::components::member_gate::MemberGate>
    }
}
