use crate::api;
use crate::i18n::t;
use crate::types::{Category, IdName};
use leptos::prelude::*;
use leptos_router::hooks::{use_navigate, use_params_map};
use serde_json::{json, Value};

async fn put(path: &str, body: Value) -> Result<(), String> {
    let req = gloo_net::http::Request::put(path).header("Content-Type", "application/json");
    let req = match api::stored_token() {
        Some(tok) => req.header("Authorization", &format!("Bearer {tok}")),
        None => req,
    };
    let fut = req.body(body.to_string()).map_err(|e| e.to_string())?.send();
    match fut.await {
        Ok(resp) if resp.ok() => Ok(()),
        Ok(resp) => {
            let v: Value = resp.json().await.unwrap_or_default();
            Err(v["message"].as_str().or_else(|| v["error"].as_str()).unwrap_or("Update failed").to_string())
        }
        Err(e) => Err(e.to_string()),
    }
}

async fn fetch_categories() -> Vec<Category> {
    match api::get::<Value>("/api/v1/categories").await {
        Ok(v) => v["categories"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|c| serde_json::from_value(c.clone()).ok())
                    .collect()
            })
            .unwrap_or_default(),
        Err(_) => vec![],
    }
}

#[component]
pub fn RoomCreatePage() -> impl IntoView {
    crate::i18n::set_title_key("Create Room");
    crate::auth::require_registered(crate::auth::use_auth());
    let navigate = use_navigate();
    let name = RwSignal::new(String::new());
    let categories = RwSignal::new(Vec::<Category>::new());
    let category = RwSignal::new(0i64);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);

    leptos::task::spawn_local(async move {
        let cats = fetch_categories().await;
        if let Some(first) = cats.first() {
            category.set(first.id);
        }
        categories.set(cats);
    });

    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        error.set(None);
        let navigate = navigate.clone();
        leptos::task::spawn_local(async move {
            let body = json!({"name": name.get_untracked(), "category_id": category.get_untracked()});
            match api::post::<Value, _>("/api/v1/rooms", &body).await {
                Ok(v) => {
                    let slug = v["room"]["slug"].as_str().unwrap_or_default().to_string();
                    navigate(&format!("/rooms/{slug}/edit"), Default::default());
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
            <h1 class="retro-title retro-title--white mb-8 text-2xl">{t("Create a room")}</h1>
            <form on:submit=submit class="space-y-5 rounded-xl border border-white/10 bg-surface-raised/60 p-6">
                {move || error.get().map(|e| view! {
                    <div class="rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">{e}</div>
                })}
                <div>
                    <label class="mb-1.5 block text-sm text-white/70">{t("Name")}</label>
                    <input type="text" maxlength="25" required
                        class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none focus:border-brand-accent/50"
                        prop:value=name on:input=move |ev| name.set(event_target_value(&ev))/>
                </div>
                <div>
                    <label class="mb-1.5 block text-sm text-white/70">{t("Category")}</label>
                    <select class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white"
                        on:change=move |ev| category.set(event_target_value(&ev).parse().unwrap_or(0))>
                        <For each=move || categories.get() key=|c| c.id children=|c: Category| {
                            view! { <option value=c.id>{c.name.clone()}</option> }
                        }/>
                    </select>
                </div>
                <button type="submit" class="btn-primary w-full" disabled=move || busy.get()>
                    {t("Create")}
                </button>
            </form>
        </div>
    }
}

#[component]
pub fn RoomEditPage() -> impl IntoView {
    crate::i18n::set_title_key("Edit Room");
    crate::auth::require_registered(crate::auth::use_auth());
    let params = use_params_map();
    let key = move || params.get().get("id").unwrap_or_default();
    let navigate = StoredValue::new(use_navigate());

    let room = RwSignal::new(Option::<Value>::None);
    let owned = RwSignal::new(false);
    let categories = RwSignal::new(Vec::<Category>::new());
    let available = RwSignal::new(Vec::<Value>::new());
    let loading = RwSignal::new(true);
    let flash = RwSignal::new(Option::<String>::None);
    let err = RwSignal::new(Option::<String>::None);

    let room_id = RwSignal::new(0i64);
    let name = RwSignal::new(String::new());
    let description = RwSignal::new(String::new());
    let category_id = RwSignal::new(0i64);
    let playlists = RwSignal::new(Vec::<IdName>::new());
    let mods = RwSignal::new(Vec::<IdName>::new());
    let tracks_by_round = RwSignal::new(10i64);
    let track_duration = RwSignal::new(30i64);
    let pause_tracks = RwSignal::new(5i64);
    let pause_rounds = RwSignal::new(30i64);
    let chat_active = RwSignal::new(true);
    let autostart = RwSignal::new(true);
    let random = RwSignal::new(false);
    let active = RwSignal::new(true);
    let color = RwSignal::new(String::new());
    let has_password = RwSignal::new(false);
    let password = RwSignal::new(String::new());
    let attach_id = RwSignal::new(0i64);
    let photo_path = RwSignal::new(Option::<String>::None);

    let flash_for = move |msg: &'static str| {
        flash.set(Some(msg.to_string()));
        leptos::task::spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(2500).await;
            flash.set(None);
        });
    };

    leptos::task::spawn_local(async move {
        categories.set(fetch_categories().await);
        let k = key();
        match api::get::<Value>(&format!("/api/v1/rooms/{k}/edit")).await {
            Ok(v) => {
                owned.set(v["owned"].as_bool().unwrap_or(false));
                let r = &v["room"];
                room_id.set(r["id"].as_i64().unwrap_or(0));
                name.set(r["name"].as_str().unwrap_or_default().to_string());
                description.set(r["description"].as_str().unwrap_or_default().to_string());
                category_id.set(r["category_id"].as_i64().unwrap_or(0));
                tracks_by_round.set(r["tracks_by_round"].as_i64().unwrap_or(10));
                track_duration.set(r["track_duration"].as_i64().unwrap_or(30));
                pause_tracks.set(r["pause_between_tracks"].as_i64().unwrap_or(5));
                pause_rounds.set(r["pause_between_rounds"].as_i64().unwrap_or(30));
                chat_active.set(r["is_chat_active"].as_bool().unwrap_or(true));
                autostart.set(r["is_autostart"].as_bool().unwrap_or(true));
                random.set(r["is_random"].as_bool().unwrap_or(false));
                active.set(r["is_active"].as_bool().unwrap_or(true));
                color.set(r["color"].as_str().unwrap_or_default().to_string());
                has_password.set(r["has_password"].as_bool().unwrap_or(false));
                photo_path.set(r["photo_path"].as_str().map(str::to_string));
                mods.set(
                    r["moderators"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|m| serde_json::from_value::<IdName>(m.clone()).ok())
                                .collect()
                        })
                        .unwrap_or_default(),
                );
                playlists.set(
                    r["playlists"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|p| serde_json::from_value::<IdName>(p.clone()).ok())
                                .collect()
                        })
                        .unwrap_or_default(),
                );
                room.set(Some(r.clone()));
            }
            Err(e) => err.set(Some(match e.status {
                403 => "403 : This action is unauthorized.".into(),
                404 => "404 : Page not found".into(),
                _ => e.message,
            })),
        }
        if let Ok(v) = api::get::<Value>("/api/v1/playlists?scope=public").await {
            available.set(v["playlists"].as_array().cloned().unwrap_or_default());
        }
        loading.set(false);
    });

    let save_main = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let id = room_id.get();
        leptos::task::spawn_local(async move {
            match put(
                &format!("/api/v1/rooms/{id}"),
                json!({
                    "name": name.get_untracked(),
                    "description": description.get_untracked(),
                    "category_id": category_id.get_untracked(),
                }),
            )
            .await
            {
                Ok(()) => flash_for("Room updated"),
                Err(e) => err.set(Some(e)),
            }
        });
    };

    let save_options = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let id = room_id.get();
        leptos::task::spawn_local(async move {
            match put(
                &format!("/api/v1/rooms/{id}/options"),
                json!({
                    "password": password.get_untracked(),
                    "has_password": has_password.get_untracked(),
                    "tracks_by_round": tracks_by_round.get_untracked(),
                    "track_duration": track_duration.get_untracked(),
                    "pause_between_tracks": pause_tracks.get_untracked(),
                    "pause_between_rounds": pause_rounds.get_untracked(),
                    "is_chat_active": chat_active.get_untracked(),
                    "is_autostart": autostart.get_untracked(),
                    "is_random": random.get_untracked(),
                    "is_active": active.get_untracked(),
                    "color": color.get_untracked(),
                }),
            )
            .await
            {
                Ok(()) => flash_for("Options updated"),
                Err(e) => err.set(Some(e)),
            }
        });
    };

    let attach = move |_| {
        let id = room_id.get();
        let pid = attach_id.get();
        if pid == 0 {
            return;
        }
        leptos::task::spawn_local(async move {
            match api::post::<Value, _>(
                &format!("/api/v1/rooms/{id}/playlists/attach"),
                &json!({"playlist_id": pid}),
            )
            .await
            {
                Ok(_) => {
                    if let Some(p) = available.get().iter().find(|p| p["id"].as_i64() == Some(pid)) {
                        playlists.update(|l| {
                            if !l.iter().any(|x| x.id == pid) {
                                l.push(IdName {
                                    id: pid,
                                    name: p["name"].as_str().unwrap_or_default().to_string(),
                                });
                            }
                        });
                    }
                    flash_for("Playlist attached");
                }
                Err(e) => err.set(Some(e.message)),
            }
        });
    };

    let detach = move |pid: i64| {
        let id = room_id.get();
        leptos::task::spawn_local(async move {
            let req = gloo_net::http::Request::delete(&format!(
                "/api/v1/rooms/{id}/playlists/detach"
            ))
            .header("Content-Type", "application/json");
            let req = match api::stored_token() {
                Some(tok) => req.header("Authorization", &format!("Bearer {tok}")),
                None => req,
            };
            let ok = req.body(json!({"playlist_id": pid}).to_string()).map(|r| r.send());
            if let Ok(f) = ok {
                match f.await {
                    Ok(resp) if resp.ok() => {
                        playlists.update(|l| l.retain(|x| x.id != pid));
                        flash_for("Playlist detached");
                    }
                    _ => err.set(Some("Detach failed".into())),
                }
            }
        });
    };

    let delete_room = move |_| {
        if !web_sys::window()
            .and_then(|w| w.confirm_with_message("Supprimer définitivement ce salon ?").ok())
            .unwrap_or(false)
        {
            return;
        }
        let id = room_id.get();
        leptos::task::spawn_local(async move {
            match api::delete::<Value>(&format!("/api/v1/rooms/{id}")).await {
                Ok(_) => navigate.get_value()("/", Default::default()),
                Err(e) => err.set(Some(e.message)),
            }
        });
    };

    view! {
        <div class="mx-auto max-w-4xl px-4 py-8">
            <Show when=move || !loading.get() fallback=move || view! { <div class="text-white/60">{t("Loading")}"…"</div> }>
                <Show when=move || room.get().is_some() fallback=move || view! {
                    <div class="rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-red-300">
                        {move || err.get().unwrap_or_else(|| "Room not found".into())}
                    </div>
                }>
                    <h1 class="retro-title retro-title--white mb-6 text-2xl">
                        {move || room.get().map(|r| r["name"].as_str().unwrap_or_default().to_string()).unwrap_or_default()}
                    </h1>

                    {move || flash.get().map(|m| view! {
                        <div class="mb-4 rounded-lg border border-green-500/30 bg-green-500/10 px-4 py-2 text-sm text-green-300">{m}</div>
                    })}
                    {move || err.get().map(|e| view! {
                        <div class="mb-4 rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-2 text-sm text-red-300">{e}</div>
                    })}

                    <form on:submit=save_main class="mb-6 space-y-4 rounded-xl border border-white/10 bg-surface-raised/60 p-5">
                        <div class="grid gap-4 sm:grid-cols-2">
                            <div>
                                <label class="mb-1 block text-sm text-white/70">{t("Name")}</label>
                                <input type="text" maxlength="25" required
                                    class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none focus:border-brand-accent/50"
                                    prop:value=name on:input=move |ev| name.set(event_target_value(&ev))/>
                            </div>
                            <div>
                                <label class="mb-1 block text-sm text-white/70">{t("Category")}</label>
                                <select class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white"
                                    on:change=move |ev| category_id.set(event_target_value(&ev).parse().unwrap_or(0))>
                                    <For each=move || categories.get() key=|c| c.id children=move |c: Category| {
                                        view! { <option value=c.id selected=move || category_id.get() == c.id>{c.name.clone()}</option> }
                                    }/>
                                </select>
                            </div>
                        </div>
                        <div>
                            <label class="mb-1 block text-sm text-white/70">{t("Description")}</label>
                            <textarea rows="2"
                                class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none focus:border-brand-accent/50"
                                prop:value=description on:input=move |ev| description.set(event_target_value(&ev))></textarea>
                        </div>
                        <div class="flex justify-end">
                            <button type="submit" class="btn-primary !px-4 !py-2">{t("Save")}</button>
                        </div>
                    </form>

                    <div class="mb-6 rounded-xl border border-white/10 bg-surface-raised/60 p-5">
                        <h2 class="mb-3 font-semibold text-white">{t("Photo")}</h2>
                        <div class="flex items-center gap-4">
                            {move || match photo_path.get() {
                                Some(p) => view! {
                                    <img src=format!("/storage/{p}") class="h-16 w-16 rounded-lg object-cover" alt=""/>
                                }.into_any(),
                                None => view! {
                                    <div class="flex h-16 w-16 items-center justify-center rounded-lg border border-white/10 bg-surface-base text-white/30">"?"</div>
                                }.into_any(),
                            }}
                            <label class="btn-secondary cursor-pointer !px-4 !py-2 text-sm">
                                {t("Upload")}
                                <input type="file" accept="image/jpeg,image/png,image/webp" class="hidden"
                                    on:change=move |ev| {
                                        let id = room_id.get();
                                        let file = event_target::<web_sys::HtmlInputElement>(&ev)
                                            .files().and_then(|f| f.get(0));
                                        let Some(file) = file else { return };
                                        leptos::task::spawn_local(async move {
                                            let form = web_sys::FormData::new().unwrap();
                                            let _ = form.append_with_blob("photo", &file);
                                            let req = gloo_net::http::Request::post(
                                                &format!("/api/v1/rooms/{id}/photo"));
                                            let req = match api::stored_token() {
                                                Some(tok) => req.header("Authorization", &format!("Bearer {tok}")),
                                                None => req,
                                            };
                                            if let Ok(req) = req.body(form) {
                                                match api::finish(req.send().await).await {
                                                    Ok(resp) => {
                                                        if let Ok(v) = resp.json::<Value>().await {
                                                            photo_path.set(v["photo_path"].as_str().map(str::to_string));
                                                        }
                                                        flash_for("Photo updated");
                                                    }
                                                    Err(e) => err.set(Some(e.first("photo").unwrap_or(e.message))),
                                                }
                                            }
                                        });
                                    }/>
                            </label>
                            <Show when=move || photo_path.get().is_some()>
                                <button type="button" class="text-xs text-red-400 hover:underline"
                                    on:click=move |_| {
                                        let id = room_id.get();
                                        leptos::task::spawn_local(async move {
                                            match api::delete::<Value>(&format!("/api/v1/rooms/{id}/picture")).await {
                                                Ok(_) => { photo_path.set(None); flash_for("Photo deleted"); }
                                                Err(e) => err.set(Some(e.message)),
                                            }
                                        });
                                    }>
                                    {t("Delete")}
                                </button>
                            </Show>
                        </div>
                    </div>

                    <form on:submit=save_options class="mb-6 space-y-4 rounded-xl border border-white/10 bg-surface-raised/60 p-5">
                        <h2 class="font-semibold text-white">{t("Options")}</h2>
                        <div class="grid grid-cols-2 gap-4 sm:grid-cols-4">
                            <div>
                                <label class="mb-1 block text-xs text-white/60">{t("Tracks by round")}</label>
                                <input type="number" min="1" max="50" class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white"
                                    prop:value=tracks_by_round on:input=move |ev| tracks_by_round.set(event_target_value(&ev).parse().unwrap_or(10))/>
                            </div>
                            <div>
                                <label class="mb-1 block text-xs text-white/60">{t("Track duration")}</label>
                                <input type="number" min="5" max="30" class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white"
                                    prop:value=track_duration on:input=move |ev| track_duration.set(event_target_value(&ev).parse().unwrap_or(30))/>
                            </div>
                            <div>
                                <label class="mb-1 block text-xs text-white/60">{t("Pause tracks")}</label>
                                <input type="number" min="0" max="60" class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white"
                                    prop:value=pause_tracks on:input=move |ev| pause_tracks.set(event_target_value(&ev).parse().unwrap_or(5))/>
                            </div>
                            <div>
                                <label class="mb-1 block text-xs text-white/60">{t("Pause rounds")}</label>
                                <input type="number" min="0" max="60" class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white"
                                    prop:value=pause_rounds on:input=move |ev| pause_rounds.set(event_target_value(&ev).parse().unwrap_or(30))/>
                            </div>
                        </div>
                        <div class="flex flex-wrap gap-5 text-sm text-white/70">
                            <label class="flex items-center gap-2">
                                <input type="checkbox" prop:checked=chat_active on:change=move |ev| chat_active.set(event_target_checked(&ev))/>
                                {t("Chat")}
                            </label>
                            <label class="flex items-center gap-2">
                                <input type="checkbox" prop:checked=autostart on:change=move |ev| autostart.set(event_target_checked(&ev))/>
                                {t("Autostart")}
                            </label>
                            <label class="flex items-center gap-2">
                                <input type="checkbox" prop:checked=random on:change=move |ev| random.set(event_target_checked(&ev))/>
                                {t("Random tracks")}
                            </label>
                            <label class="flex items-center gap-2">
                                <input type="checkbox" prop:checked=has_password on:change=move |ev| has_password.set(event_target_checked(&ev))/>
                                {t("Password")}
                            </label>
                            <label class="flex items-center gap-2" title=t("Uncheck to hide this room from public lists (seasonal rooms)")>
                                <input type="checkbox" prop:checked=active on:change=move |ev| active.set(event_target_checked(&ev))/>
                                {t("Active")}
                            </label>
                            <Show when=move || has_password.get()>
                                <input type="text" placeholder="…" class="rounded-lg border border-white/10 bg-surface-base px-3 py-1.5 text-white"
                                    prop:value=password on:input=move |ev| password.set(event_target_value(&ev))/>
                            </Show>
                        </div>
                        <div class="flex justify-end">
                            <button type="submit" class="btn-primary !px-4 !py-2">{t("Save options")}</button>
                        </div>
                    </form>

                    <Show when=move || owned.get()>
                        <div class="mb-6 rounded-xl border border-white/10 bg-surface-raised/60 p-5">
                            <h2 class="mb-3 font-semibold text-white">{t("Playlists")}</h2>
                            <div class="mb-3 space-y-2">
                                <For each=move || playlists.get() key=|p| p.id children=move |p: IdName| {
                                    view! {
                                        <div class="flex items-center justify-between rounded-lg border border-white/5 bg-surface-base/50 px-4 py-2">
                                            <span class="text-sm text-white">{p.name.clone()}</span>
                                            <button type="button" class="text-xs text-red-400 hover:underline"
                                                on:click=move |_| detach(p.id)>
                                                {t("Detach")}
                                            </button>
                                        </div>
                                    }
                                }/>
                                <Show when=move || playlists.get().is_empty()>
                                    <div class="rounded-lg border border-yellow-500/30 bg-yellow-500/10 px-4 py-2 text-sm text-yellow-300">
                                        {t("No playlist : the room won't start")}
                                    </div>
                                </Show>
                            </div>
                            <div class="flex items-center gap-3">
                                <select class="min-w-0 flex-1 rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white"
                                    on:change=move |ev| attach_id.set(event_target_value(&ev).parse().unwrap_or(0))>
                                    <option value="0">{t("Choose a playlist…")}</option>
                                    <For each=move || available.get() key=|p| p["id"].as_i64().unwrap_or(0) children=|p: Value| {
                                        view! {
                                            <option value=p["id"].as_i64().unwrap_or(0)>
                                                {format!("{} ({} tracks)", p["name"].as_str().unwrap_or_default(), p["tracks_count"].as_i64().unwrap_or(0))}
                                            </option>
                                        }
                                    }/>
                                </select>
                                <button type="button" class="btn-primary !px-4 !py-2" on:click=move |ev| attach(ev)>{t("Attach")}</button>
                            </div>
                        </div>

                        <crate::components::moderators::ModeratorsSection
                            moderators=mods.get()
                            attach_url=format!("/api/v1/rooms/{}/moderators/attach", room_id.get())
                            detach_url=format!("/api/v1/rooms/{}/moderators/detach", room_id.get())
                            can_manage=true
                            on_changed=move |_| {
                                let id = room_id.get_untracked();
                                leptos::task::spawn_local(async move {
                                    if let Ok(v) = api::get::<Value>(&format!("/api/v1/rooms/{id}/edit")).await {
                                        mods.set(
                                            v["room"]["moderators"]
                                                .as_array()
                                                .map(|a| a.iter().filter_map(|m| serde_json::from_value::<IdName>(m.clone()).ok()).collect())
                                                .unwrap_or_default(),
                                        );
                                    }
                                });
                            }
                            on_err=move |m| err.set(Some(m))/>
                    </Show>

                    <div class="flex justify-end">
                        <button type="button" class="rounded-lg border border-red-500/30 px-4 py-2 text-sm text-red-400 hover:bg-red-500/10"
                            on:click=move |ev| delete_room(ev)>
                            {t("Delete room")}
                        </button>
                    </div>
                </Show>
            </Show>
        </div>
    }
}
