//! Teams — `/teams` (index + search + create) and `/teams/:id` (show with
//! member contributions + owner controls). Ports `Teams/Index|Create|Show`.

use crate::api;
use crate::auth::use_auth;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::{use_navigate, use_params_map};
use serde_json::{json, Value};

// ---- index ----------------------------------------------------------------------

#[component]
pub fn TeamsPage() -> impl IntoView {
    crate::i18n::set_title_key("Teams");
    let auth = use_auth();
    // `Route::resource('teams')` sits in `auth` + `not.guest` — anonymous
    // users are sent to `/login`, guest accounts get a 404.
    crate::auth::require_registered(auth);
    let navigate = StoredValue::new(use_navigate());
    let teams = RwSignal::new(Vec::<Value>::new());
    let total = RwSignal::new(0i64);
    let search = RwSignal::new(String::new());
    let page = RwSignal::new(1i64);
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);
    let show_create = RwSignal::new(false);
    let new_name = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    // `viewer` — PHP `auth.user` pending/declined request ids + team_id.
    let viewer = RwSignal::new(Value::Null);

    Effect::new(move |_| {
        let (s, p) = (search.get(), page.get());
        loading.set(true);
        leptos::task::spawn_local(async move {
            let url = format!("/api/v1/teams?search={}&page={p}",
                urlencoding(&s));
            match api::get::<Value>(&url).await {
                Ok(v) => {
                    teams.set(v["teams"].as_array().cloned().unwrap_or_default());
                    total.set(v["total"].as_i64().unwrap_or(0));
                    viewer.set(v["viewer"].clone());
                }
                Err(e) => error.set(Some(e.message)),
            }
            loading.set(false);
        });
    });

    let create = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        error.set(None);
        leptos::task::spawn_local(async move {
            let body = json!({"name": new_name.get_untracked()});
            match api::post::<Value, _>("/api/v1/teams", &body).await {
                Ok(v) => {
                    let id = v["team_id"].as_i64().unwrap_or(0);
                    navigate.get_value()(&format!("/teams/{id}"), Default::default());
                }
                Err(e) => {
                    error.set(Some(
                        e.first("name").or(e.first("team")).unwrap_or(e.message),
                    ));
                    busy.set(false);
                }
            }
        });
    };

    let last_page = move || (total.get() + 11) / 12;

    view! {
        <div class="mx-auto max-w-6xl px-4 py-8">
            // PHP `Teams/Index.vue` centered hero + tagline + CTAs.
            <div class="mx-auto mb-10 text-center">
                <h1 class="mb-4 bg-gradient-to-r from-white via-violet-100 to-fuchsia-200 bg-clip-text text-4xl font-black tracking-tight text-transparent sm:text-5xl md:text-6xl">
                    {t("Teams")}
                </h1>
                <p class="mx-auto max-w-xl text-base text-white/60 sm:text-lg">
                    {t("Join a team and share your scores with other members to skyrocket the scores!")}
                </p>
                <div class="mt-8 flex flex-wrap items-center justify-center gap-3">
                    {move || {
                        let u = auth.user.get();
                        match u {
                            Some(u) if !u.is_guest => {
                                if let Some(tid) = u.team_id {
                                    view! {
                                        <A href=format!("/teams/{tid}") attr:class="inline-flex items-center gap-2 rounded-xl border border-violet-400/40 bg-violet-500/10 px-6 py-3 text-sm font-bold text-violet-100 transition hover:border-violet-300/60 hover:bg-violet-500/20">
                                            {t("Show my team")}
                                        </A>
                                    }.into_any()
                                } else {
                                    view! {
                                        <button type="button"
                                            class="inline-flex items-center gap-2 rounded-xl bg-gradient-to-r from-violet-600 to-fuchsia-600 px-6 py-3 text-sm font-bold text-white shadow-lg shadow-violet-900/40 transition hover:from-violet-500 hover:to-fuchsia-500"
                                            on:click=move |_| show_create.update(|v| *v = !*v)>
                                            "+ " {t("Create a team")}
                                        </button>
                                    }.into_any()
                                }
                            }
                            _ => view! {}.into_any(),
                        }
                    }}
                    <A href="/rankings" attr:class="inline-flex items-center gap-2 rounded-xl border border-white/10 bg-white/5 px-5 py-3 text-sm font-semibold text-white/80 transition hover:bg-white/10">
                        {t("Team rankings")}
                    </A>
                </div>
            </div>

            // PHP search — centered max-w-xl.
            <div class="mx-auto mb-10 max-w-xl">
                <input type="text" placeholder={t("Search a team")}
                    class="form-input w-full rounded-xl border-white/10 bg-surface-base/80 py-3 pl-11 text-white placeholder-white/40"
                    prop:value=search
                    on:input=move |ev| { search.set(event_target_value(&ev)); page.set(1); }/>
            </div>

            {move || error.get().map(|e| view! {
                <div class="mb-4 rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">{e}</div>
            })}

            <Show when=move || show_create.get()>
                <form on:submit=create
                    class="mb-6 flex gap-3 rounded-xl border border-white/10 bg-surface-raised/60 p-4">
                    <input type="text" maxlength="30" required placeholder={t("Team name")}
                        class="flex-1 rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none focus:border-brand-accent/50"
                        prop:value=new_name
                        on:input=move |ev| new_name.set(event_target_value(&ev))/>
                    <button type="submit" class="btn-primary" disabled=move || busy.get()>
                        {t("Create")}
                    </button>
                </form>
            </Show>

            <Show
                when=move || !loading.get()
                fallback=move || view! { <div class="text-white/60">{t("Loading")}"…"</div> }
            >
                <Show
                    when=move || !teams.get().is_empty()
                    fallback=move || view! {
                        <div class="rounded-xl border border-white/10 bg-surface-raised/60 p-8 text-center text-white/60">
                            {t("No teams yet")}
                        </div>
                    }
                >
                    // PHP `Teams/Index.vue` card grid — photo, seats badge,
                    // Squad score / Rounds together boxes, join button states.
                    <div class="grid grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
                        <For
                            each=move || teams.get()
                            key=|tm| tm["id"].as_i64().unwrap_or(0)
                            children=move |tm: Value| {
                                let id = tm["id"].as_i64().unwrap_or(0);
                                let name = tm["name"].as_str().unwrap_or("").to_string();
                                let photo = tm["photo"].as_str().unwrap_or("").to_string();
                                let owner = tm["owner_name"].as_str().unwrap_or("").to_string();
                                let members = tm["members_count"].as_i64().unwrap_or(0);
                                let seats = tm["seats"].as_i64().unwrap_or(8);
                                let full = members >= seats;
                                let rounds = tm["rounds_played"].as_i64().unwrap_or(0);
                                let points_abbr = tm["team_points_abbreviated"].as_str().unwrap_or("0").to_string();
                                let points_full = format!("{:.0}", tm["team_points"].as_f64().unwrap_or(0.0));
                                let refetch = move |post_path: Option<String>| {
                                    let (s, p) = (search.get_untracked(), page.get_untracked());
                                    leptos::task::spawn_local(async move {
                                        if let Some(path) = post_path {
                                            let _ = api::post_empty::<Value>(&path).await;
                                        }
                                        if let Ok(v) = api::get::<Value>(
                                            &format!("/api/v1/teams?search={}&page={p}", urlencoding(&s))
                                        ).await {
                                            teams.set(v["teams"].as_array().cloned().unwrap_or_default());
                                            viewer.set(v["viewer"].clone());
                                        }
                                    });
                                };
                                let send_request = move |ev: leptos::ev::MouseEvent| {
                                    ev.prevent_default();
                                    ev.stop_propagation();
                                    refetch(Some(format!("/api/v1/teams/{id}/request")));
                                };
                                let cancel_request = move |ev: leptos::ev::MouseEvent| {
                                    ev.prevent_default();
                                    ev.stop_propagation();
                                    refetch(Some(format!("/api/v1/teams/{id}/request/cancel")));
                                };
                                view! {
                                    <article class="group relative flex flex-col overflow-hidden rounded-2xl border border-white/10 bg-surface-raised/60 shadow-lg transition duration-300 hover:border-violet-500/35">
                                        <div class="pointer-events-none absolute right-3 top-3 z-10 flex items-center gap-1 rounded-full border border-amber-500/30 bg-amber-500/10 px-2.5 py-1 text-xs font-bold text-amber-200">
                                            <span class="opacity-80">{members}</span>
                                            <span class="text-white/40">"/"</span>
                                            <span>{seats}</span>
                                        </div>
                                        <A href=format!("/teams/{id}")
                                            attr:class="flex flex-1 flex-col items-center px-5 pb-4 pt-8 text-left outline-none transition hover:brightness-105">
                                            <div class="relative mb-4">
                                                <div class="absolute inset-0 rounded-full bg-gradient-to-br from-violet-500/40 to-fuchsia-500/30 opacity-0 blur-xl transition group-hover:opacity-100"></div>
                                                <img class="relative h-28 w-28 rounded-full object-cover ring-2 ring-violet-500/40 ring-offset-2 ring-offset-surface-base transition group-hover:ring-fuchsia-400/50 sm:h-32 sm:w-32"
                                                    src=photo alt=name.clone()/>
                                            </div>
                                            <h3 class="mb-1 text-center text-xl font-bold text-white transition group-hover:text-violet-200">
                                                {name}
                                            </h3>
                                            <p class="mb-4 text-center text-sm text-white/50">"@"{owner}</p>
                                            <div class="grid w-full grid-cols-2 gap-2 text-center">
                                                <div class="relative overflow-hidden rounded-xl border border-violet-500/25 bg-gradient-to-b from-violet-500/15 to-transparent px-2 py-3">
                                                    <div class="text-[10px] font-bold uppercase tracking-wider text-violet-300/90">
                                                        {t("Squad score")}
                                                    </div>
                                                    <div class="mt-1 flex flex-wrap items-baseline justify-center gap-0.5" title=format!("{} {}", points_full, t("PTS"))>
                                                        <span class="bg-gradient-to-r from-violet-200 via-fuchsia-200 to-violet-200 bg-clip-text text-2xl font-black tabular-nums tracking-tight text-transparent">
                                                            {points_abbr}
                                                        </span>
                                                        <span class="text-xs font-bold uppercase text-violet-400/90">{t("PTS")}</span>
                                                    </div>
                                                </div>
                                                <div class="rounded-xl border border-white/10 bg-white/5 px-2 py-2">
                                                    <div class="text-[10px] font-semibold uppercase tracking-wider text-white/40">
                                                        {t("Rounds together")}
                                                    </div>
                                                    <div class="text-lg font-bold tabular-nums text-fuchsia-200">
                                                        {rounds}
                                                    </div>
                                                </div>
                                            </div>
                                        </A>
                                        <div class="mt-auto w-full px-5 pb-5">
                                            {move || {
                                                let v = viewer.get();
                                                if v.is_null() || v["is_guest"].as_bool().unwrap_or(false)
                                                    || v["team_id"].as_i64().is_some() {
                                                    return view! {}.into_any();
                                                }
                                                let declined = v["declined_requests"].as_array().cloned().unwrap_or_default()
                                                    .iter().any(|x| x.as_i64() == Some(id));
                                                let pending = v["pending_requests"].as_array().cloned().unwrap_or_default()
                                                    .iter().any(|x| x.as_i64() == Some(id));
                                                if declined {
                                                    view! {
                                                        <button type="button" class="w-full rounded-xl border border-red-500/40 bg-red-500/10 px-4 py-2.5 text-sm font-semibold text-red-300 transition hover:bg-red-500/20"
                                                            on:click=cancel_request>
                                                            {t("Declined request")}
                                                        </button>
                                                    }.into_any()
                                                } else if pending {
                                                    view! {
                                                        <button type="button" class="w-full rounded-xl border border-amber-500/40 bg-amber-500/10 px-4 py-2.5 text-sm font-semibold text-amber-200 transition hover:bg-amber-500/20"
                                                            on:click=cancel_request>
                                                            {t("Cancel join request")}
                                                        </button>
                                                    }.into_any()
                                                } else if full {
                                                    view! {
                                                        <button type="button" disabled class="w-full cursor-not-allowed rounded-xl border border-white/10 bg-white/5 px-4 py-2.5 text-sm font-semibold text-white/40"
                                                            title=t("It is not possible to join this team. The maximum number of members has been reached")>
                                                            {t("Team is full")}
                                                        </button>
                                                    }.into_any()
                                                } else {
                                                    view! {
                                                        <button type="button" class="w-full rounded-xl bg-gradient-to-r from-violet-600/90 to-fuchsia-600/90 px-4 py-2.5 text-sm font-bold text-white shadow-md transition hover:from-violet-500 hover:to-fuchsia-500"
                                                            on:click=send_request>
                                                            {t("Send a join request")}
                                                        </button>
                                                    }.into_any()
                                                }
                                            }}
                                        </div>
                                    </article>
                                }
                            }
                        />
                    </div>
                    <Show when=move || { last_page() > 1 }>
                        <div class="mt-6 flex items-center justify-center gap-2 text-sm text-white/60">
                            <button type="button" class="rounded border border-white/10 px-3 py-1 disabled:opacity-30"
                                disabled=move || { page.get() <= 1 }
                                on:click=move |_| page.update(|p| *p -= 1)>"←"</button>
                            <span class="px-2 py-1">{move || format!("{} / {}", page.get(), last_page())}</span>
                            <button type="button" class="rounded border border-white/10 px-3 py-1 disabled:opacity-30"
                                disabled=move || { page.get() >= last_page() }
                                on:click=move |_| page.update(|p| *p += 1)>"→"</button>
                        </div>
                    </Show>
                </Show>
            </Show>
        </div>
    }
}

// ---- show ------------------------------------------------------------------------

#[component]
pub fn TeamShowPage() -> impl IntoView {
    crate::auth::require_registered(crate::auth::use_auth());
    let params = use_params_map();
    let navigate = StoredValue::new(use_navigate());
    let data = RwSignal::new(Value::Null);
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);
    let editing_name = RwSignal::new(String::new());
    let photo_file = RwSignal::new(Option::<web_sys::File>::None);
    let saving = RwSignal::new(false);

    let team_id = move || params.get().get("id").unwrap_or_default();

    let load = move || {
        let id = team_id();
        leptos::task::spawn_local(async move {
            match api::get::<Value>(&format!("/api/v1/teams/{id}")).await {
                Ok(v) => {
                    let name = v["team"]["name"].as_str().unwrap_or("").to_string();
                    crate::i18n::set_title(Some(&name));
                    editing_name.set(name);
                    data.set(v);
                }
                Err(e) => error.set(Some(e.message)),
            }
            loading.set(false);
        });
    };
    load();

    let act = move |path: String, then: Option<Box<dyn Fn() + Send + Sync>>| {
        error.set(None);
        leptos::task::spawn_local(async move {
            match api::post_empty::<Value>(&path).await {
                Ok(_) => {
                    if let Some(f) = then {
                        f();
                    }
                }
                Err(e) => error.set(Some(e.first("team").unwrap_or(e.message))),
            }
        });
    };

    // PHP `formatNumber` — ≥1000 locale-grouped, else one decimal.
    fn fmt_num(n: f64) -> String {
        if n >= 1000.0 {
            let s = format!("{:.1}", n);
            let (i, d) = s.split_once('.').unwrap_or((&s, ""));
            let mut grouped = String::new();
            for (k, c) in i.chars().rev().enumerate() {
                if k > 0 && k % 3 == 0 {
                    grouped.push(' ');
                }
                grouped.push(c);
            }
            let int_part: String = grouped.chars().rev().collect();
            if d == "0" { int_part } else { format!("{int_part},{d}") }
        } else {
            format!("{}", (n * 10.0).round() / 10.0)
        }
    }

    // PHP `formatDate` — `toLocaleString(locale, {dateStyle:'medium',timeStyle:'short'})`.
    fn fmt_date(iso: &str) -> Option<String> {
        if iso.is_empty() {
            return None;
        }
        let d = js_sys::Date::new(&iso.into());
        if d.get_time().is_nan() {
            return Some(iso.to_string());
        }
        let opts = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&opts, &"dateStyle".into(), &"medium".into());
        let _ = js_sys::Reflect::set(&opts, &"timeStyle".into(), &"short".into());
        Some(d.to_locale_string(crate::i18n::locale(), &opts).into())
    }

    view! {
        <div class="mx-auto max-w-4xl pb-16 pt-4 md:pt-8">
            <Show
                when=move || !loading.get()
                fallback=move || view! { <div class="px-4 text-white/60">{t("Loading")}"…"</div> }
            >
                {move || {
                    let d = data.get();
                    if d.is_null() {
                        return view! {
                            <div class="py-24 text-center text-white/60">{t("Team not found")}</div>
                        }.into_any();
                    }
                    let team = d["team"].clone();
                    let tid = team["id"].as_i64().unwrap_or(0);
                    let owner_id = team["user_id"].as_i64().unwrap_or(0);
                    let owner = team["owner"].clone();
                    let seats = team["seats"].as_i64().unwrap_or(8);
                    let viewer = d["viewer"].clone();
                    let my_id = viewer["id"].as_i64().unwrap_or(0);
                    let is_owner = viewer["is_owner"].as_bool().unwrap_or(false);
                    let my_team = viewer["is_member"].as_bool().unwrap_or(false);
                    let requested = viewer["has_pending_request"].as_bool().unwrap_or(false);
                    let declined = viewer["has_declined_request"].as_bool().unwrap_or(false);
                    // PHP shows `Send a join request` to any logged-in
                    // non-member (even one already in a team).
                    let logged_in = viewer["logged_in"].as_bool().unwrap_or(false);
                    let can_pic = viewer["can_change_team_picture"].as_bool().unwrap_or(false);
                    let members = d["members"].as_array().cloned().unwrap_or_default();
                    let stats = d["stats"].clone();
                    let score = d["score"].as_f64().unwrap_or(0.0);
                    let single_member = members.len() == 1;
                    let team_full = members.len() as i64 >= seats;
                    let last_played = fmt_date(stats["last_played_at"].as_str().unwrap_or(""));

                    view! {
                        <div>
                            // ← "All squads"
                            <div class="mb-6 px-4">
                                <A href="/teams" attr:class="inline-flex items-center gap-2 text-sm font-medium text-violet-300/90 transition hover:text-violet-200">
                                    <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" viewBox="0 0 20 20" fill="currentColor">
                                        <path fill-rule="evenodd" d="M9.707 16.707a1 1 0 01-1.414 0l-6-6a1 1 0 010-1.414l6-6a1 1 0 011.414 1.414L5.414 9H17a1 1 0 110 2H5.414l4.293 4.293a1 1 0 010 1.414z" clip-rule="evenodd"/>
                                    </svg>
                                    {t("All squads")}
                                </A>
                            </div>

                            // Hero — violet/fuchsia gradient card.
                            <div class="relative mx-4 mb-8 overflow-hidden rounded-2xl border border-violet-500/25 bg-gradient-to-br from-violet-950/70 via-neutral-900 to-fuchsia-950/40 px-6 py-8 shadow-[0_0_48px_-12px_rgba(139,92,246,0.3)] sm:px-10 sm:py-10">
                                <div class="pointer-events-none absolute -right-16 -top-16 h-48 w-48 rounded-full bg-fuchsia-500/10 blur-3xl"></div>
                                <div class="relative flex flex-col gap-6 sm:flex-row sm:items-center sm:gap-10">
                                    <div class="relative mx-auto shrink-0 sm:mx-0">
                                        <div class="absolute inset-0 rounded-full bg-gradient-to-br from-violet-500 to-fuchsia-500 opacity-60 blur-lg"></div>
                                        <img src=team["photo"].as_str().unwrap_or("").to_string()
                                            alt=team["name"].as_str().unwrap_or("").to_string()
                                            class="relative h-28 w-28 rounded-full object-cover ring-4 ring-violet-400/40 ring-offset-4 ring-offset-neutral-950 sm:h-36 sm:w-36"/>
                                    </div>
                                    <div class="min-w-0 flex-1 text-center sm:text-left">
                                        <div class="mb-1 flex flex-wrap items-center justify-center gap-2 sm:justify-start">
                                            <span class="rounded-full border border-amber-400/35 bg-amber-400/10 px-2.5 py-0.5 text-xs font-bold uppercase tracking-wide text-amber-200">
                                                {t("Squad")}
                                            </span>
                                            <A href="/rankings?tab=teams" attr:class="text-xs font-semibold text-violet-300/90 underline-offset-2 hover:text-violet-200 hover:underline">
                                                {t("Team rankings")} " →"
                                            </A>
                                        </div>
                                        <h1 class="mb-2 text-3xl font-black tracking-tight text-white sm:text-4xl">
                                            {team["name"].as_str().unwrap_or("").to_string()}
                                        </h1>
                                        <p class="mb-4 text-sm text-neutral-400">
                                            {t("Captain")} ": "
                                            {owner["id"].as_i64().map(|oid| view! {
                                                <A href=format!("/profile/{oid}") attr:class="font-semibold text-violet-300 hover:text-violet-200">
                                                    "@" {owner["name"].as_str().unwrap_or("").to_string()}
                                                </A>
                                            })}
                                        </p>
                                        <div class="flex flex-wrap items-end justify-center gap-2 sm:justify-start">
                                            <span class="text-4xl font-black tabular-nums tracking-tight text-white sm:text-5xl">
                                                {fmt_num(score)}
                                            </span>
                                            <span class="mb-1.5 text-lg font-bold text-violet-300/80">{t("PTS")}</span>
                                            <span class="mb-2 ml-2 text-sm text-neutral-500">{t("Squad score")}</span>
                                        </div>
                                    </div>
                                </div>
                            </div>

                            // Stats — 3 cards.
                            <div class="mx-4 mb-8 grid grid-cols-1 gap-4 sm:grid-cols-3">
                                <div class="rounded-xl border border-neutral-700/70 bg-neutral-900/50 px-4 py-4 text-center sm:text-left">
                                    <div class="text-[10px] font-bold uppercase tracking-wider text-neutral-500">{t("Rounds played")}</div>
                                    <div class="mt-1 text-2xl font-black tabular-nums text-fuchsia-200">
                                        {stats["rounds_played"].as_i64().unwrap_or(0)}
                                    </div>
                                </div>
                                <div class="rounded-xl border border-neutral-700/70 bg-neutral-900/50 px-4 py-4 text-center sm:text-left">
                                    <div class="text-[10px] font-bold uppercase tracking-wider text-neutral-500">{t("Avg. per round")}</div>
                                    <div class="mt-1 text-2xl font-black tabular-nums text-violet-200">
                                        {fmt_num(stats["avg_points_per_round"].as_f64().unwrap_or(0.0))}
                                        <span class="text-sm font-normal text-neutral-500">" " {t("PTS")}</span>
                                    </div>
                                </div>
                                <div class="rounded-xl border border-neutral-700/70 bg-neutral-900/50 px-4 py-4 text-center sm:text-left">
                                    <div class="text-[10px] font-bold uppercase tracking-wider text-neutral-500">{t("Last round")}</div>
                                    <div class="mt-1 text-sm font-semibold text-neutral-200">
                                        {last_played.unwrap_or_else(|| t("No finished rounds yet").to_string())}
                                    </div>
                                </div>
                            </div>

                            {move || error.get().map(|e| view! {
                                <div class="mx-4 mb-4 rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">{e}</div>
                            })}

                            // Manage squad — owner only (`user.id === team.user_id`).
                            {is_owner.then(|| {
                                let photo_input = NodeRef::<leptos::html::Input>::new();
                                view! {
                                    <div class="mx-4 mb-8 rounded-xl border border-white/10 bg-surface-raised/60 p-4">
                                        <div class="mb-4 text-sm font-bold text-neutral-200">{t("Manage squad")}</div>
                                        <form on:submit=move |ev| {
                                            ev.prevent_default();
                                            if saving.get() { return; }
                                            saving.set(true);
                                            error.set(None);
                                            let name = editing_name.get_untracked();
                                            let file = photo_file.get_untracked();
                                            leptos::task::spawn_local(async move {
                                                let ok = api::put::<Value, _>(
                                                    &format!("/api/v1/teams/{tid}"),
                                                    &json!({"name": name}),
                                                ).await.map_err(|e| error.set(Some(e.first("name").unwrap_or(e.message)))).is_ok();
                                                if ok {
                                                    if let Some(f) = file {
                                                        let form = web_sys::FormData::new().unwrap();
                                                        let _ = form.append_with_blob("photo", &f);
                                                        let token = api::stored_token().unwrap_or_default();
                                                        let _ = gloo_net::http::Request::post(&format!("/api/v1/teams/{tid}/photo"))
                                                            .header("Authorization", &format!("Bearer {token}"))
                                                            .body(form).unwrap()
                                                            .send().await;
                                                    }
                                                    photo_file.set(None);
                                                    load();
                                                }
                                                saving.set(false);
                                            });
                                        }>
                                            <label class="mb-1 block text-xs font-medium text-neutral-400">{t("Name")}</label>
                                            <input type="text" maxlength="30" required
                                                class="mb-4 w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none"
                                                prop:value=editing_name
                                                on:input=move |ev| editing_name.set(event_target_value(&ev))/>
                                            {can_pic.then(|| view! {
                                                <div class="mb-4">
                                                    <label class="mb-1 block text-xs font-medium text-neutral-400">{t("Image")}</label>
                                                    <input type="file" accept="image/*" node_ref=photo_input
                                                        class="w-full text-sm text-neutral-400 file:mr-3 file:rounded-lg file:border-0 file:bg-violet-600 file:px-3 file:py-1.5 file:text-xs file:font-semibold file:text-white"
                                                        on:change=move |_| {
                                                            photo_file.set(photo_input.get().and_then(|i| i.files()).and_then(|f| f.get(0)));
                                                        }/>
                                                </div>
                                            })}
                                            {(!can_pic).then(|| view! {
                                                <p class="mb-4 rounded-lg border border-amber-400/20 bg-amber-400/5 p-3 text-xs text-amber-200/80">
                                                    {t("In order to change team picture, you need to have a minimum of three months of seniority and a total score above two thousand")}
                                                    <sup>{t("PTS")}</sup> "."
                                                </p>
                                            })}
                                            <button type="submit" class="btn-primary ml-auto block" disabled=move || saving.get()>
                                                {t("Update")}
                                            </button>
                                        </form>
                                    </div>
                                }
                            })}

                            // Squad roster card.
                            <div class="mx-4 rounded-xl border border-white/10 bg-surface-raised/60">
                                <div class="flex w-full flex-wrap items-center justify-between gap-2 border-b border-white/10 px-4 py-3">
                                    <span class="text-sm font-bold text-neutral-200">{t("Squad roster")}</span>
                                    <span class="rounded-full border border-neutral-600 bg-neutral-800/80 px-3 py-1 text-xs font-semibold text-neutral-300">
                                        {members.len()} " / " {seats}
                                    </span>
                                </div>
                                <ul class="flex flex-col gap-3 p-4">
                                    {members.into_iter().enumerate().map(|(index, m)| {
                                        let mid = m["id"].as_i64().unwrap_or(0);
                                        let is_row_owner = mid == owner_id;
                                        let is_me = mid == my_id;
                                        let pct = m["contribution_percent"].as_f64().unwrap_or(0.0);
                                        let photo = m["photo"].as_str().unwrap_or("").to_string();
                                        let name = m["name"].as_str().unwrap_or("").to_string();
                                        let initial = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_default();
                                        let rounds = m["rounds_played"].as_i64().unwrap_or(0);
                                        let mscore = m["score"].as_f64().unwrap_or(0.0);
                                        let medal = ["🥇", "🥈", "🥉"].get(index).copied();
                                        let row_style = match index {
                                            0 => "border-yellow-500/25 bg-gradient-to-r from-yellow-500/5 to-transparent",
                                            1 => "border-neutral-400/20 bg-gradient-to-r from-neutral-400/5 to-transparent",
                                            2 => "border-amber-700/25 bg-gradient-to-r from-amber-700/10 to-transparent",
                                            _ => "border-neutral-700/50 bg-neutral-900/40",
                                        };
                                        view! {
                                            <li class=format!("flex flex-col gap-3 rounded-xl border p-4 sm:flex-row sm:items-center {row_style}")>
                                                <div class="flex min-w-0 flex-1 items-center gap-3">
                                                    <div class="flex h-10 w-10 shrink-0 items-center justify-center text-xl font-black tabular-nums text-neutral-500">
                                                        {medal.map(|m| m.to_string()).unwrap_or_else(|| (index + 1).to_string())}
                                                    </div>
                                                    <A href=format!("/profile/{mid}") attr:class="flex min-w-0 items-center gap-3 transition hover:opacity-90">
                                                        {if photo.is_empty() {
                                                            view! {
                                                                <div class="flex h-11 w-11 shrink-0 items-center justify-center rounded-full bg-neutral-700 text-sm font-bold text-neutral-300">
                                                                    {initial}
                                                                </div>
                                                            }.into_any()
                                                        } else {
                                                            view! {
                                                                <img src=photo alt="" class="h-11 w-11 shrink-0 rounded-full object-cover ring-2 ring-neutral-600"/>
                                                            }.into_any()
                                                        }}
                                                        <div class="min-w-0">
                                                            <div class="flex flex-wrap items-center gap-2">
                                                                <span class="truncate font-bold text-white">{name}</span>
                                                                {is_me.then(|| view! {
                                                                    <span class="shrink-0 rounded bg-violet-500/20 px-1.5 py-0.5 text-[10px] font-bold uppercase text-violet-200">{t("You")}</span>
                                                                })}
                                                                {is_row_owner.then(|| view! {
                                                                    <span class="text-amber-400" title="Captain">"★"</span>
                                                                })}
                                                            </div>
                                                            <div class="mt-1 text-xs text-neutral-500">
                                                                {rounds} " " {t("Rounds played")} " · " {t("Share of squad points")} " " {pct} "%"
                                                            </div>
                                                        </div>
                                                    </A>
                                                </div>
                                                <div class="flex w-full flex-col gap-2 sm:w-48 sm:shrink-0">
                                                    <div class="h-2 overflow-hidden rounded-full bg-neutral-800">
                                                        <div class="h-full rounded-full bg-gradient-to-r from-violet-600 to-fuchsia-500 transition-all"
                                                            style=format!("width: {}%", pct.min(100.0))/>
                                                    </div>
                                                    <div class="flex items-center justify-between text-sm">
                                                        <span class="font-black tabular-nums text-violet-200">{fmt_num(mscore)} " " {t("PTS")}</span>
                                                        {(is_owner && !is_row_owner).then(|| view! {
                                                            <div class="flex items-center gap-1">
                                                                <button type="button" class="rounded p-1 text-amber-400/90 transition hover:bg-amber-500/10 hover:text-amber-300"
                                                                    title={t("Transfer captain")}
                                                                    on:click=move |_| act(
                                                                        format!("/api/v1/teams/{tid}/owner/{mid}"),
                                                                        Some(Box::new(move || load())))>
                                                                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="h-5 w-5">
                                                                        <path stroke-linecap="round" stroke-linejoin="round" d="M11.48 3.499a.562.562 0 011.04 0l2.125 5.111a.563.563 0 00.475.345l5.518.442c.499.04.701.663.321.988l-4.204 3.602a.563.563 0 00-.182.557l1.285 5.385a.562.562 0 01-.84.61l-4.725-2.885a.563.563 0 00-.586 0L6.982 20.54a.562.562 0 01-.84-.61l1.285-5.386a.562.562 0 00-.182-.557l-4.204-3.602a.563.563 0 01.321-.988l5.518-.442a.563.563 0 00.475-.345L11.48 3.5z"/>
                                                                    </svg>
                                                                </button>
                                                                <button type="button" class="rounded p-1 text-red-400 transition hover:bg-red-500/10 hover:text-red-300"
                                                                    title={t("Remove member")}
                                                                    on:click=move |_| {
                                                                        if web_sys::window().unwrap().confirm_with_message(&t("Do you really want to remove this member from the team?")).unwrap_or(false) {
                                                                            act(format!("/api/v1/teams/{tid}/members/{mid}/remove"),
                                                                                Some(Box::new(move || load())));
                                                                        }
                                                                    }>
                                                                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="h-5 w-5">
                                                                        <path stroke-linecap="round" stroke-linejoin="round" d="M22 10.5h-6m-2.25-4.125a3.375 3.375 0 11-6.75 0 3.375 3.375 0 016.75 0zM4 19.235v-.11a6.375 6.375 0 0112.75 0v.109A12.318 12.318 0 0110.374 21c-2.331 0-4.512-.645-6.374-1.766z"/>
                                                                    </svg>
                                                                </button>
                                                            </div>
                                                        })}
                                                    </div>
                                                </div>
                                            </li>
                                        }
                                    }).collect::<Vec<_>>()}
                                </ul>
                            </div>

                            // Footer actions — PHP order: Delete (solo owner) /
                            // Leave (member) / join-request states.
                            <div class="mx-4 mt-8 flex flex-wrap items-center gap-3">
                                {(is_owner && single_member).then(|| view! {
                                    <button type="button" class="rounded-lg bg-red-600 px-4 py-2 text-sm font-semibold text-white hover:bg-red-500"
                                        on:click=move |_| {
                                            if web_sys::window().unwrap().confirm_with_message(&t("Do you really want to delete this team?")).unwrap_or(false) {
                                                let nav = navigate.get_value();
                                                leptos::task::spawn_local(async move {
                                                    let _ = api::delete::<Value>(&format!("/api/v1/teams/{tid}")).await;
                                                    nav("/teams", Default::default());
                                                });
                                            }
                                        }>
                                        {t("Delete")} " " {t("The Team")}
                                    </button>
                                })}
                                {(my_team && !is_owner).then(|| view! {
                                    <button type="button" class="rounded-lg bg-red-600 px-4 py-2 text-sm font-semibold text-white hover:bg-red-500"
                                        on:click=move |_| {
                                            let nav = navigate.get_value();
                                            act(format!("/api/v1/teams/{tid}/leave"),
                                                Some(Box::new(move || nav("/teams", Default::default()))));
                                        }>
                                        {t("Leave the team")}
                                    </button>
                                })}
                                {(!my_team).then(|| view! {
                                    <div class="flex flex-wrap gap-3">
                                        {declined.then(|| view! {
                                            <button type="button" class="rounded-lg bg-red-600 px-4 py-2 text-sm font-semibold text-white hover:bg-red-500"
                                                on:click=move |_| act(
                                                    format!("/api/v1/teams/{tid}/request/cancel"),
                                                    Some(Box::new(move || load())))>
                                                {t("Declined request")}
                                            </button>
                                        })}
                                        {(!declined && requested).then(|| view! {
                                            <button type="button" class="rounded-lg bg-red-600 px-4 py-2 text-sm font-semibold text-white hover:bg-red-500"
                                                on:click=move |_| act(
                                                    format!("/api/v1/teams/{tid}/request/cancel"),
                                                    Some(Box::new(move || load())))>
                                                {t("Cancel join request")}
                                            </button>
                                        })}
                                        {(!declined && !requested && team_full).then(|| view! {
                                            <button type="button" disabled
                                                class="cursor-not-allowed rounded-lg border border-neutral-600 bg-neutral-800/80 px-4 py-2 text-sm font-semibold text-neutral-500"
                                                title={t("It is not possible to join this team. The maximum number of members has been reached")}>
                                                {t("Team is full")}
                                            </button>
                                        })}
                                        {(!declined && !requested && !team_full && logged_in).then(|| view! {
                                            <button type="button" class="rounded-lg border border-white/10 bg-white/5 px-4 py-2 text-sm font-semibold text-white hover:bg-white/10"
                                                on:click=move |_| act(
                                                    format!("/api/v1/teams/{tid}/request"),
                                                    Some(Box::new(move || load())))>
                                                {t("Send a join request")}
                                            </button>
                                        })}
                                    </div>
                                })}
                            </div>
                        </div>
                    }.into_any()
                }}
            </Show>
        </div>
    }
}

fn urlencoding(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || "-_.~".contains(c) { c.to_string() } else { format!("%{:02X}", c as u32) })
        .collect()
}
