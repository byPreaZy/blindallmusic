use crate::api;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use serde_json::Value;

fn euros(cents: i64) -> String {
    format!("{:.2} €", cents as f64 / 100.0)
}

fn month_label(key: &str) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun",
        "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    match key.split_once('-') {
        Some((y, m)) => {
            let mi = m.parse::<usize>().unwrap_or(1).clamp(1, 12);
            format!("{} {}", MONTHS[mi - 1], y)
        }
        None => key.to_string(),
    }
}

#[component]
pub fn DonationsPage() -> impl IntoView {
    crate::i18n::set_title_key("Donations");
    let progress = RwSignal::new(Value::Null);
    let recent = RwSignal::new(Vec::<Value>::new());
    let history = RwSignal::new(Vec::<Value>::new());
    let loading = RwSignal::new(true);

    leptos::task::spawn_local(async move {
        if let Ok(v) = api::get::<Value>("/api/v1/donations/progress").await {
            progress.set(v);
        }
        if let Ok(v) = api::get::<Value>("/api/v1/donations/recent").await {
            recent.set(v["donations"].as_array().cloned().unwrap_or_default());
        }
        if let Ok(v) = api::get::<Value>("/api/v1/donations/history").await {
            history.set(v["history"].as_array().cloned().unwrap_or_default());
        }
        loading.set(false);
    });

    view! {
        <div class="mx-auto max-w-4xl px-4 py-8">
            <h1 class="retro-title retro-title--white mb-2 text-2xl">{t("Support Blind'All Music")}</h1>
            <p class="mb-8 text-white/60">
                {move || crate::i18n::tr("Docs support intro", &[("goal", crate::ads::goal_or(
                    &progress.get()["goal_cents"].as_i64().map(crate::ads::euros).unwrap_or_default()
                ))])}
            </p>

            <Show
                when=move || !loading.get()
                fallback=move || view! { <div class="text-white/60">{t("Loading")}"…"</div> }
            >
                {move || {
                    let p = progress.get();
                    if p.is_null() {
                        return view! { <div/> }.into_any();
                    }
                    let raised = p["raised_cents"].as_i64().unwrap_or(0);
                    let carryover = p["carryover_cents"].as_i64().unwrap_or(0);
                    let goal = p["goal_cents"].as_i64().unwrap_or(1).max(1);
                    let segs = &p["progress_segments"];
                    let seg_co = segs["carryover_percent"].as_i64().unwrap_or(0);
                    let seg_ra = segs["raised_percent"].as_i64().unwrap_or(0);
                    let seg_su = segs["surplus_percent"].as_i64().unwrap_or(0);
                    let goal_reached = p["goal_reached"].as_bool().unwrap_or(false);
                    let days = p["days_remaining"].as_i64().unwrap_or(0);
                    let payment_url = p["payment_url"].as_str().unwrap_or("").to_string();
                    let supporters = p["monthly_supporters"].as_array().cloned().unwrap_or_default();
                    let has_supporters = !supporters.is_empty();
                    view! {
                        <div class="mb-8 rounded-xl border border-white/10 bg-surface-raised/60 p-6">
                            <div class="mb-3 flex flex-wrap items-baseline justify-between gap-2">
                                <h2 class="text-lg font-semibold text-white">
                                    {t("Monthly goal")} " : " {month_label(p["month_key"].as_str().unwrap_or(""))}
                                </h2>
                                <span class=move || if goal_reached { "text-sm font-bold text-teal-400" } else { "text-sm text-white/50" }>
                                    {euros(carryover + raised)} " / " {euros(goal)}
                                </span>
                            </div>
                            <div class="flex h-4 overflow-hidden rounded-full bg-white/10">
                                {if seg_co > 0 { view! {
                                    <div class="h-full bg-white/30" style=format!("width:{seg_co}%")/>
                                }.into_any() } else { view!{<div/>}.into_any() }}
                                {if seg_ra > 0 { view! {
                                    <div class="h-full bg-brand-accent" style=format!("width:{seg_ra}%")/>
                                }.into_any() } else { view!{<div/>}.into_any() }}
                                {if seg_su > 0 { view! {
                                    <div class="h-full bg-teal-400" style=format!("width:{seg_su}%")/>
                                }.into_any() } else { view!{<div/>}.into_any() }}
                            </div>
                            <div class="mt-3 flex flex-wrap items-center justify-between gap-3">
                                <span class="text-sm text-white/50">
                                    {days} " " {t("days left this month")}
                                    {goal_reached.then(|| view! {
                                        <span class="ml-2 rounded bg-teal-500/20 px-2 py-0.5 text-xs text-teal-300">
                                            {t("Goal reached : ads disabled!")}
                                        </span>
                                    })}
                                </span>
                                {(!payment_url.is_empty()).then(|| view! {
                                    <a href=payment_url.clone()
                                        class="btn-primary !px-5 !py-2" target="_blank" rel="noopener">
                                        "☕ " {t("Donate")}
                                    </a>
                                })}
                            </div>
                            <Show when=move || has_supporters>
                                <div class="mt-5 border-t border-white/10 pt-4">
                                    <h3 class="mb-3 text-sm font-semibold uppercase tracking-wider text-white/40">
                                        {t("This month's supporters")}
                                    </h3>
                                    <div class="flex flex-wrap gap-2">
                                        {supporters.clone().into_iter().map(|s| {
                                            let u = s["user"].clone();
                                            let uid = u["id"].as_i64().unwrap_or(0);
                                            let photo = u["photo"].as_str().unwrap_or("").to_string();
                                            let name = u["name"].as_str().unwrap_or("").to_string();
                                            view! {
                                                <A href=format!("/profile/{uid}")
                                                    attr:class="flex items-center gap-2 rounded-full border border-amber-400/30 bg-amber-400/10 py-1 pl-1 pr-3 text-sm text-amber-200 hover:bg-amber-400/20">
                                                    <img src=photo class="h-6 w-6 rounded-full" alt=""/>
                                                    {name}
                                                </A>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                </div>
                            </Show>
                        </div>
                    }.into_any()
                }}

                <div class="grid gap-8 lg:grid-cols-2">
                    <div>
                        <h2 class="mb-3 text-lg font-semibold text-white">{t("Recent donations")}</h2>
                        <div class="space-y-2">
                            {move || {
                                let rows = recent.get();
                                if rows.is_empty() {
                                    return vec![view! {
                                        <div class="rounded-xl border border-white/10 bg-surface-raised/60 p-6 text-center text-white/50">
                                            {t("No donations yet")}
                                        </div>
                                    }.into_any()];
                                }
                                rows.into_iter().map(|d| {
                                    let u = d["user"].clone();
                                    let uid = u["id"].as_i64().unwrap_or(0);
                                    let photo = u["photo"].as_str().unwrap_or("").to_string();
                                    let name = u["name"].as_str().unwrap_or("").to_string();
                                    let supporter = u["is_supporter"].as_bool().unwrap_or(false);
                                    let amount = d["amount_cents"].as_i64().unwrap_or(0);
                                    view! {
                                        <div class="flex items-center gap-3 rounded-lg border border-white/10 bg-surface-raised/60 px-4 py-3">
                                            <img src=photo class="h-8 w-8 rounded-full" alt=""/>
                                            <div class="min-w-0 flex-1">
                                                <A href=format!("/profile/{uid}") attr:class="truncate font-medium text-white hover:text-brand-accent-light">
                                                    {name}
                                                </A>
                                                {supporter.then(|| view! {
                                                    <span class="ml-2 rounded bg-amber-400/20 px-1.5 py-0.5 text-xs text-amber-300">"★"</span>
                                                })}
                                            </div>
                                            <span class="font-bold text-teal-400">{euros(amount)}</span>
                                        </div>
                                    }.into_any()
                                }).collect()
                            }}
                        </div>
                    </div>
                    <div>
                        <h2 class="mb-3 text-lg font-semibold text-white">{t("History")}</h2>
                        <div class="space-y-2">
                            {move || {
                                let rows = history.get();
                                if rows.is_empty() {
                                    return vec![view! {
                                        <div class="rounded-xl border border-white/10 bg-surface-raised/60 p-6 text-center text-white/50">
                                            {t("No history")}
                                        </div>
                                    }.into_any()];
                                }
                                rows.into_iter().map(|h| {
                                    let pct = h["percent"].as_i64().unwrap_or(0);
                                    let reached = h["goal_reached"].as_bool().unwrap_or(false);
                                    view! {
                                        <div class="rounded-lg border border-white/10 bg-surface-raised/60 px-4 py-3">
                                            <div class="mb-2 flex items-center justify-between text-sm">
                                                <span class="font-medium text-white">
                                                    {month_label(h["month_key"].as_str().unwrap_or(""))}
                                                </span>
                                                <span class=move || if reached { "font-bold text-teal-400" } else { "text-white/50" }>
                                                    {euros(h["raised_cents"].as_i64().unwrap_or(0))}
                                                    {format!(" ({pct}%)")}
                                                </span>
                                            </div>
                                            <div class="h-2 overflow-hidden rounded-full bg-white/10">
                                                <div class=move || if reached { "h-full rounded-full bg-teal-400" } else { "h-full rounded-full bg-brand-accent" }
                                                    style=format!("width:{}%", pct.min(100))/>
                                            </div>
                                        </div>
                                    }.into_any()
                                }).collect()
                            }}
                        </div>
                    </div>
                </div>
            </Show>
        </div>
    }
}
