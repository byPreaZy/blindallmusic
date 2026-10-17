use crate::i18n::t;
use crate::types::RoomConfig;
use leptos::prelude::*;
use send_wrapper::SendWrapper;
use serde_json::Value;

fn podium_name(entry: &Value) -> String {
    entry["user"]["name"]
        .as_str()
        .or(entry["team"]["name"].as_str())
        .unwrap_or("")
        .to_string()
}

fn podium_photo(entry: &Value) -> String {
    entry["user"]["photo"]
        .as_str()
        .or(entry["team"]["photo"].as_str())
        .unwrap_or("")
        .to_string()
}

fn podium_score(entry: &Value) -> f64 {
    entry["score"]
        .as_f64()
        .or(entry["total"].as_f64())
        .unwrap_or(0.0)
}

fn elo_badge(elo: i64) -> impl IntoView {
    view! { <crate::components::elo::EloBadge elo=elo size="sm" variant="compact"/> }
}

fn podium_spot(entry: &Value, rank: usize) -> impl IntoView {
    let medal_class = match rank {
        1 => "medal gold",
        2 => "medal silver",
        _ => "medal bronze",
    };
    let score_class = match rank {
        1 => "text-brand-secondary",
        2 => "text-brand-accent",
        _ => "text-brand-primary",
    };
    let avatar_ring = match rank {
        1 => "border-brand-secondary",
        2 => "border-brand-accent",
        _ => "border-brand-primary",
    };
    let stand_class = match rank {
        1 => "podium-stand bg-brand-secondary text-brand-midnight",
        2 => "podium-stand bg-brand-accent text-brand-midnight",
        _ => "podium-stand bg-brand-primary",
    };
    view! {
        <div
            class="podium-column min-w-0 max-w-[34%] flex-1"
            class:podium-column--first=rank == 1
        >
            <div class="flex flex-col items-center gap-1.5 sm:gap-1">
                <div class="avatar-container">
                    <img
                        class=format!(
                            "rounded-full border-2 object-cover shadow-lg {} {}",
                            if rank == 1 { "h-16 w-16 sm:h-14 sm:w-14" } else { "h-14 w-14 sm:h-12 sm:w-12" },
                            avatar_ring
                        )
                        src=podium_photo(entry)
                        alt=podium_name(entry)
                    />
                    <div class=medal_class>{rank}</div>
                </div>
                <p
                    class=format!(
                        "podium-name w-full truncate text-center font-semibold leading-tight text-white {}",
                        if rank == 1 { "text-sm sm:text-xs" } else { "text-xs sm:text-[11px]" }
                    )
                    title=podium_name(entry)
                >
                    {podium_name(entry)}
                </p>
                <div class="flex justify-center">
                    <crate::components::tag::UserTags
                        role=entry["user"]["role"].as_str().unwrap_or("").to_string()
                        custom_tag=entry["user"]["custom_tag"].as_str().unwrap_or("").to_string()/>
                </div>
                <div class="hidden min-h-[1.25rem] items-center justify-center sm:flex">
                    {entry["user"]["elo"].as_i64().map(elo_badge)}
                </div>
                <p class=format!(
                    "podium-score font-bold leading-none {} {}",
                    score_class,
                    if rank == 1 { "text-lg sm:text-sm" } else { "text-base sm:text-sm" }
                )>
                    {format!("{:.0}", podium_score(entry))}
                    <span class="text-[11px] font-medium text-white/60 sm:text-[10px]">"pts"</span>
                </p>
                <div class=format!("podium-stand hidden sm:flex {}", stand_class)>
                    <span class=format!("font-bold {}", if rank == 1 { "text-xl" } else { "text-lg" })>
                        {rank}
                    </span>
                </div>
            </div>
        </div>
    }
}

fn podium_extra(offset: usize, entry: &Value) -> impl IntoView {
    view! {
        <div class="flex min-w-0 max-w-full items-center gap-2.5 border border-white/10 bg-brand-midnight px-3 py-2 sm:gap-2 sm:py-1">
            <div class="relative shrink-0">
                <img
                    class=format!(
                        "h-9 w-9 rounded-full border object-cover sm:h-8 sm:w-8 {}",
                        if offset == 0 { "border-brand-accent" } else { "border-brand-primary" }
                    )
                    src=podium_photo(entry)
                    alt=podium_name(entry)
                />
                <div class=format!(
                    "absolute -right-1 -top-1 flex h-4 w-4 items-center justify-center text-[10px] font-bold {}",
                    if offset == 0 { "bg-brand-accent text-brand-midnight" } else { "bg-brand-primary text-brand-midnight" }
                )>
                    {offset + 4}
                </div>
            </div>
            <div class="min-w-0 text-sm sm:text-xs">
                <div class="flex min-w-0 items-center gap-1 font-medium text-white/90">
                    <span class="truncate">{podium_name(entry)}</span>
                    <crate::components::tag::UserTags
                        role=entry["user"]["role"].as_str().unwrap_or("").to_string()
                        custom_tag=entry["user"]["custom_tag"].as_str().unwrap_or("").to_string()/>
                </div>
                <div class="hidden sm:block">
                    {entry["user"]["elo"].as_i64().map(elo_badge)}
                </div>
                <div class=format!(
                    "text-xs font-bold sm:text-[10px] {}",
                    if offset == 0 { "text-brand-accent" } else { "text-brand-primary" }
                )>
                    {format!("{:.0} pts", podium_score(entry))}
                </div>
            </div>
        </div>
    }
}

#[component]
fn Podium(list: Vec<Value>) -> impl IntoView {
    if list.is_empty() {
        return ().into_any();
    }
    let ordered: Vec<(usize, Value)> = match list.len() {
        1 => vec![(1, list[0].clone())],
        2 => vec![(2, list[1].clone()), (1, list[0].clone())],
        _ => vec![
            (3, list[2].clone()),
            (1, list[0].clone()),
            (2, list[1].clone()),
        ],
    };
    let extra: Vec<(usize, Value)> = list
        .into_iter()
        .skip(3)
        .take(2)
        .enumerate()
        .collect();
    let has_extra = !extra.is_empty();

    view! {
        <div class="podium-root relative w-full min-w-0 py-3 sm:py-6">
            <div class="absolute inset-0 hidden opacity-10 sm:block">
                <div class="absolute left-1/4 top-0 h-32 w-32 rounded-full bg-brand-secondary opacity-20 blur-3xl"></div>
                <div class="absolute bottom-0 right-1/4 h-32 w-32 rounded-full bg-brand-accent opacity-20 blur-3xl"></div>
            </div>
            <div class="relative z-10 flex items-end justify-center gap-2 px-1 sm:h-48 sm:gap-4">
                {ordered
                    .into_iter()
                    .map(|(rank, entry)| podium_spot(&entry, rank).into_any())
                    .collect_view()}
            </div>
            {has_extra.then(|| {
                view! {
                    <div class="mt-3 flex flex-wrap justify-center gap-2 px-2 sm:mt-4 sm:gap-4">
                        {extra
                            .into_iter()
                            .map(|(offset, entry)| podium_extra(offset, &entry).into_any())
                            .collect_view()}
                    </div>
                }
            })}
        </div>
    }
    .into_any()
}

#[component]
pub fn FinishedRoundModal(
    show: RwSignal<bool>,
    users_podium: RwSignal<Vec<Value>>,
    teams_podium: RwSignal<Vec<Value>>,
    room: RwSignal<RoomConfig>,
) -> impl IntoView {
    let active_tab = RwSignal::new("players");
    let countdown = RwSignal::new(0_i64);

    let countdown_iv = StoredValue::new(None::<SendWrapper<gloo_timers::callback::Interval>>);
    Effect::new(move |_| {
        if show.get() {
            active_tab.set("players");
            countdown.set(room.get_untracked().pause_between_rounds as i64);
            let iv = gloo_timers::callback::Interval::new(1_000, move || {
                countdown.update(|c| {
                    if *c > 0 {
                        *c -= 1;
                    }
                });
            });
            countdown_iv.set_value(Some(SendWrapper::new(iv)));
        } else {
            countdown_iv.set_value(None);
        }
    });

    let has_players = move || !users_podium.get().is_empty();
    let has_teams = move || !teams_podium.get().is_empty();
    let show_tabs = move || has_players() && has_teams();
    let visible_tab = move || {
        if !show_tabs() {
            return if has_players() { "players" } else { "teams" };
        }
        if active_tab.get() == "teams" && has_teams() {
            "teams"
        } else {
            "players"
        }
    };
    let countdown_total = move || (room.get().pause_between_rounds as i64).max(1);

    view! {
        <Show when=move || show.get() && (has_players() || has_teams())>
            <div class="fixed inset-0 z-50 flex items-center justify-center px-4">
                <div class="retro-modal-overlay" on:click=move |_| show.set(false)></div>
                <div class="retro-modal-panel relative w-full max-w-3xl">
                    <div class="border-b border-white/10 px-6 py-4">
                        <h3 class="retro-title retro-title--primary">{t("Round finished")}</h3>
                    </div>
                    <div class="flex min-w-0 flex-col px-6 pb-2">
                        <Show when=show_tabs>
                            <div class="mb-4 flex gap-1 overflow-x-auto border-b border-white/10">
                                <button
                                    type="button"
                                    class="relative -mb-px shrink-0 border-b-2 px-4 py-2 text-sm font-medium transition-colors"
                                    class=("border-brand-accent", move || visible_tab() == "players")
                                    class=("text-white", move || visible_tab() == "players")
                                    class=("border-transparent", move || visible_tab() != "players")
                                    class=("text-white/60", move || visible_tab() != "players")
                                    on:click=move |_| active_tab.set("players")
                                >
                                    {t("Ranking")}
                                </button>
                                <button
                                    type="button"
                                    class="relative -mb-px shrink-0 border-b-2 px-4 py-2 text-sm font-medium transition-colors"
                                    class=("border-brand-accent", move || visible_tab() == "teams")
                                    class=("text-white", move || visible_tab() == "teams")
                                    class=("border-transparent", move || visible_tab() != "teams")
                                    class=("text-white/60", move || visible_tab() != "teams")
                                    on:click=move |_| active_tab.set("teams")
                                >
                                    {t("Teams")}
                                </button>
                            </div>
                        </Show>

                        <Show when=move || visible_tab() == "players" && has_players()>
                            <div class="w-full min-w-0">
                                <Show when=move || !show_tabs()>
                                    <h3 class="retro-title retro-title--secondary mb-2 py-2 text-center text-lg sm:text-xl">
                                        {t("Ranking")}
                                    </h3>
                                </Show>
                                <Podium list=users_podium.get()/>
                                <ul class="max-h-64 overflow-y-auto overflow-x-hidden">
                                    <For
                                        each=move || users_podium.get()
                                        key=|u| u["user"]["id"].as_i64().or(u["user_id"].as_i64()).unwrap_or_default()
                                        children=move |u: Value| {
                                            let pos = users_podium
                                                .get_untracked()
                                                .iter()
                                                .position(|x| {
                                                    x["user_id"].as_i64() == u["user_id"].as_i64()
                                                })
                                                .map(|i| i + 1)
                                                .unwrap_or_default();
                                            view! {
                                                <li class="room-rank-row m-1 min-w-0 text-sm sm:text-base">
                                                    <span class="shrink-0 text-lg font-bold sm:text-xl">{pos}</span>
                                                    <span class="flex min-w-0 flex-grow items-center gap-2">
                                                        <span class="truncate font-medium">{podium_name(&u)}</span>
                                                        <crate::components::tag::UserTags
                                                            role=u["user"]["role"].as_str().unwrap_or("").to_string()
                                                            custom_tag=u["user"]["custom_tag"].as_str().unwrap_or("").to_string()/>
                                                        {u["user"]["elo"].as_i64().filter(|e| *e > 0).map(elo_badge)}
                                                    </span>
                                                    <span class="shrink-0 whitespace-nowrap font-semibold">
                                                        {format!("{:.0}", podium_score(&u))}
                                                        <sup class="ml-1 text-xs">{t("PTS")}</sup>
                                                    </span>
                                                </li>
                                            }
                                        }
                                    />
                                </ul>
                            </div>
                        </Show>

                        <Show when=move || visible_tab() == "teams" && has_teams()>
                            <div class="w-full min-w-0">
                                <Show when=move || !show_tabs()>
                                    <h3 class="retro-title retro-title--secondary mb-2 py-2 text-center text-lg sm:text-xl">
                                        {t("Teams")}
                                    </h3>
                                </Show>
                                <Podium list=teams_podium.get()/>
                                <ul class="max-h-64 overflow-y-auto overflow-x-hidden">
                                    <For
                                        each=move || teams_podium.get()
                                        key=|e| e["team"]["id"].as_i64().or(e["team_id"].as_i64()).unwrap_or_default()
                                        children=move |e: Value| {
                                            let idx = teams_podium
                                                .get_untracked()
                                                .iter()
                                                .position(|x| x["team_id"].as_i64() == e["team_id"].as_i64())
                                                .map(|i| i + 1)
                                                .unwrap_or_default();
                                            view! {
                                                <li class="room-rank-row m-1 min-w-0 text-sm sm:text-base">
                                                    <span class="shrink-0 text-lg font-bold sm:text-xl">{idx}</span>
                                                    <span class="min-w-0 flex-grow truncate font-medium">{podium_name(&e)}</span>
                                                    <span class="shrink-0 whitespace-nowrap font-semibold">
                                                        {format!("{:.0}", podium_score(&e))}
                                                        <sup class="ml-1 text-xs">{t("PTS")}</sup>
                                                    </span>
                                                </li>
                                            }
                                        }
                                    />
                                </ul>
                            </div>
                        </Show>

                        <Show when=move || !has_players() && !has_teams()>{t("No scores")}</Show>
                    </div>

                    <div class="flex w-full flex-col gap-3 px-6 pb-4 sm:flex-row sm:items-center sm:gap-6">
                        <div class="flex min-w-0 flex-grow flex-col">
                            <Show when=move || room.get().is_autostart>
                                <div class="relative flex h-6 w-full min-w-0 items-center overflow-hidden bg-brand-midnight">
                                    <div
                                        class="flex h-6 items-center justify-center bg-brand-accent/80 text-white transition-all duration-1000 ease-linear"
                                        style=move || {
                                            format!(
                                                "width:{}%",
                                                countdown.get() as f64 / countdown_total() as f64 * 100.0
                                            )
                                        }
                                    >
                                        <span class="absolute bottom-0 left-0 right-0 top-0 flex items-center justify-center text-xs text-white sm:text-sm">
                                            {t("Next game in")} " " {countdown.get()}
                                        </span>
                                    </div>
                                </div>
                            </Show>
                        </div>
                        <div class="flex shrink-0 items-center justify-end">
                            <button type="button" class="btn-secondary" on:click=move |_| show.set(false)>
                                {t("Close")}
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </Show>
    }
}
