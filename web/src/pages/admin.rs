use crate::api;
use crate::auth::use_auth;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use serde_json::{json, Value};

// ---- shared shell ------------------------------------------------------------

const SECTIONS: &[(&str, &str)] = &[
    ("/admin", "Dashboard"),
    ("/admin/users", "Users"),
    ("/admin/teams", "Teams"),
    ("/admin/rooms", "Rooms"),
    ("/admin/playlists", "Playlists"),
    ("/admin/categories", "Categories"),
    ("/admin/answer-types", "Answer Types"),
    ("/admin/pages", "Pages"),
    ("/admin/faqs", "FAQ"),
];

#[component]
pub fn AdminShell(active: &'static str, children: ChildrenFn) -> impl IntoView {
    let auth = use_auth();
    view! {
        <div class="mx-auto max-w-6xl px-4 py-8">
            <Show when=move || {
                auth.ready.get()
                    && auth.user.get().map(|u| u.is_administrator).unwrap_or(false)
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
                <h1 class="retro-title retro-title--white mb-6 text-3xl">"Admin"</h1>
                <nav class="mb-8 flex flex-wrap gap-2">
                    {SECTIONS.iter().map(|(href, label)| {
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

// ---- helpers -----------------------------------------------------------------

fn pager(page: RwSignal<i64>, last_page: RwSignal<i64>, reload: impl Fn() + Copy + Send + Sync + 'static) -> impl IntoView {
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

fn filter_bar(
    search: RwSignal<String>,
    trashed: Option<RwSignal<String>>,
    reload: impl Fn() + Copy + Send + Sync + 'static,
) -> impl IntoView {
    view! {
        <form class="mb-4 flex flex-wrap gap-2" on:submit=move |ev| { ev.prevent_default(); reload(); }>
            <input type="text" placeholder=t("Search")
                class="rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-sm text-white outline-none"
                prop:value=search on:input=move |ev| search.set(event_target_value(&ev))/>
            {trashed.map(|tr| view! {
                <select class="rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-sm text-white"
                    prop:value=tr on:change=move |ev| { tr.set(event_target_value(&ev)); reload(); }>
                    <option value="">{t("Active")}</option>
                    <option value="with">{t("With trashed")}</option>
                    <option value="only">{t("Only trashed")}</option>
                </select>
            })}
            <button type="submit" class="btn-primary !px-4 !py-2 text-sm">{t("Filter")}</button>
        </form>
    }
}

#[component]
fn ErrorLine(error: RwSignal<Option<String>>) -> impl IntoView {
    view! {
        {move || error.get().map(|e| view! {
            <div class="mb-3 rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-2 text-sm text-red-300">{e}</div>
        })}
    }
}

fn badge(ok: bool, yes: &'static str, no: &'static str) -> impl IntoView {
    view! {
        <span class=move || if ok {
            "rounded bg-emerald-500/20 px-2 py-0.5 text-xs text-emerald-300"
        } else {
            "rounded bg-white/10 px-2 py-0.5 text-xs text-white/60"
        }>{t(if ok { yes } else { no })}</span>
    }
}

// ---- dashboard -----------------------------------------------------------------

#[component]
pub fn AdminDashboard() -> impl IntoView {
    let data = RwSignal::new(Value::Null);
    let msg = RwSignal::new(Option::<String>::None);
    leptos::task::spawn_local(async move {
        if let Ok(v) = api::get::<Value>("/api/v1/admin/dashboard").await {
            data.set(v);
        }
    });
    let action = move |path: &'static str| {
        msg.set(None);
        leptos::task::spawn_local(async move {
            match api::post_empty::<Value>(path).await {
                Ok(v) => msg.set(Some(v["message"].as_str().unwrap_or("OK").to_string())),
                Err(e) => msg.set(Some(e.message)),
            }
        });
    };
    view! {
        <AdminShell active="/admin">
            <ErrorLine error=msg/>
            {move || msg.get().map(|m| view! {
                <div class="mb-4 rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-4 py-2 text-sm text-emerald-300">{m}</div>
            })}
            <div class="grid gap-4 md:grid-cols-2">
                <div class="card p-5">
                    <h2 class="mb-2 font-bold text-white">{t("Disk space")}</h2>
                    <pre class="max-h-40 overflow-auto whitespace-pre-wrap text-xs text-white/60">
                        {move || data.get()["diskSpace"].as_str().unwrap_or("…").to_string()}
                    </pre>
                </div>
                <div class="card p-5">
                    <h2 class="mb-2 font-bold text-white">{t("Cache")}</h2>
                    <p class="text-sm text-white/60">
                        "driver: " {move || data.get()["cacheDriver"].as_str().unwrap_or("…").to_string()}
                    </p>
                    <div class="mt-4 flex flex-wrap gap-2">
                        <button type="button" class="rounded-lg border border-white/10 px-3 py-2 text-sm text-white/70 hover:bg-white/5"
                            on:click=move |_| action("/api/v1/admin/clear-cache")>
                            {t("Clear cache")}
                        </button>
                        <button type="button" class="rounded-lg border border-white/10 px-3 py-2 text-sm text-white/70 hover:bg-white/5"
                            on:click=move |_| action("/api/v1/admin/force-clear-rounds")>
                            {t("Force clear rounds")}
                        </button>
                        <button type="button" class="rounded-lg border border-white/10 px-3 py-2 text-sm text-white/70 hover:bg-white/5"
                            on:click=move |_| action("/api/v1/admin/regenerate-top-10")>
                            {t("Regenerate top 10")}
                        </button>
                    </div>
                </div>
            </div>
        </AdminShell>
    }
}

// ---- users -------------------------------------------------------------------

#[component]
pub fn AdminUsers() -> impl IntoView {
    let data = RwSignal::new(Value::Null);
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);
    let page = RwSignal::new(1i64);
    let last_page = RwSignal::new(1i64);
    let search = RwSignal::new(String::new());
    let trashed = RwSignal::new(String::new());
    let role = RwSignal::new(String::new());
    let editing = RwSignal::new(Option::<i64>::None);
    let form = RwSignal::new(json!({"name":"","email":"","team_id":null,"is_administrator":false,"password":""}));
    let creating = RwSignal::new(false);

    let load = move || {
        let s = search.get_untracked();
        let tr = trashed.get_untracked();
        let ro = role.get_untracked();
        let p = page.get_untracked();
        leptos::task::spawn_local(async move {
            let mut q = format!("/api/v1/admin/users?page={p}&search={}", urlencoding(&s));
            if !tr.is_empty() { q += &format!("&trashed={tr}"); }
            if !ro.is_empty() { q += &format!("&role={ro}"); }
            match api::get::<Value>(&q).await {
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
        <AdminShell active="/admin/users">
            <ErrorLine error=error/>
            <div class="mb-4 flex flex-wrap items-end gap-2">
                <form class="flex flex-wrap gap-2" on:submit=move |ev| { ev.prevent_default(); page.set(1); load(); }>
                    <input type="text" placeholder=t("Search")
                        class="rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-sm text-white outline-none"
                        prop:value=search on:input=move |ev| search.set(event_target_value(&ev))/>
                    <select class="rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-sm text-white"
                        prop:value=role on:change=move |ev| { role.set(event_target_value(&ev)); page.set(1); load(); }>
                        <option value="">{t("All")}</option>
                        <option value="admin">{t("Admins")}</option>
                        <option value="guest">{t("Guests")}</option>
                    </select>
                    <select class="rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-sm text-white"
                        prop:value=trashed on:change=move |ev| { trashed.set(event_target_value(&ev)); page.set(1); load(); }>
                        <option value="">{t("Active")}</option>
                        <option value="with">{t("With trashed")}</option>
                        <option value="only">{t("Only trashed")}</option>
                    </select>
                    <button type="submit" class="btn-primary !px-4 !py-2 text-sm">{t("Filter")}</button>
                </form>
                <button type="button" class="rounded-lg border border-emerald-500/30 px-3 py-2 text-sm text-emerald-300"
                    on:click=move |_| {
                        form.set(json!({"name":"","email":"","team_id":null,"is_administrator":false,"password":""}));
                        creating.set(true); editing.set(None);
                    }>{t("New user")}</button>
            </div>

            <Show when=move || loading.get() fallback=move || view! {
                <div class="card overflow-x-auto">
                    <table class="w-full text-left text-sm">
                        <thead class="text-xs uppercase text-white/40">
                            <tr>
                                <th class="px-4 py-3">{t("User")}</th>
                                <th class="px-4 py-3">{t("Email")}</th>
                                <th class="px-4 py-3">{t("Team")}</th>
                                <th class="px-4 py-3">{t("Role")}</th>
                                <th class="px-4 py-3">{t("Created")}</th>
                                <th class="px-4 py-3 text-right">{t("Actions")}</th>
                            </tr>
                        </thead>
                        <tbody>
                            {move || data.get()["data"].as_array().cloned().unwrap_or_default()
                                .into_iter().map(|u| {
                                    let uid = u["id"].as_i64().unwrap_or(0);
                                    let name = u["name"].as_str().unwrap_or("").to_string();
                                    let email = u["email"].as_str().unwrap_or("—").to_string();
                                    let team = u["team"]["name"].as_str().unwrap_or("—").to_string();
                                    let is_admin = u["is_administrator"].as_bool().unwrap_or(false);
                                    let provider = u["provider"].as_str().unwrap_or("blindall").to_string();
                                    let deleted = !u["deleted_at"].is_null();
                                    let created = u["created_at"].as_str().unwrap_or("").chars().take(10).collect::<String>();
                                    let edit_u = u.clone();
                                    view! {
                                        <tr class="border-b border-white/5 last:border-0">
                                            <td class="px-4 py-2">
                                                <A href=format!("/profile/{uid}") attr:class="font-medium text-white hover:text-brand-accent-light">
                                                    {name}
                                                </A>
                                                <div class="text-xs text-white/40">{provider}</div>
                                            </td>
                                            <td class="px-4 py-2 text-white/70">{email}</td>
                                            <td class="px-4 py-2 text-white/70">{team}</td>
                                            <td class="px-4 py-2">{badge(is_admin, "Admin", "User")}</td>
                                            <td class="px-4 py-2 text-white/50">{created}</td>
                                            <td class="px-4 py-2 text-right">
                                                <div class="flex justify-end gap-1">
                                                    {(!deleted).then(|| view! {
                                                        <button type="button" class="rounded px-2 py-1 text-xs text-white/60 hover:bg-white/10"
                                                            on:click=move |_| {
                                                                form.set(json!({
                                                                    "name": edit_u["name"].as_str().unwrap_or(""),
                                                                    "email": edit_u["email"].as_str().unwrap_or(""),
                                                                    "team_id": edit_u["team_id"],
                                                                    "is_administrator": edit_u["is_administrator"].as_bool().unwrap_or(false),
                                                                    "password": "",
                                                                }));
                                                                editing.set(Some(uid)); creating.set(false);
                                                            }>{t("Edit")}</button>
                                                    })}
                                                    {if deleted { view! {
                                                        <button type="button" class="rounded px-2 py-1 text-xs text-emerald-300 hover:bg-emerald-500/10"
                                                            on:click=move |_| {
                                                                leptos::task::spawn_local(async move {
                                                                    let _ = api::put::<Value,_>(&format!("/api/v1/admin/users/{uid}/restore"), &json!({})).await;
                                                                    load();
                                                                });
                                                            }>{t("Restore")}</button>
                                                    }.into_any() } else { view! {
                                                        <button type="button" class="rounded px-2 py-1 text-xs text-red-400 hover:bg-red-500/10"
                                                            on:click=move |_| {
                                                                leptos::task::spawn_local(async move {
                                                                    let _ = api::delete::<Value>(&format!("/api/v1/admin/users/{uid}")).await;
                                                                    load();
                                                                });
                                                            }>{t("Delete")}</button>
                                                    }.into_any() }}
                                                </div>
                                            </td>
                                        </tr>
                                    }
                                }).collect::<Vec<_>>()}
                        </tbody>
                    </table>
                </div>
                {pager(page, last_page, load)}
            }>
                <div class="py-16 text-center text-white/60">{t("Loading")}"…"</div>
            </Show>

            <Show when=move || creating.get() || editing.get().is_some()>
                <form class="card mt-6 grid gap-3 p-5 md:grid-cols-2" on:submit=move |ev| {
                    ev.prevent_default();
                    let f = form.get();
                    let eid = editing.get();
                    leptos::task::spawn_local(async move {
                        let mut body = f.clone();
                        if body["password"].as_str().map(|p| p.is_empty()).unwrap_or(true) {
                            body.as_object_mut().map(|o| o.remove("password"));
                        }
                        let r = if let Some(id) = eid {
                            api::put::<Value,_>(&format!("/api/v1/admin/users/{id}"), &body).await
                        } else {
                            api::post::<Value,_>("/api/v1/admin/users", &body).await
                        };
                        match r {
                            Ok(_) => { creating.set(false); editing.set(None); load(); }
                            Err(e) => error.set(Some(e.message)),
                        }
                    });
                }>
                    <h2 class="font-bold text-white md:col-span-2">
                        {move || if editing.get().is_some() { t("Edit user") } else { t("New user") }}
                    </h2>
                    <label class="text-sm text-white/60">{t("Name")}
                        <input type="text" required maxlength="50"
                            class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                            prop:value=move || form.get()["name"].as_str().unwrap_or("").to_string()
                            on:input=move |ev| form.update(|f| f["name"] = json!(event_target_value(&ev)))/>
                    </label>
                    <label class="text-sm text-white/60">{t("Email")}
                        <input type="email" required maxlength="50"
                            class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                            prop:value=move || form.get()["email"].as_str().unwrap_or("").to_string()
                            on:input=move |ev| form.update(|f| f["email"] = json!(event_target_value(&ev)))/>
                    </label>
                    <label class="text-sm text-white/60">{t("Password")}
                        <input type="password" minlength="8"
                            class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                            prop:value=move || form.get()["password"].as_str().unwrap_or("").to_string()
                            on:input=move |ev| form.update(|f| f["password"] = json!(event_target_value(&ev)))/>
                    </label>
                    <label class="text-sm text-white/60">{t("Team ID")}
                        <input type="number" min="0"
                            class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                            prop:value=move || form.get()["team_id"].as_i64().map(|i| i.to_string()).unwrap_or_default()
                            on:input=move |ev| form.update(|f| {
                                let v = event_target_value(&ev);
                                f["team_id"] = v.parse::<i64>().ok().map(|n| json!(n)).unwrap_or(Value::Null);
                            })/>
                    </label>
                    <label class="flex items-center gap-2 text-sm text-white/70 md:col-span-2">
                        <input type="checkbox"
                            prop:checked=move || form.get()["is_administrator"].as_bool().unwrap_or(false)
                            on:change=move |ev| form.update(|f| f["is_administrator"] = json!(event_target_checked(&ev)))/>
                        {t("Administrator")}
                    </label>
                    <div class="flex gap-2 md:col-span-2">
                        <button type="submit" class="btn-primary !px-4 !py-2 text-sm">{t("Save")}</button>
                        <button type="button" class="rounded-lg border border-white/10 px-4 py-2 text-sm text-white/70"
                            on:click=move |_| { creating.set(false); editing.set(None); }>{t("Cancel")}</button>
                    </div>
                </form>
            </Show>
        </AdminShell>
    }
}

fn urlencoding(s: &str) -> String {
    s.replace('%', "%25").replace('&', "%26").replace('=', "%3D").replace(' ', "%20").replace('#', "%23")
}

// ---- generic list pages (teams / rooms / playlists) ---------------------------

#[component]
pub fn AdminTeams() -> impl IntoView {
    let data = RwSignal::new(Value::Null);
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);
    let page = RwSignal::new(1i64);
    let last_page = RwSignal::new(1i64);
    let search = RwSignal::new(String::new());
    let trashed = RwSignal::new(String::new());
    let editing = RwSignal::new(Option::<i64>::None);
    let form = RwSignal::new(json!({"name":"","user_id":0}));

    let load = move || {
        let s = search.get_untracked();
        let tr = trashed.get_untracked();
        let p = page.get_untracked();
        leptos::task::spawn_local(async move {
            let mut q = format!("/api/v1/admin/teams?page={p}&search={}", urlencoding(&s));
            if !tr.is_empty() { q += &format!("&trashed={tr}"); }
            match api::get::<Value>(&q).await {
                Ok(v) => { last_page.set(v["teams"]["last_page"].as_i64().unwrap_or(1)); data.set(v["teams"].clone()); }
                Err(e) => error.set(Some(e.message)),
            }
            loading.set(false);
        });
    };
    load();

    view! {
        <AdminShell active="/admin/teams">
            <ErrorLine error=error/>
            {filter_bar(search, Some(trashed), move || { page.set(1); load(); })}
            <Show when=move || !loading.get() fallback=move || view! {
                <div class="py-16 text-center text-white/60">{t("Loading")}"…"</div>
            }>
                <div class="card overflow-x-auto">
                    <table class="w-full text-left text-sm">
                        <thead class="text-xs uppercase text-white/40">
                            <tr><th class="px-4 py-3">{t("Name")}</th><th class="px-4 py-3">{t("Owner")}</th>
                                <th class="px-4 py-3">{t("Members")}</th><th class="px-4 py-3 text-right">{t("Actions")}</th></tr>
                        </thead>
                        <tbody>
                            {move || data.get()["data"].as_array().cloned().unwrap_or_default()
                                .into_iter().map(|tm| {
                                    let tid = tm["id"].as_i64().unwrap_or(0);
                                    let name = tm["name"].as_str().unwrap_or("").to_string();
                                    let owner = tm["owner"]["name"].as_str().unwrap_or("—").to_string();
                                    let count = tm["members_count"].as_i64().unwrap_or(0);
                                    let deleted = !tm["deleted_at"].is_null();
                                    let edit_tm = tm.clone();
                                    view! {
                                        <tr class="border-b border-white/5 last:border-0">
                                            <td class="px-4 py-2 font-medium text-white">
                                                <A href=format!("/teams/{tid}") attr:class="hover:text-brand-accent-light">{name}</A>
                                            </td>
                                            <td class="px-4 py-2 text-white/70">{owner}</td>
                                            <td class="px-4 py-2 text-white/70">{count}</td>
                                            <td class="px-4 py-2 text-right">
                                                <div class="flex justify-end gap-1">
                                                    {(!deleted).then(|| view! {
                                                        <button type="button" class="rounded px-2 py-1 text-xs text-white/60 hover:bg-white/10"
                                                            on:click=move |_| {
                                                                form.set(json!({"name": edit_tm["name"].as_str().unwrap_or(""), "user_id": edit_tm["user_id"].as_i64().unwrap_or(0)}));
                                                                editing.set(Some(tid));
                                                            }>{t("Edit")}</button>
                                                    })}
                                                    {if deleted { view! {
                                                        <button type="button" class="rounded px-2 py-1 text-xs text-emerald-300 hover:bg-emerald-500/10"
                                                            on:click=move |_| leptos::task::spawn_local(async move {
                                                                let _ = api::put::<Value,_>(&format!("/api/v1/admin/teams/{tid}/restore"), &json!({})).await; load();
                                                            })>{t("Restore")}</button>
                                                    }.into_any() } else { view! {
                                                        <button type="button" class="rounded px-2 py-1 text-xs text-red-400 hover:bg-red-500/10"
                                                            on:click=move |_| leptos::task::spawn_local(async move {
                                                                let _ = api::delete::<Value>(&format!("/api/v1/admin/teams/{tid}")).await; load();
                                                            })>{t("Delete")}</button>
                                                    }.into_any() }}
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
            <Show when=move || editing.get().is_some()>
                <form class="card mt-6 grid max-w-lg gap-3 p-5" on:submit=move |ev| {
                    ev.prevent_default();
                    let f = form.get(); let tid = editing.get().unwrap_or(0);
                    leptos::task::spawn_local(async move {
                        match api::put::<Value,_>(&format!("/api/v1/admin/teams/{tid}"), &f).await {
                            Ok(_) => { editing.set(None); load(); }
                            Err(e) => error.set(Some(e.message)),
                        }
                    });
                }>
                    <h2 class="font-bold text-white">{t("Edit team")}</h2>
                    <label class="text-sm text-white/60">{t("Name")}
                        <input type="text" required maxlength="50"
                            class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                            prop:value=move || form.get()["name"].as_str().unwrap_or("").to_string()
                            on:input=move |ev| form.update(|f| f["name"] = json!(event_target_value(&ev)))/>
                    </label>
                    <label class="text-sm text-white/60">{t("Owner (user id)")}
                        <input type="number" required min="1"
                            class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                            prop:value=move || form.get()["user_id"].as_i64().unwrap_or(0).to_string()
                            on:input=move |ev| form.update(|f| f["user_id"] = json!(event_target_value(&ev).parse::<i64>().unwrap_or(0)))/>
                    </label>
                    <div class="flex gap-2">
                        <button type="submit" class="btn-primary !px-4 !py-2 text-sm">{t("Save")}</button>
                        <button type="button" class="rounded-lg border border-white/10 px-4 py-2 text-sm text-white/70"
                            on:click=move |_| editing.set(None)>{t("Cancel")}</button>
                    </div>
                </form>
            </Show>
        </AdminShell>
    }
}

#[component]
pub fn AdminRooms() -> impl IntoView {
    let data = RwSignal::new(Value::Null);
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);
    let page = RwSignal::new(1i64);
    let last_page = RwSignal::new(1i64);
    let search = RwSignal::new(String::new());
    let trashed = RwSignal::new(String::new());
    let editing = RwSignal::new(Option::<i64>::None);
    let creating = RwSignal::new(false);
    let form = RwSignal::new(Value::Null);
    let categories = RwSignal::new(Vec::<Value>::new());

    let load = move || {
        let s = search.get_untracked();
        let tr = trashed.get_untracked();
        let p = page.get_untracked();
        leptos::task::spawn_local(async move {
            let mut q = format!("/api/v1/admin/rooms?page={p}&search={}", urlencoding(&s));
            if !tr.is_empty() { q += &format!("&trashed={tr}"); }
            match api::get::<Value>(&q).await {
                Ok(v) => { last_page.set(v["rooms"]["last_page"].as_i64().unwrap_or(1)); data.set(v["rooms"].clone()); }
                Err(e) => error.set(Some(e.message)),
            }
            loading.set(false);
        });
    };
    load();
    leptos::task::spawn_local(async move {
        if let Ok(v) = api::get::<Value>("/api/v1/categories").await {
            categories.set(v["categories"].as_array().cloned().unwrap_or_default());
        }
    });

    view! {
        <AdminShell active="/admin/rooms">
            <ErrorLine error=error/>
            <div class="mb-4 flex items-start justify-between gap-2">
                {filter_bar(search, Some(trashed), move || { page.set(1); load(); })}
                <button type="button" class="btn-primary shrink-0 !px-4 !py-2 text-sm"
                    on:click=move |_| { form.set(json!({})); editing.set(None); creating.set(true); }>
                    {t("New room")}</button>
            </div>
            <Show when=move || !loading.get() fallback=move || view! {
                <div class="py-16 text-center text-white/60">{t("Loading")}"…"</div>
            }>
                <div class="card overflow-x-auto">
                    <table class="w-full text-left text-sm">
                        <thead class="text-xs uppercase text-white/40">
                            <tr><th class="px-4 py-3">{t("Room")}</th><th class="px-4 py-3">{t("Owner")}</th>
                                <th class="px-4 py-3">{t("Playlists")}</th><th class="px-4 py-3">{t("Public")}</th>
                                <th class="px-4 py-3 text-right">{t("Actions")}</th></tr>
                        </thead>
                        <tbody>
                            {move || data.get()["data"].as_array().cloned().unwrap_or_default()
                                .into_iter().map(|r| {
                                    let rid = r["id"].as_i64().unwrap_or(0);
                                    let name = r["name"].as_str().unwrap_or("").to_string();
                                    let slug = r["slug"].as_str().unwrap_or("").to_string();
                                    let owner = r["owner"]["name"].as_str().unwrap_or("—").to_string();
                                    let playlists = r["playlists"].as_array().map(|p| p.iter()
                                        .filter_map(|x| x["name"].as_str()).collect::<Vec<_>>().join(", "))
                                        .unwrap_or_else(|| "—".to_string());
                                    let is_public = r["is_public"].as_bool().unwrap_or(false);
                                    let deleted = !r["deleted_at"].is_null();
                                    let edit_r = r.clone();
                                    view! {
                                        <tr class="border-b border-white/5 last:border-0">
                                            <td class="px-4 py-2 font-medium text-white">
                                                <A href=format!("/rooms/{slug}") attr:class="hover:text-brand-accent-light">{name}</A>
                                            </td>
                                            <td class="px-4 py-2 text-white/70">{owner}</td>
                                            <td class="px-4 py-2 text-xs text-white/60">{playlists}</td>
                                            <td class="px-4 py-2">{badge(is_public, "Public", "Private")}</td>
                                            <td class="px-4 py-2 text-right">
                                                <div class="flex justify-end gap-1">
                                                    {(!deleted).then(|| view! {
                                                        <button type="button" class="rounded px-2 py-1 text-xs text-white/60 hover:bg-white/10"
                                                            on:click=move |_| {
                                                                form.set(edit_r.clone());
                                                                editing.set(Some(rid));
                                                            }>{t("Edit")}</button>
                                                    })}
                                                    {if deleted { view! {
                                                        <button type="button" class="rounded px-2 py-1 text-xs text-emerald-300 hover:bg-emerald-500/10"
                                                            on:click=move |_| leptos::task::spawn_local(async move {
                                                                let _ = api::put::<Value,_>(&format!("/api/v1/admin/rooms/{rid}/restore"), &json!({})).await; load();
                                                            })>{t("Restore")}</button>
                                                    }.into_any() } else { view! {
                                                        <button type="button" class="rounded px-2 py-1 text-xs text-red-400 hover:bg-red-500/10"
                                                            on:click=move |_| leptos::task::spawn_local(async move {
                                                                let _ = api::delete::<Value>(&format!("/api/v1/admin/rooms/{rid}")).await; load();
                                                            })>{t("Delete")}</button>
                                                    }.into_any() }}
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
            <Show when=move || editing.get().is_some() || creating.get()>
                {move || {
                    let f = form.get();
                    if f.is_null() { return view! {<div/>}.into_any(); }
                    let rid = editing.get().unwrap_or(0);
                    let is_new = creating.get_untracked();
                    let name = RwSignal::new(f["name"].as_str().unwrap_or("").to_string());
                    let description = RwSignal::new(f["description"].as_str().unwrap_or("").to_string());
                    let category_id = RwSignal::new(f["category_id"].as_i64().unwrap_or(1));
                    let tracks_by_round = RwSignal::new(f["tracks_by_round"].as_i64().unwrap_or(15));
                    let track_duration = RwSignal::new(f["track_duration"].as_i64().unwrap_or(30));
                    let pbt = RwSignal::new(f["pause_between_tracks"].as_i64().unwrap_or(5));
                    let pbr = RwSignal::new(f["pause_between_rounds"].as_i64().unwrap_or(10));
                    let is_public = RwSignal::new(f["is_public"].as_bool().unwrap_or(false));
                    let is_pro = RwSignal::new(f["is_pro"].as_bool().unwrap_or(false));
                    let is_random = RwSignal::new(f["is_random"].as_bool().unwrap_or(false));
                    let is_active = RwSignal::new(f["is_active"].as_bool().unwrap_or(true));
                    let is_chat = RwSignal::new(f["is_chat_active"].as_bool().unwrap_or(true));
                    let webhook = RwSignal::new(f["discord_webhook_url"].as_str().unwrap_or("").to_string());
                    let color = RwSignal::new(f["color"].as_str().unwrap_or("").to_string());
                    let password = RwSignal::new(String::new());
                    view! {
                        <form class="card mt-6 grid gap-3 p-5 md:grid-cols-3" on:submit=move |ev| {
                            ev.prevent_default();
                            let body = json!({
                                "name": name.get_untracked(), "description": description.get_untracked(),
                                "category_id": category_id.get_untracked(),
                                "tracks_by_round": tracks_by_round.get_untracked(),
                                "track_duration": track_duration.get_untracked(),
                                "pause_between_tracks": pbt.get_untracked(),
                                "pause_between_rounds": pbr.get_untracked(),
                                "is_public": is_public.get_untracked(), "is_pro": is_pro.get_untracked(),
                                "is_random": is_random.get_untracked(), "is_active": is_active.get_untracked(),
                                "is_chat_active": is_chat.get_untracked(),
                                "discord_webhook_url": webhook.get_untracked(),
                                "color": color.get_untracked(),
                                "password": if password.get_untracked().is_empty() { Value::Null } else { json!(password.get_untracked()) },
                            });
                            leptos::task::spawn_local(async move {
                                let r = if is_new {
                                    api::post::<Value,_>("/api/v1/admin/rooms", &body).await
                                } else {
                                    api::put::<Value,_>(&format!("/api/v1/admin/rooms/{rid}"), &body).await
                                };
                                match r {
                                    Ok(_) => { editing.set(None); creating.set(false); load(); }
                                    Err(e) => error.set(Some(e.message)),
                                }
                            });
                        }>
                            <h2 class="font-bold text-white md:col-span-3">
                                {move || if creating.get() { t("New room") } else { t("Edit room") }}</h2>
                            <label class="text-sm text-white/60">{t("Name")}
                                <input type="text" required maxlength="30" class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                                    prop:value=name on:input=move |ev| name.set(event_target_value(&ev))/></label>
                            <label class="text-sm text-white/60">{t("Category")}
                                <select class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white"
                                    prop:value=move || category_id.get().to_string()
                                    on:change=move |ev| category_id.set(event_target_value(&ev).parse().unwrap_or(1))>
                                    {categories.get().into_iter().map(|c| view! {
                                        <option value=c["id"].as_i64().unwrap_or(0).to_string()>{c["name"].as_str().unwrap_or("").to_string()}</option>
                                    }).collect::<Vec<_>>()}
                                </select></label>
                            <label class="text-sm text-white/60 md:col-span-1">{t("Description")}
                                <input type="text" class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                                    prop:value=description on:input=move |ev| description.set(event_target_value(&ev))/></label>
                            <label class="text-sm text-white/60">{t("Tracks/round")}
                                <input type="number" min="1" max="50" class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                                    prop:value=move || tracks_by_round.get().to_string()
                                    on:input=move |ev| tracks_by_round.set(event_target_value(&ev).parse().unwrap_or(15))/></label>
                            <label class="text-sm text-white/60">{t("Track duration")}
                                <input type="number" min="5" max="30" class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                                    prop:value=move || track_duration.get().to_string()
                                    on:input=move |ev| track_duration.set(event_target_value(&ev).parse().unwrap_or(30))/></label>
                            <label class="text-sm text-white/60">{t("Pause tracks")}
                                <input type="number" min="0" max="60" class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                                    prop:value=move || pbt.get().to_string()
                                    on:input=move |ev| pbt.set(event_target_value(&ev).parse().unwrap_or(5))/></label>
                            <label class="text-sm text-white/60">{t("Pause rounds")}
                                <input type="number" min="0" max="60" class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                                    prop:value=move || pbr.get().to_string()
                                    on:input=move |ev| pbr.set(event_target_value(&ev).parse().unwrap_or(10))/></label>
                            <label class="text-sm text-white/60">{t("Discord webhook")}
                                <input type="url" class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                                    prop:value=webhook on:input=move |ev| webhook.set(event_target_value(&ev))/></label>
                            <label class="text-sm text-white/60">{t("New password")}
                                <input type="text" class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                                    prop:value=password on:input=move |ev| password.set(event_target_value(&ev))/></label>
                            <div class="flex flex-wrap gap-4 md:col-span-3">
                                {[("Public", is_public), ("Pro", is_pro), ("Random", is_random), ("Active", is_active), ("Chat", is_chat)]
                                    .into_iter().map(|(lbl, sig)| view! {
                                        <label class="flex items-center gap-2 text-sm text-white/70">
                                            <input type="checkbox" prop:checked=sig
                                                on:change=move |ev| sig.set(event_target_checked(&ev))/>{t(lbl)}
                                        </label>
                                    }).collect::<Vec<_>>()}
                            </div>
                            <div class="flex gap-2 md:col-span-3">
                                <button type="submit" class="btn-primary !px-4 !py-2 text-sm">{t("Save")}</button>
                                <button type="button" class="rounded-lg border border-white/10 px-4 py-2 text-sm text-white/70"
                                    on:click=move |_| { editing.set(None); creating.set(false); }>{t("Cancel")}</button>
                            </div>
                        </form>
                    }.into_any()
                }}
            </Show>
        </AdminShell>
    }
}

#[component]
pub fn AdminPlaylists() -> impl IntoView {
    let data = RwSignal::new(Value::Null);
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);
    let page = RwSignal::new(1i64);
    let last_page = RwSignal::new(1i64);
    let search = RwSignal::new(String::new());
    let editing = RwSignal::new(Option::<i64>::None);
    let creating = RwSignal::new(false);
    let form = RwSignal::new(json!({"name":"","description":"","is_public":false}));

    let load = move || {
        let s = search.get_untracked();
        let p = page.get_untracked();
        leptos::task::spawn_local(async move {
            match api::get::<Value>(&format!("/api/v1/admin/playlists?page={p}&search={}", urlencoding(&s))).await {
                Ok(v) => { last_page.set(v["playlists"]["last_page"].as_i64().unwrap_or(1)); data.set(v["playlists"].clone()); }
                Err(e) => error.set(Some(e.message)),
            }
            loading.set(false);
        });
    };
    load();

    view! {
        <AdminShell active="/admin/playlists">
            <ErrorLine error=error/>
            <div class="mb-4 flex items-center justify-between gap-2">
                {filter_bar(search, None, move || { page.set(1); load(); })}
                <button type="button" class="btn-primary !px-4 !py-2 text-sm"
                    on:click=move |_| {
                        form.set(json!({"name":"","description":"","is_public":false}));
                        editing.set(None);
                        creating.set(true);
                    }>{t("New playlist")}</button>
            </div>
            <Show when=move || !loading.get() fallback=move || view! {
                <div class="py-16 text-center text-white/60">{t("Loading")}"…"</div>
            }>
                <div class="card overflow-x-auto">
                    <table class="w-full text-left text-sm">
                        <thead class="text-xs uppercase text-white/40">
                            <tr><th class="px-4 py-3">{t("Playlist")}</th><th class="px-4 py-3">{t("Owner")}</th>
                                <th class="px-4 py-3">{t("Tracks")}</th><th class="px-4 py-3">{t("Public")}</th>
                                <th class="px-4 py-3 text-right">{t("Actions")}</th></tr>
                        </thead>
                        <tbody>
                            {move || data.get()["data"].as_array().cloned().unwrap_or_default()
                                .into_iter().map(|p| {
                                    let pid = p["id"].as_i64().unwrap_or(0);
                                    let name = p["name"].as_str().unwrap_or("").to_string();
                                    let owner = p["owner"]["name"].as_str().unwrap_or("—").to_string();
                                    let count = p["tracks_count"].as_i64().unwrap_or(0);
                                    let is_public = p["is_public"].as_bool().unwrap_or(false);
                                    let edit_p = p.clone();
                                    view! {
                                        <tr class="border-b border-white/5 last:border-0">
                                            <td class="px-4 py-2 font-medium text-white">
                                                <A href=format!("/playlists/{pid}/edit") attr:class="hover:text-brand-accent-light">{name}</A>
                                            </td>
                                            <td class="px-4 py-2 text-white/70">{owner}</td>
                                            <td class="px-4 py-2 text-white/70">{count}</td>
                                            <td class="px-4 py-2">{badge(is_public, "Public", "Private")}</td>
                                            <td class="px-4 py-2 text-right">
                                                <div class="flex justify-end gap-1">
                                                    <button type="button" class="rounded px-2 py-1 text-xs text-white/60 hover:bg-white/10"
                                                        on:click=move |_| {
                                                            form.set(json!({
                                                                "name": edit_p["name"].as_str().unwrap_or(""),
                                                                "description": edit_p["description"].as_str().unwrap_or(""),
                                                                "is_public": edit_p["is_public"].as_bool().unwrap_or(false),
                                                            }));
                                                            editing.set(Some(pid));
                                                        }>{t("Edit")}</button>
                                                    <button type="button" class="rounded px-2 py-1 text-xs text-red-400 hover:bg-red-500/10"
                                                        on:click=move |_| leptos::task::spawn_local(async move {
                                                            let _ = api::delete::<Value>(&format!("/api/v1/admin/playlists/{pid}")).await; load();
                                                        })>{t("Delete")}</button>
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
            <Show when=move || editing.get().is_some() || creating.get()>
                <form class="card mt-6 grid max-w-lg gap-3 p-5" on:submit=move |ev| {
                    ev.prevent_default();
                    let f = form.get(); let pid = editing.get().unwrap_or(0); let is_new = creating.get_untracked();
                    leptos::task::spawn_local(async move {
                        let r = if is_new {
                            api::post::<Value,_>("/api/v1/admin/playlists", &f).await
                        } else {
                            api::put::<Value,_>(&format!("/api/v1/admin/playlists/{pid}"), &f).await
                        };
                        match r {
                            Ok(_) => { editing.set(None); creating.set(false); load(); }
                            Err(e) => error.set(Some(e.message)),
                        }
                    });
                }>
                    <h2 class="font-bold text-white">{move || if creating.get() { t("New playlist") } else { t("Edit playlist") }}</h2>
                    <label class="text-sm text-white/60">{t("Name")}
                        <input type="text" required maxlength="50" class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                            prop:value=move || form.get()["name"].as_str().unwrap_or("").to_string()
                            on:input=move |ev| form.update(|f| f["name"] = json!(event_target_value(&ev)))/></label>
                    <label class="text-sm text-white/60">{t("Description")}
                        <input type="text" class="mt-1 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                            prop:value=move || form.get()["description"].as_str().unwrap_or("").to_string()
                            on:input=move |ev| form.update(|f| f["description"] = json!(event_target_value(&ev)))/></label>
                    <label class="flex items-center gap-2 text-sm text-white/70">
                        <input type="checkbox" prop:checked=move || form.get()["is_public"].as_bool().unwrap_or(false)
                            on:change=move |ev| form.update(|f| f["is_public"] = json!(event_target_checked(&ev)))/>
                        {t("Public")}
                    </label>
                    <div class="flex gap-2">
                        <button type="submit" class="btn-primary !px-4 !py-2 text-sm">{t("Save")}</button>
                        <button type="button" class="rounded-lg border border-white/10 px-4 py-2 text-sm text-white/70"
                            on:click=move |_| { editing.set(None); creating.set(false); }>{t("Cancel")}</button>
                    </div>
                </form>
            </Show>
        </AdminShell>
    }
}

// ---- categories / answer-types / pages / faqs --------------------------------

#[component]
pub fn AdminCategories() -> impl IntoView {
    let items = RwSignal::new(Vec::<Value>::new());
    let error = RwSignal::new(Option::<String>::None);
    let name = RwSignal::new(String::new());
    let editing = RwSignal::new(Option::<i64>::None);
    let edit_name = RwSignal::new(String::new());

    let load = move || leptos::task::spawn_local(async move {
        if let Ok(v) = api::get::<Value>("/api/v1/admin/categories").await {
            items.set(v["categories"].as_array().cloned().unwrap_or_default());
        }
    });
    load();

    view! {
        <AdminShell active="/admin/categories">
            <ErrorLine error=error/>
            <form class="mb-6 flex max-w-md gap-2" on:submit=move |ev| {
                ev.prevent_default();
                let n = name.get_untracked();
                leptos::task::spawn_local(async move {
                    match api::post::<Value,_>("/api/v1/admin/categories", &json!({"name": n})).await {
                        Ok(_) => { name.set(String::new()); load(); }
                        Err(e) => error.set(Some(e.message)),
                    }
                });
            }>
                <input type="text" required maxlength="50" placeholder=t("New category")
                    class="flex-1 rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                    prop:value=name on:input=move |ev| name.set(event_target_value(&ev))/>
                <button type="submit" class="btn-primary !px-4 !py-2 text-sm">{t("Add")}</button>
            </form>
            <div class="card">
                {move || items.get().into_iter().map(|c| {
                    let cid = c["id"].as_i64().unwrap_or(0);
                    let cname = c["name"].as_str().unwrap_or("").to_string();
                    let ec = cname.clone();
                    if editing.get() == Some(cid) {
                        view! {
                            <div class="flex items-center justify-between border-b border-white/5 px-4 py-3 last:border-0">
                                <input type="text" class="rounded border border-white/10 bg-surface-base px-2 py-1 text-white"
                                    prop:value=edit_name on:input=move |ev| edit_name.set(event_target_value(&ev))/>
                                <div class="flex gap-1">
                                    <button type="button" class="rounded px-2 py-1 text-xs text-emerald-300 hover:bg-emerald-500/10"
                                        on:click=move |_| {
                                            let n = edit_name.get_untracked();
                                            leptos::task::spawn_local(async move {
                                                let _ = api::put::<Value,_>(&format!("/api/v1/admin/categories/{cid}"), &json!({"name": n})).await;
                                                editing.set(None); load();
                                            });
                                        }>{t("Save")}</button>
                                    <button type="button" class="rounded px-2 py-1 text-xs text-white/60 hover:bg-white/10"
                                        on:click=move |_| editing.set(None)>{t("Cancel")}</button>
                                </div>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="flex items-center justify-between border-b border-white/5 px-4 py-3 last:border-0">
                                <span class="text-white">{cname}</span>
                                <div class="flex gap-1">
                                    <button type="button" class="rounded px-2 py-1 text-xs text-white/60 hover:bg-white/10"
                                        on:click=move |_| { edit_name.set(ec.clone()); editing.set(Some(cid)); }>
                                        {t("Edit")}
                                    </button>
                                    <button type="button" class="rounded px-2 py-1 text-xs text-red-400 hover:bg-red-500/10"
                                        on:click=move |_| leptos::task::spawn_local(async move {
                                            let _ = api::delete::<Value>(&format!("/api/v1/admin/categories/{cid}")).await; load();
                                        })>{t("Delete")}</button>
                                </div>
                            </div>
                        }.into_any()
                    }
                }).collect::<Vec<_>>()}
            </div>
        </AdminShell>
    }
}

#[component]
pub fn AdminAnswerTypes() -> impl IntoView {
    let items = RwSignal::new(Vec::<Value>::new());
    let error = RwSignal::new(Option::<String>::None);
    let form = RwSignal::new(json!({"name":"","pronoun":"","svg_icon":""}));
    let editing = RwSignal::new(Option::<i64>::None);

    let load = move || leptos::task::spawn_local(async move {
        if let Ok(v) = api::get::<Value>("/api/v1/admin/answer-types").await {
            items.set(v["answer_types"].as_array().cloned().unwrap_or_default());
        }
    });
    load();

    view! {
        <AdminShell active="/admin/answer-types">
            <ErrorLine error=error/>
            <form class="card mb-6 grid gap-3 p-5 md:grid-cols-4" on:submit=move |ev| {
                ev.prevent_default();
                let f = form.get(); let eid = editing.get();
                leptos::task::spawn_local(async move {
                    let r = if let Some(id) = eid {
                        api::put::<Value,_>(&format!("/api/v1/admin/answer-types/{id}"), &f).await
                    } else {
                        api::post::<Value,_>("/api/v1/admin/answer-types", &f).await
                    };
                    match r {
                        Ok(_) => { form.set(json!({"name":"","pronoun":"","svg_icon":""})); editing.set(None); load(); }
                        Err(e) => error.set(Some(e.message)),
                    }
                });
            }>
                <input type="text" required maxlength="50" placeholder=t("Name")
                    class="rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                    prop:value=move || form.get()["name"].as_str().unwrap_or("").to_string()
                    on:input=move |ev| form.update(|f| f["name"] = json!(event_target_value(&ev)))/>
                <input type="text" maxlength="10" placeholder=t("Pronoun")
                    class="rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                    prop:value=move || form.get()["pronoun"].as_str().unwrap_or("").to_string()
                    on:input=move |ev| form.update(|f| f["pronoun"] = json!(event_target_value(&ev)))/>
                <input type="text" placeholder="svg icon"
                    class="rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                    prop:value=move || form.get()["svg_icon"].as_str().unwrap_or("").to_string()
                    on:input=move |ev| form.update(|f| f["svg_icon"] = json!(event_target_value(&ev)))/>
                <div class="flex gap-2">
                    <button type="submit" class="btn-primary !px-4 !py-2 text-sm">
                        {move || if editing.get().is_some() { t("Save") } else { t("Add") }}
                    </button>
                    <Show when=move || editing.get().is_some()>
                        <button type="button" class="rounded-lg border border-white/10 px-4 py-2 text-sm text-white/70"
                            on:click=move |_| { editing.set(None); form.set(json!({"name":"","pronoun":"","svg_icon":""})); }>
                            {t("Cancel")}
                        </button>
                    </Show>
                </div>
            </form>
            <div class="card">
                {move || items.get().into_iter().map(|a| {
                    let aid = a["id"].as_i64().unwrap_or(0);
                    let aname = a["name"].as_str().unwrap_or("").to_string();
                    let ea = a.clone();
                    view! {
                        <div class="flex items-center justify-between border-b border-white/5 px-4 py-3 last:border-0">
                            <span class="text-white">{aname}</span>
                            <div class="flex gap-1">
                                <button type="button" class="rounded px-2 py-1 text-xs text-white/60 hover:bg-white/10"
                                    on:click=move |_| {
                                        form.set(json!({
                                            "name": ea["name"].as_str().unwrap_or(""),
                                            "pronoun": ea["pronoun"].as_str().unwrap_or(""),
                                            "svg_icon": ea["icon"].as_str().or(ea["svg_icon"].as_str()).unwrap_or(""),
                                        }));
                                        editing.set(Some(aid));
                                    }>{t("Edit")}</button>
                                <button type="button" class="rounded px-2 py-1 text-xs text-red-400 hover:bg-red-500/10"
                                    on:click=move |_| leptos::task::spawn_local(async move {
                                        let _ = api::delete::<Value>(&format!("/api/v1/admin/answer-types/{aid}")).await; load();
                                    })>{t("Delete")}</button>
                            </div>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </AdminShell>
    }
}

#[component]
pub fn AdminPages() -> impl IntoView {
    let items = RwSignal::new(Vec::<Value>::new());
    let error = RwSignal::new(Option::<String>::None);
    let form = RwSignal::new(json!({"title":"","content":"","parent_id":null}));
    let editing = RwSignal::new(Option::<i64>::None);
    let show_form = RwSignal::new(false);

    let load = move || leptos::task::spawn_local(async move {
        if let Ok(v) = api::get::<Value>("/api/v1/admin/pages").await {
            items.set(v["pages"].as_array().cloned().unwrap_or_default());
        }
    });
    load();

    view! {
        <AdminShell active="/admin/pages">
            <ErrorLine error=error/>
            <button type="button" class="btn-primary mb-4 !px-4 !py-2 text-sm"
                on:click=move |_| { form.set(json!({"title":"","content":"","parent_id":null})); editing.set(None); show_form.set(true); }>
                {t("New page")}
            </button>
            <Show when=move || show_form.get()>
                <form class="card mb-6 grid gap-3 p-5" on:submit=move |ev| {
                    ev.prevent_default();
                    let f = form.get(); let eid = editing.get();
                    leptos::task::spawn_local(async move {
                        let r = if let Some(id) = eid {
                            api::put::<Value,_>(&format!("/api/v1/admin/pages/{id}"), &f).await
                        } else {
                            api::post::<Value,_>("/api/v1/admin/pages", &f).await
                        };
                        match r {
                            Ok(_) => { show_form.set(false); editing.set(None); load(); }
                            Err(e) => error.set(Some(e.message)),
                        }
                    });
                }>
                    <input type="text" required maxlength="50" placeholder=t("Title")
                        class="rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                        prop:value=move || form.get()["title"].as_str().unwrap_or("").to_string()
                        on:input=move |ev| form.update(|f| f["title"] = json!(event_target_value(&ev)))/>
                    <textarea required rows="10" placeholder="Contenu (markdown)"
                        class="rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                        prop:value=move || form.get()["content"].as_str().unwrap_or("").to_string()
                        on:input=move |ev| form.update(|f| f["content"] = json!(event_target_value(&ev)))/>
                    <div class="flex gap-2">
                        <button type="submit" class="btn-primary !px-4 !py-2 text-sm">{t("Save")}</button>
                        <button type="button" class="rounded-lg border border-white/10 px-4 py-2 text-sm text-white/70"
                            on:click=move |_| show_form.set(false)>{t("Cancel")}</button>
                    </div>
                </form>
            </Show>
            <div class="card">
                {move || items.get().into_iter().map(|p| {
                    let pid = p["id"].as_i64().unwrap_or(0);
                    let title = p["title"].as_str().unwrap_or("").to_string();
                    let slug = p["slug"].as_str().unwrap_or("").to_string();
                    let ep = p.clone();
                    view! {
                        <div class="flex items-center justify-between border-b border-white/5 px-4 py-3 last:border-0">
                            <A href=format!("/pages/{slug}") attr:class="text-white hover:text-brand-accent-light">{title}</A>
                            <div class="flex gap-1">
                                <button type="button" class="rounded px-2 py-1 text-xs text-white/60 hover:bg-white/10"
                                    on:click=move |_| {
                                        form.set(json!({
                                            "title": ep["title"].as_str().unwrap_or(""),
                                            "content": ep["content"].as_str().unwrap_or(""),
                                            "parent_id": ep["original_id"].as_i64().or(ep["id"].as_i64()),
                                        }));
                                        editing.set(Some(pid)); show_form.set(true);
                                    }>{t("Edit")}</button>
                                <button type="button" class="rounded px-2 py-1 text-xs text-red-400 hover:bg-red-500/10"
                                    on:click=move |_| leptos::task::spawn_local(async move {
                                        let _ = api::delete::<Value>(&format!("/api/v1/admin/pages/{pid}")).await; load();
                                    })>{t("Delete")}</button>
                            </div>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </AdminShell>
    }
}

#[component]
pub fn AdminFaqs() -> impl IntoView {
    let items = RwSignal::new(Vec::<Value>::new());
    let error = RwSignal::new(Option::<String>::None);
    let form = RwSignal::new(json!({"locale":"en","question":"","answer":""}));
    let editing = RwSignal::new(Option::<i64>::None);
    let show_form = RwSignal::new(false);

    let load = move || leptos::task::spawn_local(async move {
        if let Ok(v) = api::get::<Value>("/api/v1/admin/faqs").await {
            items.set(v["faqs"].as_array().cloned().unwrap_or_default());
        }
    });
    load();

    view! {
        <AdminShell active="/admin/faqs">
            <ErrorLine error=error/>
            <button type="button" class="btn-primary mb-4 !px-4 !py-2 text-sm"
                on:click=move |_| { form.set(json!({"locale":"en","question":"","answer":""})); editing.set(None); show_form.set(true); }>
                {t("New question")}
            </button>
            <Show when=move || show_form.get()>
                <form class="card mb-6 grid gap-3 p-5" on:submit=move |ev| {
                    ev.prevent_default();
                    let f = form.get(); let eid = editing.get();
                    leptos::task::spawn_local(async move {
                        let r = if let Some(id) = eid {
                            api::put::<Value,_>(&format!("/api/v1/admin/faqs/{id}"), &f).await
                        } else {
                            api::post::<Value,_>("/api/v1/admin/faqs", &f).await
                        };
                        match r {
                            Ok(_) => { show_form.set(false); editing.set(None); load(); }
                            Err(e) => error.set(Some(e.message)),
                        }
                    });
                }>
                    <div class="flex gap-3">
                        <select class="rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white"
                            prop:value=move || form.get()["locale"].as_str().unwrap_or("en").to_string()
                            on:change=move |ev| form.update(|f| f["locale"] = json!(event_target_value(&ev)))>
                            <option value="en">"en"</option>
                            <option value="fr">"fr"</option>
                        </select>
                        <input type="text" required maxlength="255" placeholder=t("Question")
                            class="flex-1 rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                            prop:value=move || form.get()["question"].as_str().unwrap_or("").to_string()
                            on:input=move |ev| form.update(|f| f["question"] = json!(event_target_value(&ev)))/>
                    </div>
                    <textarea required rows="6" placeholder=t("Answer")
                        class="rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                        prop:value=move || form.get()["answer"].as_str().unwrap_or("").to_string()
                        on:input=move |ev| form.update(|f| f["answer"] = json!(event_target_value(&ev)))/>
                    <div class="flex gap-2">
                        <button type="submit" class="btn-primary !px-4 !py-2 text-sm">{t("Save")}</button>
                        <button type="button" class="rounded-lg border border-white/10 px-4 py-2 text-sm text-white/70"
                            on:click=move |_| show_form.set(false)>{t("Cancel")}</button>
                    </div>
                </form>
            </Show>
            <div class="card">
                {move || items.get().into_iter().map(|fq| {
                    let fid = fq["id"].as_i64().unwrap_or(0);
                    let locale = fq["locale"].as_str().unwrap_or("").to_string();
                    let question = fq["question"].as_str().unwrap_or("").to_string();
                    let efq = fq.clone();
                    view! {
                        <div class="flex items-center justify-between border-b border-white/5 px-4 py-3 last:border-0">
                            <div>
                                <span class="mr-2 rounded bg-white/10 px-1.5 py-0.5 text-xs text-white/60">{locale}</span>
                                <span class="text-white">{question}</span>
                            </div>
                            <div class="flex gap-1">
                                <button type="button" class="rounded px-2 py-1 text-xs text-white/60 hover:bg-white/10"
                                    on:click=move |_| {
                                        form.set(json!({
                                            "locale": efq["locale"].as_str().unwrap_or("en"),
                                            "question": efq["question"].as_str().unwrap_or(""),
                                            "answer": efq["answer"].as_str().unwrap_or(""),
                                        }));
                                        editing.set(Some(fid)); show_form.set(true);
                                    }>{t("Edit")}</button>
                                <button type="button" class="rounded px-2 py-1 text-xs text-red-400 hover:bg-red-500/10"
                                    on:click=move |_| leptos::task::spawn_local(async move {
                                        let _ = api::delete::<Value>(&format!("/api/v1/admin/faqs/{fid}")).await; load();
                                    })>{t("Delete")}</button>
                            </div>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </AdminShell>
    }
}
