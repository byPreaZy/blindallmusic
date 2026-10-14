use crate::api;
use crate::auth::use_auth;
use crate::i18n::t;
use crate::types::PlaylistSummary;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_navigate;
use serde_json::{json, Value};

#[component]
pub fn PlaylistsPage() -> impl IntoView {
    crate::i18n::set_title_key("My Playlists");
    let auth = use_auth();
    crate::auth::require_registered(auth);
    let playlists = RwSignal::new(Vec::<PlaylistSummary>::new());
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);

    leptos::task::spawn_local(async move {
        match api::get::<Value>("/api/v1/playlists").await {
            Ok(v) => {
                let parsed: Vec<PlaylistSummary> = v["playlists"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|p| serde_json::from_value(p.clone()).ok())
                            .collect()
                    })
                    .unwrap_or_default();
                playlists.set(parsed);
            }
            Err(e) => error.set(Some(e.message)),
        }
        loading.set(false);
    });

    let search = RwSignal::new(String::new());
    let filtered = move || {
        let q = search.get().to_lowercase();
        let all = playlists.get();
        if q.is_empty() {
            return all;
        }
        all.into_iter()
            .filter(|p| {
                p.name.to_lowercase().contains(&q)
                    || p.description
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&q)
            })
            .collect::<Vec<_>>()
    };

    view! {
        <div class="mx-auto max-w-5xl px-4 py-8">
            <header class="mb-6 flex flex-wrap items-center justify-between gap-4">
                <div>
                    <h1 class="retro-title retro-title--white text-2xl sm:text-3xl">{t("My Playlists")}</h1>
                    <p class="mt-1 text-sm text-white/50">
                        {t("Total")}": "<span class="font-medium text-white/80">{move || playlists.get().len()}</span>" "{t("playlists")}
                    </p>
                </div>
                <Show when=move || auth.user.get().is_some()>
                    <A href="/playlists/create"
                        attr:class="group inline-flex items-center justify-center gap-2 rounded-xl bg-gradient-to-r from-teal-500 to-teal-600 px-6 py-3 text-sm font-semibold text-white shadow-lg shadow-teal-500/25 transition-all duration-200 hover:scale-105 hover:from-teal-400 hover:to-teal-500">
                        <svg class="h-5 w-5 transition-transform duration-200 group-hover:rotate-90" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" aria-hidden="true">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M12 4.5v15m7.5-7.5h-15" />
                        </svg>
                        <span>{t("Create a playlist")}</span>
                    </A>
                </Show>
            </header>

            <div class="mb-6 rounded-xl border border-white/10 bg-surface-raised/60 p-4">
                <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
                    <div class="relative flex-1">
                        <svg class="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-white/40" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" aria-hidden="true">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z" />
                        </svg>
                        <input bind:value=search type="text" class="form-input w-full pl-9"
                            placeholder=format!("{}...", t("Search playlists"))
                            aria-label=t("Search playlists by name or description")/>
                    </div>
                    <Show when=move || !search.get().is_empty()>
                        <button type="button" class="rounded px-4 py-2 text-sm text-white/50 transition-colors hover:text-white/80"
                            on:click=move |_| search.set(String::new())>
                            {t("Reset")}
                        </button>
                    </Show>
                </div>
            </div>

            {move || error.get().map(|e| view! {
                <div class="mb-4 rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">{e}</div>
            })}

            <Show
                when=move || !loading.get()
                fallback=move || view! { <div class="text-white/60">{t("Loading")}"…"</div> }
            >
                <div class="hidden overflow-hidden rounded-xl border border-white/10 bg-surface-raised/60 lg:block">
                    <div class="overflow-x-auto">
                        <table class="w-full" role="table" aria-label=t("Playlists list")>
                            <thead>
                                <tr class="text-left">
                                    <th scope="col" class="px-4 pb-4 pt-6 text-sm font-semibold text-white/80">{t("Name")}</th>
                                    <th scope="col" class="px-4 pb-4 pt-6 text-sm font-semibold text-white/80">{t("Description")}</th>
                                    <th scope="col" class="px-4 pb-4 pt-6 text-sm font-semibold text-white/80">{t("Tracks")}</th>
                                    <th scope="col" class="px-4 pb-4 pt-6 text-sm font-semibold text-white/80">{t("Owner")}</th>
                                    <th scope="col" class="px-4 pb-4 pt-6 text-sm font-semibold text-white/80">{t("Moderators")}</th>
                                    <th scope="col" class="px-4 pb-4 pt-6 text-sm font-semibold text-white/80" colspan="2">{t("Actions")}</th>
                                </tr>
                            </thead>
                            <tbody>
                                <For
                                    each=filtered
                                    key=|p| p.id
                                    children=move |p: PlaylistSummary| {
                                        let pid = p.id;
                                        let name = p.name.clone();
                                        let mods: Vec<String> = p.moderators.iter().map(|m| m.name.clone()).collect();
                                        let shown_mods: Vec<String> = mods.iter().take(3).cloned().collect();
                                        let extra_mods = mods.len().saturating_sub(3);
                                        let updated = p.updated_at.as_deref().unwrap_or("");
                                        let updated_short = updated.get(..10).unwrap_or(updated).to_string();
                                        let aria = format!("{}: {}", t("Edit playlist"), name);
                                        view! {
                                            <tr class="group border-t border-white/5 transition-colors hover:bg-white/[0.03]">
                                                <td class="px-4 py-4">
                                                    <A href=format!("/playlists/{}/edit", pid)
                                                        attr:class="rounded font-medium text-white transition-colors group-hover:text-teal-400 focus:outline-none focus:ring-2 focus:ring-teal-500/50"
                                                        attr:aria-label=aria.clone()>
                                                        <span class="truncate">{name.clone()}</span>
                                                    </A>
                                                </td>
                                                <td class="px-4 py-4">
                                                    {match p.description.clone() {
                                                        Some(d) if !d.is_empty() => view! {
                                                            <p class="max-w-md truncate text-sm text-white/70">{d}</p>
                                                        }.into_any(),
                                                        _ => view! {
                                                            <span class="text-sm text-white/40">{t("No description")}</span>
                                                        }.into_any(),
                                                    }}
                                                </td>
                                                <td class="px-4 py-4">
                                                    <div class="flex items-center gap-1.5">
                                                        <svg class="h-4 w-4 text-white/40" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                                                            <path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/>
                                                        </svg>
                                                        <span class="text-sm font-medium text-white/80">{p.total_tracks}</span>
                                                    </div>
                                                </td>
                                                <td class="px-4 py-4">
                                                    <span class="text-sm text-white/80">{p.owner_name.clone()}</span>
                                                </td>
                                                <td class="px-4 py-4">
                                                    <div class="flex flex-wrap gap-2">
                                                        {shown_mods.into_iter().map(|m| view! {
                                                            <span class="rounded bg-white/10 px-2 py-0.5 text-sm text-white/80">{m}</span>
                                                        }).collect_view()}
                                                        {(extra_mods > 0).then(|| view! {
                                                            <span class="text-sm text-white/40" aria-label=t("More moderators")>"+"{extra_mods}</span>
                                                        })}
                                                        {mods.is_empty().then(|| view! {
                                                            <span class="text-sm text-white/40">{t("None")}</span>
                                                        })}
                                                    </div>
                                                </td>
                                                <td class="px-4 py-4">
                                                    <span class="text-sm text-white/40">{t("Updated")}": "{updated_short}</span>
                                                </td>
                                                <td class="w-px px-4">
                                                    <A href=format!("/playlists/{}/edit", pid)
                                                        attr:class="flex items-center justify-center rounded focus:outline-none focus:ring-2 focus:ring-teal-500/50"
                                                        attr:aria-label=aria.clone()>
                                                        <svg class="block h-6 w-6 fill-white/40 transition-colors group-hover:fill-teal-400" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" aria-hidden="true">
                                                            <path d="M9.97 4.97a.75.75 0 011.06 0l5.5 5.5a.75.75 0 010 1.06l-5.5 5.5a.75.75 0 01-1.06-1.06L14.44 12 9.97 7.53a.75.75 0 010-1.06z"/>
                                                        </svg>
                                                    </A>
                                                </td>
                                            </tr>
                                        }
                                    }
                                />
                                <Show when=move || filtered().is_empty()>
                                    <tr>
                                        <td colspan="7" class="border-t border-white/5 px-4 py-8 text-center text-sm text-white/60">
                                            {t("No playlists found")}
                                        </td>
                                    </tr>
                                </Show>
                            </tbody>
                        </table>
                    </div>
                </div>

                <div class="grid gap-4 sm:grid-cols-2 lg:hidden">
                    <For
                        each=filtered
                        key=|p| p.id
                        children=move |p: PlaylistSummary| {
                            let pid = p.id;
                            let name = p.name.clone();
                            let mods: Vec<String> = p.moderators.iter().map(|m| m.name.clone()).collect();
                            let shown_mods: Vec<String> = mods.iter().take(3).cloned().collect();
                            let extra_mods = mods.len().saturating_sub(3);
                            let aria = format!("{}: {}", t("Edit playlist"), name);
                            view! {
                                <div class="group rounded-xl border border-white/10 bg-surface-raised/60 transition-all hover:border-teal-500/50 hover:shadow-xl">
                                    <A href=format!("/playlists/{}/edit", pid)
                                        attr:class="block rounded focus:outline-none focus:ring-2 focus:ring-teal-500/50"
                                        attr:aria-label=aria>
                                        <div class="p-4">
                                            <div class="mb-4 flex items-start justify-between gap-3">
                                                <div class="min-w-0 flex-1">
                                                    <h3 class="mb-1 truncate text-lg font-bold text-white/80 transition-colors group-hover:text-teal-400">{name}</h3>
                                                    {p.description.clone().filter(|d| !d.is_empty()).map(|d| view! {
                                                        <p class="line-clamp-2 text-sm text-white/60">{d}</p>
                                                    })}
                                                </div>
                                            </div>
                                            <div class="mb-4 flex items-center gap-4 text-xs text-white/40">
                                                <div class="flex items-center gap-1.5">
                                                    <svg class="h-3.5 w-3.5" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                                                        <path d="M12 3v10.55c-.59-.34-1.27-.55-2-.55-2.21 0-4 1.79-4 4s1.79 4 4 4 4-1.79 4-4V7h4V3h-6z"/>
                                                    </svg>
                                                    <span>{p.total_tracks}" "{t("tracks")}</span>
                                                </div>
                                                <div class="flex items-center gap-1.5">
                                                    <svg class="h-3.5 w-3.5" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                                                        <path d="M4.5 6.375a4.125 4.125 0 118.25 0 4.125 4.125 0 01-8.25 0zM14.25 8.625a3.375 3.375 0 116.75 0 3.375 3.375 0 01-6.75 0zM1.5 19.125a7.125 7.125 0 0114.25 0v.003l-.001.119a.75.75 0 01-.363.63 13.067 13.067 0 01-6.761 1.873c-2.472 0-4.786-.684-6.76-1.873a.75.75 0 01-.364-.63l-.001-.122zM17.25 19.128l-.001.144a2.25 2.25 0 01-.233.96 10.088 10.088 0 005.06-1.01.75.75 0 00.42-.643 4.875 4.875 0 00-6.957-4.611 8.586 8.586 0 011.71 5.157v.003z"/>
                                                    </svg>
                                                    <span>{mods.len()}" "{t("Moderators")}</span>
                                                </div>
                                            </div>
                                            <div class="space-y-2 border-t border-white/5 pt-3">
                                                <div class="flex items-center gap-2 text-xs">
                                                    <span class="text-white/40">{t("Owner")}": "</span>
                                                    <span class="text-white/80">{p.owner_name.clone()}</span>
                                                </div>
                                                {(!mods.is_empty()).then(|| view! {
                                                    <div class="flex flex-wrap items-center gap-1.5">
                                                        <span class="text-xs text-white/40">{t("Moderators")}": "</span>
                                                        {shown_mods.into_iter().map(|m| view! {
                                                            <span class="inline-flex items-center rounded-full bg-white/10 px-2 py-0.5 text-xs text-white/80">{m}</span>
                                                        }).collect_view()}
                                                        {(extra_mods > 0).then(|| view! {
                                                            <span class="text-xs text-white/40">"+"{extra_mods}</span>
                                                        })}
                                                    </div>
                                                })}
                                            </div>
                                            <div class="mt-4 flex items-center justify-end">
                                                <svg class="h-5 w-5 fill-white/40 transition-colors group-hover:fill-teal-500" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" aria-hidden="true">
                                                    <path d="M9.97 4.97a.75.75 0 011.06 0l5.5 5.5a.75.75 0 010 1.06l-5.5 5.5a.75.75 0 01-1.06-1.06L14.44 12 9.97 7.53a.75.75 0 010-1.06z"/>
                                                </svg>
                                            </div>
                                        </div>
                                    </A>
                                </div>
                            }
                        }
                    />
                    <Show when=move || filtered().is_empty()>
                        <div class="rounded-xl border border-white/10 bg-surface-raised/60 p-8 text-center text-white/60 sm:col-span-2">
                            {t("No playlists found")}
                        </div>
                    </Show>
                </div>
            </Show>
        </div>
    }
}

#[component]
pub fn PlaylistCreatePage() -> impl IntoView {
    crate::i18n::set_title_key("Create Playlist");
    crate::auth::require_registered(crate::auth::use_auth());
    let navigate = use_navigate();
    let name = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let navigate = navigate.clone();
        busy.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            let body = json!({"name": name.get_untracked()});
            match api::post::<Value, _>("/api/v1/playlists", &body).await {
                Ok(v) => {
                    let id = v["id"].as_i64().unwrap_or(0);
                    navigate(&format!("/playlists/{id}/edit"), Default::default());
                }
                Err(e) => {
                    error.set(Some(e.message));
                    busy.set(false);
                }
            }
        });
    };

    view! {
        <div class="mx-auto max-w-md px-4 py-12">
            <h1 class="retro-title retro-title--white mb-8 text-2xl">{t("Create playlist")}</h1>
            <form on:submit=submit class="space-y-5 rounded-xl border border-white/10 bg-surface-raised/60 p-6">
                {move || error.get().map(|e| view! {
                    <div class="rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">{e}</div>
                })}
                <div>
                    <label class="mb-1.5 block text-sm text-white/70" for="name">{t("Name")}</label>
                    <input id="name" type="text" maxlength="50" required
                        class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none focus:border-brand-accent/50"
                        prop:value=name
                        on:input=move |ev| name.set(event_target_value(&ev))/>
                </div>
                <button type="submit" class="btn-primary w-full" disabled=move || busy.get()>
                    {move || if busy.get() { t("Creating") } else { t("Create") }}
                </button>
            </form>
        </div>
    }
}
