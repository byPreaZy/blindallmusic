use crate::api;
use crate::i18n::t;
use crate::types::{RoomUser, Track};
use leptos::prelude::*;
use leptos_router::components::A;
use serde_json::Value;
use std::collections::HashMap;

#[component]
pub fn RankingPanel(
    users: RwSignal<Vec<RoomUser>>,
    scores: RwSignal<HashMap<i64, f64>>,
    answers_by_user: RwSignal<HashMap<i64, Vec<Value>>>,
    track: RwSignal<Option<Track>>,
    room_id: i64,
    me_id: Signal<Option<i64>>,
    podium_open: RwSignal<bool>,
) -> impl IntoView {
    let sorted = Memo::new(move |_| {
        let sc = scores.get();
        let mut us = users.get();
        us.sort_by(|a, b| {
            let sa = sc.get(&a.id).copied().unwrap_or(0.0);
            let sb = sc.get(&b.id).copied().unwrap_or(0.0);
            sb.partial_cmp(&sa)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(a.name.cmp(&b.name))
        });
        us.into_iter()
            .map(|u| {
                let s = sc.get(&u.id).copied().unwrap_or(0.0);
                (u, s)
            })
            .collect::<Vec<_>>()
    });
    let podium_data = RwSignal::new(Option::<Value>::None);

    Effect::new(move |_| {
        if !podium_open.get() {
            return;
        }
        podium_data.set(None);
        leptos::task::spawn_local(async move {
            if let Ok(v) = api::get::<Value>(&format!("/api/v1/rooms/{room_id}/scores")).await {
                podium_data.set(Some(v));
            }
        });
    });

    let open_podium = move |_| podium_open.set(true);

    view! {
        <div>
            <section class="retro-card">
                <header class="retro-card__header">
                    <div class="flex w-full items-center justify-between">
                        <h3 class="retro-title retro-title--secondary flex items-center text-xl">
                            {t("Ranking")}
                        </h3>
                        <div class="flex items-center">
                            <span class="badge mr-2 border border-white/10 bg-brand-midnight text-white/80">
                                {move || sorted.get().len()}" "{t("players")}
                            </span>
                            <button type="button" class="retro-icon-btn !h-8 !w-8 text-brand-secondary"
                                aria-label=t("Show rankings for this room")
                                on:click=open_podium>
                                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="size-5">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M16.5 18.75h-9m9 0a3 3 0 013 3h-15a3 3 0 013-3m9 0v-3.375c0-.621-.503-1.125-1.125-1.125h-.871M7.5 18.75v-3.375c0-.621.504-1.125 1.125-1.125h.872m5.007 0H9.497m5.007 0a7.454 7.454 0 01-.982-3.172M9.497 14.25a7.454 7.454 0 00.981-3.172M5.25 4.236c-.982.143-1.954.317-2.916.52A6.003 6.003 0 007.73 9.728M5.25 4.236V4.5c0 2.108.966 3.99 2.48 5.228M5.25 4.236V2.721C7.456 2.41 9.71 2.25 12 2.25c2.291 0 4.545.16 6.75.47v1.516M7.73 9.728a6.726 6.726 0 002.748 1.35m8.272-6.842V4.5c0 2.108-.966 3.99-2.48 5.228m2.48-5.492a46.32 46.32 0 012.916.52 6.003 6.003 0 01-5.395 4.972m0 0a6.726 6.726 0 01-2.749 1.35m0 0a6.772 6.772 0 01-3.044 0" />
                                </svg>
                            </button>
                        </div>
                    </div>
                </header>
                <main class="retro-card__body">
                    <div class="h-48 overflow-y-auto pr-2 sm:h-64 md:h-80 2xl:h-96"
                        style="scrollbar-width:thin;scrollbar-color:rgb(242 201 76 / 0.4) rgb(30 25 46 / 0.5)">
                        {move || {
                            let list = sorted.get();
                            if list.is_empty() {
                                return view! {
                                    <div class="flex items-center justify-center py-8 text-white/50">
                                        <div class="text-center">
                                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="mx-auto mb-2 h-12 w-12 opacity-50">
                                                <path stroke-linecap="round" stroke-linejoin="round" d="M15 19.128a9.38 9.38 0 002.625.372 9.337 9.337 0 004.121-.952 4.125 4.125 0 00-7.533-2.493M15 19.128v-.003c0-1.113-.285-2.16-.786-3.07M15 19.128v.106A12.318 12.318 0 018.624 21c-2.331 0-4.512-.645-6.374-1.766l-.001-.109a6.375 6.375 0 0111.964-3.07M12 6.375a3.375 3.375 0 11-6.75 0 3.375 3.375 0 016.75 0zm8.25 2.25a2.625 2.625 0 11-5.25 0 2.625 2.625 0 015.25 0z" />
                                            </svg>
                                            <p>{t("No players yet")}</p>
                                        </div>
                                    </div>
                                }.into_any();
                            }
                            view! {
                                <ul class="space-y-1 sm:space-y-2">
                                    {list.into_iter().enumerate().map(|(i, (u, score))| {
                                        let is_me = me_id.get() == Some(u.id);
                                        let square = match i {
                                            0 => "bg-brand-secondary text-brand-midnight",
                                            1 => "bg-brand-accent text-brand-midnight",
                                            2 => "bg-brand-primary text-brand-midnight",
                                            _ => "bg-brand-midnight text-white/70",
                                        };
                                        let ring = match i {
                                            0 => "ring-brand-secondary",
                                            1 => "ring-brand-accent",
                                            2 => "ring-brand-primary",
                                            _ => "ring-white/20",
                                        };
                                        let found = answers_by_user
                                            .get()
                                            .get(&u.id)
                                            .cloned()
                                            .unwrap_or_default();
                                        let found_ids: Vec<i64> = found
                                            .iter()
                                            .filter_map(|a| a["id"].as_i64())
                                            .collect();
                                        let has_found = !found.is_empty();
                                        let ghosts: Vec<String> = track
                                            .get()
                                            .map(|tr| {
                                                tr.answers
                                                    .iter()
                                                    .filter(|a| !found_ids.contains(&a.id))
                                                    .map(|a| a.name.clone())
                                                    .collect()
                                            })
                                            .unwrap_or_default();
                                        view! {
                                            <li class=format!(
                                                "room-rank-row {}",
                                                if is_me { "room-rank-row--me" } else { "" }
                                            )>
                                                <div class=format!(
                                                    "flex h-6 w-6 shrink-0 items-center justify-center sm:h-8 sm:w-8 {square}"
                                                )>
                                                    <span class="text-base font-bold sm:text-lg">{i + 1}</span>
                                                </div>
                                                <div class="relative inline-flex shrink-0 flex-col items-center justify-center">
                                                    <div class="relative flex items-center justify-center">
                                                        <span class="user-avatar inline-flex">
                                                            {u.photo.clone().map(|p| view! {
                                                                <img src=p alt=u.name.clone() loading="lazy"
                                                                    class=format!("h-8 w-8 rounded-full object-cover object-center shadow-lg ring-2 sm:h-10 sm:w-10 {ring}")/>
                                                            })}
                                                            {u.donor_perks.iter().any(|k| k == "avatar_crown").then(|| view! {
                                                                <span class="user-avatar__crown" title=t("Donor crown title").to_string() aria-label=t("Donor crown title").to_string()>
                                                                    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                                                                        <path d="M11.562 3.266a.5.5 0 0 1 .876 0L15.39 8.87a1 1 0 0 0 1.516.294L21.183 5.5a.5.5 0 0 1 .798.519l-2.834 10.246a1 1 0 0 1-.956.734H5.81a1 1 0 0 1-.957-.734L2.02 6.02a.5.5 0 0 1 .798-.519l4.276 3.664a1 1 0 0 0 1.516-.294z" />
                                                                    </svg>
                                                                </span>
                                                            })}
                                                        </span>
                                                        {u.user_level.as_ref().map(|ul| view! {
                                                            <div class="absolute -right-1 -top-1 z-10 flex size-5 items-center justify-center rounded-full border-2 border-neutral-900 bg-purple-600 text-[11px] font-bold text-white shadow-lg"
                                                                title=format!("{} {}", t("Level"), ul.level)>
                                                                {ul.level}
                                                            </div>
                                                        })}
                                                    </div>
                                                    <div class="mt-0.5 flex flex-col items-center justify-center gap-0.5">
                                                        {u.is_supporter.then(|| view! {
                                                            <span class="text-[10px] font-semibold text-amber-300">{t("Supporter").to_string()}</span>
                                                        })}
                                                        {if u.elo == 0 || (u.elo == 1500 && u.public_rounds_played_count == 0) {
                                                            view! {
                                                                <div class="text-[10px] font-medium text-neutral-500"
                                                                    title=t("Player has not played yet - ELO will be calculated after first game").to_string()>
                                                                    {t("N/A")}
                                                                </div>
                                                            }.into_any()
                                                        } else {
                                                            view! {
                                                                <crate::components::elo::EloBadge elo=u.elo size="sm" variant="compact"/>
                                                            }.into_any()
                                                        }}
                                                    </div>
                                                </div>
                                                <div class="flex min-w-0 flex-grow flex-col">
                                                    <div class="mb-1 flex flex-wrap items-center gap-2 sm:mb-2">
                                                        {if !u.is_guest {
                                                            view! {
                                                                <A href=format!("/profile/{}", u.id)
                                                                    attr:class="truncate font-medium text-white transition-colors hover:text-brand-secondary">
                                                                    {u.name.clone()}
                                                                </A>
                                                            }.into_any()
                                                        } else {
                                                            view! {
                                                                <span class="truncate font-medium text-white/60">
                                                                    {if u.name.is_empty() { t("Deleted user").to_string() } else { u.name.clone() }}
                                                                </span>
                                                            }.into_any()
                                                        }}
                                                        {u.team.clone().map(|tm| view! {
                                                            <A href=format!("/teams/{}", tm.id)
                                                                attr:class="whitespace-nowrap border border-white/10 bg-brand-midnight px-1.5 py-0.5 text-[8px] uppercase text-white/70 transition-colors hover:border-white/20 sm:px-2 sm:text-[9px]">
                                                                {tm.name}
                                                            </A>
                                                        })}
                                                    </div>
                                                    <Show when=move || has_found>
                                                        <div class="flex flex-wrap items-center gap-1">
                                                            {found.iter().map(|a| {
                                                                let name = a["name"].as_str().unwrap_or_default().to_string();
                                                                let order = a["order"].as_i64().unwrap_or(99);
                                                                let speed = a["speedBonus"].as_bool().unwrap_or(false);
                                                                view! {
                                                                    <span class=format!(
                                                                        "room-answer-badge {}",
                                                                        if order < 4 { "mr-2" } else { "" }
                                                                    )>
                                                                        {speed.then(|| view! {
                                                                            <span class="mr-0.5 text-brand-secondary sm:mr-1">
                                                                                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="h-2.5 w-2.5 sm:h-3 sm:w-3" aria-hidden="true">
                                                                                    <path fill-rule="evenodd" d="M13.5 4.938a7 7 0 11-9.006 1.737c.202-.257.59-.218.793.039.278.352.594.672.943.954.332.269.786-.049.773-.476a5.977 5.977 0 01.572-2.759 6.026 6.026 0 012.486-2.665c.247-.14.55-.016.677.238A6.967 6.967 0 0013.5 4.938zM14 12a4 4 0 01-4 4c-1.913 0-3.52-1.398-3.91-3.182-.093-.429.44-.643.814-.413a4.043 4.043 0 001.601.564c.303.038.531-.24.51-.544a5.975 5.975 0 011.315-4.192.447.447 0 01.431-.16A4.001 4.001 0 0114 12z" clip-rule="evenodd"/>
                                                                                </svg>
                                                                            </span>
                                                                        })}
                                                                        <span class="max-w-[4.5rem] truncate sm:max-w-none">{t(name.as_str()).to_string()}</span>
                                                                        {if order < 4 {
                                                                            view! {
                                                                                <span class="absolute -right-2 -top-1 flex h-3 w-3 items-center justify-center bg-brand-secondary text-[8px] font-bold text-brand-midnight sm:h-4 sm:w-4 sm:text-[10px]">
                                                                                    {order}
                                                                                </span>
                                                                            }.into_any()
                                                                        } else {
                                                                            view! { <span></span> }.into_any()
                                                                        }}
                                                                    </span>
                                                                }
                                                            }).collect_view()}
                                                            {ghosts.iter().map(|g| view! {
                                                                <span class="room-answer-badge--ghost">{t(g.as_str()).to_string()}</span>
                                                            }).collect_view()}
                                                        </div>
                                                    </Show>
                                                </div>
                                                <div class=format!(
                                                    "shrink-0 text-base font-bold sm:text-lg {}",
                                                    if i == 0 { "text-brand-secondary" } else { "text-white" }
                                                )>
                                                    {format!("{score:.0}")}
                                                    <sup class="text-xs text-white/50">{t("PTS")}</sup>
                                                </div>
                                            </li>
                                        }
                                    }).collect_view()}
                                </ul>
                            }.into_any()
                        }}
                    </div>
                </main>
            </section>

            <Show when=move || podium_open.get()>
                <div class="fixed inset-0 z-50 flex items-center justify-center px-4">
                    <div class="retro-modal-overlay" on:click=move |_| podium_open.set(false)></div>
                    <div class="retro-modal-panel relative max-w-2xl">
                        <div class="flex items-center justify-between border-b border-white/10 px-6 py-4">
                            <h3 class="retro-title retro-title--secondary text-lg">{t("Rankings")}</h3>
                            <button type="button" class="retro-icon-btn !h-8 !w-8" on:click=move |_| podium_open.set(false)>
                                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="h-5 w-5">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                                </svg>
                            </button>
                        </div>
                        <div class="grid gap-6 p-6 md:grid-cols-2">
                            {move || {
                                let Some(d) = podium_data.get() else {
                                    return view! { <p class="text-sm text-white/50">{t("Loading")}"…"</p> }.into_any();
                                };
                                ["week", "lifetime"].iter().map(|key| {
                                    let title = if *key == "week" { t("This week") } else { t("All time") };
                                    let rows = d[key].as_array().cloned().unwrap_or_default();
                                    view! {
                                        <div>
                                            <h4 class="mb-2 text-sm font-medium uppercase tracking-wider text-brand-secondary">{title}</h4>
                                            <ul class="space-y-1">
                                                {if rows.is_empty() {
                                                    view! { <li class="py-2 text-sm text-white/40">"…"</li> }.into_any()
                                                } else {
                                                    rows.iter().take(10).enumerate().map(|(i, r)| {
                                                        let name = r["name"].as_str()
                                                            .or(r["user"]["name"].as_str())
                                                            .unwrap_or("?")
                                                            .to_string();
                                                        let score = r["total"].as_f64()
                                                            .or(r["score"].as_f64())
                                                            .unwrap_or(0.0);
                                                        view! {
                                                            <li class="flex items-center justify-between rounded bg-brand-deep/60 px-3 py-1.5 text-sm">
                                                                <span class="min-w-0 truncate text-white/85">
                                                                    <span class="mr-2 inline-block w-5 text-center font-bold text-white/40">{i + 1}</span>
                                                                    {name}
                                                                </span>
                                                                <span class="font-semibold text-brand-accent-light">{format!("{score:.0}")}</span>
                                                            </li>
                                                        }
                                                    }).collect_view().into_any()
                                                }}
                                            </ul>
                                        </div>
                                    }
                                }).collect_view().into_any()
                            }}
                        </div>
                    </div>
                </div>
            </Show>
        </div>
    }
}
