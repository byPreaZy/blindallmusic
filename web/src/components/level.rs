use crate::auth::use_auth;
use crate::i18n::t;
use leptos::prelude::*;
use serde_json::Value;

fn level_color(level: i64) -> &'static str {
    if level >= 50 {
        "#a855f7"
    } else if level >= 30 {
        "#eab308"
    } else if level >= 20 {
        return "#3b82f6";
    } else if level >= 10 {
        "#22c55e"
    } else {
        "#737373"
    }
}

fn level_bg_class(level: i64) -> &'static str {
    if level >= 50 {
        "bg-purple-500"
    } else if level >= 30 {
        "bg-yellow-500"
    } else if level >= 20 {
        "bg-blue-500"
    } else if level >= 10 {
        "bg-green-500"
    } else {
        "bg-neutral-500"
    }
}

#[component]
pub fn LevelBadge(
    #[prop(into)] level: Signal<i64>,
    #[prop(into)] current_xp: Signal<i64>,
    #[prop(into)] xp_for_next_level: Signal<i64>,
    #[prop(default = 40)] size: u32,
) -> impl IntoView {
    let stroke_w = if size <= 28 { 2.5 } else { 3.5 };
    let radius = if size <= 28 { 12.0 } else { 17.0 };
    let circumference = 2.0 * std::f64::consts::PI * radius;
    let center = size as f64 / 2.0;
    let number_cls = if size <= 28 { "text-[10px]" } else { "text-sm" };
    view! {
        <div class="inline-flex items-center justify-center px-1"
            title=move || format!("{} {}", t("Level"), level.get())>
            <div class="relative" style=format!("width:{size}px;height:{size}px")>
                <svg width=size height=size class="-rotate-90 transform">
                    <circle cx=center cy=center r=radius stroke-width=stroke_w
                        stroke="rgba(255,255,255,0.1)" fill="none" />
                    <circle cx=center cy=center r=radius stroke-width=stroke_w
                        stroke=move || level_color(level.get()) fill="none"
                        stroke-dasharray=circumference
                        stroke-dashoffset=move || {
                            let next = xp_for_next_level.get();
                            let pct = if next <= 0 { 100.0 } else {
                                (current_xp.get() as f64 / next as f64 * 100.0).min(100.0)
                            };
                            circumference - pct / 100.0 * circumference
                        }
                        stroke-linecap="round"
                        class="transition-all duration-500 ease-out" />
                </svg>
                <div class="pointer-events-none absolute inset-0 flex items-center justify-center">
                    <span class=format!("font-extrabold leading-none {number_cls} transition-all duration-300")
                        style=move || format!("color:{}", level_color(level.get()))>
                        {move || level.get()}
                    </span>
                </div>
            </div>
        </div>
    }
}

#[component]
pub fn LevelInfo(
    level: i64,
    current_xp: i64,
    xp_for_next_level: i64,
    total_xp: i64,
    level_metrics: Option<Value>,
) -> impl IntoView {
    let pct = if xp_for_next_level <= 0 {
        100.0
    } else {
        (current_xp as f64 / xp_for_next_level as f64 * 100.0).min(100.0)
    };
    let metrics: Vec<(String, String, i64, Option<i64>)> = level_metrics
        .map(|m| {
            let g = |k: &str| m[k].as_i64().unwrap_or(0);
            let has_team = m["has_team"].as_bool().unwrap_or(false);
            vec![
                (
                    t("Total score").to_string(),
                    format!("{} {}", g("score_public_rooms"), t("points")),
                    g("score_public_rooms"),
                    None,
                ),
                (
                    t("Mini-games score").to_string(),
                    format!("{} {}", g("minigame_scores_total"), t("points")),
                    g("minigame_scores_total").min(1000),
                    Some(1000),
                ),
                (
                    t("Seniority").to_string(),
                    format!("{} {}", g("seniority_months"), t("months")),
                    (g("seniority_months") * 50).min(600),
                    Some(600),
                ),
                (
                    t("Consecutive days streak").to_string(),
                    format!("{} {}", g("consecutive_days_streak"), t("days")),
                    (g("consecutive_days_streak") * 10).min(300),
                    Some(300),
                ),
                (
                    t("Rooms created").to_string(),
                    g("rooms_created_count").to_string(),
                    (g("rooms_created_count") * 100).min(1000),
                    Some(1000),
                ),
                (
                    t("Playlists created").to_string(),
                    g("playlists_created_count").to_string(),
                    (g("playlists_created_count") * 20).min(2000),
                    Some(2000),
                ),
                (
                    t("Tracks liked").to_string(),
                    g("tracks_liked_count").to_string(),
                    (g("tracks_liked_count") * 5).min(1000),
                    Some(1000),
                ),
                (
                    t("Team").to_string(),
                    if has_team {
                        t("Yes").to_string()
                    } else {
                        t("No").to_string()
                    },
                    if has_team { 200 } else { 0 },
                    Some(200),
                ),
            ]
        })
        .unwrap_or_default();
    let metrics = StoredValue::new(metrics);
    view! {
        <div class="space-y-4">
            <div class="flex items-center gap-3">
                <LevelBadge level=level current_xp=current_xp xp_for_next_level=xp_for_next_level size=40 />
                <div>
                    <h3 class="text-lg font-bold" style=format!("color:{}", level_color(level))>
                        {t("Level")} " " {level}
                    </h3>
                    <p class="text-xs text-neutral-400">
                        {t("Total XP")} ": "
                        <span class="font-semibold" style=format!("color:{}", level_color(level))>
                            {total_xp} " " {t("XP")}
                        </span>
                    </p>
                </div>
            </div>
            <div class="rounded-lg border border-neutral-700 bg-neutral-800/50 p-4">
                <div class="mb-3">
                    <div class="mb-2 flex items-center justify-between">
                        <span class="text-xs font-medium text-neutral-400">{t("XP in this level")}</span>
                        <span class="text-xs font-bold" style=format!("color:{}", level_color(level))>
                            {current_xp} " / " {xp_for_next_level} " " {t("XP")}
                        </span>
                    </div>
                    <div class="relative h-2 w-full overflow-hidden rounded-full bg-neutral-700">
                        <div class=format!("h-full rounded-full transition-all duration-500 ease-out {}", level_bg_class(level))
                            style=format!("width:{pct}%") />
                    </div>
                    <div class="mt-1 flex justify-between text-xs text-neutral-500">
                        <span>{t("Progress to next level")}</span>
                        <span>{format!("{pct:.0}%")}</span>
                    </div>
                </div>
                <div class="border-t border-neutral-700 pt-3">
                    <div class="flex items-center justify-between">
                        <span class="text-xs text-neutral-400">{t("XP needed")}</span>
                        <span class="text-sm font-bold" style=format!("color:{}", level_color(level))>
                            {xp_for_next_level - current_xp} " " {t("XP")}
                        </span>
                    </div>
                </div>
            </div>
            <Show when=move || !metrics.get_value().is_empty()>
                <div class="rounded-lg border border-neutral-700 bg-neutral-800/50 p-4">
                    <h4 class="mb-3 text-sm font-bold text-neutral-300">{t("XP Breakdown")}</h4>
                    <div class="space-y-2">
                        {metrics.get_value().iter().map(|(label, value, xp, max)| {
                            let capped = max.is_some_and(|m| *xp >= m);
                            view! {
                                <div class="flex items-center justify-between rounded-lg bg-neutral-900/50 px-3 py-2">
                                    <div class="min-w-0 flex-1">
                                        <div class="mb-0.5 flex items-center gap-2">
                                            <span class="truncate text-xs font-semibold text-neutral-300">{label.clone()}</span>
                                            {capped.then(|| view! {
                                                <span class="flex-shrink-0 rounded bg-yellow-500/20 px-1.5 py-0.5 text-xs font-semibold text-yellow-400">
                                                    {t("MAX")}
                                                </span>
                                            })}
                                        </div>
                                        <p class="truncate text-xs text-neutral-500">{value.clone()}</p>
                                    </div>
                                    <div class="ml-2 flex-shrink-0 text-right">
                                        <p class="text-sm font-bold" style={if *xp > 0 {
                                            format!("color:{}", level_color(level))
                                        } else {
                                            "color:#737373".to_string()
                                        }}>
                                            {*xp}
                                        </p>
                                        <p class="text-xs text-neutral-500">{t("XP")}</p>
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                    </div>
                </div>
            </Show>
        </div>
    }
}

#[component]
pub fn LevelDisplay() -> impl IntoView {
    let auth = use_auth();
    let show_modal = RwSignal::new(false);
    view! {
        {move || {
            let u = auth.user.get();
            match u {
                Some(u) if !u.is_guest => view! {
                    <div>
                        <div class="flex h-10 w-10 cursor-pointer items-center justify-center transition-all duration-300"
                            on:click=move |_| show_modal.set(true)>
                            <LevelBadge
                                level=Signal::derive(move || auth.user.get().map(|x| x.level).unwrap_or(1))
                                current_xp=Signal::derive(move || auth.user.get().map(|x| x.current_xp).unwrap_or(0))
                                xp_for_next_level=Signal::derive(move || auth.user.get().map(|x| x.xp_for_next_level).unwrap_or(100))
                                size=40 />
                        </div>
                        {move || show_modal.get().then(|| {
                            let u2 = auth.user.get();
                            view! {
                                <div class="fixed inset-0 z-50 flex items-center justify-center overflow-y-auto bg-black/70 p-4"
                                    on:click=move |_| show_modal.set(false)>
                                    <div class="max-h-[calc(100vh-2rem)] w-full max-w-2xl overflow-y-auto rounded-xl bg-gradient-to-br from-slate-800 to-slate-900 shadow-2xl"
                                        on:click=move |e| e.stop_propagation()>
                                        <div class="flex items-center justify-between border-b border-neutral-700 px-6 py-6">
                                            <div class="flex items-center gap-4">
                                                <LevelBadge
                                                    level=u2.as_ref().map(|x| x.level).unwrap_or(1)
                                                    current_xp=u2.as_ref().map(|x| x.current_xp).unwrap_or(0)
                                                    xp_for_next_level=u2.as_ref().map(|x| x.xp_for_next_level).unwrap_or(100)
                                                    size=40 />
                                                <div>
                                                    <h2 class="text-3xl font-bold" style=format!("color:{}", level_color(u2.as_ref().map(|x| x.level).unwrap_or(1)))>
                                                        {t("Level")} " " {u2.as_ref().map(|x| x.level).unwrap_or(1)}
                                                    </h2>
                                                    <p class="mt-1 text-sm text-neutral-400">
                                                        {t("Total XP")} ": "
                                                        <span class="font-bold" style=format!("color:{}", level_color(u2.as_ref().map(|x| x.level).unwrap_or(1)))>
                                                            {u2.as_ref().map(|x| x.total_xp).unwrap_or(0)} " " {t("XP")}
                                                        </span>
                                                    </p>
                                                </div>
                                            </div>
                                            <button type="button" class="text-neutral-400 transition-colors hover:text-neutral-200"
                                                on:click=move |_| show_modal.set(false)>
                                                <svg class="h-6 w-6" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                                                </svg>
                                            </button>
                                        </div>
                                        <div class="space-y-6 px-6 py-6">
                                            <LevelInfo
                                                level=u2.as_ref().map(|x| x.level).unwrap_or(1)
                                                current_xp=u2.as_ref().map(|x| x.current_xp).unwrap_or(0)
                                                xp_for_next_level=u2.as_ref().map(|x| x.xp_for_next_level).unwrap_or(100)
                                                total_xp=u2.as_ref().map(|x| x.total_xp).unwrap_or(0)
                                                level_metrics=u2.as_ref().and_then(|x| x.level_metrics.clone()) />
                                            <div class="pt-2 text-center">
                                                <a href="/docs/level" class="inline-flex items-center gap-1 text-sm text-blue-400 transition-colors hover:text-blue-300"
                                                    on:click=move |_| show_modal.set(false)>
                                                    <svg class="h-4 w-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                                        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 16h-1v-4h-1m1-4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                                                    </svg>
                                                    {t("How does it work?")}
                                                </a>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            }
                        })}
                    </div>
                }.into_any(),
                _ => view! { <span/> }.into_any(),
            }
        }}
    }
}
