use crate::api;
use crate::auth::use_auth;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use serde_json::Value;

#[component]
pub fn ProfilePage() -> impl IntoView {
    let params = use_params_map();
    let auth = use_auth();
    crate::auth::require_registered(auth);
    let profile = RwSignal::new(Value::Null);
    let highlights = RwSignal::new(Value::Null);
    let donations = RwSignal::new(Vec::<Value>::new());
    let evolution = RwSignal::new(Vec::<(String, f64)>::new());
    let initial_tab = leptos_router::hooks::use_query_map()
        .get_untracked()
        .get("tab")
        .filter(|tb| ["scores", "likes", "bookmarks", "minigames"].contains(&tb.as_str()))
        .unwrap_or_else(|| "scores".to_string());
    let tab = RwSignal::new(initial_tab);
    let tab_data = RwSignal::new(Value::Null);
    let loading = RwSignal::new(true);
    let not_found = RwSignal::new(false);
    let level_modal = RwSignal::new(false);

    let user_id = move || params.get().get("id").unwrap_or_default();

    leptos::task::spawn_local({
        async move {
            let uid = user_id();
            match api::get::<Value>(&format!("/api/v1/profile/{uid}")).await {
                Ok(v) => {
                    crate::i18n::set_title(v["name"].as_str());
                    profile.set(v);
                }
                Err(e) if e.status == 404 => not_found.set(true),
                Err(_) => {}
            }
            loading.set(false);
            if let Ok(v) = api::get::<Value>(&format!("/api/v1/profile/{uid}/highlights")).await {
                highlights.set(v);
            }
            if let Ok(v) = api::get::<Value>(&format!("/api/v1/users/{uid}/donations")).await {
                donations.set(v["donations"].as_array().cloned().unwrap_or_default());
            }
            if let Ok(v) = api::get::<Value>(&format!("/api/v1/profile/{uid}/score-evolution")).await {
                evolution.set(
                    v["score_evolution"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|e| {
                                    Some((
                                        e["date"].as_str()?.to_string(),
                                        e["total_score"].as_f64()?,
                                    ))
                                })
                                .collect()
                        })
                        .unwrap_or_default(),
                );
            }
        }
    });

    let tab_page = RwSignal::new(1i64);
    Effect::new(move |_| {
        let (tab_v, page_v) = (tab.get(), tab_page.get());
        let uid = user_id();
        leptos::task::spawn_local(async move {
            let url = match tab_v.as_str() {
                "likes" => format!("/api/v1/profile/{uid}/likes?page={page_v}"),
                "bookmarks" => format!("/api/v1/profile/{uid}/bookmarks?page={page_v}"),
                "minigames" => format!("/api/v1/profile/{uid}/minigames?page={page_v}"),
                _ => format!("/api/v1/profile/{uid}/scores?page={page_v}"),
            };
            if let Ok(v) = api::get::<Value>(&url).await {
                tab_data.set(v);
            }
        });
    });

    let unlike = move |track_id: i64| {
        leptos::task::spawn_local(async move {
            let _ = api::delete::<Value>(&format!("/api/v1/profile/likes/{track_id}")).await;
            tab_data.update(|d| {
                if let Some(arr) = d["data"].as_array_mut() {
                    arr.retain(|r| r["id"].as_i64() != Some(track_id));
                }
            });
        });
    };

    view! {
        <div class="mx-auto max-w-5xl px-4 py-8">
            <Show when=move || not_found.get()>
                <div class="py-24 text-center text-white/60">{t("Player not found")}</div>
            </Show>
            <Show when=move || !loading.get() && !not_found.get()>
                <ProfileHeader profile=profile on_level=move |_| level_modal.set(true) />
                {move || {
                    let h = highlights.get();
                    if h.is_null() {
                        return view! {
                            <div class="mb-6 space-y-4">
                                <div class="h-28 animate-pulse rounded-2xl border border-white/10 bg-white/5"></div>
                                <div class="grid gap-3 sm:grid-cols-3">
                                    <div class="h-36 animate-pulse rounded-xl border border-white/10 bg-white/5"></div>
                                    <div class="h-36 animate-pulse rounded-xl border border-white/10 bg-white/5"></div>
                                    <div class="h-36 animate-pulse rounded-xl border border-white/10 bg-white/5"></div>
                                </div>
                            </div>
                        }.into_any();
                    }
                    view! {
                        <div class="mb-6 space-y-4">
                            <PerformanceBanner perf=h["performance"].clone() rank=h["rank"].clone() />
                            <ProfileBadges badges=h["badges"].as_array().cloned().unwrap_or_default() />
                            <ProfileTopRooms rooms=h["top_rooms"].as_array().cloned().unwrap_or_default() profile_id=user_id() />
                        </div>
                    }.into_any()
                }}

                <div class="grid gap-6 lg:grid-cols-12">
                    <aside class="space-y-4 lg:col-span-4">
                        <div class="rounded-xl border border-white/10 bg-surface-raised/60 p-4">
                            <LevelInfoCard profile=profile compact=true />
                        </div>
                        {move || {
                            let ds = profile.get()["donation_summary"].clone();
                            (ds["donation_count"].as_i64().unwrap_or(0) > 0).then(|| {
                                view! {
                                    <div class="rounded-xl border border-white/10 bg-surface-raised/60">
                                        <div class="border-b border-white/10 px-4 py-3">
                                            <h2 class="text-lg font-bold text-white">{t("Donation history")}</h2>
                                        </div>
                                        <div class="p-4">
                                            <DonationHistoryList items=donations.get() summary=ds />
                                        </div>
                                    </div>
                                }
                            })
                        }}
                        <div class="rounded-xl border border-white/10 bg-surface-raised/60">
                            <div class="border-b border-white/10 px-4 py-3">
                                <h2 class="text-lg font-bold text-white">{t("Evolution")}</h2>
                            </div>
                            <div class="p-3">
                                <ScoreEvolutionChart points=evolution />
                            </div>
                        </div>
                    </aside>

                    <main class="space-y-4 lg:col-span-8">
                        <nav class="flex gap-1 overflow-x-auto rounded-xl border border-white/10 bg-brand-deep/80 p-1">
                            {["scores", "likes", "bookmarks", "minigames"].into_iter().map(|tb| {
                                let tb_s = tb.to_string();
                                let tb_s2 = tb.to_string();
                                view! {
                                    <button type="button"
                                        class=move || if tab.get() == tb_s {
                                            "flex-1 whitespace-nowrap rounded-lg bg-brand-primary/20 px-4 py-2.5 text-center text-sm font-semibold text-white ring-1 ring-brand-primary/40 transition"
                                        } else {
                                            "flex-1 whitespace-nowrap rounded-lg px-4 py-2.5 text-center text-sm font-semibold text-white/55 transition hover:bg-white/5 hover:text-white"
                                        }
                                        on:click=move |_| { tab.set(tb_s2.clone()); tab_page.set(1); }>
                                        {t(match tb {
                                            "scores" => "Scores", "likes" => "Likes",
                                            "bookmarks" => "Bookmarks", _ => "Mini-games",
                                        })}
                                    </button>
                                }
                            }).collect::<Vec<_>>()}
                        </nav>
                        <div class="rounded-xl border border-white/10 bg-surface-raised/60 p-4">
                            {move || {
                                match tab.get().as_str() {
                                    "likes" => view! {
                                        <LikesTab data=tab_data auth=auth unlike=unlike />
                                    }.into_any(),
                                    "bookmarks" => view! { <BookmarksTab data=tab_data /> }.into_any(),
                                    "minigames" => view! { <MinigamesTab data=tab_data profile_id=user_id() /> }.into_any(),
                                    _ => view! { <ScoresTab data=tab_data /> }.into_any(),
                                }
                            }}
                        </div>
                    </main>
                </div>

                {move || level_modal.get().then(|| {
                    view! {
                        <div class="fixed inset-0 z-50 flex items-center justify-center p-4"
                            on:click=move |_| level_modal.set(false)>
                            <div class="absolute inset-0 bg-black/70"></div>
                            <div class="relative w-full max-w-md rounded-2xl border border-white/10 bg-brand-midnight p-6 shadow-2xl"
                                on:click=move |ev| ev.stop_propagation()>
                                <button type="button" class="absolute right-3 top-3 text-white/40 hover:text-white"
                                    on:click=move |_| level_modal.set(false)>
                                    <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"/>
                                    </svg>
                                </button>
                                <LevelInfoCard profile=profile compact=false />
                            </div>
                        </div>
                    }
                })}
            </Show>
        </div>
    }
}

#[component]
fn ProfileHeader(profile: RwSignal<Value>, on_level: impl Fn(leptos::ev::MouseEvent) + Copy + Send + Sync + 'static) -> impl IntoView {
    view! {
        {move || {
            let p = profile.get();
            if p.is_null() {
                return view! { <div/> }.into_any();
            }
            let level = p["level"].as_i64().unwrap_or(1);
            let created = p["created_at"].as_str().unwrap_or("").chars().take(10).collect::<String>();
            let photo = p["photo"].as_str().unwrap_or("").to_string();
            let name = p["name"].as_str().unwrap_or("").to_string();
            let initial = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
            view! {
                <section class="mb-6 overflow-hidden rounded-2xl border border-white/10 bg-gradient-to-br from-brand-deep via-brand-midnight to-neutral-900 shadow-lg">
                    <div class="p-6 sm:p-8">
                        <div class="flex flex-col items-center gap-6 text-center sm:flex-row sm:items-start sm:text-left">
                            <div class="relative shrink-0">
                                {if photo.is_empty() {
                                    view! {
                                        <div class="flex h-24 w-24 items-center justify-center rounded-full border-2 border-white/15 bg-neutral-700 text-3xl font-black text-neutral-300 shadow-lg sm:h-28 sm:w-28">
                                            {initial}
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {
                                        <img src=photo class="h-24 w-24 rounded-full border-2 border-white/15 object-cover shadow-lg sm:h-28 sm:w-28" alt=""/>
                                    }.into_any()
                                }}
                                <button type="button"
                                    class="absolute -bottom-1 -right-1 flex h-8 w-8 items-center justify-center rounded-full border-2 border-brand-midnight bg-brand-primary text-sm font-black text-white shadow-lg transition hover:scale-110"
                                    title={t("XP Breakdown")}
                                    on:click=on_level>
                                    {level}
                                </button>
                            </div>
                            <div class="min-w-0 flex-1">
                                <h1 class="retro-title retro-title--white text-3xl sm:text-4xl">
                                    {name.clone()}
                                    {p["is_supporter"].as_bool().unwrap_or(false).then(|| view! {
                                        <span class="ml-2 inline-block rounded-full bg-amber-400/20 px-2.5 py-1 align-middle text-xs font-bold text-amber-300">
                                            "★ " {t("Supporter")}
                                        </span>
                                    })}
                                </h1>
                                <p class="mt-1 text-sm text-white/50">{t("Member since")} " " {created}</p>
                            </div>
                            {p["team"].as_object().map(|tm| {
                                let tid = tm["id"].as_i64().unwrap_or(0);
                                let tname = tm["name"].as_str().unwrap_or("").to_string();
                                let tphoto = tm["photo"].as_str().unwrap_or("").to_string();
                                let tinitial = tname.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
                                view! {
                                    <A href=format!("/teams/{tid}") attr:class="group inline-flex max-w-full items-center gap-3 rounded-xl border border-violet-500/25 bg-violet-950/30 px-3 py-2 transition hover:border-violet-400/40">
                                        {if tphoto.is_empty() {
                                            view! {
                                                <div class="flex h-10 w-10 items-center justify-center rounded-full bg-neutral-800 text-sm font-black text-violet-300 ring-2 ring-violet-400/30">
                                                    {tinitial}
                                                </div>
                                            }.into_any()
                                        } else {
                                            view! {
                                                <img src=tphoto alt=tname.clone() class="h-10 w-10 rounded-full object-cover ring-2 ring-violet-400/40"/>
                                            }.into_any()
                                        }}
                                        <div class="min-w-0 text-left">
                                            <p class="text-[10px] font-bold uppercase tracking-wider text-violet-300/80">{t("Squad")}</p>
                                            <p class="truncate font-semibold text-white group-hover:text-violet-100">{tname}</p>
                                        </div>
                                    </A>
                                }
                            })}
                        </div>

                        <div class="mt-6 grid grid-cols-2 gap-3 sm:grid-cols-3">
                            <div class="rounded-xl border border-white/10 bg-white/5 px-3 py-3 text-center">
                                <p class="text-2xl font-black tabular-nums text-white">
                                    {p["total_score"].as_f64().map(|v| format!("{v:.0}")).unwrap_or_else(|| "0".into())}
                                </p>
                                <p class="text-[10px] font-semibold uppercase tracking-wide text-white/45">{t("PTS")}</p>
                            </div>
                            <div class="rounded-xl border border-white/10 bg-white/5 px-3 py-3 text-center">
                                <div class="flex justify-center">
                                    <crate::components::elo::EloBadge
                                        elo=p["elo"].as_i64().unwrap_or(1500) size="md" variant="compact"/>
                                </div>
                                <p class="mt-1 text-[10px] font-semibold uppercase tracking-wide text-white/45">"ELO"</p>
                            </div>
                            <div class="rounded-xl border border-white/10 bg-white/5 px-3 py-3 text-center">
                                <p class="text-xl font-bold text-white">{level}</p>
                                <p class="text-[10px] font-semibold uppercase tracking-wide text-white/45">{t("Level")}</p>
                            </div>
                        </div>
                        <div class="mt-3 grid grid-cols-3 gap-2">
                            {[
                                ("Rooms", p["stats"]["rooms"].as_i64().unwrap_or(0)),
                                ("Playlists", p["stats"]["playlists"].as_i64().unwrap_or(0)),
                                ("Bookmarks", p["stats"]["bookmarks"].as_i64().unwrap_or(0)),
                            ].into_iter().map(|(label, v)| view! {
                                <div class="rounded-lg border border-white/5 bg-white/[0.03] px-2 py-2 text-center">
                                    <p class="text-sm font-bold text-white/90">{v}</p>
                                    <p class="text-[10px] text-white/40">{t(label)}</p>
                                </div>
                            }).collect::<Vec<_>>()}
                        </div>
                        <div class="mt-4 flex flex-wrap justify-center gap-4 text-xs text-white/55 sm:justify-start">
                            <span>
                                {t("Public")} " " <strong class="text-white">{p["total_public_score"].as_f64().map(|v| format!("{v:.0}")).unwrap_or_else(|| "0".into())}</strong> " " {t("PTS")}
                            </span>
                            <span>
                                {t("Community")} " " <strong class="text-white">{p["total_private_score"].as_f64().map(|v| format!("{v:.0}")).unwrap_or_else(|| "0".into())}</strong> " " {t("PTS")}
                            </span>
                        </div>
                    </div>
                </section>
            }.into_any()
        }}
    }
}

#[component]
fn PerformanceBanner(perf: Value, rank: Value) -> impl IntoView {
    let position = rank["position"].as_i64();
    let fmt = |v: f64| -> String {
        if v >= 1000.0 { format!("{v:.0}") } else { format!("{}", (v * 10.0).round() / 10.0) }
    };
    let stats: Vec<(&'static str, String, &'static str)> = vec![
        ("Rounds played", perf["rounds_played"].as_i64().unwrap_or(0).to_string(), "text-white"),
        ("Best round", fmt(perf["best_round_score"].as_f64().unwrap_or(0.0)), "text-brand-secondary"),
        ("Best win streak", perf["best_win_streak"].as_i64().unwrap_or(0).to_string(), "text-white"),
        ("Day streak", perf["consecutive_days_streak"].as_i64().unwrap_or(0).to_string(), "text-amber-300"),
        ("This week", fmt(perf["week_score"].as_f64().unwrap_or(0.0)), "text-white"),
    ];
    view! {
        <section class="rounded-2xl border border-white/10 bg-brand-deep/60 p-4 sm:p-5">
            <div class="mb-4 flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                <h2 class="text-sm font-bold uppercase tracking-wider text-white/70">{t("Performance")}</h2>
                {match position {
                    Some(pos) => view! {
                        <A href="/rankings?sort=elo" attr:class="inline-flex items-center gap-2 rounded-lg border border-brand-secondary/30 bg-brand-secondary/10 px-3 py-1.5 text-sm font-semibold text-brand-secondary transition hover:bg-brand-secondary/20">
                            <span>#{pos}</span>
                            <span class="text-white/60">{t("Global ELO rank")}</span>
                        </A>
                    }.into_any(),
                    None => view! {
                        <span class="text-sm text-white/45">{t("Not ranked yet")}</span>
                    }.into_any(),
                }}
            </div>
            <div class="grid grid-cols-2 gap-3 sm:grid-cols-5">
                {stats.into_iter().enumerate().map(|(i, (k, v, color))| {
                    let span = if i == 4 { "col-span-2 sm:col-span-1" } else { "" };
                    view! {
                        <div class=format!("rounded-xl border border-white/10 bg-white/5 px-3 py-3 text-center {span}")>
                            <p class=format!("text-xl font-black tabular-nums {color}")>{v}</p>
                            <p class="text-[10px] font-semibold uppercase tracking-wide text-white/45">{t(k)}</p>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </section>
    }
}

fn badge_icon(name: &str) -> impl IntoView {
    let paths: Vec<&'static str> = match name {
        "crown" => vec![
            "M11.562 3.266a.5.5 0 0 1 .876 0L15.39 8.87a1 1 0 0 0 1.516.294L21.183 5.5a.5.5 0 0 1 .798.519l-2.834 10.246a1 1 0 0 1-.956.735H5.81a1 1 0 0 1-.957-.735L2.02 6.02a.5.5 0 0 1 .798-.519l4.276 3.664a1 1 0 0 0 1.516-.294z",
            "M5 21h14",
        ],
        "sparkles" => vec![
            "m12 3-1.912 5.813a2 2 0 0 1-1.275 1.275L3 12l5.813 1.912a2 2 0 0 1 1.275 1.275L12 21l1.912-5.813a2 2 0 0 1 1.275-1.275L21 12l-5.813-1.912a2 2 0 0 1-1.275-1.275L12 3Z",
            "M5 3v4",
            "M19 17v4",
            "M3 5h4",
            "M17 19h4",
        ],
        "map-pin" => vec![
            "M20 10c0 6-8 12-8 12s-8-6-8-12a8 8 0 0 1 16 0Z",
        ],
        "flame" => vec![
            "M8.5 14.5A2.5 2.5 0 0 0 11 12c0-1.38-.5-2-1-3-1.072-2.143-.224-4.054 2-6 .5 2.5 2 4.9 4 6.5 2 1.6 3 3.5 3 5.5a7 7 0 1 1-14 0c0-1.153.433-2.294 1-3a2.5 2.5 0 0 0 2.5 2.5z",
        ],
        "gamepad" => vec![
            "M6 11h4",
            "M8 9v4",
            "M15 12h.01",
            "M18 10h.01",
            "M17.32 5H6.68a4 4 0 0 0-3.978 3.59c-.006.052-.01.101-.017.152C2.604 9.416 2 14.456 2 16a3 3 0 0 0 3 3c1 0 1.5-.5 2-1l1.414-1.414A2 2 0 0 1 9.828 16h4.344a2 2 0 0 1 1.414.586L17 18c.5.5 1 1 2 1a3 3 0 0 0 3-3c0-1.545-.604-6.584-.685-7.258-.007-.05-.011-.1-.017-.151A4 4 0 0 0 17.32 5z",
        ],
        "badge-check" => vec![
            "M3.85 8.62a4 4 0 0 1 4.78-4.77 4 4 0 0 1 6.74 0 4 4 0 0 1 4.78 4.78 4 4 0 0 1 0 6.74 4 4 0 0 1-4.77 4.78 4 4 0 0 1-6.75 0 4 4 0 0 1-4.78-4.77 4 4 0 0 1 0-6.76Z",
            "m9 12 2 2 4-4",
        ],
        "trophy" => vec![
            "M6 9H4.5a2.5 2.5 0 0 1 0-5H6",
            "M18 9h1.5a2.5 2.5 0 0 0 0-5H18",
            "M4 22h16",
            "M10 14.66V17c0 .55-.47.98-.97 1.21C7.85 18.75 7 20.24 7 22",
            "M14 14.66V17c0 .55.47.98.97 1.21C16.15 18.75 17 20.24 17 22",
            "M18 2H6v7a6 6 0 0 0 12 0V2Z",
        ],
        "heart" => vec![
            "M19 14c1.49-1.46 3-3.21 3-5.5A5.5 5.5 0 0 0 16.5 3c-1.76 0-3 .5-4.5 2-1.5-1.5-2.74-2-4.5-2A5.5 5.5 0 0 0 2 8.5c0 2.3 1.5 4.05 3 5.5l7 7Z",
        ],
        _ => vec!["M12 2l3 6 7 1-5 5 1 7-6-3-6 3 1-7-5-5 7-1z"],
    };
    let circle = (name == "map-pin").then(|| view! {
        <circle cx="12" cy="10" r="3"/>
    });
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none"
            stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"
            class="h-4 w-4 shrink-0">
            {paths.into_iter().map(|d| view! { <path d=d/> }).collect::<Vec<_>>()}
            {circle}
        </svg>
    }
}

#[component]
fn ProfileBadges(badges: Vec<Value>) -> impl IntoView {
    if badges.is_empty() {
        return view! { <div/> }.into_any();
    }
    let earned: Vec<Value> = badges.iter().filter(|b| b["earned"].as_bool().unwrap_or(false)).cloned().collect();
    let in_progress: Vec<Value> = badges.iter()
        .filter(|b| !b["earned"].as_bool().unwrap_or(false)
            && b["target"].as_i64().is_some()
            && b["current"].as_i64().unwrap_or(0) < b["target"].as_i64().unwrap_or(0))
        .take(3).cloned().collect();
    view! {
        <section class="overflow-visible rounded-2xl border border-white/10 bg-brand-deep/40 p-4 sm:p-5">
            <h2 class="mb-3 text-sm font-bold uppercase tracking-wider text-white/70">{t("Badges")}</h2>
            {if earned.is_empty() {
                view! { <p class="text-sm text-white/45">{t("No badges earned yet")}</p> }.into_any()
            } else {
                view! {
                    <div class="flex flex-wrap gap-2 overflow-visible">
                        {earned.into_iter().map(|b| {
                            let icon = b["icon"].as_str().unwrap_or("").to_string();
                            let label = t(b["label_key"].as_str().unwrap_or("")).to_string();
                            let desc = t(b["description_key"].as_str().unwrap_or("")).to_string();
                            view! {
                                <div class="group relative inline-flex max-w-full cursor-default items-center gap-1.5 rounded-full border border-brand-primary/30 bg-brand-primary/10 px-3 py-1.5 text-sm font-medium text-white">
                                    <span class="text-brand-secondary">{badge_icon(&icon)}</span>
                                    <span class="truncate">{label}</span>
                                    <div class="pointer-events-none invisible absolute left-1/2 top-full z-50 mt-2 w-56 -translate-x-1/2 rounded-md border border-white/15 bg-brand-midnight px-2.5 py-2 text-[11px] font-normal normal-case leading-snug tracking-normal text-white/90 opacity-0 shadow-xl transition-opacity group-hover:visible group-hover:opacity-100">
                                        {desc}
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                }.into_any()
            }}
            {(!in_progress.is_empty()).then(|| view! {
                <div class="mt-4 space-y-3">
                    <p class="text-xs font-semibold uppercase tracking-wide text-white/45">{t("Badges in progress")}</p>
                    {in_progress.into_iter().map(|b| {
                        let icon = b["icon"].as_str().unwrap_or("").to_string();
                        let label = t(b["label_key"].as_str().unwrap_or("")).to_string();
                        let desc = t(b["description_key"].as_str().unwrap_or("")).to_string();
                        let cur = b["current"].as_i64().unwrap_or(0);
                        let tgt = b["target"].as_i64().unwrap_or(1).max(1);
                        let pct = (cur as f64 / tgt as f64 * 100.0).min(100.0);
                        let url = b["progress_url"].as_str().unwrap_or("#").to_string();
                        view! {
                            <A href=url attr:class="group/badge block rounded-xl border border-white/10 bg-white/5 p-3 transition hover:border-brand-primary/30">
                                <div class="mb-1 flex items-start justify-between gap-3">
                                    <div class="min-w-0">
                                        <div class="flex items-center gap-2">
                                            <span class="text-white/50">{badge_icon(&icon)}</span>
                                            <span class="truncate text-sm font-medium text-white/80">{label}</span>
                                        </div>
                                        <p class="mt-1 text-xs leading-relaxed text-white/45">{desc}</p>
                                    </div>
                                    <span class="shrink-0 text-xs tabular-nums text-white/45">{cur}"/"{tgt}</span>
                                </div>
                                <div class="h-1.5 overflow-hidden rounded-full bg-white/10">
                                    <div class="h-full rounded-full bg-brand-secondary transition-all" style=format!("width: {pct:.0}%")/>
                                </div>
                            </A>
                        }
                    }).collect::<Vec<_>>()}
                </div>
            })}
        </section>
    }.into_any()
}

#[component]
fn ProfileTopRooms(rooms: Vec<Value>, profile_id: String) -> impl IntoView {
    if rooms.is_empty() {
        return view! { <div/> }.into_any();
    }
    view! {
        <section class="rounded-2xl border border-white/10 bg-brand-deep/40 p-4 sm:p-5">
            <div class="mb-3 flex items-center justify-between gap-3">
                <h2 class="text-sm font-bold uppercase tracking-wider text-white/70">{t("Top rooms")}</h2>
                <A href=format!("/profile/{profile_id}?tab=scores") attr:class="text-xs font-semibold text-brand-secondary hover:text-brand-secondary/80">
                    {t("View all")}
                </A>
            </div>
            <div class="grid gap-3 sm:grid-cols-3">
                {rooms.into_iter().enumerate().map(|(i, entry)| {
                    let room = entry["room"].clone();
                    let slug = room["slug"].as_str().unwrap_or("").to_string();
                    let photo = room["photo"].as_str().unwrap_or("").to_string();
                    let name = room["name"].as_str().unwrap_or("").to_string();
                    let initial = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
                    view! {
                        <A href=format!("/rooms/{slug}") attr:class="group relative overflow-hidden rounded-xl border border-white/10 bg-white/5 transition hover:border-brand-primary/30 hover:bg-white/10">
                            <div class="absolute left-2 top-2 z-10 flex h-6 w-6 items-center justify-center rounded-full bg-brand-midnight/90 text-xs font-black text-brand-secondary">
                                {i + 1}
                            </div>
                            {if photo.is_empty() {
                                view! {
                                    <div class="flex h-24 items-center justify-center bg-brand-midnight/80 text-2xl font-black text-white/30">
                                        {initial}
                                    </div>
                                }.into_any()
                            } else {
                                view! {
                                    <img src=photo alt=name.clone() class="h-24 w-full object-cover opacity-90 transition group-hover:opacity-100" loading="lazy"/>
                                }.into_any()
                            }}
                            <div class="space-y-1 p-3">
                                <p class="truncate font-semibold text-white group-hover:text-brand-secondary">{name}</p>
                                <p class="text-sm font-bold text-brand-secondary">
                                    {entry["score"].as_f64().unwrap_or(0.0)} " " {t("PTS")}
                                </p>
                                <p class="text-[10px] text-white/45">{entry["updated_at"].as_str().unwrap_or("").to_string()}</p>
                            </div>
                        </A>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </section>
    }.into_any()
}

#[component]
fn LevelInfoCard(profile: RwSignal<Value>, compact: bool) -> impl IntoView {
    view! {
        {move || {
            let p = profile.get();
            if p.is_null() {
                return view! { <div/> }.into_any();
            }
            let level = p["level"].as_i64().unwrap_or(1);
            let xp = p["current_xp"].as_i64().unwrap_or(0);
            let next = p["xp_for_next_level"].as_i64().unwrap_or(100).max(1);
            let total = p["total_xp"].as_i64().unwrap_or(0);
            let pct = (xp as f64 / next as f64 * 100.0).min(100.0);
            let (txt, bar) = match level {
                l if l >= 50 => ("text-purple-500", "bg-purple-500"),
                l if l >= 30 => ("text-yellow-500", "bg-yellow-500"),
                l if l >= 20 => ("text-blue-500", "bg-blue-500"),
                l if l >= 10 => ("text-green-500", "bg-green-500"),
                _ => ("text-neutral-400", "bg-neutral-500"),
            };
            let m = &p["level_metrics"];
            let metrics: Vec<(&'static str, String, i64, Option<i64>)> = vec![
                ("Total score", m["score_public_rooms"].as_i64().unwrap_or(0).to_string(),
                    m["score_public_rooms"].as_i64().unwrap_or(0), None),
                ("Mini-games score", m["minigame_scores_total"].as_i64().unwrap_or(0).to_string(),
                    m["minigame_scores_total"].as_i64().unwrap_or(0).min(1000), Some(1000)),
                ("Seniority", format!("{}", m["seniority_months"].as_i64().unwrap_or(0)),
                    (m["seniority_months"].as_i64().unwrap_or(0) * 50).min(600), Some(600)),
                ("Consecutive days streak", m["consecutive_days_streak"].as_i64().unwrap_or(0).to_string(),
                    (m["consecutive_days_streak"].as_i64().unwrap_or(0) * 10).min(300), Some(300)),
                ("Rooms created", m["rooms_created_count"].as_i64().unwrap_or(0).to_string(),
                    (m["rooms_created_count"].as_i64().unwrap_or(0) * 100).min(1000), Some(1000)),
                ("Playlists created", m["playlists_created_count"].as_i64().unwrap_or(0).to_string(),
                    (m["playlists_created_count"].as_i64().unwrap_or(0) * 20).min(2000), Some(2000)),
                ("Tracks liked", m["tracks_liked_count"].as_i64().unwrap_or(0).to_string(),
                    (m["tracks_liked_count"].as_i64().unwrap_or(0) * 5).min(1000), Some(1000)),
                ("Team", if m["has_team"].as_bool().unwrap_or(false) { t("Yes").to_string() } else { t("No").to_string() },
                    if m["has_team"].as_bool().unwrap_or(false) { 200 } else { 0 }, Some(200)),
            ];
            view! {
                <div class="space-y-4">
                    <div class="flex items-center gap-3">
                        <span class=format!("flex h-9 w-9 items-center justify-center rounded-full bg-brand-primary text-sm font-black text-white")>
                            {level}
                        </span>
                        <div>
                            <h3 class=format!("text-lg font-bold {txt}")>{t("Level")} " " {level}</h3>
                            <p class="text-xs text-neutral-400">
                                {t("Total XP")} ": " <span class=format!("font-semibold {txt}")>{total} " " {t("XP")}</span>
                            </p>
                        </div>
                    </div>
                    <div class="rounded-lg border border-neutral-700 bg-neutral-800/50 p-4">
                        <div class="mb-3">
                            <div class="mb-2 flex items-center justify-between">
                                <span class="text-xs font-medium text-neutral-400">{t("XP in this level")}</span>
                                <span class=format!("text-xs font-bold {txt}")>{xp} " / " {next} " " {t("XP")}</span>
                            </div>
                            <div class="relative h-2 w-full overflow-hidden rounded-full bg-neutral-700">
                                <div class=format!("h-full rounded-full transition-all duration-500 ease-out {bar}")
                                    style=format!("width: {pct:.0}%")/>
                            </div>
                            <div class="mt-1 flex justify-between text-xs text-neutral-500">
                                <span>{t("Progress to next level")}</span>
                                <span>{format!("{pct:.0}")} "%"</span>
                            </div>
                        </div>
                        <div class="border-t border-neutral-700 pt-3">
                            <div class="flex items-center justify-between">
                                <span class="text-xs text-neutral-400">{t("XP needed")}</span>
                                <span class=format!("text-sm font-bold {txt}")>{next - xp} " " {t("XP")}</span>
                            </div>
                        </div>
                    </div>
                    {(!compact).then(|| view! {
                        <div class="rounded-lg border border-neutral-700 bg-neutral-800/50 p-4">
                            <h4 class="mb-3 text-sm font-bold text-neutral-300">{t("XP Breakdown")}</h4>
                            <div class="space-y-2">
                                {metrics.into_iter().map(|(key, val, mxp, max)| {
                                    let unit = match key {
                                        "Seniority" => t("months"),
                                        "Consecutive days streak" => t("days"),
                                        "Team" => "",
                                        "Total score" | "Mini-games score" => t("points"),
                                        _ => "",
                                    };
                                    view! {
                                        <div class="flex items-center justify-between rounded-lg bg-neutral-900/50 px-3 py-2">
                                            <div class="min-w-0 flex-1">
                                                <div class="mb-0.5 flex items-center gap-2">
                                                    <span class="truncate text-xs font-semibold text-neutral-300">{t(key)}</span>
                                                    {max.filter(|mx| mxp >= *mx).map(|_| view! {
                                                        <span class="flex-shrink-0 rounded bg-yellow-500/20 px-1.5 py-0.5 text-xs font-semibold text-yellow-400">"MAX"</span>
                                                    })}
                                                </div>
                                                <p class="truncate text-xs text-neutral-500">{val} " " {unit}</p>
                                            </div>
                                            <div class="ml-2 flex-shrink-0 text-right">
                                                <p class=format!("text-sm font-bold {}", if mxp > 0 { txt } else { "text-neutral-500" })>{mxp}</p>
                                                <p class="text-xs text-neutral-500">{t("XP")}</p>
                                            </div>
                                        </div>
                                    }
                                }).collect::<Vec<_>>()}
                            </div>
                        </div>
                    })}
                </div>
            }.into_any()
        }}
    }
}

#[component]
fn DonationHistoryList(items: Vec<Value>, summary: Value) -> impl IntoView {
    if items.is_empty() {
        return view! { <div/> }.into_any();
    }
    view! {
        <div class="space-y-4">
            <div class="grid grid-cols-3 gap-2">
                <div class="rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-center">
                    <p class="text-lg font-bold text-white">
                        {format!("{:.0} €", summary["total_cents"].as_f64().unwrap_or(0.0) / 100.0)}
                    </p>
                    <p class="text-[10px] uppercase tracking-wide text-white/50">{t("Total donated")}</p>
                </div>
                <div class="rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-center">
                    <p class="text-lg font-bold text-white">{summary["donation_count"].as_i64().unwrap_or(0)}</p>
                    <p class="text-[10px] uppercase tracking-wide text-white/50">{t("Donations")}</p>
                </div>
                <div class="rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-center">
                    <p class="text-lg font-bold text-white">{summary["months_supported"].as_i64().unwrap_or(0)}</p>
                    <p class="text-[10px] uppercase tracking-wide text-white/50">{t("Months supported")}</p>
                </div>
            </div>
            <ul class="divide-y divide-white/10 rounded-lg border border-white/10 bg-white/5">
                {items.into_iter().map(|d| {
                    let date = d["donated_at"].as_str().unwrap_or("").chars().take(10).collect::<String>();
                    view! {
                        <li class="flex items-center justify-between gap-3 px-3 py-2.5">
                            <span class="text-xs text-white/50">{date}</span>
                            <span class="text-sm font-bold text-amber-300">
                                {format!("{:.0} {}", d["amount_cents"].as_f64().unwrap_or(0.0) / 100.0, "€")}
                            </span>
                        </li>
                    }
                }).collect::<Vec<_>>()}
            </ul>
        </div>
    }.into_any()
}

#[component]
fn ScoreEvolutionChart(points: RwSignal<Vec<(String, f64)>>) -> impl IntoView {
    view! {
        {move || {
            let pts = points.get();
            if pts.len() < 2 {
                return view! {
                    <p class="p-4 text-center text-sm text-white/45">{t("No data yet")}</p>
                }.into_any();
            }
            let max_v = pts.iter().map(|(_, v)| *v).fold(0f64, f64::max).max(1.0);
            let (w, h, pad) = (560.0, 160.0, 8.0);
            let n = pts.len() as f64;
            let path: String = pts
                .iter()
                .enumerate()
                .map(|(i, (_, v))| {
                    let x = pad + (i as f64 / (n - 1.0).max(1.0)) * (w - pad * 2.0);
                    let y = h - pad - (v / max_v) * (h - pad * 2.0);
                    format!("{x:.1},{y:.1}")
                })
                .collect::<Vec<_>>()
                .join(" ");
            let first_label = pts.first().map(|(d, _)| d.chars().take(7).collect::<String>()).unwrap_or_default();
            let last_label = pts.last().map(|(d, _)| d.chars().take(7).collect::<String>()).unwrap_or_default();
            view! {
                <div>
                    <svg viewBox=format!("0 0 {w} {h}") class="h-40 w-full">
                        <polyline
                            points=path
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2"
                            class="text-brand-secondary"
                            stroke-linecap="round"
                            stroke-linejoin="round"/>
                    </svg>
                    <div class="flex justify-between px-1 text-[10px] text-white/40">
                        <span>{first_label}</span>
                        <span>{last_label}</span>
                    </div>
                </div>
            }.into_any()
        }}
    }
}

#[component]
fn ScoresTab(data: RwSignal<Value>) -> impl IntoView {
    view! {
        {move || {
            let rows = data.get()["data"].as_array().cloned().unwrap_or_default();
            if rows.is_empty() {
                return view! {
                    <div class="rounded-xl border border-white/10 bg-surface-raised/60 p-8 text-center text-white/60">
                        {t("No scores yet")}
                    </div>
                }.into_any();
            }
            view! {
                <div class="overflow-hidden rounded-xl border border-white/10 bg-surface-raised/60">
                    <table class="w-full text-sm">
                        <thead class="border-b border-white/10 text-left text-xs uppercase tracking-wider text-white/40">
                            <tr>
                                <th class="px-4 py-3">{t("Room")}</th>
                                <th class="px-4 py-3 text-right">{t("Score")}</th>
                                <th class="px-4 py-3 text-right">{t("Updated")}</th>
                            </tr>
                        </thead>
                        <tbody>
                            {rows.into_iter().map(|r| {
                                let room = r["room"].clone();
                                let slug = room["slug"].as_str().unwrap_or("").to_string();
                                view! {
                                    <tr class="border-b border-white/5 last:border-0">
                                        <td class="px-4 py-3">
                                            <A href=format!("/rooms/{slug}") attr:class="font-medium text-white hover:text-brand-accent-light">
                                                {room["name"].as_str().unwrap_or("").to_string()}
                                            </A>
                                        </td>
                                        <td class="px-4 py-3 text-right font-medium text-brand-accent-light">
                                            {format!("{:.1}", r["score"].as_f64().unwrap_or(0.0))}
                                        </td>
                                        <td class="px-4 py-3 text-right text-white/50">
                                            {r["updated_at"].as_str().unwrap_or("").to_string()}
                                        </td>
                                    </tr>
                                }
                            }).collect::<Vec<_>>()}
                        </tbody>
                    </table>
                </div>
            }.into_any()
        }}
    }
}

#[component]
fn LikesTab(
    data: RwSignal<Value>,
    auth: crate::auth::Auth,
    unlike: impl Fn(i64) + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        {move || {
            let rows = data.get()["data"].as_array().cloned().unwrap_or_default();
            if rows.is_empty() {
                return view! {
                    <div class="rounded-xl border border-white/10 bg-surface-raised/60 p-8 text-center text-white/60">
                        {t("No liked tracks")}
                    </div>
                }.into_any();
            }
            let own = auth.user.get().map(|u| {
                u.id
            });
            view! {
                <div class="overflow-hidden rounded-xl border border-white/10 bg-surface-raised/60">
                    {rows.into_iter().map(|r| {
                        let tid = r["id"].as_i64().unwrap_or(0);
                        view! {
                            <div class="flex items-center gap-3 border-b border-white/5 px-4 py-3 last:border-0">
                                {r["artwork_url"].as_str().filter(|s| !s.is_empty()).map(|src| view! {
                                    <img src=src.to_string() class="h-10 w-10 rounded" alt=""/>
                                })}
                                <div class="min-w-0 flex-1">
                                    <div class="truncate font-medium text-white">
                                        {r["artist"].as_str().unwrap_or("?").to_string()}
                                        " — "
                                        {r["title"].as_str().unwrap_or("?").to_string()}
                                    </div>
                                </div>
                                <Show when=move || own.is_some()>
                                    <button type="button" class="text-xs text-red-400 hover:underline"
                                        on:click=move |_| unlike(tid)>
                                        {t("Unlike")}
                                    </button>
                                </Show>
                            </div>
                        }
                    }).collect::<Vec<_>>()}
                </div>
            }.into_any()
        }}
    }
}

#[component]
fn BookmarksTab(data: RwSignal<Value>) -> impl IntoView {
    view! {
        {move || {
            let rows = data.get()["data"].as_array().cloned().unwrap_or_default();
            if rows.is_empty() {
                return view! {
                    <div class="rounded-xl border border-white/10 bg-surface-raised/60 p-8 text-center text-white/60">
                        {t("No bookmarked rooms")}
                    </div>
                }.into_any();
            }
            view! {
                <div class="grid gap-3 sm:grid-cols-2">
                    {rows.into_iter().map(|r| {
                        let slug = r["slug"].as_str().unwrap_or("").to_string();
                        view! {
                            <A href=format!("/rooms/{slug}") attr:class="flex items-center gap-3 rounded-xl border border-white/10 bg-surface-raised/60 p-4 hover:border-brand-accent/40">
                                <div class="min-w-0">
                                    <div class="truncate font-medium text-white">
                                        {r["name"].as_str().unwrap_or("").to_string()}
                                    </div>
                                    <div class="text-xs text-white/50">
                                        {r["category"]["name"].as_str().unwrap_or("").to_string()}
                                        " · " {r["members_count"].as_i64().unwrap_or(0)} " " {t("members")}
                                    </div>
                                </div>
                            </A>
                        }
                    }).collect::<Vec<_>>()}
                </div>
            }.into_any()
        }}
    }
}

#[component]
fn MinigamesTab(data: RwSignal<Value>, profile_id: String) -> impl IntoView {
    let rows = RwSignal::new(Vec::<Value>::new());
    let next_page = RwSignal::new(2i64);
    let last_page = RwSignal::new(1i64);
    let loading = RwSignal::new(false);

    Effect::new(move |_| {
        let mg = data.get()["minigames"].clone();
        if mg.is_null() {
            return;
        }
        let h = &mg["history"];
        let page = h["current_page"].as_i64().unwrap_or(1);
        let incoming: Vec<Value> = h["data"].as_array().cloned().unwrap_or_default();
        rows.update(|r| {
            if page <= 1 {
                *r = incoming;
            } else {
                r.extend(incoming);
            }
        });
        next_page.set(page + 1);
        last_page.set(h["last_page"].as_i64().unwrap_or(1));
    });

    let profile_id = StoredValue::new(profile_id);
    let load_more = move |_| {
        let pid = profile_id.get_value();
        let page = next_page.get_untracked();
        loading.set(true);
        leptos::task::spawn_local(async move {
            if let Ok(v) =
                api::get::<Value>(&format!("/api/v1/profile/{pid}/minigames?page={page}")).await
            {
                data.set(v);
            }
            loading.set(false);
        });
    };

    view! {
        {move || {
            let mg = data.get()["minigames"].clone();
            if mg.is_null() {
                return view! { <div class="py-10 text-center text-white/45">{t("Loading")}"…"</div> }.into_any();
            }
            let games = mg["games"].as_array().cloned().unwrap_or_default();
            let rankings_url = mg["rankings_url"].as_str().unwrap_or("/rankings").to_string();
            let rank = mg["user_rank"].as_i64();
            let total = mg["user_total_score"].as_i64().unwrap_or(0);
            view! {
                <div class="space-y-6">
                    <div class="flex flex-wrap items-center justify-between gap-3">
                        <p class="text-sm text-white/55">{t("Mini-games scores on profile intro")}</p>
                        <div class="flex flex-wrap items-center gap-3">
                            {rank.map(|r| view! {
                                <p class="text-xs font-semibold text-brand-secondary">
                                    {crate::i18n::tr("Profile minigames rank", &[("rank", r.to_string()), ("score", total.to_string())])}
                                </p>
                            })}
                            <A href=rankings_url attr:class="text-xs font-semibold text-brand-secondary hover:text-brand-secondary/80">
                                {t("Mini-games rankings")}
                            </A>
                        </div>
                    </div>
                    <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
                        {games.into_iter().map(|g| {
                            let play = g["play_url"].as_str().unwrap_or("/minigames").to_string();
                            view! {
                                <div class="rounded-xl border border-white/10 bg-white/5 p-4">
                                    <div class="flex items-start justify-between gap-3">
                                        <div class="min-w-0">
                                            <p class="truncate font-semibold text-white">{t(g["label_key"].as_str().unwrap_or("")).to_string()}</p>
                                            <p class="mt-1 text-2xl font-black tabular-nums text-brand-secondary">{g["total"].as_i64().unwrap_or(0)}</p>
                                            <p class="text-[10px] uppercase tracking-wide text-white/45">{t("PTS")}</p>
                                        </div>
                                        <A href=play attr:class="shrink-0 rounded-lg border border-white/15 px-2.5 py-1 text-xs font-semibold text-white/80 transition hover:border-brand-primary/40 hover:text-white">
                                            {t("Play")}
                                        </A>
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                    <div class="space-y-3">
                        <h3 class="text-sm font-bold uppercase tracking-wider text-white/70">{t("Recent sessions")}</h3>
                        {move || rows.get().into_iter().map(|s| view! {
                            <div class="retro-list-row">
                                <div class="min-w-0 flex-1">
                                    <p class="font-medium text-white">{t(s["label_key"].as_str().unwrap_or("")).to_string()}</p>
                                    <p class="text-xs text-white/55">{s["played_at"].as_str().unwrap_or("").to_string()}</p>
                                </div>
                                <div class="text-lg font-bold text-brand-secondary">{s["score"].as_i64().unwrap_or(0)} " " {t("PTS")}</div>
                            </div>
                        }).collect::<Vec<_>>()}
                        {move || rows.get().is_empty().then(|| view! {
                            <div class="py-10 text-center text-white/45">{t("No mini-game sessions yet")}</div>
                        })}
                        {move || (next_page.get() <= last_page.get() && !loading.get() && !rows.get().is_empty()).then(|| view! {
                            <div class="flex justify-center pt-2">
                                <button type="button" on:click=load_more
                                    class="squircle-nested-xs border border-neutral-600 bg-neutral-700 px-6 py-2 text-sm font-medium text-white transition-colors hover:bg-neutral-600">
                                    {t("Load more")}
                                </button>
                            </div>
                        })}
                    </div>
                </div>
            }.into_any()
        }}
    }
}
