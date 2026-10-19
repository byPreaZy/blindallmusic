use crate::api;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use serde_json::Value;

#[derive(Clone, Default)]
struct MineRoom {
    id: i64,
    name: String,
    description: Option<String>,
    photo_url: Option<String>,
    category: Option<String>,
    playlists: Vec<String>,
    moderators: Vec<String>,
    rounds_count: i64,
    is_autostart: bool,
    is_public: bool,
    has_password: bool,
    deleted: bool,
}

impl MineRoom {
    fn from_json(v: &Value) -> Self {
        Self {
            id: v["id"].as_i64().unwrap_or_default(),
            name: v["name"].as_str().unwrap_or_default().to_string(),
            description: v["description"].as_str().map(str::to_string),
            photo_url: v["photo_path"].as_str().map(|p| format!("/storage/{p}")),
            category: v["category"]["name"].as_str().map(str::to_string),
            playlists: v["playlists"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|p| p["name"].as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            moderators: v["moderators"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|m| m["name"].as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default(),
            rounds_count: v["rounds_count"].as_i64().unwrap_or_default(),
            is_autostart: v["is_autostart"].as_bool().unwrap_or(false),
            is_public: v["is_public"].as_bool().unwrap_or(false),
            has_password: v["has_password"].as_bool().unwrap_or(false),
            deleted: !v["deleted_at"].is_null(),
        }
    }
}

#[component]
pub fn RoomsIndexPage() -> impl IntoView {
    crate::i18n::set_title_key("Rooms");
    crate::auth::require_registered(crate::auth::use_auth());
    let rooms = RwSignal::new(Vec::<MineRoom>::new());
    let loading = RwSignal::new(true);
    let error = RwSignal::new(Option::<String>::None);
    let search = RwSignal::new(String::new());
    let trashed = RwSignal::new(String::new());

    Effect::new(move |_| {
        let search_v = search.get();
        leptos::task::spawn_local(async move {
            let mut url = "/api/v1/rooms/mine?trashed=with".to_string();
            if !search_v.is_empty() {
                url.push_str(&format!(
                    "&search={}",
                    js_sys::encode_uri_component(&search_v)
                ));
            }
            match api::get::<Value>(&url).await {
                Ok(v) => rooms.set(
                    v["rooms"]
                        .as_array()
                        .map(|a| a.iter().map(MineRoom::from_json).collect())
                        .unwrap_or_default(),
                ),
                Err(e) => error.set(Some(e.message)),
            }
            loading.set(false);
        });
    });

    let visible = move || {
        let mode = trashed.get();
        rooms
            .get()
            .into_iter()
            .filter(|r| match mode.as_str() {
                "with" => true,
                "only" => r.deleted,
                _ => !r.deleted,
            })
            .collect::<Vec<_>>()
    };

    view! {
        <div class="mx-auto max-w-6xl px-4 py-8">
            <div class="mb-8 flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
                <div>
                    <h1 class="retro-title retro-title--white text-2xl">{t("My Rooms")}</h1>
                    <p class="mt-1 text-sm text-white/50">
                        {move || format!("{} {}", visible().len(), t("rooms"))}
                    </p>
                </div>
                <A href="/rooms/create" attr:class="btn-primary !px-4 !py-2">
                    "+ " {t("Create a room")}
                </A>
            </div>

            <div class="mb-6 flex flex-col gap-3 rounded-xl border border-white/10 bg-surface-raised/60 p-4 sm:flex-row sm:items-center">
                <input
                    type="search"
                    class="w-full flex-1 rounded-lg border border-white/10 bg-black/30 px-3 py-2 text-sm text-white placeholder-white/40 focus:border-brand-accent focus:outline-none"
                    placeholder=move || format!("{}…", t("Search rooms"))
                    prop:value=search
                    on:input=move |ev| search.set(event_target_value(&ev))
                />
                <select
                    class="rounded-lg border border-white/10 bg-black/30 px-3 py-2 text-sm text-white focus:border-brand-accent focus:outline-none"
                    on:change=move |ev| trashed.set(event_target_value(&ev))
                >
                    <option value="" selected>{t("Active")}</option>
                    <option value="with">{t("All")}</option>
                    <option value="only">{t("Trashed")}</option>
                </select>
            </div>

            {move || error.get().map(|e| view! {
                <div class="mb-4 rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">{e}</div>
            })}

            <Show
                when=move || !loading.get()
                fallback=move || view! { <div class="text-white/60">{t("Loading")}"…"</div> }
            >
                <Show
                    when=move || !visible().is_empty()
                    fallback=move || view! {
                        <div class="rounded-xl border border-white/10 bg-surface-raised/60 p-8 text-center text-white/60">
                            {t("No rooms found")}
                        </div>
                    }
                >
                    <div class="overflow-x-auto overflow-hidden rounded-xl border border-white/10 bg-surface-raised/60">
                        <table class="w-full text-sm">
                            <thead class="border-b border-white/10 text-left text-xs uppercase tracking-wider text-white/40">
                                <tr>
                                    <th class="px-4 py-3">{t("Name")}</th>
                                    <th class="px-4 py-3">{t("Moderators")}</th>
                                    <th class="px-4 py-3">{t("Category")}</th>
                                    <th class="px-4 py-3">{t("Playlists")}</th>
                                    <th class="px-4 py-3">{t("Rounds played")}</th>
                                    <th class="px-4 py-3">{t("Autostart")}</th>
                                    <th class="px-4 py-3">{t("Visibility")}</th>
                                    <th class="px-4 py-3 text-right">{t("Actions")}</th>
                                </tr>
                            </thead>
                            <tbody>
                                <For
                                    each=visible
                                    key=|r| r.id
                                    children=move |r: MineRoom| {
                                        let rid = r.id;
                                        view! {
                                            <tr class="border-b border-white/5 last:border-0 hover:bg-white/[0.03]">
                                                <td class="px-4 py-3">
                                                    <A href=format!("/rooms/{rid}/edit") attr:class="flex items-center gap-3 font-medium text-white hover:text-brand-accent-light">
                                                        {r.photo_url.clone().map(|u| view! {
                                                            <img src=u class="h-9 w-9 flex-shrink-0 rounded-full object-cover ring-1 ring-white/20" alt=""/>
                                                        })}
                                                        <span class="min-w-0">
                                                            <span class="block truncate">{r.name.clone()}</span>
                                                            {r.description.clone().map(|d| view! {
                                                                <small class="block max-w-56 truncate text-white/50">{d}</small>
                                                            })}
                                                        </span>
                                                        {r.deleted.then(|| view! {
                                                            <span class="text-red-400" title=t("Trashed")>"🗑"</span>
                                                        })}
                                                    </A>
                                                </td>
                                                <td class="px-4 py-3">
                                                    <div class="flex max-w-48 flex-wrap gap-1">
                                                        {r.moderators.iter().map(|m| view! {
                                                            <span class="rounded bg-white/10 px-1.5 py-0.5 text-xs text-white/70">{m.clone()}</span>
                                                        }).collect_view()}
                                                        {r.moderators.is_empty().then(|| view! {
                                                            <span class="text-xs text-white/40">{t("None")}</span>
                                                        })}
                                                    </div>
                                                </td>
                                                <td class="px-4 py-3">
                                                    {match r.category.clone() {
                                                        Some(c) => view! {
                                                            <span class="rounded bg-white/10 px-1.5 py-0.5 text-xs text-white/70">{c}</span>
                                                        }.into_any(),
                                                        None => view! {
                                                            <span class="text-xs text-white/40">{t("No category")}</span>
                                                        }.into_any(),
                                                    }}
                                                </td>
                                                <td class="px-4 py-3">
                                                    <div class="flex max-w-48 flex-wrap gap-1">
                                                        {r.playlists.iter().map(|p| view! {
                                                            <span class="rounded bg-white/10 px-1.5 py-0.5 text-xs text-white/70">{p.clone()}</span>
                                                        }).collect_view()}
                                                        {r.playlists.is_empty().then(|| view! {
                                                            <span class="text-xs text-white/40">{t("No playlist")}</span>
                                                        })}
                                                    </div>
                                                </td>
                                                <td class="px-4 py-3 text-white/70">{r.rounds_count}</td>
                                                <td class="px-4 py-3">
                                                    <span class=move || if r.is_autostart {
                                                        "rounded bg-teal-500/20 px-1.5 py-0.5 text-xs text-teal-300"
                                                    } else {
                                                        "rounded bg-red-500/20 px-1.5 py-0.5 text-xs text-red-300"
                                                    }>
                                                        {if r.is_autostart { t("Yes") } else { t("No") }}
                                                    </span>
                                                </td>
                                                <td class="px-4 py-3">
                                                    <span class=move || if r.has_password {
                                                        "rounded bg-white/10 px-1.5 py-0.5 text-xs text-white/70"
                                                    } else if r.is_public {
                                                        "rounded bg-teal-500/20 px-1.5 py-0.5 text-xs text-teal-300"
                                                    } else {
                                                        "rounded bg-white/10 px-1.5 py-0.5 text-xs text-white/70"
                                                    }>
                                                        {if r.has_password { t("Private") } else if r.is_public { t("Public") } else { t("Private") }}
                                                    </span>
                                                </td>
                                                <td class="px-4 py-3 text-right">
                                                    <Show when=move || !r.deleted>
                                                        <A href=format!("/rooms/{rid}/edit") attr:class="text-brand-accent-light hover:underline">
                                                            {t("Edit")}
                                                        </A>
                                                    </Show>
                                                </td>
                                            </tr>
                                        }
                                    }
                                />
                            </tbody>
                        </table>
                    </div>
                </Show>
            </Show>
        </div>
    }
}
