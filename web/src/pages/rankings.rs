use crate::api;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_query_map;
use serde_json::Value;

fn photo(u: &Value) -> String {
    u["photo"].as_str().unwrap_or("").to_string()
}

fn user_link(id: i64) -> String {
    format!("/profile/{id}")
}

const PLAYER_SORTS: &[&str] = &["level", "score", "elo", "week", "avg_time", "best_round"];

#[component]
pub fn RankingsPage() -> impl IntoView {
    crate::i18n::set_title_key("Rankings");
    crate::seo::apply_page("/rankings", t("Rankings meta description"));
    let auth = crate::auth::use_auth();
    let query = use_query_map();
    let path = leptos_router::hooks::use_location()
        .pathname
        .get_untracked();
    let initial_tab = match path.as_str() {
        "/rankings/season" => "season",
        "/rankings/teams" => "teams",
        "/rankings/minigames" => "minigames",
        _ => "players",
    };
    let initial = query.get_untracked();
    let initial_sort = initial
        .get("sort")
        .filter(|s| PLAYER_SORTS.contains(&s.as_str()))
        .unwrap_or_else(|| "elo".to_string());
    let initial_room = initial
        .get("room")
        .and_then(|r| r.parse::<i64>().ok())
        .unwrap_or(0);
    let initial_tab_q = initial
        .get("tab")
        .filter(|s| ["players", "season", "teams", "minigames"].contains(&s.as_str()))
        .map(|s| s.to_string());
    let tab = RwSignal::new(initial_tab_q.unwrap_or_else(|| initial_tab.to_string()));
    let sort = RwSignal::new(initial_sort);
    let room = RwSignal::new(initial_room);
    let page = RwSignal::new(
        initial
            .get("page")
            .and_then(|p| p.parse::<i64>().ok())
            .filter(|p| *p > 0)
            .unwrap_or(1),
    );
    let payload = RwSignal::new(Value::Null);
    let loading = RwSignal::new(true);

    let authed_tab = matches!(initial_tab, "teams");
    Effect::new(move |_| {
        if authed_tab && auth.ready.get() && auth.user.get().is_none() {
            leptos_router::hooks::use_navigate()("/login", Default::default());
        }
    });

    Effect::new(move |_| {
        let q = query.get();
        if let Some(s) = q.get("sort").filter(|s| PLAYER_SORTS.contains(&s.as_str())) {
            if sort.get_untracked() != *s {
                sort.set(s.clone());
                page.set(1);
            }
        }
        let room_v = q
            .get("room")
            .and_then(|r| r.parse::<i64>().ok())
            .unwrap_or(0);
        if room.get_untracked() != room_v {
            room.set(room_v);
            page.set(1);
        }
        let page_v = q
            .get("page")
            .and_then(|p| p.parse::<i64>().ok())
            .filter(|p| *p > 0)
            .unwrap_or(1);
        if page.get_untracked() != page_v {
            page.set(page_v);
        }
    });

    let fetch = move || {
        let (tab_v, sort_v, room_v, page_v) = (tab.get(), sort.get(), room.get(), page.get());
        loading.set(true);
        leptos::task::spawn_local(async move {
            let url = match tab_v.as_str() {
                "season" => format!("/api/v1/seasons?page={page_v}"),
                "teams" => format!("/api/v1/rankings/teams?page={page_v}"),
                "minigames" => format!("/api/v1/rankings/minigames?page={page_v}"),
                _ => format!("/api/v1/rankings?sort={sort_v}&page={page_v}&room={room_v}"),
            };
            match api::get::<Value>(&url).await {
                Ok(v) => payload.set(v),
                Err(_) => payload.set(Value::Null),
            }
            loading.set(false);
        });
    };
    Effect::new(move |_| fetch());

    view! {
        <div class="mx-auto max-w-5xl px-4 py-4 sm:py-8">
            <div class="mx-auto mb-6 text-center sm:mb-8">
                <h1 class="mb-2 bg-gradient-to-r from-yellow-400 via-yellow-500 to-yellow-600 bg-clip-text text-3xl font-extrabold text-transparent sm:mb-4 sm:text-5xl">
                    {t("Rankings")}
                </h1>
                <p class="text-sm text-white/50 sm:text-lg">{t("Compete with the best players")}</p>
                <p class="mx-auto mt-3 max-w-2xl text-sm leading-relaxed text-white/40 sm:text-base">
                    {t("Rankings SEO intro")}
                </p>
                <A href="/docs" attr:class="mt-3 inline-flex items-center gap-1.5 text-sm text-yellow-500/90 transition-colors hover:text-yellow-400">
                    {t("How do rankings work?")}
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" class="h-4 w-4" aria-hidden="true">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M13.5 4.5L21 12m0 0l-7.5 7.5M21 12H3" />
                    </svg>
                </A>
            </div>

            <Show when=move || crate::auth::use_auth().user.get().is_none()>
                <div class="mb-6 rounded-xl border border-yellow-500/20 bg-yellow-500/5 p-4 sm:p-5">
                    <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                        <div class="space-y-1">
                            <p class="font-semibold text-white">{t("Rankings guest CTA title")}</p>
                            <p class="text-sm text-white/50">{t("Rankings guest CTA body")}</p>
                        </div>
                        <div class="flex flex-wrap gap-2">
                            <A href="/register" attr:class="game-btn-secondary inline-flex justify-center">
                                {t("Rankings guest CTA button")}
                            </A>
                            <A href="/login" attr:class="game-link-action inline-flex items-center justify-center px-3 py-2">
                                {t("Log in")}
                            </A>
                        </div>
                    </div>
                </div>
            </Show>

            <div class="mb-6 border-b border-white/10 sm:mb-8">
                <nav class="-mb-px flex gap-2 overflow-x-auto" aria-label="Tabs">
                    {move || {
                        let authed = auth.user.get().is_some();
                        [("players", "/rankings"), ("season", "/rankings/season"), ("minigames", "/rankings/minigames"), ("teams", "/rankings/teams")]
                            .into_iter()
                            .filter(|(tb, _)| *tb != "teams" || authed)
                            .map(|(tb, href)| {
                                let tb_s = tb.to_string();
                                view! {
                                    <A href=href
                                        attr:class=move || if tab.get() == tb_s {
                                            "group inline-flex flex-shrink-0 items-center gap-2 border-b-2 border-yellow-500 bg-yellow-500/5 px-3 py-2 text-xs font-medium text-yellow-500 transition-all duration-200 sm:px-4 sm:py-3 sm:text-sm"
                                        } else {
                                            "group inline-flex flex-shrink-0 items-center gap-2 border-b-2 border-transparent px-3 py-2 text-xs font-medium text-white/50 transition-all duration-200 hover:border-white/30 hover:text-white/80 sm:px-4 sm:py-3 sm:text-sm"
                                        }>
                                        {t(match tb { "players" => "Players", "season" => "Season", "teams" => "Teams", _ => "Mini-games" })}
                                    </A>
                                }
                            }).collect::<Vec<_>>()
                    }}
                </nav>
            </div>

            {move || {
                match tab.get().as_str() {
                    "season" => view! { <SeasonBoard payload=payload loading=loading page=page /> }.into_any(),
                    "teams" => view! { <TeamsBoard payload=payload loading=loading page=page /> }.into_any(),
                    "minigames" => view! { <MinigamesBoard payload=payload loading=loading page=page /> }.into_any(),
                    _ => view! {
                        <PlayersBoard payload=payload loading=loading sort=sort room=room page=page />
                    }.into_any(),
                }
            }}
        </div>
    }
}

// ---- players -------------------------------------------------------------------

#[component]
fn PlayersBoard(
    payload: RwSignal<Value>,
    loading: RwSignal<bool>,
    sort: RwSignal<String>,
    room: RwSignal<i64>,
    page: RwSignal<i64>,
) -> impl IntoView {
    // PHP: clicking a ranked user's level lazily fetches level-metrics.
    let level_modal = RwSignal::new(Option::<Value>::None);
    let open_level = move |uid: i64| {
        leptos::task::spawn_local(async move {
            if let Ok(v) =
                api::get::<Value>(&format!("/api/v1/rankings/users/{uid}/level-metrics")).await
            {
                level_modal.set(Some(v));
            }
        });
    };
    view! {
        <div>
            <div class="mb-4 flex flex-col gap-3 sm:mb-6 sm:flex-row sm:items-end sm:justify-between">
                <div class="flex flex-wrap gap-2">
                    {["level", "score", "elo", "week", "avg_time", "best_round"].into_iter().map(|s| {
                        let s_s = s.to_string();
                        view! {
                            <A href=move || {
                                let room_v = room.get();
                                if room_v != 0 {
                                    format!("/rankings?sort={s}&room={room_v}")
                                } else {
                                    format!("/rankings?sort={s}")
                                }
                            }
                                attr:class=move || if sort.get() == s_s {
                                    "rounded-full border border-yellow-500 bg-yellow-500/15 px-3 py-1.5 text-xs font-medium text-yellow-400 transition-colors sm:text-sm"
                                } else {
                                    "rounded-full border border-white/10 px-3 py-1.5 text-xs font-medium text-white/50 transition-colors hover:border-white/30 hover:text-white/80 sm:text-sm"
                                }>
                                {t(match s {
                                    "elo" => "ELO", "level" => "Level", "score" => "Score",
                                    "week" => "Top Week", "avg_time" => "Avg. response time",
                                    _ => "Best round",
                                })}
                            </A>
                        }
                    }).collect::<Vec<_>>()}
                </div>
                <label class="flex min-w-[12rem] flex-col gap-1 text-xs text-white/50 sm:text-sm">
                    <span>{t("Filter by room")}</span>
                    <select
                        class="rounded-lg border border-white/10 bg-surface-raised px-3 py-2 text-sm text-white focus:border-yellow-500 focus:outline-none"
                        on:change=move |ev| {
                            let room_v = event_target_value(&ev).parse::<i64>().unwrap_or(0);
                            let sort_v = sort.get_untracked();
                            let url = if room_v != 0 {
                                format!("/rankings?sort={sort_v}&room={room_v}")
                            } else {
                                format!("/rankings?sort={sort_v}")
                            };
                            leptos_router::hooks::use_navigate()(&url, Default::default());
                        }
                    >
                        <option value="0">{t("All official rooms")}</option>
                        {move || payload.get()["official_rooms"].as_array().cloned().unwrap_or_default()
                            .into_iter().map(|r| {
                                let id = r["id"].as_i64().unwrap_or(0);
                                view! {
                                    <option value=id selected=move || room.get() == id>
                                        {r["name"].as_str().unwrap_or("").to_string()}
                                    </option>
                                }
                            }).collect::<Vec<_>>()}
                    </select>
                </label>
            </div>

            {move || {
                let auth = crate::auth::use_auth();
                let pos = payload.get()["user_context"]["position"].as_i64();
                pos.map(|pos| {
                    let entry = payload.get()["user_context"]["entry"].clone();
                    let stats = entry["stats"].clone();
                    let uname = auth.user.get().map(|u| u.name.clone()).unwrap_or_default();
                    let uid = auth.user.get().map(|u| u.id).unwrap_or(0);
                    let sort_v = sort.get();
                    let badge = match pos { 1 => "🥇".into(), 2 => "🥈".into(), 3 => "🥉".into(), _ => format!("#{pos}") };
                    let chip = |metric: &str| if sort_v == metric {
                        "inline-flex items-baseline gap-1.5 rounded-md border border-yellow-500/50 bg-yellow-500/10 px-2 py-1"
                    } else {
                        "inline-flex items-baseline gap-1.5 rounded-md border border-neutral-700/60 bg-neutral-900/60 px-2 py-1"
                    };
                    view! {
                        <div class="mb-4 mt-6 rounded-xl border-2 border-yellow-500/30 bg-gradient-to-br from-yellow-500/10 to-yellow-600/5 p-4 shadow-lg sm:mt-8 sm:p-6">
                            <div class="flex flex-col gap-4">
                                <div class="flex items-center gap-3 sm:gap-4">
                                    <div class="flex h-12 w-12 shrink-0 items-center justify-center rounded-full border-2 border-yellow-500/30 bg-gradient-to-br from-yellow-500/20 to-yellow-600/10 text-xl font-bold text-yellow-400 sm:h-16 sm:w-16 sm:text-2xl">
                                        {badge}
                                    </div>
                                    <div class="min-w-0 flex-1">
                                        <h3 class="truncate text-base font-bold text-white sm:text-lg">{uname}</h3>
                                        <p class="text-xs text-neutral-400 sm:text-sm">{t("Your ranking")}</p>
                                    </div>
                                    <A href=user_link(uid) attr:class="shrink-0 rounded-lg bg-yellow-500/20 px-3 py-1.5 text-xs font-medium text-yellow-400 transition-colors hover:bg-yellow-500/30 sm:px-4 sm:py-2 sm:text-sm">
                                        {t("View Profile")}
                                    </A>
                                </div>
                                <div class="flex flex-wrap gap-2">
                                    <div class=chip("level")>
                                        <span class="text-[10px] font-medium uppercase tracking-wide text-neutral-500">{t("Level")}</span>
                                        <span class="text-sm font-bold text-white">{stats["level"].as_i64().unwrap_or(1)}</span>
                                    </div>
                                    <div class=chip("elo")>
                                        <span class="text-[10px] font-medium uppercase tracking-wide text-neutral-500">{t("ELO")}</span>
                                        <span class="text-sm font-bold text-white">{stats["elo"].as_i64().unwrap_or(1500)}</span>
                                    </div>
                                    <div class=chip("score")>
                                        <span class="text-[10px] font-medium uppercase tracking-wide text-neutral-500">{t("Score")}</span>
                                        <span class="text-sm font-bold text-white">{format!("{:.1}", stats["score"].as_f64().unwrap_or(0.0))}</span>
                                        <span class="text-[10px] uppercase text-neutral-500">{t("PTS")}</span>
                                    </div>
                                    {(stats["week_score"].as_f64().unwrap_or(0.0) > 0.0).then(|| view! {
                                        <div class=chip("week")>
                                            <span class="text-[10px] font-medium uppercase tracking-wide text-neutral-500">{t("Top Week")}</span>
                                            <span class="text-sm font-bold text-white">{format!("{:.1}", stats["week_score"].as_f64().unwrap_or(0.0))}</span>
                                        </div>
                                    })}
                                </div>
                            </div>
                        </div>
                    }
                })
            }}

            {move || {
                let p = payload.get();
                let data = p["leaderboard"]["data"].as_array().cloned().unwrap_or_default();
                let total = p["leaderboard"]["total"].as_i64().unwrap_or(0);
                let last = p["leaderboard"]["last_page"].as_i64().unwrap_or(1);
                if loading.get() {
                    return view! { <div class="text-white/60">{t("Loading")}"…"</div> }.into_any();
                }
                if data.is_empty() {
                    return view! {
                        <div class="py-16 text-center">
                            <p class="text-white/40">{t("No rankings available")}</p>
                        </div>
                    }.into_any();
                }
                let sort_v = sort.get();
                let cur_page = page.get();
                let first_page = cur_page == 1;
                view! {
                    <div>
                        <div class="space-y-2.5 sm:space-y-3">
                            {data.into_iter().enumerate().map(|(i, e)| {
                                let u = e["user"].clone();
                                let stats = e["stats"].clone();
                                let uid = u["id"].as_i64().unwrap_or(0);
                                let is_guest = u["is_guest"].as_bool().unwrap_or(false);
                                let medal = if first_page {
                                    ["🥇","🥈","🥉"].get(i).copied()
                                } else { None };
                                let row_cls = match (first_page, i) {
                                    (true, 0) => "border-yellow-500/30 bg-yellow-500/5",
                                    (true, 1) => "border-gray-400/25 bg-gray-400/5",
                                    (true, 2) => "border-amber-600/25 bg-amber-600/5",
                                    _ => "border-neutral-800 bg-neutral-900/40",
                                };
                                let chip = |metric: &str| if sort_v == metric {
                                    "inline-flex items-baseline gap-1.5 rounded-md border border-yellow-500/50 bg-yellow-500/10 px-2 py-1"
                                } else {
                                    "inline-flex items-baseline gap-1.5 rounded-md border border-neutral-700/60 bg-neutral-800/50 px-2 py-1"
                                };
                                let stat_box = |metric: &str| if sort_v == metric {
                                    "rounded-md border border-yellow-500/50 bg-yellow-500/10 px-2 py-1.5"
                                } else {
                                    "rounded-md border border-neutral-700/60 bg-neutral-800/50 px-2 py-1.5"
                                };
                                let elo_v = stats["elo"].as_i64().unwrap_or(1500);
                                let photo_src = photo(&u);
                                let name = u["name"].as_str().unwrap_or("").to_string();
                                let alt = name.clone();
                                let real_idx = (cur_page - 1) * 10 + i as i64 + 1;
                                view! {
                                    <article class=format!("rounded-xl border p-3 shadow-sm transition-colors hover:border-neutral-700 sm:p-4 {row_cls}")>
                                        <div class="flex flex-col gap-3">
                                            <div class="flex items-start gap-3">
                                                <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-neutral-800 text-sm font-bold sm:h-11 sm:w-11">
                                                    {match medal {
                                                        Some(m) => view! { <span class="text-lg">{m}</span> }.into_any(),
                                                        None => view! { <span class="text-neutral-400">"#"{real_idx}</span> }.into_any(),
                                                    }}
                                                </div>
                                                {if !is_guest {
                                                    let alt2 = alt.clone();
                                                    view! {
                                                        <A href=user_link(uid) attr:class="shrink-0">
                                                            <img src=photo_src.clone() alt=alt2 class="h-10 w-10 rounded-full border-2 border-neutral-600 object-cover sm:h-11 sm:w-11" loading="lazy"/>
                                                        </A>
                                                    }.into_any()
                                                } else {
                                                    view! {
                                                        <img src=photo_src.clone() alt=alt.clone() class="h-10 w-10 shrink-0 rounded-full border-2 border-neutral-600 object-cover sm:h-11 sm:w-11" loading="lazy"/>
                                                    }.into_any()
                                                }}
                                                <div class="min-w-0 flex-1">
                                                    <div class="flex min-w-0 items-center gap-1.5">
                                                        {if !is_guest {
                                                            let nm = name.clone();
                                                            view! {
                                                                <A href=user_link(uid) attr:class="block truncate text-base font-bold text-white hover:text-yellow-400">
                                                                    {nm}
                                                                </A>
                                                            }.into_any()
                                                        } else {
                                                            let nm = if name.is_empty() { t("Deleted user").to_string() } else { name.clone() };
                                                            view! {
                                                                <span class="block truncate text-base font-bold text-neutral-400">{nm}</span>
                                                            }.into_any()
                                                        }}
                                                        <crate::components::tag::UserTags
                                                            role=u["role"].as_str().unwrap_or("").to_string()
                                                            custom_tag=u["custom_tag"].as_str().unwrap_or("").to_string()/>
                                                    </div>
                                                    <div class="mt-2 flex flex-wrap gap-2">
                                                        {if !is_guest {
                                                            view! {
                                                                <button type="button" class=format!("{} cursor-pointer transition-colors hover:border-yellow-500/70", chip("level"))
                                                                    on:click=move |_| open_level(uid)>
                                                                    <MetricTip label=t("Level") tip=t("Your experience level based on XP earned while playing.")/>
                                                                    <span class="text-sm font-bold text-white">{stats["level"].as_i64().unwrap_or(1)}</span>
                                                                </button>
                                                            }.into_any()
                                                        } else {
                                                            view! {
                                                                <div class=chip("level")>
                                                                    <MetricTip label=t("Level") tip=t("Your experience level based on XP earned while playing.")/>
                                                                    <span class="text-sm font-bold text-white">{stats["level"].as_i64().unwrap_or(1)}</span>
                                                                </div>
                                                            }.into_any()
                                                        }}
                                                        <div class=chip("elo")>
                                                            <MetricTip label=t("ELO") tip=t("Your competitive skill rating. It can go up or down depending on your results.")/>
                                                            <span class="text-sm font-bold text-white">{elo_v}</span>
                                                        </div>
                                                        <div class=chip("score")>
                                                            <MetricTip label=t("Score") tip=t("Total points earned in official rooms across all games played.")/>
                                                            <span class="text-sm font-bold text-white">{format!("{:.1}", stats["score"].as_f64().unwrap_or(0.0))}</span>
                                                            <span class="text-[10px] uppercase text-neutral-500">{t("PTS")}</span>
                                                        </div>
                                                    </div>
                                                </div>
                                            </div>
                                            <dl class="grid grid-cols-2 gap-2 border-t border-neutral-800 pt-2 text-xs sm:grid-cols-3 lg:grid-cols-5">
                                                <div class=stat_box("week")>
                                                    <dt><MetricTip label=t("Top Week") tip=t("Points earned in the last 7 days in official rooms. This is a score, not a rank.")/></dt>
                                                    <dd class="font-semibold text-white">{format!("{:.1}", stats["week_score"].as_f64().unwrap_or(0.0))}</dd>
                                                </div>
                                                <div class=stat_box("avg_time")>
                                                    <dt><MetricTip label=t("Avg. response time") tip=t("Average time to complete an excerpt (all answers found), in seconds. Lower is faster.")/></dt>
                                                    <dd class="font-semibold text-white">{stats["avg_response_time"].as_f64().map(|v| format!("{v:.1}s")).unwrap_or_else(|| "—".into())}</dd>
                                                </div>
                                                <div class=stat_box("best_round")>
                                                    <dt><MetricTip label=t("Best round") tip=t("Your highest score achieved in a single round.")/></dt>
                                                    <dd class="font-semibold text-white">{stats["best_round_score"].as_f64().map(|v| format!("{v:.1}")).unwrap_or_else(|| "—".into())}</dd>
                                                </div>
                                                <div class=stat_box("")>
                                                    <dt><MetricTip label=t("Best streak") tip=t("Your longest streak of consecutive round wins in the same room.")/></dt>
                                                    <dd class="font-semibold text-white">{stats["best_win_streak"].as_i64().map(|v| v.to_string()).unwrap_or_else(|| "—".into())}</dd>
                                                </div>
                                                <div class=stat_box("")>
                                                    <dt><MetricTip label=t("Rounds played") tip=t("Number of complete rounds played in official rooms.")/></dt>
                                                    <dd class="font-semibold text-white">{stats["rounds_played"].as_i64().unwrap_or(0)}</dd>
                                                </div>
                                            </dl>
                                        </div>
                                    </article>
                                }
                            }).collect_view()}
                        </div>
                        <Pagination page=page last=last total=total />
                    </div>
                }.into_any()
            }}
            {move || level_modal.get().map(|m| {
                view! {
                    <div class="fixed inset-0 z-50 flex items-center justify-center overflow-y-auto bg-black/70 p-4"
                        on:click=move |_| level_modal.set(None)>
                        <div class="max-h-[calc(100vh-2rem)] w-full max-w-2xl overflow-y-auto rounded-xl bg-gradient-to-br from-slate-800 to-slate-900 shadow-2xl"
                            on:click=move |e| e.stop_propagation()>
                            <div class="flex items-center justify-between border-b border-neutral-700 px-6 py-4">
                                <h2 class="text-lg font-bold text-white">{t("Level")} " " {m["level"].as_i64().unwrap_or(1)}</h2>
                                <button type="button" class="text-neutral-400 hover:text-neutral-200"
                                    on:click=move |_| level_modal.set(None)>
                                    <svg class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                                    </svg>
                                </button>
                            </div>
                            <div class="px-6 py-6">
                                <crate::components::level::LevelInfo
                                    level=m["level"].as_i64().unwrap_or(1)
                                    current_xp=m["current_xp"].as_i64().unwrap_or(0)
                                    xp_for_next_level=m["xp_for_next_level"].as_i64().unwrap_or(100)
                                    total_xp=m["total_xp"].as_i64().unwrap_or(0)
                                    level_metrics=m.get("level_metrics").filter(|v| !v.is_null()).cloned() />
                            </div>
                        </div>
                    </div>
                }
            })}
        </div>
    }
}

// ---- teams -----------------------------------------------------------------------

#[component]
fn TeamsBoard(
    payload: RwSignal<Value>,
    loading: RwSignal<bool>,
    page: RwSignal<i64>,
) -> impl IntoView {
    view! {
        <div>
            {move || payload.get()["user_position"]["position"].as_i64().map(|pos| {
                let score = payload.get()["user_position"]["team_score"].as_f64().unwrap_or(0.0);
                let team_id = payload.get()["user_position"]["team_id"].as_i64().unwrap_or(0);
                view! {
                    <div class="mb-4 flex items-center gap-4 rounded-xl border border-brand-accent/30 bg-brand-accent/10 px-4 py-3">
                        <span class="retro-title text-xl text-brand-accent-light">{format!("#{pos}")}</span>
                        <A href=format!("/teams/{team_id}") attr:class="text-white">
                            {t("My team")} " : " {format!("{score:.1}")} " " {t("points")}
                        </A>
                    </div>
                }
            })}
            {move || {
                let p = payload.get();
                let data = p["top_teams"]["data"].as_array().cloned().unwrap_or_default();
                if loading.get() {
                    return view! { <div class="text-white/60">{t("Loading")}"…"</div> }.into_any();
                }
                if data.is_empty() {
                    return view! {
                        <div class="rounded-xl border border-white/10 bg-surface-raised/60 p-8 text-center text-white/60">
                            {t("No teams yet")}
                        </div>
                    }.into_any();
                }
                let last = p["top_teams"]["last_page"].as_i64().unwrap_or(1);
                let total = p["top_teams"]["total"].as_i64().unwrap_or(0);
                let first_page = page.get() == 1;
                view! {
                    <div>
                        <div class="space-y-2.5 sm:space-y-3">
                            {data.into_iter().enumerate().map(|(i, e)| {
                                let team = e["team"].clone();
                                let tid = team["id"].as_i64().unwrap_or(0);
                                let tname = team["name"].as_str().unwrap_or("").to_string();
                                let tname_alt = tname.clone();
                                let tphoto = team["photo"].as_str().unwrap_or("").to_string();
                                let medal = if first_page { ["🥇","🥈","🥉"].get(i).copied() } else { None };
                                let rank_box = match (first_page, i) {
                                    (true, 0) => "bg-gradient-to-br from-yellow-400 to-yellow-600 text-yellow-900",
                                    (true, 1) => "bg-gradient-to-br from-gray-300 to-gray-500 text-gray-900",
                                    (true, 2) => "bg-gradient-to-br from-amber-600 to-amber-800 text-amber-100",
                                    _ => "bg-neutral-800 text-neutral-400",
                                };
                                let border_cls = match (first_page, i) {
                                    (true, 0) => "border-yellow-500/30",
                                    (true, 1) => "border-gray-400/30",
                                    (true, 2) => "border-amber-600/30",
                                    _ => "border-neutral-700",
                                };
                                let bg_cls = match (first_page, i) {
                                    (true, 0) => "from-yellow-500/5 to-yellow-600/0",
                                    (true, 1) => "from-gray-400/5 to-gray-500/0",
                                    (true, 2) => "from-amber-600/5 to-amber-700/0",
                                    _ => "from-neutral-800/50 to-neutral-900/50",
                                };
                                let score_cls = match (first_page, i) {
                                    (true, 0) => "text-yellow-400",
                                    (true, 1) => "text-gray-300",
                                    (true, 2) => "text-amber-600",
                                    _ => "text-white",
                                };
                                view! {
                                    <div class=format!("group relative overflow-hidden rounded-xl border bg-gradient-to-r p-3.5 shadow-sm transition-all duration-300 hover:shadow-md hover:shadow-yellow-500/10 sm:p-4 {border_cls} {bg_cls}")>
                                        <div class="relative flex items-center gap-3 sm:gap-4">
                                            <div class=format!("flex h-11 w-11 flex-shrink-0 items-center justify-center rounded-lg font-bold shadow-sm sm:h-14 sm:w-14 sm:rounded-xl {rank_box}")>
                                                {match medal {
                                                    Some(m) => view! { <span class="text-xl sm:text-2xl">{m}</span> }.into_any(),
                                                    None => view! { <span class="text-sm font-extrabold sm:text-base">"#"{(page.get() - 1) * 50 + i as i64 + 1}</span> }.into_any(),
                                                }}
                                            </div>
                                            <A href=format!("/teams/{tid}") attr:class="relative flex-shrink-0 transition-transform duration-200 hover:scale-105">
                                                <img src=tphoto alt=tname_alt class="h-11 w-11 rounded-full border-2 border-neutral-600 object-cover shadow-md sm:h-14 sm:w-14"/>
                                            </A>
                                            <div class="min-w-0 flex-1">
                                                <A href=format!("/teams/{tid}") attr:class="block truncate text-base font-bold text-white transition-colors hover:text-yellow-400 sm:text-lg">
                                                    {tname}
                                                </A>
                                            </div>
                                            <div class="text-right">
                                                <div class=format!("text-lg font-extrabold sm:text-xl {score_cls}")>
                                                    {format!("{:.1}", e["total_score"].as_f64().unwrap_or(0.0))}
                                                    <span class="ml-1 text-xs font-normal text-neutral-400 sm:text-sm">{t("PTS")}</span>
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                        <Pagination page=page last=last total=total />
                    </div>
                }.into_any()
            }}
        </div>
    }
}

// ---- minigames -------------------------------------------------------------------

#[component]
fn MinigamesBoard(
    payload: RwSignal<Value>,
    loading: RwSignal<bool>,
    page: RwSignal<i64>,
) -> impl IntoView {
    view! {
        <div>
            {move || payload.get()["user_context"]["position"].as_i64().map(|pos| {
                let score = payload.get()["user_context"]["score"].as_i64().unwrap_or(0);
                view! {
                    <div class="mb-4 flex items-center gap-4 rounded-xl border border-brand-accent/30 bg-brand-accent/10 px-4 py-3">
                        <span class="retro-title text-xl text-brand-accent-light">{format!("#{pos}")}</span>
                        <span class="text-white">{score} " " {t("points")}</span>
                    </div>
                }
            })}
            {move || {
                let p = payload.get();
                let data = p["top_by_minigames"]["data"].as_array().cloned().unwrap_or_default();
                if loading.get() {
                    return view! { <div class="text-white/60">{t("Loading")}"…"</div> }.into_any();
                }
                if data.is_empty() {
                    return view! {
                        <div class="rounded-xl border border-white/10 bg-surface-raised/60 p-8 text-center text-white/60">
                            {t("No results yet")}
                        </div>
                    }.into_any();
                }
                let last = p["top_by_minigames"]["last_page"].as_i64().unwrap_or(1);
                let total = p["top_by_minigames"]["total"].as_i64().unwrap_or(0);
                let first_page = page.get() == 1;
                view! {
                    <div>
                        <div class="space-y-2.5 sm:space-y-3">
                            {data.into_iter().enumerate().map(|(i, e)| {
                                let u = e["user"].clone();
                                let uid = u["id"].as_i64().unwrap_or(0);
                                let lvl = u["userLevel"].clone();
                                let elo_v = u["elo"].as_i64().unwrap_or(1500);
                                let name = u["name"].as_str().unwrap_or("").to_string();
                                let medal = if first_page { ["🥇","🥈","🥉"].get(i).copied() } else { None };
                                let (rank_box, border_cls, bg_cls, ring_cls, score_cls) = match (first_page, i) {
                                    (true, 0) => ("bg-gradient-to-br from-yellow-400 to-yellow-600 text-yellow-900", "border-yellow-500/30", "from-yellow-500/5 to-yellow-600/0", "border-yellow-400 ring-2 ring-yellow-500/30", "text-yellow-400"),
                                    (true, 1) => ("bg-gradient-to-br from-gray-300 to-gray-500 text-gray-900", "border-gray-400/30", "from-gray-400/5 to-gray-500/0", "border-gray-300 ring-2 ring-gray-400/30", "text-gray-300"),
                                    (true, 2) => ("bg-gradient-to-br from-amber-600 to-amber-800 text-amber-100", "border-amber-600/30", "from-amber-600/5 to-amber-700/0", "border-amber-600 ring-2 ring-amber-700/30", "text-amber-600"),
                                    _ => ("bg-neutral-800 text-neutral-400", "border-neutral-700", "from-neutral-800/50 to-neutral-900/50", "border-neutral-600", "text-white"),
                                };
                                let dot_cls = match i { 0 => "bg-yellow-400", 1 => "bg-gray-300", _ => "bg-amber-600" };
                                let alt = name.clone();
                                let nm = name.clone();
                                let photo_src = photo(&u);
                                view! {
                                    <div class=format!("group relative overflow-hidden rounded-xl border bg-gradient-to-r p-3.5 shadow-sm transition-all duration-300 hover:shadow-md hover:shadow-yellow-500/10 sm:p-4 {border_cls} {bg_cls}")>
                                        <div class="relative flex items-center gap-3 sm:gap-4">
                                            <div class=format!("flex h-11 w-11 flex-shrink-0 items-center justify-center rounded-lg font-bold shadow-sm sm:h-14 sm:w-14 sm:rounded-xl {rank_box}")>
                                                {match medal {
                                                    Some(m) => view! { <span class="text-xl sm:text-2xl">{m}</span> }.into_any(),
                                                    None => view! { <span class="text-sm font-extrabold sm:text-base">"#"{(page.get() - 1) * 50 + i as i64 + 1}</span> }.into_any(),
                                                }}
                                            </div>
                                            <A href=user_link(uid) attr:class="relative flex-shrink-0 transition-transform duration-200 hover:scale-105">
                                                <img src=photo_src alt=alt class=format!("h-11 w-11 rounded-full border-2 object-cover shadow-md sm:h-14 sm:w-14 {ring_cls}")/>
                                                {medal.map(|_| view! {
                                                    <div class=format!("absolute -bottom-0.5 -right-0.5 h-3 w-3 rounded-full border-2 border-neutral-900 {dot_cls}")></div>
                                                })}
                                            </A>
                                            <div class="min-w-0 flex-1">
                                                <div class="flex min-w-0 items-center gap-1.5">
                                                    <A href=user_link(uid) attr:class="block truncate text-base font-bold text-white transition-colors hover:text-yellow-400 sm:text-lg">
                                                        {nm}
                                                    </A>
                                                    <crate::components::tag::UserTags
                                                        role=u["role"].as_str().unwrap_or("").to_string()
                                                        custom_tag=u["custom_tag"].as_str().unwrap_or("").to_string()/>
                                                </div>
                                                <div class="mt-1.5 flex items-center gap-1.5 sm:mt-2">
                                                    {lvl["level"].as_i64().map(|l| view! {
                                                        <crate::components::level::LevelBadge
                                                            level=l
                                                            current_xp=lvl["current_xp"].as_i64().unwrap_or(0)
                                                            xp_for_next_level=lvl["xp_for_next_level"].as_i64().unwrap_or(100)
                                                            size=28 />
                                                    })}
                                                    <crate::components::elo::EloBadge elo=elo_v size="sm" variant="minimal"/>
                                                </div>
                                            </div>
                                            <div class="text-right">
                                                <div class=format!("text-lg font-extrabold sm:text-xl {score_cls}")>
                                                    {e["total_score"].as_i64().unwrap_or(0)}
                                                    <span class="ml-1 text-xs font-normal text-neutral-400 sm:text-sm">{t("PTS")}</span>
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                }
                            }).collect_view()}
                        </div>
                        <Pagination page=page last=last total=total />
                    </div>
                }.into_any()
            }}
        </div>
    }
}

#[component]
fn MetricTip(label: &'static str, tip: &'static str) -> impl IntoView {
    let open = RwSignal::new(false);
    view! {
        <span class="inline-flex items-baseline gap-1">
            <span class="text-[10px] font-medium uppercase tracking-wide text-neutral-500">{label}</span>
            <span class="relative">
                <button type="button" class="text-neutral-500 hover:text-neutral-300"
                    on:click=move |ev| { ev.prevent_default(); ev.stop_propagation(); open.update(|o| *o = !*o); }>
                    <svg class="h-3 w-3" fill="currentColor" viewBox="0 0 20 20">
                        <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-8-3a1 1 0 00-.867.5 1 1 0 11-1.731-1A3 3 0 0113 8a3.001 3.001 0 01-2 2.83V11a1 1 0 11-2 0v-1a1 1 0 011-1 1 1 0 100-2zm0 8a1 1 0 100-2 1 1 0 000 2z" clip-rule="evenodd"/>
                    </svg>
                </button>
                {move || open.get().then(|| view! {
                    <span class="absolute bottom-full left-1/2 z-20 mb-1 w-44 -translate-x-1/2 rounded-md border border-neutral-700 bg-neutral-900 p-2 text-[11px] normal-case tracking-normal text-neutral-300 shadow-lg">
                        {tip}
                    </span>
                })}
            </span>
        </span>
    }
}

#[component]
fn Pagination(page: RwSignal<i64>, last: i64, total: i64) -> impl IntoView {
    let query = use_query_map();
    let location = leptos_router::hooks::use_location();
    let url_for = move |p: i64| {
        let q = query.get_untracked();
        let mut params: Vec<String> = Vec::new();
        if let Some(s) = q.get("sort") {
            params.push(format!("sort={s}"));
        }
        if let Some(r) = q.get("room") {
            params.push(format!("room={r}"));
        }
        if let Some(tb) = q.get("tab") {
            params.push(format!("tab={tb}"));
        }
        if p > 1 {
            params.push(format!("page={p}"));
        }
        let qs = if params.is_empty() {
            String::new()
        } else {
            format!("?{}", params.join("&"))
        };
        format!("{}{qs}", location.pathname.get_untracked())
    };
    let cur = move || page.get();
    view! {
        <Show when=move || { last > 1 }>
            <div class="mt-4">
                <div class="flex flex-wrap justify-center gap-1.5 sm:gap-2">
                    {move || {
                        let p = cur();
                        if p > 1 {
                            let href = url_for(p - 1);
                            view! { <A href=href attr:class="retro-pagination-link">"«"</A> }.into_any()
                        } else {
                            view! { <span class="retro-pagination-link cursor-default text-white/60">"«"</span> }.into_any()
                        }
                    }}
                    {move || {
                        let (cur_p, last_p) = (cur(), last);
                        const SIDE: i64 = 3;
                        let window = SIDE + 4;
                        let mut items: Vec<Option<i64>> = Vec::new(); // None = "..."
                        if last_p < SIDE * 2 + 8 {
                            items.extend((1..=last_p).map(Some));
                        } else if cur_p <= window {
                            items.extend((1..=(SIDE * 2 + 4)).map(Some));
                            items.push(None);
                            items.extend(((last_p - 1)..=last_p).map(Some));
                        } else if cur_p > last_p - window {
                            items.extend([1, 2].into_iter().map(Some));
                            items.push(None);
                            items.extend(((last_p - (SIDE + window - 2))..=last_p).map(Some));
                        } else {
                            items.extend([1, 2].into_iter().map(Some));
                            items.push(None);
                            items.extend(((cur_p - SIDE)..=(cur_p + SIDE)).map(Some));
                            items.push(None);
                            items.extend(((last_p - 1)..=last_p).map(Some));
                        }
                        items.into_iter().map(|it| match it {
                            None => view! {
                                <span class="retro-pagination-link cursor-default text-white/60">"…"</span>
                            }.into_any(),
                            Some(n) => {
                                let href = url_for(n);
                                view! {
                                    <A href=href attr:class=move || {
                                        if cur() == n { "retro-pagination-link retro-pagination-link--active" } else { "retro-pagination-link" }
                                    }>{n}</A>
                                }.into_any()
                            }
                        }).collect_view()
                    }}
                    {move || {
                        let p = cur();
                        if p < last {
                            let href = url_for(p + 1);
                            view! { <A href=href attr:class="retro-pagination-link">"»"</A> }.into_any()
                        } else {
                            view! { <span class="retro-pagination-link cursor-default text-white/60">"»"</span> }.into_any()
                        }
                    }}
                </div>
                <p class="mt-2 text-center text-xs text-white/40">{total} " " {t("results")}</p>
            </div>
        </Show>
    }
}

// ---- season --------------------------------------------------------------------

#[component]
fn SeasonBoard(
    payload: RwSignal<Value>,
    loading: RwSignal<bool>,
    page: RwSignal<i64>,
) -> impl IntoView {
    let me_id = Signal::derive(move || crate::auth::use_auth().user.get().map(|u| u.id));
    view! {
        <div>
            {move || {
                if loading.get() {
                    return view! { <div class="text-white/60">{t("Loading")}"…"</div> }.into_any();
                }
                let p = payload.get();
                if p.is_null() {
                    return view! {
                        <div class="py-16 text-center">
                            <p class="text-white/40">{t("No rankings available")}</p>
                        </div>
                    }.into_any();
                }
                let cur = p["current"].clone();
                let name = cur["name"].as_str().unwrap_or("").to_string();
                let ends_in = cur["ends_in_seconds"].as_i64().unwrap_or(0);
                let days = ends_in / 86400;
                let hours = (ends_in % 86400) / 3600;
                let countdown = crate::i18n::tr("Season ends in", &[
                    ("time", crate::i18n::tr("Season duration format", &[
                        ("d", days.to_string()),
                        ("h", hours.to_string()),
                    ])),
                ]);
                let my = p["user_rank"].clone();
                let rows = p["board"]["data"].as_array().cloned().unwrap_or_default();
                let total = p["board"]["total"].as_i64().unwrap_or(0);
                let last = p["board"]["last_page"].as_i64().unwrap_or(1);
                let past = p["past"].as_array().cloned().unwrap_or_default();
                let first_page = page.get() == 1;
                view! {
                    <div>
                        <div class="mb-5 rounded-xl border border-yellow-500/25 bg-gradient-to-r from-yellow-500/10 to-amber-600/5 p-4 sm:p-5">
                            <div class="flex flex-wrap items-center justify-between gap-3">
                                <div>
                                    <p class="text-[11px] font-semibold uppercase tracking-widest text-yellow-500/80">{t("Season")}</p>
                                    <h2 class="text-xl font-extrabold text-white sm:text-2xl">{name}</h2>
                                    <p class="mt-1 text-xs text-white/50">{t("Season reward hint")}</p>
                                </div>
                                <div class="text-right">
                                    <p class="text-[11px] uppercase tracking-widest text-white/40">{countdown}</p>
                                    {if my["rank"].is_number() {
                                        view! {
                                            <p class="mt-1 text-sm font-bold text-yellow-400">
                                                {t("Your ranking")} " · #" {my["rank"].as_i64().unwrap_or(0)}
                                                " · " {format!("{:.0}", my["total"].as_f64().unwrap_or(0.0))} " pts"
                                            </p>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <p class="mt-1 text-sm text-white/40">{t("Not ranked yet")}</p>
                                        }.into_any()
                                    }}
                                </div>
                            </div>
                        </div>

                        {if rows.is_empty() {
                            view! {
                                <div class="py-16 text-center">
                                    <p class="text-white/40">{t("No scores yet")}</p>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="space-y-2.5 sm:space-y-3">
                                    {rows.into_iter().map(|e| {
                                        let u = e["user"].clone();
                                        let rank = e["rank"].as_i64().unwrap_or(0);
                                        let uid = u["id"].as_i64().unwrap_or(0);
                                        let is_guest = u["is_guest"].as_bool().unwrap_or(false);
                                        let is_me = me_id.get() == Some(uid);
                                        let medal = if first_page {
                                            ["🥇","🥈","🥉"].get((rank - 1) as usize).copied()
                                        } else { None };
                                        let row_cls = if is_me {
                                            "border-yellow-500/50 bg-yellow-500/10"
                                        } else {
                                            match (first_page, rank) {
                                                (true, 1) => "border-yellow-500/30 bg-yellow-500/5",
                                                (true, 2) => "border-gray-400/25 bg-gray-400/5",
                                                (true, 3) => "border-amber-600/25 bg-amber-600/5",
                                                _ => "border-neutral-800 bg-neutral-900/40",
                                            }
                                        };
                                        let photo_src = photo(&u);
                                        let name = u["name"].as_str().unwrap_or("").to_string();
                                        let alt = name.clone();
                                        let total_v = e["total"].as_f64().unwrap_or(0.0);
                                        let rounds_v = e["rounds_played"].as_i64().unwrap_or(0);
                                        let best_v = e["best_round"].as_f64();
                                        let level_v = u["level"].as_i64().unwrap_or(1);
                                        let elo_v = u["elo"].as_i64().unwrap_or(1500);
                                        view! {
                                            <article class=format!("rounded-xl border p-3 shadow-sm transition-colors hover:border-neutral-700 sm:p-4 {row_cls}")>
                                                <div class="flex items-center gap-3">
                                                    <div class="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-neutral-800 text-sm font-bold sm:h-11 sm:w-11">
                                                        {match medal {
                                                            Some(m) => view! { <span class="text-lg">{m}</span> }.into_any(),
                                                            None => view! { <span class="text-neutral-400">"#"{rank}</span> }.into_any(),
                                                        }}
                                                    </div>
                                                    {if !is_guest {
                                                        let alt2 = alt.clone();
                                                        view! {
                                                            <A href=user_link(uid) attr:class="shrink-0">
                                                                <img src=photo_src.clone() alt=alt2 class="h-10 w-10 rounded-full border-2 border-neutral-600 object-cover sm:h-11 sm:w-11" loading="lazy"/>
                                                            </A>
                                                        }.into_any()
                                                    } else {
                                                        view! {
                                                            <img src=photo_src.clone() alt=alt.clone() class="h-10 w-10 shrink-0 rounded-full border-2 border-neutral-600 object-cover sm:h-11 sm:w-11" loading="lazy"/>
                                                        }.into_any()
                                                    }}
                                                    <div class="min-w-0 flex-1">
                                                        <div class="flex min-w-0 items-center gap-1.5">
                                                            {if !is_guest {
                                                                let nm = name.clone();
                                                                view! {
                                                                    <A href=user_link(uid) attr:class="block truncate text-base font-bold text-white hover:text-yellow-400">
                                                                        {nm}
                                                                    </A>
                                                                }.into_any()
                                                            } else {
                                                                let nm = if name.is_empty() { t("Deleted user").to_string() } else { name.clone() };
                                                                view! {
                                                                    <span class="block truncate text-base font-bold text-neutral-400">{nm}</span>
                                                                }.into_any()
                                                            }}
                                                            <crate::components::tag::UserTags
                                                                role=u["role"].as_str().unwrap_or("").to_string()
                                                                custom_tag=u["custom_tag"].as_str().unwrap_or("").to_string()/>
                                                        </div>
                                                        <div class="mt-1.5 flex flex-wrap gap-1.5 text-[11px] text-neutral-400">
                                                            <span class="rounded-md border border-neutral-700/60 bg-neutral-800/50 px-1.5 py-0.5">{t("Level")} " " {level_v}</span>
                                                            <span class="rounded-md border border-neutral-700/60 bg-neutral-800/50 px-1.5 py-0.5">"ELO " {elo_v}</span>
                                                            <span class="rounded-md border border-neutral-700/60 bg-neutral-800/50 px-1.5 py-0.5">{rounds_v} " " {t("Rounds played")}</span>
                                                            {best_v.map(|b| view! {
                                                                <span class="rounded-md border border-neutral-700/60 bg-neutral-800/50 px-1.5 py-0.5">{t("Best round")} " " {format!("{b:.0}")}</span>
                                                            })}
                                                        </div>
                                                    </div>
                                                    <div class="shrink-0 text-right">
                                                        <span class="block text-lg font-extrabold text-yellow-400 sm:text-xl">{format!("{total_v:.0}")}</span>
                                                        <span class="block text-[10px] uppercase text-neutral-500">{t("PTS")}</span>
                                                    </div>
                                                </div>
                                            </article>
                                        }
                                    }).collect_view()}
                                </div>
                                <Pagination page=page last=last total=total />
                            }.into_any()
                        }}

                        {if !past.is_empty() {
                            view! {
                                <div class="mt-10">
                                    <h3 class="mb-3 text-sm font-semibold uppercase tracking-widest text-white/40">{t("Past seasons")}</h3>
                                    <div class="grid gap-3 sm:grid-cols-2">
                                        {past.into_iter().map(|s| {
                                            let sname = s["name"].as_str().unwrap_or("").to_string();
                                            let podium = s["podium"].as_array().cloned().unwrap_or_default();
                                            view! {
                                                <div class="rounded-xl border border-neutral-800 bg-neutral-900/40 p-4">
                                                    <p class="mb-2 text-xs font-semibold uppercase tracking-widest text-white/40">{sname}</p>
                                                    <ul class="space-y-1.5">
                                                        {podium.into_iter().take(3).map(|e| {
                                                            let prank = e["rank"].as_i64().unwrap_or(0);
                                                            let pname = e["name"].as_str().unwrap_or("?").to_string();
                                                            let ptotal = e["total"].as_f64().unwrap_or(0.0);
                                                            let pxp = e["reward_xp"].as_i64().unwrap_or(0);
                                                            let pm = ["🥇","🥈","🥉"].get((prank - 1) as usize).copied().unwrap_or("");
                                                            view! {
                                                                <li class="flex items-center justify-between text-sm">
                                                                    <span class="min-w-0 truncate text-white/80">{pm} " " {pname}</span>
                                                                    <span class="ml-2 shrink-0 text-xs text-white/50">
                                                                        {format!("{ptotal:.0} pts · +{pxp} XP")}
                                                                    </span>
                                                                </li>
                                                            }
                                                        }).collect_view()}
                                                    </ul>
                                                </div>
                                            }
                                        }).collect_view()}
                                    </div>
                                </div>
                            }.into_any()
                        } else {
                            view! { <span></span> }.into_any()
                        }}
                    </div>
                }.into_any()
            }}
        </div>
    }
}
