use crate::api;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use serde_json::Value;

fn room_href(r: &Value, suffix: &str) -> String {
    let slug = r["slug"].as_str().unwrap_or("");
    let id = r["id"].as_i64().unwrap_or(0).to_string();
    let key = if slug.is_empty() { id } else { slug.to_string() };
    format!("/rooms/{key}{suffix}")
}

fn short_date(v: &Value) -> String {
    v.as_str()
        .map(|s| s.chars().take(10).collect())
        .unwrap_or_default()
}

#[component]
fn ProRoomCard(room: Value) -> impl IntoView {
    let name = room["name"].as_str().unwrap_or("").to_string();
    let venue = room["venue_name"].as_str().unwrap_or("").to_string();
    let href_edit = room_href(&room, "/edit");
    let href_live = room_href(&room, "/live");
    let href_show = room_href(&room, "");
    let playlists_n = room["playlists_count"].as_i64().unwrap_or(0);
    let players_n = room["players_count"].as_i64().unwrap_or(0);
    let rounds_n = room["rounds_count"].as_i64().unwrap_or(0);
    let last = short_date(&room["last_round_at"]);
    let is_active = room["is_active"].as_bool().unwrap_or(false);
    let state_cls = if is_active {
        "shrink-0 rounded bg-emerald-500/15 px-2 py-0.5 text-[10px] font-semibold uppercase text-emerald-300"
    } else {
        "shrink-0 rounded bg-white/10 px-2 py-0.5 text-[10px] font-semibold uppercase text-white/50"
    };
    let state_label = if is_active { t("Active") } else { t("Inactive") };
    view! {
        <div class="rounded-2xl border border-white/10 bg-surface-raised/60 p-5">
            <div class="flex items-start justify-between gap-2">
                <div class="min-w-0">
                    <p class="truncate font-bold text-white">{name}</p>
                    {(!venue.is_empty()).then(|| view! {
                        <p class="mt-0.5 truncate text-xs text-purple-300">{"📍 "}{venue}</p>
                    })}
                </div>
                <span class=state_cls>{state_label}</span>
            </div>
            <div class="mt-3 grid grid-cols-3 gap-2 text-center">
                <div class="rounded-lg bg-white/5 py-2">
                    <p class="text-lg font-bold text-white">{playlists_n}</p>
                    <p class="text-[10px] uppercase text-white/40">{t("Playlists")}</p>
                </div>
                <div class="rounded-lg bg-white/5 py-2">
                    <p class="text-lg font-bold text-white">{players_n}</p>
                    <p class="text-[10px] uppercase text-white/40">{t("Players")}</p>
                </div>
                <div class="rounded-lg bg-white/5 py-2">
                    <p class="text-lg font-bold text-white">{rounds_n}</p>
                    <p class="text-[10px] uppercase text-white/40">{t("Rounds")}</p>
                </div>
            </div>
            <p class="mt-2 text-xs text-white/40">
                {t("Last activity")}": "{last}
            </p>
            <div class="mt-3 flex gap-2">
                <A href=href_edit attr:class="flex-1 rounded-lg border border-white/15 px-3 py-1.5 text-center text-xs font-semibold text-white/80 transition hover:bg-white/10">{t("Edit")}</A>
                <A href=href_live attr:class="flex-1 rounded-lg bg-purple-500/70 px-3 py-1.5 text-center text-xs font-semibold text-white transition hover:bg-purple-400">{t("TV mode")}</A>
                <A href=href_show attr:class="rounded-lg border border-white/15 px-3 py-1.5 text-xs font-semibold text-white/60 transition hover:bg-white/10" attr:title=t("View room")>"👁"</A>
            </div>
        </div>
    }
}

#[component]
fn SetupChecklist(has_rooms: bool, has_branding: bool, has_playlists: bool, edit_href: String, live_href: String) -> impl IntoView {
    let steps = [
        ("Create your event room", "Private room for your bar or event", has_rooms, "/rooms/create".to_string(), "Create"),
        ("Name your venue", "Shown above the room title on the TV screen", has_branding, edit_href.clone(), "Edit"),
        ("Attach your playlists", "Music catalog for the quiz rounds", has_playlists, edit_href, "Edit"),
        ("Open the TV mode", "Full-screen display for the room screen", false, live_href.clone(), "Open"),
        ("Share the QR code", "Players join from their phones, no app needed", false, live_href, "Open"),
    ];
    view! {
        <section class="rounded-2xl border border-purple-400/20 bg-gradient-to-br from-purple-500/10 to-transparent p-5 sm:p-6">
            <h2 class="text-lg font-bold text-purple-200">{t("Room setup checklist")}</h2>
            <p class="mt-1 text-sm text-white/50">
                {t("Five steps to a branded quiz night. A TV or projector plus your players' phones is all you need.")}
            </p>
            <ol class="mt-4 space-y-3">
                {steps
                    .into_iter()
                    .map(|(title, desc, done, href, cta)| {
                        let dot_cls = if done {
                            "mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-emerald-500/20 text-emerald-300"
                        } else {
                            "mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-white/10 text-white/50"
                        };
                        let title_cls = if done {
                            "text-sm font-semibold text-white/80 line-through decoration-white/30"
                        } else {
                            "text-sm font-semibold text-white"
                        };
                        let mark = if done { "✓" } else { "•" };
                        view! {
                            <li class="flex items-start gap-3 rounded-xl border border-white/10 bg-white/5 p-3">
                                <span class=dot_cls>{mark}</span>
                                <div class="min-w-0 flex-1">
                                    <p class=title_cls>{t(title)}</p>
                                    <p class="text-xs text-white/50">{t(desc)}</p>
                                </div>
                                <A href=href attr:class="shrink-0 rounded-lg border border-purple-400/30 px-3 py-1.5 text-xs font-semibold text-purple-200 transition hover:bg-purple-400/10">
                                    {t(cta)}
                                </A>
                            </li>
                        }
                    })
                    .collect_view()}
            </ol>
        </section>
    }
}

#[component]
fn ProDashboard(data: RwSignal<Value>) -> impl IntoView {
    let plan = move || data.get()["plan"].as_str().unwrap_or("").to_string();
    let is_admin = move || data.get()["is_admin"].as_bool().unwrap_or(false);
    let is_pro = move || plan() == "pro" || is_admin();
    let rooms = move || {
        data.get()["rooms"]
            .as_array()
            .cloned()
            .unwrap_or_default()
    };
    let first_room = move || rooms().into_iter().next();
    let has_rooms = move || !rooms().is_empty();
    let has_branding = move || {
        rooms()
            .iter()
            .any(|r| r["venue_name"].as_str().map(|s| !s.is_empty()).unwrap_or(false))
    };
    let has_playlists = move || {
        rooms()
            .iter()
            .any(|r| r["playlists_count"].as_i64().unwrap_or(0) > 0)
    };
    let edit_href = move || {
        first_room()
            .map(|r| room_href(&r, "/edit"))
            .unwrap_or_else(|| "/rooms/create".to_string())
    };
    let live_href = move || {
        first_room()
            .map(|r| room_href(&r, "/live"))
            .unwrap_or_else(|| "/rooms/create".to_string())
    };

    view! {
        {move || {
            if is_pro() {
                view! {
                    <div class="space-y-6">
                        <SetupChecklist
                            has_rooms=has_rooms()
                            has_branding=has_branding()
                            has_playlists=has_playlists()
                            edit_href=edit_href()
                            live_href=live_href()/>

                        <section>
                            <div class="mb-3 flex items-center justify-between">
                                <h2 class="text-lg font-bold text-white">{t("Your rooms")}</h2>
                                <A href="/rooms/create" attr:class="rounded-lg bg-purple-500/80 px-4 py-2 text-sm font-semibold text-white transition hover:bg-purple-400">
                                    {t("New event room")}
                                </A>
                            </div>
                            {move || {
                                if rooms().is_empty() {
                                    return view! {
                                        <div class="rounded-xl border border-white/10 bg-surface-raised/60 p-6 text-center text-white/50">
                                            {t("No room yet. Create your first event room to get started.")}
                                        </div>
                                    }
                                    .into_any();
                                }
                                view! {
                                    <div class="grid gap-4 sm:grid-cols-2">
                                        <For
                                            each=rooms
                                            key=|r| r["id"].as_i64().unwrap_or(0)
                                            children=move |r: Value| view! { <ProRoomCard room=r /> }/>
                                    </div>
                                }
                                .into_any()
                            }}
                        </section>

                        <section class="rounded-2xl border border-white/10 bg-surface-raised/60 p-5">
                            <h2 class="text-base font-bold text-white">{t("Need a hand?")}</h2>
                            <p class="mt-1 text-sm text-white/50">
                                {t("Pro subscribers get priority support for room setup and event nights.")}
                            </p>
                            <A href="/contact" attr:class="mt-3 inline-block rounded-lg border border-white/15 px-4 py-2 text-sm font-semibold text-white/80 transition hover:bg-white/10">
                                {t("Contact support")}
                            </A>
                        </section>
                    </div>
                }
                .into_any()
            } else {
                view! {
                    <div class="space-y-6">
                        <section class="rounded-2xl border border-purple-400/25 bg-gradient-to-br from-purple-500/10 to-transparent p-6">
                            <h2 class="text-xl font-bold text-purple-200">{t("What Pro adds for your venue")}</h2>
                            <ul class="mt-4 space-y-3 text-sm text-white/75">
                                <li class="flex gap-3"><span>"📍"</span><span>{t("Venue name and photo on the TV screen, your brand on every quiz night")}</span></li>
                                <li class="flex gap-3"><span>"🧭"</span><span>{t("This setup checklist and per-room stats: playlists, players, rounds, last activity")}</span></li>
                                <li class="flex gap-3"><span>"🎪"</span><span>{t("Multiple branded event rooms: weekly quiz, karaoke night, private parties")}</span></li>
                                <li class="flex gap-3"><span>"👑"</span><span>{t("Everything in Plus: no ads, daily revive, custom tag, supporter perks")}</span></li>
                                <li class="flex gap-3"><span>"🛟"</span><span>{t("Priority support for setup and event nights")}</span></li>
                            </ul>
                            <div class="mt-5 flex flex-wrap gap-3">
                                <A href="/premium" attr:class="btn-primary inline-block">{t("See Pro plans")}</A>
                                <A href="/bars" attr:class="inline-block rounded-lg border border-white/15 px-5 py-2.5 text-sm font-semibold text-white/80 transition hover:bg-white/10">{t("How it works for bars")}</A>
                            </div>
                            {move || {
                                if plan() == "plus" {
                                    return view! {
                                        <p class="mt-4 text-xs text-purple-200/70">
                                            {t("You already have Plus. Pro keeps every Plus perk and adds the venue branding and this event space.")}
                                        </p>
                                    }
                                    .into_any();
                                }
                                view! {
                                    <p class="mt-4 text-xs text-white/40">
                                        {t("Free plan limits: 2 rooms, 3 playlists, 50 tracks per playlist.")}
                                    </p>
                                }
                                .into_any()
                            }}
                        </section>
                        <section class="rounded-2xl border border-white/10 bg-surface-raised/60 p-5">
                            <h2 class="text-base font-bold text-white">{t("How a quiz night works")}</h2>
                            <ol class="mt-3 space-y-2 text-sm text-white/60">
                                <li>"1. " {t("Create a private room and attach your music playlists")}</li>
                                <li>"2. " {t("Open the TV mode on the bar screen, players join with the QR code")}</li>
                                <li>"3. " {t("Each round plays a track, everyone answers on their phone")}</li>
                                <li>"4. " {t("Scores update live, the room restarts for the next round automatically")}</li>
                            </ol>
                        </section>
                    </div>
                }
                .into_any()
            }
        }}
    }
}

#[component]
pub fn ProPage() -> impl IntoView {
    crate::i18n::set_title_key("Pro Space");
    crate::auth::require_registered(crate::auth::use_auth());
    let data = RwSignal::new(Value::Null);
    let loading = RwSignal::new(true);

    leptos::task::spawn_local(async move {
        if let Ok(v) = api::get::<Value>("/api/v1/pro/overview").await {
            data.set(v);
        }
        loading.set(false);
    });

    view! {
        <div class="mx-auto max-w-5xl px-4 py-8">
            <header class="mb-8">
                <h1 class="retro-title retro-title--white text-2xl sm:text-3xl">{t("Pro Space")}</h1>
                <p class="mt-1 text-sm text-white/50">
                    {t("Set up your bar or event room, then follow the stats.")}
                </p>
            </header>

            <Show
                when=move || !loading.get()
                fallback=move || view! { <div class="text-white/60">{t("Loading")}"…"</div> }
            >
                <ProDashboard data=data/>
            </Show>
        </div>
    }
}
