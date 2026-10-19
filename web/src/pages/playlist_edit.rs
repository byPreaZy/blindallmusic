use crate::api;
use crate::i18n::t;
use crate::types::{IdName, PlaylistDetail, PlaylistTrack, ProviderResult};
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::{use_navigate, use_params_map};
use serde_json::{json, Value};
use wasm_bindgen::JsCast;

#[component]
pub fn PlaylistEditPage() -> impl IntoView {
    crate::auth::require_registered(crate::auth::use_auth());
    let params = use_params_map();
    let playlist_id = move || {
        params
            .get()
            .get("id")
            .and_then(|v| v.parse::<i64>().ok())
            .unwrap_or(0)
    };
    let id = playlist_id();

    let detail = RwSignal::new(Option::<PlaylistDetail>::None);
    let tracks = RwSignal::new(Vec::<PlaylistTrack>::new());
    let total = RwSignal::new(0i64);
    let page = RwSignal::new(1i64);
    let per_page = 20i64;
    let answer_types = RwSignal::new(Vec::<IdName>::new());
    let loading = RwSignal::new(true);
    let flash = RwSignal::new(Option::<String>::None);
    let err = RwSignal::new(Option::<String>::None);

    let flash_for = move |msg: &'static str| {
        flash.set(Some(msg.to_string()));
        leptos::task::spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(2500).await;
            flash.set(None);
        });
    };

    let load_detail = move |id: i64| {
        leptos::task::spawn_local(async move {
            match api::get::<PlaylistDetail>(&format!("/api/v1/playlists/{id}")).await {
                Ok(d) => {
                    crate::i18n::set_title(Some(&d.playlist.name));
                    detail.set(Some(d));
                }
                Err(e) => err.set(Some(match e.status {
                    403 => "403 : This action is unauthorized.".into(),
                    404 => "404 : Page not found".into(),
                    _ => e.message,
                })),
            }
            loading.set(false);
        });
    };

    let load_tracks = move |id: i64, page: i64| {
        leptos::task::spawn_local(async move {
            match api::get::<Value>(&format!(
                "/api/v1/playlists/{id}/tracks?page={page}&per_page={per_page}"
            ))
            .await
            {
                Ok(v) => {
                    total.set(v["total"].as_i64().unwrap_or(0));
                    let rows: Vec<PlaylistTrack> = v["data"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|t| serde_json::from_value(t.clone()).ok())
                                .collect()
                        })
                        .unwrap_or_default();
                    tracks.set(rows);
                }
                Err(e) => err.set(Some(e.message)),
            }
        });
    };

    load_detail(id);
    load_tracks(id, 1);
    leptos::task::spawn_local(async move {
        if let Ok(v) = api::get::<Value>("/api/v1/answer-types").await {
            let types: Vec<IdName> = v["answer_types"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|t| serde_json::from_value(t.clone()).ok())
                        .collect()
                })
                .unwrap_or_default();
            answer_types.set(types);
        }
    });

    view! {
        <div class="mx-auto max-w-6xl px-4 py-8">
            <Show when=move || !loading.get() fallback=move || view! { <div class="text-white/60">{t("Loading")}"…"</div> }>
                {move || match detail.get() {
                    None => view! {
                        <div class="rounded-xl border border-white/10 bg-surface-raised/60 p-10 text-center">
                            <h1 class="mb-2 text-2xl font-bold text-white">404</h1>
                            <p class="text-white/60">{err.get().unwrap_or_else(|| t("Page not found").to_string())}</p>
                        </div>
                    }.into_any(),
                    Some(d) => {
                    let owned = d.owned;
                    let pname = d.playlist.name.clone();
                    let total_tracks = d.playlist.total_tracks;
                    let room_links: Vec<(String, String)> = d.playlist.rooms.iter()
                        .map(|r| (
                            format!("/rooms/{}", if r.slug.is_empty() { r.id.to_string() } else { r.slug.clone() }),
                            r.name.clone(),
                        ))
                        .collect();
                    view! {
                        <div class="mb-6 flex flex-wrap items-center gap-4">
                            <h1 class="retro-title retro-title--white text-2xl">{pname}</h1>
                            <span class="text-sm text-white/50">{format!("{} {}", total_tracks, t("Tracks"))}</span>
                            <div class="ml-auto text-sm text-white/40">
                                {t("Used by")} ": "
                                {room_links.into_iter().map(|(url, name)| view! {
                                    <A href=url attr:class="text-brand-accent-light hover:underline">{name}</A>
                                    " "
                                }).collect_view()}
                            </div>
                        </div>

                        {move || flash.get().map(|m| view! {
                            <div class="mb-4 rounded-lg border border-green-500/30 bg-green-500/10 px-4 py-2 text-sm text-green-300">{m}</div>
                        })}
                        {move || err.get().map(|e| view! {
                            <div class="mb-4 rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-2 text-sm text-red-300">{e}</div>
                        })}

                        {owned.then(|| view! {
                            <MetaForm detail=d.clone() on_saved=flash_for on_err=move |m| err.set(Some(m))/>
                            <crate::components::moderators::ModeratorsSection
                                moderators=d.playlist.moderators.clone()
                                attach_url=format!("/api/v1/playlists/{id}/moderators")
                                detach_url=format!("/api/v1/playlists/{id}/moderators")
                                can_manage=true
                                on_changed=move |_| load_detail(id)
                                on_err=move |m| err.set(Some(m))/>
                            <ExportButton playlist_id=id name=d.playlist.name.clone()/>
                        })}

                        <ProviderImport playlist_id=id
                            on_done=move |_| { load_tracks(id, page.get()); load_detail(id); }
                            on_err=move |m| err.set(Some(m))/>

                        <ProviderSearch playlist_id=id answer_types=answer_types
                            on_added=move |_| { load_tracks(id, page.get()); load_detail(id); }
                            on_err=move |m| err.set(Some(m))/>

                        <TrackList
                            tracks=tracks total=total page=page per_page=per_page
                            playlist_id=id answer_types=answer_types
                            on_page=move |p| { page.set(p); load_tracks(id, p); }
                            on_changed=move |_| { load_tracks(id, page.get()); }
                            on_err=move |m| err.set(Some(m))/>
                    }.into_any()
                }}}
            </Show>
        </div>
    }
}

#[component]
fn MetaForm(
    detail: PlaylistDetail,
    on_saved: impl Fn(&'static str) + 'static + Copy + Send + Sync,
    on_err: impl Fn(String) + 'static + Copy + Send + Sync,
) -> impl IntoView {
    let navigate = use_navigate();
    let name = RwSignal::new(detail.playlist.name.clone());
    let description = RwSignal::new(detail.playlist.description.clone().unwrap_or_default());
    let busy = RwSignal::new(false);
    let pid = detail.playlist.id;
    let user_id = detail.playlist.user_id;

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        leptos::task::spawn_local(async move {
            let body = json!({
                "name": name.get_untracked(),
                "description": description.get_untracked(),
                "user_id": user_id,
            });
            let req = gloo_net::http::Request::put(&format!("/api/v1/playlists/{pid}"))
                .header("Content-Type", "application/json");
            let req = match api::stored_token() {
                Some(tok) => req.header("Authorization", &format!("Bearer {tok}")),
                None => req,
            };
            let ok = req.body(body.to_string()).map(|r| async move {
                match r.send().await {
                    Ok(resp) if resp.ok() => on_saved("Playlist updated"),
                    Ok(_) => on_err("Update failed".into()),
                    Err(e) => on_err(e.to_string()),
                }
            });
            if let Ok(f) = ok {
                f.await;
            }
            busy.set(false);
        });
    };

    let delete_playlist = move |_| {
        if !web_sys::window()
            .and_then(|w| {
                w.confirm_with_message("Supprimer définitivement cette playlist ?")
                    .ok()
            })
            .unwrap_or(false)
        {
            return;
        }
        let navigate = navigate.clone();
        leptos::task::spawn_local(async move {
            match api::delete::<Value>(&format!("/api/v1/playlists/{pid}")).await {
                Ok(_) => navigate("/playlists", Default::default()),
                Err(e) => on_err(e.message),
            }
        });
    };

    view! {
        <form on:submit=submit class="mb-6 space-y-4 rounded-xl border border-white/10 bg-surface-raised/60 p-5">
            <div class="grid gap-4 sm:grid-cols-2">
                <div>
                    <label class="mb-1 block text-sm text-white/70">{t("Name")}</label>
                    <input type="text" maxlength="50" required
                        class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none focus:border-brand-accent/50"
                        prop:value=name on:input=move |ev| name.set(event_target_value(&ev))/>
                </div>
                <div>
                    <label class="mb-1 block text-sm text-white/70">{t("Description")}</label>
                    <input type="text"
                        class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none focus:border-brand-accent/50"
                        prop:value=description on:input=move |ev| description.set(event_target_value(&ev))/>
                </div>
            </div>
            <div class="flex items-center justify-between">
                <button type="button" class="text-sm text-red-400 hover:underline" on:click=move |ev| delete_playlist(ev)>
                    {t("Delete playlist")}
                </button>
                <button type="submit" class="btn-primary !px-4 !py-2" disabled=move || busy.get()>
                    {t("Save")}
                </button>
            </div>
        </form>
    }
}

#[component]
fn TrackList(
    tracks: RwSignal<Vec<PlaylistTrack>>,
    total: RwSignal<i64>,
    page: RwSignal<i64>,
    per_page: i64,
    playlist_id: i64,
    answer_types: RwSignal<Vec<IdName>>,
    on_page: impl Fn(i64) + 'static + Copy + Send + Sync,
    on_changed: impl Fn(()) + 'static + Copy + Send + Sync,
    on_err: impl Fn(String) + 'static + Copy + Send + Sync,
) -> impl IntoView {
    let page_count = move || (total.get() + per_page - 1).max(1) / per_page;
    let expanded = RwSignal::new(Option::<i64>::None);

    let save_meta = move |track_id: i64, dificulty: Option<i64>, hint: Option<String>| {
        leptos::task::spawn_local(async move {
            let body = json!({"dificulty": dificulty, "hint": hint});
            let req = gloo_net::http::Request::put(&format!(
                "/api/v1/playlists/{playlist_id}/tracks/{track_id}"
            ))
            .header("Content-Type", "application/json");
            let req = match api::stored_token() {
                Some(tok) => req.header("Authorization", &format!("Bearer {tok}")),
                None => req,
            };
            match req.body(body.to_string()).map(|r| r.send()) {
                Ok(f) => match f.await {
                    Ok(resp) if resp.ok() => on_changed(()),
                    Ok(_) => on_err("Update failed".into()),
                    Err(e) => on_err(e.to_string()),
                },
                Err(_) => on_err("Request error".into()),
            }
        });
    };

    let delete_track = move |track_id: i64| {
        leptos::task::spawn_local(async move {
            match api::delete::<Value>(&format!(
                "/api/v1/playlists/{playlist_id}/tracks/{track_id}"
            ))
            .await
            {
                Ok(_) => on_changed(()),
                Err(e) => on_err(e.message),
            }
        });
    };

    let clear_tracks = move |_| {
        let confirmed = web_sys::window()
            .map(|w| w.confirm_with_message(&t("Remove all tracks from this playlist")))
            .unwrap_or(Ok(false))
            .unwrap_or(false);
        if !confirmed {
            return;
        }
        leptos::task::spawn_local(async move {
            match api::delete::<Value>(&format!("/api/v1/playlists/{playlist_id}/tracks/clear"))
                .await
            {
                Ok(_) => on_changed(()),
                Err(e) => on_err(e.message),
            }
        });
    };

    view! {
        <div class="rounded-xl border border-white/10 bg-surface-raised/60">
            <div class="flex items-center justify-between border-b border-white/10 px-5 py-3">
                <h2 class="font-semibold text-white">{t("Tracks")}</h2>
                <Show when=move || !tracks.get().is_empty()>
                    <button type="button"
                        class="rounded-lg border border-red-500/30 bg-red-500/10 px-3 py-1.5 text-sm font-medium text-red-300 transition hover:bg-red-500/20"
                        title=t("Remove all tracks from this playlist")
                        on:click=clear_tracks>
                        {t("Remove all tracks")}
                    </button>
                </Show>
            </div>
            <div class="divide-y divide-white/5">
                <For
                    each=move || tracks.get()
                    key=|t| t.id
                    children=move |track: PlaylistTrack| {
                        let tid = track.id;
                        let is_open = move || expanded.get() == Some(tid);
                        let artist = track.answers.iter()
                            .find(|a| a.answer_type_id == "1")
                            .map(|a| a.value.clone())
                            .unwrap_or_default();
                        let title = track.answers.iter()
                            .find(|a| a.answer_type_id == "2")
                            .map(|a| a.value.clone())
                            .unwrap_or_default();
                        view! {
                            <div class="px-5 py-3">
                                <div class="flex items-center gap-4">
                                    <img src=track.artwork_url.clone() class="h-12 w-12 rounded-lg object-cover" alt=""/>
                                    <div class="min-w-0 flex-1">
                                        <div class="truncate font-medium text-white">{artist}</div>
                                        <div class="truncate text-sm text-white/50">{title}</div>
                                    </div>
                                    <span class="hidden rounded bg-white/10 px-2 py-0.5 text-xs text-white/60 sm:block">{track.provider.clone()}</span>
                                    <div class="flex items-center gap-1 text-xs text-white/50">
                                        <span class="text-green-400">{"▲"}{track.up_votes}</span>
                                        <span class="text-red-400">{"▼"}{track.down_votes}</span>
                                    </div>
                                    <button type="button"
                                        class="rounded-lg border border-white/10 px-2.5 py-1 text-xs text-white/70 hover:bg-white/5"
                                        on:click=move |_| expanded.set(if is_open() { None } else { Some(tid) })>
                                        {t("Edit")}
                                    </button>
                                    <button type="button" class="rounded-lg px-2 py-1 text-xs text-red-400 hover:bg-red-500/10"
                                        on:click=move |_| delete_track(tid)>
                                        "✕"
                                    </button>
                                </div>
                                <Show when=is_open>
                                    <TrackEditor track=track.clone()
                                        answer_types=answer_types
                                        on_save_meta=save_meta on_changed=on_changed on_err=on_err/>
                                </Show>
                            </div>
                        }
                    }
                />
                <Show when=move || tracks.get().is_empty()>
                    <div class="px-5 py-8 text-center text-sm text-white/40">
                        {t("No tracks : use the search or an import above")}
                    </div>
                </Show>
            </div>
            <Show when=move || { page_count() > 1 }>
                <div class="flex items-center justify-center gap-3 border-t border-white/10 px-5 py-3 text-sm">
                    <button type="button" class="text-brand-accent-light disabled:text-white/30"
                        disabled=move || page.get() <= 1
                        on:click=move |_| on_page(page.get() - 1)>"←"</button>
                    <span class="text-white/60">{move || format!("{} / {}", page.get(), page_count())}</span>
                    <button type="button" class="text-brand-accent-light disabled:text-white/30"
                        disabled=move || { page.get() >= page_count() }
                        on:click=move |_| on_page(page.get() + 1)>"→"</button>
                </div>
            </Show>
        </div>
    }
}

#[component]
fn TrackEditor(
    track: PlaylistTrack,
    answer_types: RwSignal<Vec<IdName>>,
    on_save_meta: impl Fn(i64, Option<i64>, Option<String>) + 'static + Copy + Send + Sync,
    on_changed: impl Fn(()) + 'static + Copy + Send + Sync,
    on_err: impl Fn(String) + 'static + Copy + Send + Sync,
) -> impl IntoView {
    let tid = track.id;
    let dificulty = RwSignal::new(track.dificulty);
    let hint = RwSignal::new(track.hint.clone().unwrap_or_default());
    let answers = RwSignal::new(track.answers.clone());
    let new_type = RwSignal::new(1i64);
    let new_value = RwSignal::new(String::new());
    let new_score = RwSignal::new(0.5f64);

    let save_answer = move |answer_id: i64, type_id: i64, value: String, score: f64| {
        leptos::task::spawn_local(async move {
            let body = json!({"answer_type_id": type_id, "value": value, "score": score});
            let req =
                gloo_net::http::Request::put(&format!("/api/v1/tracks/{tid}/answers/{answer_id}"))
                    .header("Content-Type", "application/json");
            let req = match api::stored_token() {
                Some(tok) => req.header("Authorization", &format!("Bearer {tok}")),
                None => req,
            };
            match req.body(body.to_string()).map(|r| r.send()) {
                Ok(f) => match f.await {
                    Ok(resp) if resp.ok() => on_changed(()),
                    Ok(_) => on_err("Save failed".into()),
                    Err(e) => on_err(e.to_string()),
                },
                Err(_) => on_err("Request error".into()),
            }
        });
    };

    let add_answer = move |_| {
        leptos::task::spawn_local(async move {
            let body = json!({
                "answer_type_id": new_type.get_untracked(),
                "value": new_value.get_untracked(),
                "score": new_score.get_untracked(),
            });
            match api::post::<Value, _>(&format!("/api/v1/tracks/{tid}/answers"), &body).await {
                Ok(_) => {
                    new_value.set(String::new());
                    on_changed(());
                }
                Err(e) => on_err(e.message),
            }
        });
    };

    let delete_answer = move |answer_id: i64| {
        leptos::task::spawn_local(async move {
            match api::delete::<Value>(&format!("/api/v1/tracks/{tid}/answers/{answer_id}")).await {
                Ok(_) => {
                    answers.update(|a| a.retain(|x| x.id != answer_id));
                    on_changed(());
                }
                Err(e) => on_err(e.message),
            }
        });
    };

    view! {
        <div class="mt-3 space-y-3 rounded-lg border border-white/10 bg-surface-base/60 p-4">
            <div class="flex flex-wrap items-center gap-3 text-sm">
                <label class="text-white/60">{t("Difficulty")}</label>
                <select class="rounded border border-white/10 bg-surface-base px-2 py-1 text-white"
                    on:change=move |ev| {
                        let v = event_target_value(&ev);
                        let d = v.parse::<i64>().ok();
                        dificulty.set(d);
                        on_save_meta(tid, d, Some(hint.get_untracked()));
                    }>
                    <option value="" selected=move || dificulty.get().is_none()>"—"</option>
                    {(0..=5).map(|d| {
                        view! { <option value=d selected=move || dificulty.get() == Some(d)>{d}</option> }
                    }).collect_view()}
                </select>
                <label class="text-white/60">{t("Hint")}</label>
                <input type="text" maxlength="255" placeholder="…"
                    class="min-w-0 flex-1 rounded border border-white/10 bg-surface-base px-2 py-1 text-white"
                    prop:value=hint
                    on:input=move |ev| hint.set(event_target_value(&ev))
                    on:blur=move |_| on_save_meta(tid, dificulty.get(), Some(hint.get_untracked()))/>
            </div>

            <div class="space-y-2">
                <For each=move || answers.get() key=|a| a.id children=move |a| {
                    let aid = a.id;
                    let type_id = a.answer_type_id.parse::<i64>().unwrap_or(0);
                    let value = RwSignal::new(a.value.clone());
                    let score = RwSignal::new(a.score);
                    view! {
                        <div class="flex items-center gap-2 text-sm">
                            <span class="w-24 shrink-0 truncate rounded bg-brand-accent/10 px-2 py-1 text-xs text-brand-accent-light">
                                {answer_types.get().iter().find(|t| t.id == type_id).map(|t| t.name.clone()).unwrap_or_else(|| format!("#{type_id}"))}
                            </span>
                            <input type="text" class="min-w-0 flex-1 rounded border border-white/10 bg-surface-base px-2 py-1 text-white"
                                prop:value=value on:input=move |ev| value.set(event_target_value(&ev))/>
                            <input type="number" step="0.1" min="0" max="99"
                                class="w-16 rounded border border-white/10 bg-surface-base px-2 py-1 text-white"
                                prop:value=score
                                on:input=move |ev| score.set(event_target_value(&ev).parse().unwrap_or(0.5))/>
                            <button type="button" class="text-brand-accent-light hover:underline"
                                on:click=move |_| save_answer(aid, type_id, value.get(), score.get())>
                                {t("Save")}
                            </button>
                            <button type="button" class="text-red-400 hover:underline"
                                on:click=move |_| delete_answer(aid)>"✕"</button>
                        </div>
                    }
                }/>
            </div>

            <div class="flex items-center gap-2 border-t border-white/10 pt-3 text-sm">
                <select class="rounded border border-white/10 bg-surface-base px-2 py-1 text-white"
                    on:change=move |ev| new_type.set(event_target_value(&ev).parse().unwrap_or(1))>
                    <For each=move || answer_types.get() key=|t| t.id children=|ty: IdName| {
                        view! { <option value=ty.id selected=ty.id == 1>{ty.name.clone()}</option> }
                    }/>
                </select>
                <input type="text" placeholder={t("New answer")} class="min-w-0 flex-1 rounded border border-white/10 bg-surface-base px-2 py-1 text-white"
                    prop:value=new_value on:input=move |ev| new_value.set(event_target_value(&ev))/>
                <input type="number" step="0.1" min="0" max="99" class="w-16 rounded border border-white/10 bg-surface-base px-2 py-1 text-white"
                    prop:value=new_score
                    on:input=move |ev| new_score.set(event_target_value(&ev).parse().unwrap_or(0.5))/>
                <button type="button" class="btn-primary !px-3 !py-1" on:click=move |ev| add_answer(ev)>+</button>
            </div>
        </div>
    }
}

#[component]
fn ProviderSearch(
    playlist_id: i64,
    answer_types: RwSignal<Vec<IdName>>,
    on_added: impl Fn(()) + 'static + Copy + Send + Sync,
    on_err: impl Fn(String) + 'static + Copy + Send + Sync,
) -> impl IntoView {
    let term = RwSignal::new(String::new());
    let providers = RwSignal::new(vec!["deezer".to_string()]);
    let results = RwSignal::new(Vec::<ProviderResult>::new());
    let errors = RwSignal::new(Vec::<String>::new());
    let busy = RwSignal::new(false);
    let _ = answer_types;

    // PHP offers a 'local' provider (uploaded Blind'All MP3s).
    let all_providers = ["deezer", "spotify", "itunes", "audius", "local"];
    let auth = crate::auth::use_auth();
    let can_upload = Memo::new(move |_| {
        auth.user
            .get()
            .map(|u| u.is_public_moderator)
            .unwrap_or(false)
    });

    let search = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        errors.set(vec![]);
        leptos::task::spawn_local(async move {
            let ps = providers.get_untracked().join(",");
            let url = format!(
                "/api/v1/playlists/{playlist_id}/tracks/search?term={}&providers={ps}",
                js_sys::encode_uri_component(&term.get_untracked())
            );
            match api::get::<Value>(&url).await {
                Ok(v) => {
                    let parsed: Vec<ProviderResult> = v["tracks"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|t| serde_json::from_value(t.clone()).ok())
                                .collect()
                        })
                        .unwrap_or_default();
                    results.set(parsed);
                    let errs: Vec<String> = v["errors"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|e| e["message"].as_str().map(str::to_string))
                                .collect()
                        })
                        .unwrap_or_default();
                    errors.set(errs);
                }
                Err(e) => on_err(e.message),
            }
            busy.set(false);
        });
    };

    let add_track = move |r: ProviderResult| {
        let r = r.clone();
        leptos::task::spawn_local(async move {
            let body = json!({
                "provider": r.provider,
                "provider_id": r.provider_id,
                "provider_url": r.provider_url.unwrap_or_default(),
                "artist_name": r.artist_name.unwrap_or_default(),
                "track_name": r.track_name,
                "preview_url": r.preview_url.unwrap_or_default(),
                "artwork_url": r.artwork_url.unwrap_or_default(),
            });
            match api::post::<Value, _>(&format!("/api/v1/playlists/{playlist_id}/tracks"), &body)
                .await
            {
                Ok(_) => on_added(()),
                Err(e) => on_err(e.message),
            }
        });
    };

    view! {
        <div class="mb-6 rounded-xl border border-white/10 bg-surface-raised/60 p-5">
            <h2 class="mb-3 font-semibold text-white">{t("Search tracks")}</h2>
            <form on:submit=search class="flex flex-wrap items-center gap-3">
                <input type="text" placeholder={t("Artist, title…")} required
                    class="min-w-0 flex-1 rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none focus:border-brand-accent/50"
                    prop:value=term on:input=move |ev| term.set(event_target_value(&ev))/>
                <div class="flex gap-2 text-sm text-white/70">
                    {all_providers.map(|p| view! {
                        <label class="flex items-center gap-1.5">
                            <input type="checkbox"
                                prop:checked=move || providers.get().iter().any(|x| x == p)
                                on:change=move |ev| {
                                    let checked = event_target_checked(&ev);
                                    providers.update(|v| {
                                        if checked && !v.iter().any(|x| x == p) {
                                            v.push(p.to_string());
                                        } else if !checked {
                                            v.retain(|x| x != p);
                                        }
                                    });
                                }/>
                            {if p == "local" { "Blind'All" } else { p }}
                        </label>
                    })}
                </div>
                <button type="submit" class="btn-primary !px-4 !py-2" disabled=move || busy.get()>
                    {move || if busy.get() { "…" } else { t("Search") }}
                </button>
                <Show when=move || can_upload.get()>
                    <div class="w-full sm:w-56">
                        <crate::components::upload_track::UploadTrack playlist_id=playlist_id
                            on_uploaded=move |_| on_added(())/>
                    </div>
                </Show>
            </form>

            <For each=move || errors.get() key=|e| e.clone() children=|e| view! {
                <div class="mt-2 text-sm text-red-400">{e}</div>
            }/>

            <Show when=move || !results.get().is_empty()>
                <div class="mt-4 max-h-80 space-y-2 overflow-y-auto">
                    <For each=move || results.get() key=|r| format!("{}:{}", r.provider, r.provider_id)
                        children=move |r: ProviderResult| {
                            let r2 = r.clone();
                            let added = r.added.is_some();
                            view! {
                                <div class="flex items-center gap-3 rounded-lg border border-white/5 bg-surface-base/50 p-2">
                                    <img src=r.artwork_url.clone().unwrap_or_default() class="h-10 w-10 rounded object-cover" alt=""/>
                                    <div class="min-w-0 flex-1">
                                        <div class="truncate text-sm font-medium text-white">
                                            {r.artist_name.clone().unwrap_or_default()} " : " {r.track_name.clone()}
                                        </div>
                                        <div class="text-xs text-white/40">{r.provider.clone()}</div>
                                    </div>
                                    {r.preview_url.clone().and_then(|u| if u.starts_with("http") { Some(u) } else { None }).map(|u| view! {
                                        <audio src=u controls class="h-8 w-40" />
                                    })}
                                    <button type="button"
                                        class=move || format!(
                                            "rounded-lg px-3 py-1.5 text-xs font-medium {}",
                                            if added { "bg-white/10 text-white/50" } else { "btn-primary" }
                                        )
                                        disabled=added
                                        on:click=move |_| add_track(r2.clone())>
                                        {if added { t("Added") } else { "+ Add" }}
                                    </button>
                                </div>
                            }
                        }
                    />
                </div>
            </Show>
        </div>
    }
}

#[component]
fn ProviderImport(
    playlist_id: i64,
    on_done: impl Fn(()) + 'static + Copy + Send + Sync,
    on_err: impl Fn(String) + 'static + Copy + Send + Sync,
) -> impl IntoView {
    let provider = RwSignal::new("deezer".to_string());
    let remote_id = RwSignal::new(String::new());
    let found = RwSignal::new(Option::<Value>::None);
    let busy = RwSignal::new(false);
    let imported = RwSignal::new(Option::<i64>::None);

    let find = move |_| {
        busy.set(true);
        found.set(None);
        imported.set(None);
        leptos::task::spawn_local(async move {
            let body = json!({
                "provider": provider.get_untracked(),
                "playlist_id": remote_id.get_untracked(),
            });
            match api::post::<Value, _>(
                &format!("/api/v1/playlists/{playlist_id}/providers/find"),
                &body,
            )
            .await
            {
                Ok(v) => found.set(Some(v["playlist"].clone())),
                Err(e) => on_err(e.message),
            }
            busy.set(false);
        });
    };

    let import = move |_| {
        busy.set(true);
        leptos::task::spawn_local(async move {
            let body = json!({
                "provider": provider.get_untracked(),
                "playlist_id": remote_id.get_untracked(),
            });
            match api::post::<Value, _>(
                &format!("/api/v1/playlists/{playlist_id}/providers/import"),
                &body,
            )
            .await
            {
                Ok(v) => {
                    imported.set(v["imported"].as_i64());
                    found.set(None);
                    on_done(());
                }
                Err(e) => on_err(e.message),
            }
            busy.set(false);
        });
    };

    view! {
        <div class="mb-6 rounded-xl border border-white/10 bg-surface-raised/60 p-5">
            <h2 class="mb-3 font-semibold text-white">{t("Import a playlist")}</h2>
            <div class="flex flex-wrap items-center gap-3">
                <select class="rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white"
                    on:change=move |ev| provider.set(event_target_value(&ev))>
                    <option value="deezer" selected>"Deezer"</option>
                    <option value="spotify">"Spotify"</option>
                    <option value="blindall">{t("My likes")}</option>
                </select>
                <Show when=move || provider.get() != "blindall">
                    <input type="text" placeholder={t("Playlist ID (e.g. 908622995)")}
                        class="min-w-0 flex-1 rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none focus:border-brand-accent/50"
                        prop:value=remote_id on:input=move |ev| remote_id.set(event_target_value(&ev))/>
                </Show>
                <button type="button" class="btn-primary !px-4 !py-2" disabled=move || busy.get() on:click=move |ev| find(ev)>
                    {t("Find")}
                </button>
            </div>

            {move || found.get().map(|p| {
                let img = p["image"].as_str().unwrap_or_default().to_string();
                let pname = p["name"].as_str().unwrap_or_default().to_string();
                let n = p["tracks_count"].as_i64().unwrap_or(0);
                view! {
                    <div class="mt-4 flex items-center gap-4 rounded-lg border border-white/10 bg-surface-base/50 p-4">
                        <img src=img class="h-14 w-14 rounded-lg object-cover" alt=""/>
                        <div class="min-w-0 flex-1">
                            <div class="font-medium text-white">{pname}</div>
                            <div class="text-sm text-white/50">{format!("{} {}", n, t("Tracks"))}</div>
                        </div>
                        <button type="button" class="btn-primary !px-4 !py-2" disabled=move || busy.get() on:click=move |ev| import(ev)>
                            {t("Import")}
                        </button>
                    </div>
                }
            })}

            {move || imported.get().map(|n| view! {
                <div class="mt-4 rounded-lg border border-green-500/30 bg-green-500/10 px-4 py-2 text-sm text-green-300">
                    {format!("{} {}", n, t("tracks imported"))}
                </div>
            })}
        </div>
    }
}

#[component]
fn ExportButton(playlist_id: i64, name: String) -> impl IntoView {
    let busy = RwSignal::new(false);
    let name = StoredValue::new(name);
    view! {
        <button type="button" class="btn-secondary mb-6" disabled=move || busy.get()
            on:click=move |_| {
                busy.set(true);
                let slug = name.get_value();
                leptos::task::spawn_local(async move {
                    let req = gloo_net::http::Request::get(
                        &format!("/api/v1/playlists/{playlist_id}/export"));
                    let req = if let Some(t) = api::stored_token() {
                        req.header("Authorization", &format!("Bearer {t}"))
                    } else { req };
                    if let Ok(resp) = req.send().await {
                        if let Ok(text) = resp.text().await {
                            let opts = web_sys::BlobPropertyBag::new();
                            opts.set_type("text/csv");
                            let parts = js_sys::Array::new();
                            parts.push(&wasm_bindgen::JsValue::from_str(&text));
                            if let Ok(blob) = web_sys::Blob::new_with_str_sequence_and_options(&parts, &opts) {
                                if let Ok(url) = web_sys::Url::create_object_url_with_blob(&blob) {
                                    let doc = web_sys::window().unwrap().document().unwrap();
                                    let a: web_sys::HtmlAnchorElement = doc
                                        .create_element("a").unwrap().unchecked_into();
                                    a.set_href(&url);
                                    a.set_download(&format!("{slug}.csv"));
                                    a.click();
                                    let _ = web_sys::Url::revoke_object_url(&url);
                                }
                            }
                        }
                    }
                    busy.set(false);
                });
            }>
            {t("Export (CSV)")}
        </button>
    }
}
