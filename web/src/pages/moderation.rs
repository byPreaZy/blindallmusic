use crate::api;
use crate::auth::use_auth;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use serde_json::{json, Value};

const SECTIONS: &[(&str, &str)] = &[
    ("/moderation", "Dashboard"),
    ("/moderation/users", "Users"),
    ("/moderation/banned-users", "Banned users"),
    ("/moderation/trashed-messages", "Trashed messages"),
    ("/moderation/moderators", "Moderators"),
    ("/moderation/suggestions", "Suggestions"),
    ("/moderation/tracks", "Local tracks"),
    ("/moderation/data-quality", "Data quality"),
];

#[component]
pub fn ModShell(active: &'static str, children: ChildrenFn) -> impl IntoView {
    let auth = use_auth();
    view! {
        <div class="mx-auto max-w-6xl px-4 py-8">
            <Show when=move || {
                auth.ready.get()
                    && auth.user.get()
                        .map(|u| u.is_administrator || u.is_public_moderator)
                        .unwrap_or(false)
            }
                fallback=move || {
                    if auth.ready.get() {
                        view! {
                            <div class="py-24 text-center">
                                <h1 class="retro-title retro-title--white mb-4 text-2xl">"403"</h1>
                                <p class="text-white/60">{t("You do not have access to this page.")}</p>
                            </div>
                        }.into_any()
                    } else {
                        view! { <div class="py-24 text-center text-white/60">{t("Loading")}"…"</div> }.into_any()
                    }
                }>
                <h1 class="retro-title retro-title--white mb-6 text-3xl">{t("Moderation")}</h1>
                <nav class="mb-8 flex flex-wrap gap-2">
                    {SECTIONS.iter().filter(|(href, _)| {
                        // /moderation/moderators is admin-only server-side —
                        // hide it from public moderators instead of a silent 403.
                        *href != "/moderation/moderators"
                            || auth.user.get().map(|u| u.is_administrator).unwrap_or(false)
                    }).map(|(href, label)| {
                        let is = *href == active;
                        view! {
                            <A href=*href attr:class=move || if is {
                                "rounded-lg bg-brand-accent px-3 py-1.5 text-sm font-semibold text-black"
                            } else {
                                "rounded-lg border border-white/10 px-3 py-1.5 text-sm text-white/70 hover:bg-white/5"
                            }>{t(*label)}</A>
                        }
                    }).collect::<Vec<_>>()}
                </nav>
                {children()}
            </Show>
        </div>
    }
}

fn enc(s: &str) -> String {
    s.replace('%', "%25")
        .replace('&', "%26")
        .replace('=', "%3D")
        .replace(' ', "%20")
        .replace('#', "%23")
}

fn pager(
    page: RwSignal<i64>,
    last_page: RwSignal<i64>,
    reload: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <Show when=move || { last_page.get() > 1 }>
            <div class="mt-4 flex items-center justify-center gap-3">
                <button type="button" class="rounded border border-white/10 px-3 py-1 text-sm text-white/70 disabled:opacity-40"
                    disabled=move || { page.get() <= 1 }
                    on:click=move |_| { page.update(|p| *p -= 1); reload(); }>"←"</button>
                <span class="text-sm text-white/60">{move || format!("{} / {}", page.get(), last_page.get())}</span>
                <button type="button" class="rounded border border-white/10 px-3 py-1 text-sm text-white/70 disabled:opacity-40"
                    disabled=move || { page.get() >= last_page.get() }
                    on:click=move |_| { page.update(|p| *p += 1); reload(); }>"→"</button>
            </div>
        </Show>
    }
}

fn search_bar(
    search: RwSignal<String>,
    reload: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <form class="mb-4 flex gap-2" on:submit=move |ev| { ev.prevent_default(); reload(); }>
            <input type="text" placeholder=t("Search")
                class="rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-sm text-white outline-none"
                prop:value=search on:input=move |ev| search.set(event_target_value(&ev))/>
            <button type="submit" class="btn-primary !px-4 !py-2 text-sm">{t("Filter")}</button>
        </form>
    }
}

#[component]
fn ErrLine(error: RwSignal<Option<String>>) -> impl IntoView {
    view! {
        {move || error.get().map(|e| view! {
            <div class="mb-3 rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-2 text-sm text-red-300">{e}</div>
        })}
    }
}

fn fmt_date(v: &Value) -> String {
    v.as_str()
        .map(|s| s.chars().take(10).collect())
        .unwrap_or_else(|| "—".to_string())
}

// ---- dashboard ---------------------------------------------------------------

#[component]
pub fn ModDashboard() -> impl IntoView {
    let data = RwSignal::new(Value::Null);
    leptos::task::spawn_local(async move {
        if let Ok(v) = api::get::<Value>("/api/v1/moderation").await {
            data.set(v);
        }
    });
    view! {
        <ModShell active="/moderation">
            <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
                {move || {
                    let d = data.get();
                    [
                        ("Users", "total_users"), ("Rooms", "total_rooms"),
                        ("Public rooms", "public_rooms"), ("Private rooms", "private_rooms"),
                        ("Playlists", "total_playlists"), ("Public playlists", "public_playlists"),
                        ("Private playlists", "private_playlists"),
                        ("Messages today", "todays_messages"), ("Banned users", "banned_users"),
                    ].into_iter().map(|(label, key)| {
                        let val = d[key].as_i64().unwrap_or(0);
                        view! {
                            <div class="card p-5">
                                <div class="text-sm text-white/50">{t(label)}</div>
                                <div class="retro-title retro-title--white mt-1 text-2xl">{val}</div>
                            </div>
                        }
                    }).collect::<Vec<_>>()
                }}
            </div>
        </ModShell>
    }
}

// ---- trashed messages ----------------------------------------------------------

#[component]
pub fn ModTrashedMessages() -> impl IntoView {
    let data = RwSignal::new(Value::Null);
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);
    let page = RwSignal::new(1i64);
    let last_page = RwSignal::new(1i64);
    let search = RwSignal::new(String::new());

    let load = move || {
        let s = search.get_untracked();
        let p = page.get_untracked();
        leptos::task::spawn_local(async move {
            match api::get::<Value>(&format!(
                "/api/v1/moderation/trashed-messages?page={p}&search={}",
                enc(&s)
            ))
            .await
            {
                Ok(v) => {
                    last_page.set(v["last_page"].as_i64().unwrap_or(1));
                    data.set(v);
                }
                Err(e) => error.set(Some(e.message)),
            }
            loading.set(false);
        });
    };
    load();

    view! {
        <ModShell active="/moderation/trashed-messages">
            <ErrLine error=error/>
            {search_bar(search, move || { page.set(1); load(); })}
            <Show when=move || !loading.get() fallback=move || view! {
                <div class="py-16 text-center text-white/60">{t("Loading")}"…"</div>
            }>
                <div class="card">
                    {move || data.get()["data"].as_array().cloned().unwrap_or_default()
                        .into_iter().map(|m| {
                            let mid = m["id"].as_i64().unwrap_or(0);
                            let body = m["body"].as_str().unwrap_or("").to_string();
                            let uname = m["user_name"].as_str().or(m["user"]["name"].as_str()).unwrap_or("?").to_string();
                            let uid = m["user_id"].as_i64().or(m["user"]["id"].as_i64()).unwrap_or(0);
                            let room = m["room_name"].as_str().or(m["room"]["name"].as_str()).unwrap_or("—").to_string();
                            let deleted = fmt_date(&m["deleted_at"]);
                            view! {
                                <div class="border-b border-white/5 px-4 py-3 last:border-0">
                                    <div class="flex items-start justify-between gap-4">
                                        <div>
                                            <div class="text-sm text-white/80">{body}</div>
                                            <div class="mt-1 text-xs text-white/40">
                                                <A href=format!("/profile/{uid}") attr:class="hover:text-brand-accent-light">{uname}</A>
                                                " · " {room} " · " {t("deleted")} " " {deleted}
                                            </div>
                                        </div>
                                        <div class="flex shrink-0 gap-1">
                                            <button type="button" class="rounded px-2 py-1 text-xs text-emerald-300 hover:bg-emerald-500/10"
                                                on:click=move |_| leptos::task::spawn_local(async move {
                                                    let _ = api::post_empty::<Value>(&format!("/api/v1/moderation/trashed-messages/{mid}/restore")).await;
                                                    load();
                                                })>{t("Restore")}</button>
                                            <button type="button" class="rounded px-2 py-1 text-xs text-red-400 hover:bg-red-500/10"
                                                on:click=move |_| leptos::task::spawn_local(async move {
                                                    let _ = api::delete::<Value>(&format!("/api/v1/moderation/trashed-messages/{mid}")).await;
                                                    load();
                                                })>{t("Delete")}</button>
                                        </div>
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                </div>
                {pager(page, last_page, load)}
            </Show>
        </ModShell>
    }
}

// ---- banned users --------------------------------------------------------------

#[component]
pub fn ModBannedUsers() -> impl IntoView {
    let auth = use_auth();
    let data = RwSignal::new(Value::Null);
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);
    let page = RwSignal::new(1i64);
    let last_page = RwSignal::new(1i64);
    let search = RwSignal::new(String::new());

    let load = move || {
        let s = search.get_untracked();
        let p = page.get_untracked();
        leptos::task::spawn_local(async move {
            match api::get::<Value>(&format!(
                "/api/v1/moderation/banned-users?page={p}&search={}",
                enc(&s)
            ))
            .await
            {
                Ok(v) => {
                    last_page.set(v["bannedUsers"]["last_page"].as_i64().unwrap_or(1));
                    data.set(v["bannedUsers"].clone());
                }
                Err(e) => error.set(Some(e.message)),
            }
            loading.set(false);
        });
    };
    load();

    view! {
        <ModShell active="/moderation/banned-users">
            <ErrLine error=error/>
            {search_bar(search, move || { page.set(1); load(); })}
            <Show when=move || !loading.get() fallback=move || view! {
                <div class="py-16 text-center text-white/60">{t("Loading")}"…"</div>
            }>
                <div class="card">
                    {move || data.get()["data"].as_array().cloned().unwrap_or_default()
                        .into_iter().map(|u| {
                            let uid = u["id"].as_i64().unwrap_or(0);
                            let name = u["name"].as_str().unwrap_or("").to_string();
                            let email = u["email"].as_str().unwrap_or("—").to_string();
                            let ip = u["ip"].as_str().unwrap_or("—").to_string();
                            let bans = u["bans_count"].as_i64().unwrap_or(0);
                            view! {
                                <div class="flex items-center justify-between border-b border-white/5 px-4 py-3 last:border-0">
                                    <div>
                                        <A href=format!("/moderation/users/{uid}") attr:class="font-medium text-white hover:text-brand-accent-light">
                                            {name}
                                        </A>
                                        <div class="text-xs text-white/40">{email} " · ip: " {ip} " · " {bans} " " {t("bans")}</div>
                                    </div>
                                    {move || auth.user.get().map(|me| me.is_administrator).unwrap_or(false).then(|| view! {
                                        <button type="button" class="rounded-lg border border-emerald-500/30 px-3 py-1.5 text-sm text-emerald-300"
                                            on:click=move |_| leptos::task::spawn_local(async move {
                                                let _ = api::post_empty::<Value>(&format!("/api/v1/moderation/banned-users/{uid}/unban")).await;
                                                load();
                                            })>{t("Unban")}</button>
                                    })}
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                </div>
                {pager(page, last_page, load)}
            </Show>
        </ModShell>
    }
}

// ---- suggestions queue -----------------------------------------------------------

#[component]
pub fn ModSuggestions() -> impl IntoView {
    let data = RwSignal::new(Vec::<Value>::new());
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);

    let load = move || {
        leptos::task::spawn_local(async move {
            match api::get::<Value>("/api/v1/moderation/suggestions").await {
                Ok(v) => data.set(v["suggestions"].as_array().cloned().unwrap_or_default()),
                Err(e) => error.set(Some(e.message)),
            }
            loading.set(false);
        });
    };
    load();

    view! {
        <ModShell active="/moderation/suggestions">
            <ErrLine error=error/>
            <Show when=move || !loading.get() fallback=move || view! {
                <div class="py-16 text-center text-white/60">{t("Loading")}"…"</div>
            }>
                <div class="card">
                    {move || {
                        let list = data.get();
                        if list.is_empty() {
                            return view! {
                                <div class="px-4 py-12 text-center text-sm text-white/40">{t("No suggestions yet.")}</div>
                            }.into_any();
                        }
                        list.into_iter().map(|s| {
                            let nid = s["id"].as_str().unwrap_or("").to_string();
                            let read = s["read"].as_bool().unwrap_or(false);
                            let d = s["data"].clone();
                            let room_id = d["room"]["id"].as_i64().unwrap_or(0);
                            let room_name = d["room"]["name"].as_str().unwrap_or("").to_string();
                            let user_name = d["user"]["name"].as_str().unwrap_or("—").to_string();
                            let message = d["message"].as_str().unwrap_or("").to_string();
                            let date = fmt_date(&s["created_at"]);
                            let playlists: Vec<Value> = d["room"]["playlist_list"]
                                .as_array().cloned().unwrap_or_default();
                            let playlists_legacy = d["room"]["playlists"].as_str().unwrap_or("").to_string();
                            let track: Option<Value> = d["track"].as_object().map(|_| d["track"].clone());
                            view! {
                                <div class={format!("border-b border-white/5 px-4 py-3 last:border-0 {}", if read { "opacity-50" } else { "" })}>
                                    <div class="flex items-start justify-between gap-4">
                                        <div class="min-w-0">
                                            <div class="flex flex-wrap items-center gap-2 text-xs text-white/40">
                                                <span>{date}</span>
                                                <A href=format!("/rooms/{room_id}") attr:class="text-brand-accent-light hover:underline">{room_name}</A>
                                                <span>"· " {user_name}</span>
                                            </div>
                                            <p class="mt-1 text-sm text-white/90">{message}</p>
                                            {track.map(|tr| {
                                                let tid = tr["id"].as_i64().unwrap_or(0);
                                                let answers = tr["answers"].as_array().cloned().unwrap_or_default();
                                                view! {
                                                    <div class="mt-2 rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-xs">
                                                        <div class="mb-1 font-medium text-white/50">
                                                            {t("Track")} {format!(" #{tid}")}
                                                        </div>
                                                        {answers.into_iter().map(|a| {
                                                            let ty = a["type"].as_str().unwrap_or("").to_string();
                                                            let val = a["value"].as_str().unwrap_or("").to_string();
                                                            view! { <div class="text-white/70"><span class="text-white/40">{ty} " : "</span>{val}</div> }
                                                        }).collect::<Vec<_>>()}
                                                        <RematchPanel track_id=tid/>
                                                    </div>
                                                }.into_any()
                                            })}
                                            <div class="mt-2 flex flex-wrap gap-2">
                                                {if playlists.is_empty() && !playlists_legacy.is_empty() {
                                                    view! { <span class="text-xs text-white/40">{playlists_legacy}</span> }.into_any()
                                                } else {
                                                    playlists.into_iter().map(|p| {
                                                        let pid = p["id"].as_i64().unwrap_or(0);
                                                        let pname = p["name"].as_str().unwrap_or("").to_string();
                                                        view! {
                                                            <A href=format!("/playlists/{pid}/edit") attr:class="rounded border border-white/15 px-2 py-0.5 text-xs text-brand-accent-light hover:bg-white/5">
                                                                {pname}
                                                            </A>
                                                        }
                                                    }).collect::<Vec<_>>().into_any()
                                                }}
                                            </div>
                                        </div>
                                        {(!read).then(|| {
                                            view! {
                                                <button type="button" class="shrink-0 rounded-lg border border-emerald-500/30 px-3 py-1.5 text-sm text-emerald-300"
                                                    on:click=move |_| {
                                                        let nid = nid.clone();
                                                        leptos::task::spawn_local(async move {
                                                            let _ = api::post_empty::<Value>(&format!("/api/v1/users/notifications/{nid}/done")).await;
                                                            load();
                                                        });
                                                    }>{t("Done")}</button>
                                            }.into_any()
                                        })}
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>().into_any()
                    }}
                </div>
            </Show>
        </ModShell>
    }
}

// ---- deezer rematch ------------------------------------------------------------

/// Lets a moderator re-point a track at a correct Deezer version — for wrong
/// audio reports (foreign-language covers, bad rips) raised by suggestions.
#[component]
fn RematchPanel(track_id: i64) -> impl IntoView {
    let open = RwSignal::new(false);
    let term = RwSignal::new(String::new());
    let term_loaded = RwSignal::new(false);
    let candidates = RwSignal::new(Vec::<Value>::new());
    let searching = RwSignal::new(false);
    let applied = RwSignal::new(Option::<String>::None);
    let err = RwSignal::new(Option::<String>::None);

    let search = move || {
        searching.set(true);
        err.set(None);
        let q = term.get_untracked();
        let path = if q.trim().is_empty() {
            format!("/api/v1/moderation/tracks/{track_id}/rematch")
        } else {
            format!(
                "/api/v1/moderation/tracks/{track_id}/rematch?q={}",
                js_sys::encode_uri_component(&q)
            )
        };
        leptos::task::spawn_local(async move {
            match api::get::<Value>(&path).await {
                Ok(v) => {
                    term_loaded.set(true);
                    if let Some(t) = v["term"].as_str() {
                        term.set(t.to_string());
                    }
                    candidates.set(v["tracks"].as_array().cloned().unwrap_or_default());
                }
                Err(e) => err.set(Some(e.message)),
            }
            searching.set(false);
        });
    };

    view! {
        <div class="mt-2 border-t border-white/10 pt-2">
            <Show when=move || applied.get().is_some() fallback=move || view! {
                <div>
                    <Show when=move || !open.get() fallback=move || view! {
                        <div class="space-y-2">
                            <div class="flex items-center gap-2">
                                <input type="text" class="input flex-1 !py-1 text-xs"
                                    prop:value=move || term.get()
                                    on:input=move |e| term.set(event_target_value(&e))
                                    placeholder="Artiste — titre…"/>
                                <button type="button" class="rounded border border-white/15 px-2 py-1 text-xs text-brand-accent-light hover:bg-white/5"
                                    disabled=move || searching.get()
                                    on:click=move |_| search()>
                                    {if searching.get() { t("Loading").to_string() } else { t("Search Deezer").to_string() }}
                                </button>
                            </div>
                            <ErrLine error=err/>
                            {candidates.get().into_iter().map(|c| {
                                let pid = c["provider_id"].as_str().unwrap_or("").to_string();
                                let artist = c["artist_name"].as_str().unwrap_or("").to_string();
                                let name = c["track_name"].as_str().unwrap_or("").to_string();
                                let album = c["album_name"].as_str().unwrap_or("").to_string();
                                let art = c["artwork_url"].as_str().unwrap_or("").to_string();
                                let purl = c["provider_url"].as_str().map(str::to_string);
                                let preview = c["preview_url"].as_str().unwrap_or("").to_string();
                                let label = format!("{artist} — {name}");
                                let use_label = label.clone();
                                let use_art = art.clone();
                                view! {
                                    <div class="flex items-center gap-2 rounded border border-white/10 px-2 py-1.5">
                                        {(!art.is_empty()).then(|| view! { <img src=art class="h-8 w-8 rounded" alt=""/> })}
                                        <div class="min-w-0 flex-1">
                                            <div class="truncate text-white/85">{label}</div>
                                            <div class="truncate text-white/40">{album}</div>
                                        </div>
                                        {(!preview.is_empty()).then(|| view! {
                                            <audio controls preload="none" src=preview class="h-7 w-40"/>
                                        })}
                                        <button type="button" class="shrink-0 rounded border border-emerald-500/30 px-2 py-1 text-xs text-emerald-300"
                                            on:click=move |_| {
                                                let label = use_label.clone();
                                                let purl = purl.clone();
                                                let art = use_art.clone();
                                                let pid = pid.clone();
                                                let name = name.clone();
                                                let artist = artist.clone();
                                                leptos::task::spawn_local(async move {
                                                    let body = serde_json::json!({
                                                        "provider": "deezer",
                                                        "provider_id": pid,
                                                        "provider_url": purl,
                                                        "artwork_url": art,
                                                        "track_name": name,
                                                        "artist_name": artist,
                                                    });
                                                    match api::post::<Value, _>(&format!("/api/v1/moderation/tracks/{track_id}/rematch"), &body).await {
                                                        Ok(_) => applied.set(Some(label)),
                                                        Err(e) => err.set(Some(e.message)),
                                                    }
                                                });
                                            }>{t("Use")}</button>
                                    </div>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    }>
                        <button type="button" class="text-xs text-brand-accent-light hover:underline"
                            on:click=move |_| {
                                open.set(true);
                                if !term_loaded.get_untracked() {
                                    search();
                                }
                            }>
                            {t("Rematch audio (Deezer)")}
                        </button>
                    </Show>
                </div>
            }>
                <div class="text-emerald-300">{t("Audio replaced:")} " " {applied.get().unwrap_or_default()}</div>
            </Show>
        </div>
    }
}

// ---- users management ------------------------------------------------------------

#[component]
pub fn ModUsers() -> impl IntoView {
    let auth = use_auth();
    let data = RwSignal::new(Value::Null);
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);
    let page = RwSignal::new(1i64);
    let last_page = RwSignal::new(1i64);
    let search = RwSignal::new(String::new());
    let ban_user = RwSignal::new(Option::<i64>::None);
    let ban_reason = RwSignal::new(String::new());
    let ban_duration = RwSignal::new(String::new());

    let load = move || {
        let s = search.get_untracked();
        let p = page.get_untracked();
        leptos::task::spawn_local(async move {
            match api::get::<Value>(&format!(
                "/api/v1/moderation/users?page={p}&search={}",
                enc(&s)
            ))
            .await
            {
                Ok(v) => {
                    last_page.set(v["users"]["last_page"].as_i64().unwrap_or(1));
                    data.set(v["users"].clone());
                }
                Err(e) => error.set(Some(e.message)),
            }
            loading.set(false);
        });
    };
    load();

    view! {
        <ModShell active="/moderation/users">
            <ErrLine error=error/>
            {search_bar(search, move || { page.set(1); load(); })}
            <Show when=move || !loading.get() fallback=move || view! {
                <div class="py-16 text-center text-white/60">{t("Loading")}"…"</div>
            }>
                <div class="card overflow-x-auto">
                    <table class="w-full text-left text-sm">
                        <thead class="text-xs uppercase text-white/40">
                            <tr>
                                <th class="px-4 py-3">{t("User")}</th>
                                <th class="px-4 py-3">{t("Messages")}</th>
                                <th class="px-4 py-3">{t("Reported")}</th>
                                <th class="px-4 py-3">{t("Status")}</th>
                                <th class="px-4 py-3 text-right">{t("Actions")}</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || data.get()["data"].as_array().cloned().unwrap_or_default()
                                .into_iter().map(|u| {
                                    let uid = u["id"].as_i64().unwrap_or(0);
                                    let name = u["name"].as_str().unwrap_or("").to_string();
                                    let msgs = u["messages_count"].as_i64().unwrap_or(0);
                                    let reported = u["reported_messages_count"].as_i64().unwrap_or(0);
                                    let banned = u["is_banned"].as_bool().unwrap_or(false);
                                    let staff = u["is_admin"].as_bool().unwrap_or(false) || u["is_moderator"].as_bool().unwrap_or(false);
                                    let viewer_admin = auth.user.get().map(|me| me.is_administrator).unwrap_or(false);
                                    let can_ban = u["can_ban"].as_bool().unwrap_or(!staff) && viewer_admin;
                                    view! {
                                        <tr class="border-b border-white/5 last:border-0">
                                            <td class="px-4 py-2">
                                                <A href=format!("/moderation/users/{uid}") attr:class="font-medium text-white hover:text-brand-accent-light">
                                                    {name}
                                                </A>
                                                <div class="text-xs text-white/40">
                                                    {u["provider"].as_str().unwrap_or("blindall").to_string()}
                                                    {u["is_guest"].as_bool().unwrap_or(false).then(|| " · guest")}
                                                </div>
                                            </td>
                                            <td class="px-4 py-2 text-white/70">{msgs}</td>
                                            <td class="px-4 py-2">
                                                <span class={if reported > 0 { "text-amber-300" } else { "text-white/50" }}>{reported}</span>
                                            </td>
                                            <td class="px-4 py-2">
                                                {if banned { view! {
                                                    <span class="rounded bg-red-500/20 px-2 py-0.5 text-xs text-red-300">{t("Banned")}</span>
                                                }.into_any() } else if staff { view! {
                                                    <span class="rounded bg-brand-accent/20 px-2 py-0.5 text-xs text-brand-accent-light">{t("Staff")}</span>
                                                }.into_any() } else { view! {
                                                    <span class="rounded bg-white/10 px-2 py-0.5 text-xs text-white/60">{t("Active")}</span>
                                                }.into_any() }}
                                            </td>
                                            <td class="px-4 py-2 text-right">
                                                <div class="flex justify-end gap-1">
                                                    {if banned && viewer_admin { view! {
                                                        <button type="button" class="rounded px-2 py-1 text-xs text-emerald-300 hover:bg-emerald-500/10"
                                                            on:click=move |_| leptos::task::spawn_local(async move {
                                                                let _ = api::post_empty::<Value>(&format!("/api/v1/moderation/banned-users/{uid}/unban")).await;
                                                                load();
                                                            })>{t("Unban")}</button>
                                                    }.into_any() } else if can_ban { view! {
                                                        <button type="button" class="rounded px-2 py-1 text-xs text-red-400 hover:bg-red-500/10"
                                                            on:click=move |_| ban_user.set(Some(uid))>{t("Ban")}</button>
                                                    }.into_any() } else { view! {<span/>}.into_any() }}
                                                </div>
                                            </td>
                                        </tr>
                                    }
                                }).collect::<Vec<_>>()}
                        </tbody>
                    </table>
                </div>
                {pager(page, last_page, load)}
            </Show>

            <Show when=move || ban_user.get().is_some()>
                <form class="card mt-6 grid max-w-md gap-3 p-5" on:submit=move |ev| {
                    ev.prevent_default();
                    let uid = ban_user.get().unwrap_or(0);
                    let reason = ban_reason.get_untracked();
                    let dur = ban_duration.get_untracked();
                    leptos::task::spawn_local(async move {
                        let body = json!({
                            "reason": reason,
                            "duration": dur.parse::<i64>().ok(),
                        });
                        match api::post::<Value,_>(&format!("/api/v1/moderation/users/{uid}/ban"), &body).await {
                            Ok(_) => { ban_user.set(None); ban_reason.set(String::new()); ban_duration.set(String::new()); load(); }
                            Err(e) => error.set(Some(e.message)),
                        }
                    });
                }>
                    <h2 class="font-bold text-white">{t("Ban user")}</h2>
                    <label class="text-sm text-white/60">{t("Reason")}
                        <input type="text" required maxlength="255"
                            class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                            prop:value=ban_reason on:input=move |ev| ban_reason.set(event_target_value(&ev))/></label>
                    <label class="text-sm text-white/60">{t("Duration (minutes, empty = permanent)")}
                        <input type="number" min="1"
                            class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                            prop:value=ban_duration on:input=move |ev| ban_duration.set(event_target_value(&ev))/></label>
                    <div class="flex gap-2">
                        <button type="submit" class="rounded-lg bg-red-500 px-4 py-2 text-sm font-semibold text-brand-midnight">{t("Ban")}</button>
                        <button type="button" class="rounded-lg border border-white/10 px-4 py-2 text-sm text-white/70"
                            on:click=move |_| ban_user.set(None)>{t("Cancel")}</button>
                    </div>
                </form>
            </Show>
        </ModShell>
    }
}

// ---- user detail -------------------------------------------------------------------

#[component]
pub fn ModUserShow() -> impl IntoView {
    let auth = use_auth();
    let params = use_params_map();
    let data = RwSignal::new(Value::Null);
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);
    let ban_reason = RwSignal::new(String::new());
    let ban_duration = RwSignal::new(String::new());
    let show_ban = RwSignal::new(false);

    let load = move || {
        let id = params.get().get("id").unwrap_or_default();
        leptos::task::spawn_local(async move {
            match api::get::<Value>(&format!("/api/v1/moderation/users/{id}")).await {
                Ok(v) => data.set(v),
                Err(e) => error.set(Some(e.message)),
            }
            loading.set(false);
        });
    };
    load();

    view! {
        <ModShell active="/moderation/users">
            <ErrLine error=error/>
            <Show when=move || !loading.get() fallback=move || view! {
                <div class="py-16 text-center text-white/60">{t("Loading")}"…"</div>
            }>
                {move || {
                    let d = data.get();
                    let u = d["user"].clone();
                    if u.is_null() { return view! {<div/>}.into_any(); }
                    let uid = u["id"].as_i64().unwrap_or(0);
                    let name = u["name"].as_str().unwrap_or("").to_string();
                    let provider = u["provider"].as_str().unwrap_or("blindall").to_string();
                    let created = fmt_date(&u["created_at"]);
                    let banned = u["is_banned"].as_bool().unwrap_or(false);
                    let sensitive = d["canViewSensitiveData"].as_bool().unwrap_or(false);
                    let email = u["email"].as_str().unwrap_or("—").to_string();
                    let ip = u["ip"].as_str().unwrap_or("—").to_string();
                    let elo = u["elo"].as_i64().unwrap_or(0);
                    let history = u["ban_history"].as_array().cloned().unwrap_or_default();
                    view! {
                        <div class="grid gap-4 md:grid-cols-2">
                            <div class="card p-5">
                                <div class="mb-3 flex items-center justify-between">
                                    <h2 class="text-lg font-bold text-white">{name}</h2>
                                    {let admin = auth.user.get().map(|me| me.is_administrator).unwrap_or(false);
                                     if banned && admin { view! {
                                        <button type="button" class="rounded-lg border border-emerald-500/30 px-3 py-1.5 text-sm text-emerald-300"
                                            on:click=move |_| leptos::task::spawn_local(async move {
                                                let _ = api::post_empty::<Value>(&format!("/api/v1/moderation/banned-users/{uid}/unban")).await;
                                                load();
                                            })>{t("Unban")}</button>
                                    }.into_any() } else if admin { view! {
                                        <button type="button" class="rounded-lg bg-red-500/20 px-3 py-1.5 text-sm text-red-300"
                                            on:click=move |_| show_ban.set(true)>{t("Ban")}</button>
                                    }.into_any() } else { view! {<span/>}.into_any() }}
                                </div>
                                <dl class="space-y-1 text-sm">
                                    <div class="flex justify-between"><dt class="text-white/50">"ID"</dt><dd class="text-white">{uid}</dd></div>
                                    <div class="flex justify-between"><dt class="text-white/50">{t("Provider")}</dt><dd class="text-white">{provider}</dd></div>
                                    <div class="flex justify-between"><dt class="text-white/50">"ELO"</dt><dd class="text-white">{elo}</dd></div>
                                    <div class="flex justify-between"><dt class="text-white/50">{t("Member since")}</dt><dd class="text-white">{created}</dd></div>
                                    {sensitive.then(|| view! {
                                        <div>
                                            <div class="flex justify-between"><dt class="text-white/50">{t("Email")}</dt><dd class="text-white">{email}</dd></div>
                                            <div class="flex justify-between"><dt class="text-white/50">"IP"</dt><dd class="text-white">{ip}</dd></div>
                                        </div>
                                    })}
                                </dl>
                            </div>
                            <div class="card p-5">
                                <h2 class="mb-3 font-bold text-white">{t("Ban history")}</h2>
                                {if history.is_empty() { view! {
                                    <p class="text-sm text-white/50">{t("No bans")}</p>
                                }.into_any() } else { view! {
                                    <div class="space-y-2">
                                        {history.into_iter().map(|b| {
                                            let comment = b["comment"].as_str().unwrap_or("—").to_string();
                                            let expired = b["expired_at"].as_str().unwrap_or("permanent").to_string();
                                            let at = fmt_date(&b["created_at"]);
                                            view! {
                                                <div class="rounded border border-white/10 px-3 py-2 text-xs">
                                                    <div class="text-white/80">{comment}</div>
                                                    <div class="text-white/40">{at} " → " {expired}</div>
                                                </div>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </div>
                                }.into_any() }}
                            </div>
                        </div>
                        <Show when=move || show_ban.get()>
                            <form class="card mt-4 grid max-w-md gap-3 p-5" on:submit=move |ev| {
                                ev.prevent_default();
                                let reason = ban_reason.get_untracked();
                                let dur = ban_duration.get_untracked();
                                leptos::task::spawn_local(async move {
                                    let body = json!({"reason": reason, "duration": dur.parse::<i64>().ok()});
                                    match api::post::<Value,_>(&format!("/api/v1/moderation/users/{uid}/ban"), &body).await {
                                        Ok(_) => { show_ban.set(false); load(); }
                                        Err(e) => error.set(Some(e.message)),
                                    }
                                });
                            }>
                                <label class="text-sm text-white/60">{t("Reason")}
                                    <input type="text" required maxlength="255"
                                        class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                                        prop:value=ban_reason on:input=move |ev| ban_reason.set(event_target_value(&ev))/></label>
                                <label class="text-sm text-white/60">{t("Duration (minutes)")}
                                    <input type="number" min="1"
                                        class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                                        prop:value=ban_duration on:input=move |ev| ban_duration.set(event_target_value(&ev))/></label>
                                <div class="flex gap-2">
                                    <button type="submit" class="rounded-lg bg-red-500 px-4 py-2 text-sm font-semibold text-brand-midnight">{t("Ban")}</button>
                                    <button type="button" class="rounded-lg border border-white/10 px-4 py-2 text-sm text-white/70"
                                        on:click=move |_| show_ban.set(false)>{t("Cancel")}</button>
                                </div>
                            </form>
                        </Show>
                    }.into_any()
                }}
            </Show>
        </ModShell>
    }
}

// ---- moderators ------------------------------------------------------------------

#[component]
pub fn ModModerators() -> impl IntoView {
    let data = RwSignal::new(Value::Null);
    let coverage = RwSignal::new(Value::Null);
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);
    let page = RwSignal::new(1i64);
    let last_page = RwSignal::new(1i64);

    let load = move || {
        let p = page.get_untracked();
        leptos::task::spawn_local(async move {
            match api::get::<Value>(&format!("/api/v1/moderation/moderators?page={p}")).await {
                Ok(v) => {
                    last_page.set(v["moderators"]["last_page"].as_i64().unwrap_or(1));
                    coverage.set(v["coverage"].clone());
                    data.set(v["moderators"].clone());
                }
                Err(e) => error.set(Some(e.message)),
            }
            loading.set(false);
        });
    };
    load();

    let revoke = move |kind: &'static str, target: i64, uid: i64| {
        leptos::task::spawn_local(async move {
            let url = format!("/api/v1/moderation/moderators/{kind}/{target}/users/{uid}");
            match api::delete::<Value>(&url).await {
                Ok(_) => load(),
                Err(e) => error.set(Some(e.message)),
            }
        });
    };

    view! {
        <ModShell active="/moderation/moderators">
            <ErrLine error=error/>
            <Show when=move || !loading.get() fallback=move || view! {
                <div class="py-16 text-center text-white/60">{t("Loading")}"…"</div>
            }>
                {move || {
                    let cov = coverage.get();
                    let unmod = |key: &str| cov[key].as_array().cloned().unwrap_or_default();
                    let rooms_unmod = unmod("rooms_without_moderators");
                    let pl_unmod = unmod("playlists_without_moderators");
                    if rooms_unmod.is_empty() && pl_unmod.is_empty() {
                        return ().into_any();
                    }
                    let panel = |title: String, items: Vec<Value>, href: fn(i64) -> String| {
                        view! {
                            <div class="card p-4">
                                <div class="mb-2 text-sm font-semibold text-white">{title}</div>
                                <div class="flex flex-wrap gap-2">
                                    {items.into_iter().map(|it| {
                                        let id = it["id"].as_i64().unwrap_or(0);
                                        let name = it["name"].as_str().unwrap_or("").to_string();
                                        let owner = it["owner_name"].as_str().unwrap_or("").to_string();
                                        view! {
                                            <A href=href(id) attr:class="rounded-lg border border-amber-400/30 bg-amber-400/10 px-2 py-1 text-xs text-amber-200 hover:bg-amber-400/20">
                                                {name} {move || if owner.is_empty() { String::new() } else { format!(" · {owner}") }}
                                            </A>
                                        }
                                    }).collect::<Vec<_>>()}
                                </div>
                            </div>
                        }.into_any()
                    };
                    view! {
                        <div class="mb-4 grid gap-3 md:grid-cols-2">
                            {if rooms_unmod.is_empty() { ().into_any() } else { panel(t("Public rooms without moderators").to_string(), rooms_unmod, |id| format!("/rooms/{id}")) }}
                            {if pl_unmod.is_empty() { ().into_any() } else { panel(t("Public playlists without moderators").to_string(), pl_unmod, |id| format!("/playlists/{id}")) }}
                        </div>
                    }.into_any()
                }}
                <div class="card">
                    {move || data.get()["data"].as_array().cloned().unwrap_or_default()
                        .into_iter().map(|u| {
                            let uid = u["id"].as_i64().unwrap_or(0);
                            let name = u["name"].as_str().unwrap_or("").to_string();
                            let created = fmt_date(&u["created_at"]);
                            let rooms = u["rooms"].as_array().cloned().unwrap_or_default();
                            let playlists = u["playlists"].as_array().cloned().unwrap_or_default();
                            let chip = move |a: Value, kind: &'static str| {
                                let tid = a["id"].as_i64().unwrap_or(0);
                                let aname = a["name"].as_str().unwrap_or("").to_string();
                                let is_owner = a["is_owner"].as_bool().unwrap_or(false);
                                view! {
                                    <span class="inline-flex items-center gap-1 rounded-lg border border-white/10 bg-white/5 px-2 py-0.5 text-xs text-white/80">
                                        {aname}
                                        {if is_owner {
                                            view! { <span class="text-white/30">{t("owner")}</span> }.into_any()
                                        } else {
                                            view! {
                                                <button type="button" title=t("Revoke")
                                                    class="ml-0.5 text-red-300/70 hover:text-red-300"
                                                    on:click=move |_| revoke(kind, tid, uid)>"×"</button>
                                            }.into_any()
                                        }}
                                    </span>
                                }
                            };
                            view! {
                                <div class="border-b border-white/5 px-4 py-3 last:border-0">
                                    <div class="flex items-center justify-between">
                                        <A href=format!("/profile/{uid}") attr:class="font-medium text-white hover:text-brand-accent-light">{name}</A>
                                        <span class="text-xs text-white/40">{t("since")} " " {created}</span>
                                    </div>
                                    <div class="mt-1.5 flex flex-wrap items-center gap-1.5">
                                        {rooms.into_iter().map(|a| chip(a, "rooms")).collect::<Vec<_>>()}
                                        {playlists.into_iter().map(|a| chip(a, "playlists")).collect::<Vec<_>>()}
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                </div>
                {pager(page, last_page, load)}
            </Show>
        </ModShell>
    }
}

// ---- local tracks --------------------------------------------------------------------

#[component]
pub fn ModTracks() -> impl IntoView {
    let data = RwSignal::new(Value::Null);
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);
    let page = RwSignal::new(1i64);
    let last_page = RwSignal::new(1i64);
    let search = RwSignal::new(String::new());
    let editing = RwSignal::new(Option::<i64>::None);
    let form = RwSignal::new(json!({"artist_name":"","track_name":""}));

    let load = move || {
        let s = search.get_untracked();
        let p = page.get_untracked();
        leptos::task::spawn_local(async move {
            match api::get::<Value>(&format!(
                "/api/v1/moderation/tracks?page={p}&search={}",
                enc(&s)
            ))
            .await
            {
                Ok(v) => {
                    let total = v["total"].as_i64().unwrap_or(0);
                    last_page.set((total + 19) / 20);
                    data.set(v);
                }
                Err(e) => error.set(Some(e.message)),
            }
            loading.set(false);
        });
    };
    load();

    view! {
        <ModShell active="/moderation/tracks">
            <ErrLine error=error/>
            {search_bar(search, move || { page.set(1); load(); })}
            <Show when=move || !loading.get() fallback=move || view! {
                <div class="py-16 text-center text-white/60">{t("Loading")}"…"</div>
            }>
                <div class="card">
                    {move || data.get()["data"].as_array().cloned().unwrap_or_default()
                        .into_iter().map(|tr| {
                            let tid = tr["id"].as_i64().unwrap_or(0);
                            let artist = tr["artist_name"].as_str().unwrap_or("").to_string();
                            let track = tr["track_name"].as_str().unwrap_or("").to_string();
                            let etr = tr.clone();
                            view! {
                                <div class="border-b border-white/5 px-4 py-3 last:border-0">
                                    <Show when=move || editing.get() == Some(tid)
                                        fallback=move || {
                                            let a = artist.clone(); let trk = track.clone();
                                            let etr2 = etr.clone();
                                            view! {
                                                <div class="flex items-center justify-between">
                                                    <div class="text-sm text-white">
                                                        <span class="font-medium">{a}</span>
                                                        <span class="text-white/50">" : "</span>
                                                        <span>{trk}</span>
                                                    </div>
                                                    <div class="flex gap-1">
                                                        <button type="button" class="rounded px-2 py-1 text-xs text-white/60 hover:bg-white/10"
                                                            on:click=move |_| {
                                                                form.set(json!({
                                                                    "artist_name": etr2["artist_name"].as_str().unwrap_or(""),
                                                                    "track_name": etr2["track_name"].as_str().unwrap_or(""),
                                                                }));
                                                                editing.set(Some(tid));
                                                            }>{t("Edit")}</button>
                                                        <button type="button" class="rounded px-2 py-1 text-xs text-red-400 hover:bg-red-500/10"
                                                            on:click=move |_| leptos::task::spawn_local(async move {
                                                                let _ = api::delete::<Value>(&format!("/api/v1/moderation/tracks/{tid}")).await; load();
                                                            })>{t("Delete")}</button>
                                                    </div>
                                                </div>
                                            }.into_any()
                                        }>
                                        <form class="flex flex-wrap items-center gap-2" on:submit=move |ev| {
                                            ev.prevent_default();
                                            let f = form.get();
                                            leptos::task::spawn_local(async move {
                                                match api::put::<Value,_>(&format!("/api/v1/moderation/tracks/{tid}"), &f).await {
                                                    Ok(_) => { editing.set(None); load(); }
                                                    Err(e) => error.set(Some(e.message)),
                                                }
                                            });
                                        }>
                                            <input type="text" required maxlength="255"
                                                class="rounded border border-white/10 bg-surface-base px-2 py-1 text-sm text-white"
                                                prop:value=move || form.get()["artist_name"].as_str().unwrap_or("").to_string()
                                                on:input=move |ev| form.update(|f| f["artist_name"] = json!(event_target_value(&ev)))/>
                                            <input type="text" required maxlength="255"
                                                class="rounded border border-white/10 bg-surface-base px-2 py-1 text-sm text-white"
                                                prop:value=move || form.get()["track_name"].as_str().unwrap_or("").to_string()
                                                on:input=move |ev| form.update(|f| f["track_name"] = json!(event_target_value(&ev)))/>
                                            <button type="submit" class="rounded px-2 py-1 text-xs text-emerald-300">{t("Save")}</button>
                                            <button type="button" class="rounded px-2 py-1 text-xs text-white/60"
                                                on:click=move |_| editing.set(None)>{t("Cancel")}</button>
                                        </form>
                                    </Show>
                                </div>
                            }
                        }).collect::<Vec<_>>()}
                </div>
                {pager(page, last_page, load)}
            </Show>
        </ModShell>
    }
}

// ---- data quality (shared body, admin-gated destructive ops) -------------------

#[component]
pub fn ModDataQuality() -> impl IntoView {
    view! {
        <ModShell active="/moderation/data-quality">
            <crate::pages::admin::DataQualityBody/>
        </ModShell>
    }
}
