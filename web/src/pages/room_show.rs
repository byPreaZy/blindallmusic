use crate::api;
use crate::auth::use_auth;
use crate::components::navbar::Spinner;
use crate::components::player::Player;
use crate::components::answer_input::AnswerInput;
use crate::components::answers_grid::{apply_track_voted, PlaylistPanel};
use crate::components::chat::RoomChat;
use crate::components::ranking::RankingPanel;
use crate::i18n::{t, tr};
use crate::types::{JoinedPayload, RoomConfig, RoomUser, Round, Track};
use crate::ws::Realtime;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::{use_navigate, use_params};
use leptos_router::params::Params;
use send_wrapper::SendWrapper;
use serde_json::Value;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Params, Debug, PartialEq, Clone)]
struct RoomParams {
    id: Option<String>,
}

fn parse_scores(v: &Value) -> HashMap<i64, f64> {
    match v {
        Value::Object(m) => m
            .iter()
            .filter_map(|(k, s)| k.parse::<i64>().ok().map(|id| (id, s.as_f64().unwrap_or(0.0))))
            .collect(),
        Value::Array(arr) => arr
            .iter()
            .filter_map(|s| {
                let uid = s["user_id"].as_i64()?;
                Some((uid, s["score"].as_f64().unwrap_or(0.0)))
            })
            .collect(),
        _ => HashMap::new(),
    }
}

fn apply_room_head(room: &RoomConfig, seo: Option<&Value>, password_protected: bool) {
    let tokens = |stats: Option<&Value>| {
        let cat = room
            .category
            .as_ref()
            .and_then(|c| c["name"].as_str())
            .map(|n| t(n).to_string())
            .unwrap_or_default();
        let mut rep = vec![
            ("room", room.name.clone()),
            ("category", cat),
            (
                "rounds",
                stats
                    .and_then(|s| s["rounds"].as_i64())
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
            ),
            (
                "tracks",
                stats
                    .and_then(|s| s["tracks"].as_i64())
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
            ),
            (
                "players",
                stats
                    .and_then(|s| s["players_online"].as_i64())
                    .map(|n| n.to_string())
                    .unwrap_or_default(),
            ),
        ];
        rep.retain(|(_, v)| !v.is_empty());
        rep
    };
    let stats = seo.and_then(|s| s.get("stats"));
    let description = if seo.is_some() {
        match seo
            .and_then(|s| s["content"]["intro"].as_str())
            .filter(|_| crate::i18n::locale() == "fr")
        {
            Some(intro) => crate::seo::truncate160(intro),
            None => crate::seo::truncate160(&tr("Room page meta description", &tokens(stats))),
        }
    } else {
        match &room.description {
            Some(d) if !d.is_empty() => crate::seo::truncate160(d),
            _ => tr("Room default description", &tokens(stats)),
        }
    };

    let slug = room
        .slug
        .clone()
        .unwrap_or_else(|| room.id.to_string());
    let path = format!("/rooms/{slug}");
    let url = crate::seo::canonical_url(&path);
    let title = format!("{} — {}", room.name, t("Room title suffix"));
    let full_title = format!("{title} | Blind'All Music");
    let image = room
        .photo
        .clone()
        .unwrap_or_else(|| "/images/statics/screenshot.png".into());
    let image = if image.starts_with("http") || image.starts_with('/') {
        image
    } else {
        format!("/{image}")
    };
    let image_abs = if image.starts_with("http") {
        image.clone()
    } else {
        format!(
            "{}{}",
            web_sys::window()
                .and_then(|w| w.location().origin().ok())
                .unwrap_or_default(),
            image
        )
    };
    let image_alt = tr("Room OG image alt", &[("room", room.name.clone())]);

    crate::seo::set_meta("name", "description", &description);
    crate::seo::set_canonical(&url);
    crate::seo::apply_alternates(&path);
    if password_protected {
        crate::seo::set_noindex();
    }
    crate::seo::set_meta("property", "og:type", "website");
    crate::seo::set_meta("property", "og:title", &full_title);
    crate::seo::set_meta("property", "og:description", &description);
    crate::seo::set_meta("property", "og:url", &url);
    crate::seo::set_meta("property", "og:image", &image_abs);
    crate::seo::set_meta("property", "og:image:alt", &image_alt);
    crate::seo::set_meta("property", "og:image:width", "1200");
    crate::seo::set_meta("property", "og:image:height", "630");
    crate::seo::set_meta("property", "og:site_name", "Blind'All Music");
    crate::seo::set_meta("property", "og:locale", crate::seo::og_locale());
    crate::seo::set_meta("name", "twitter:card", "summary_large_image");
    crate::seo::set_meta("name", "twitter:site", "@blindall");
    crate::seo::set_meta("name", "twitter:creator", "@blindall");
    crate::seo::set_meta("name", "twitter:title", &full_title);
    crate::seo::set_meta("name", "twitter:description", &description);
    crate::seo::set_meta("name", "twitter:image", &image_abs);
    crate::seo::set_meta("name", "twitter:image:alt", &image_alt);

    let schema_desc = seo
        .and_then(|s| s["content"]["intro"].as_str())
        .map(str::to_string)
        .or_else(|| room.description.clone())
        .unwrap_or_else(|| tr("Room schema default description", &[("room", room.name.clone())]));
    let genre = room
        .category
        .as_ref()
        .and_then(|c| c["name"].as_str())
        .map(|n| t(n).to_string())
        .unwrap_or_else(|| "Music Quiz".into());
    let mut schema = serde_json::json!({
        "@context": "https://schema.org",
        "@type": "VideoGame",
        "name": room.name,
        "description": schema_desc,
        "url": url,
        "image": image_abs,
        "gamePlatform": "Web Browser",
        "applicationCategory": "Game",
        "genre": genre,
        "gameItem": {
            "@type": "Thing",
            "name": t("Room schema game item name"),
            "description": t("Room schema game item description"),
        },
        "offers": {
            "@type": "Offer",
            "price": "0",
            "priceCurrency": "EUR",
            "availability": "https://schema.org/InStock",
        },
        "publisher": {
            "@type": "Organization",
            "name": "Blind'All Music",
            "url": web_sys::window().and_then(|w| w.location().origin().ok()).unwrap_or_default(),
            "logo": {
                "@type": "ImageObject",
                "url": format!("{}/images/statics/blindall-music-logo.png",
                    web_sys::window().and_then(|w| w.location().origin().ok()).unwrap_or_default()),
            },
        },
        "inLanguage": ["fr", "en", "es"],
        "isAccessibleForFree": true,
        "playMode": "MultiPlayer",
        "numberOfPlayers": { "minValue": 1, "maxValue": 100 },
        "mainEntity": {
            "@type": "Thing",
            "name": t("Room schema main entity name"),
            "description": t("Room schema main entity description"),
        },
    });
    if let Some(owner) = &room.owner {
        if let Some(name) = owner["name"].as_str() {
            schema["author"] = serde_json::json!({"@type": "Person", "name": name});
        }
    }
    crate::seo::set_json_ld("room", &schema);

    if let Some(s) = seo {
        let crumbs: Vec<Value> = s["breadcrumbs"].as_array().cloned().unwrap_or_default();
        if !crumbs.is_empty() {
            let items: Vec<Value> = crumbs
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let mut item = serde_json::json!({
                        "@type": "ListItem",
                        "position": i + 1,
                        "name": c["label"].as_str().unwrap_or_default(),
                    });
                    if let Some(href) = c["href"].as_str() {
                        let origin = web_sys::window()
                            .and_then(|w| w.location().origin().ok())
                            .unwrap_or_default();
                        item["item"] = serde_json::json!(format!("{origin}{href}"));
                    }
                    item
                })
                .collect();
            crate::seo::set_json_ld(
                "room-breadcrumb",
                &serde_json::json!({
                    "@context": "https://schema.org",
                    "@type": "BreadcrumbList",
                    "itemListElement": items,
                }),
            );
        }
        let faq = resolved_faq(room, s);
        if !faq.is_empty() {
            crate::seo::set_json_ld(
                "room-faq",
                &serde_json::json!({
                    "@context": "https://schema.org",
                    "@type": "FAQPage",
                    "mainEntity": faq.iter().map(|(q, a)| serde_json::json!({
                        "@type": "Question",
                        "name": q,
                        "acceptedAnswer": {"@type": "Answer", "text": a},
                    })).collect::<Vec<_>>(),
                }),
            );
        }
    }
}

fn seo_tokens(room: &RoomConfig, s: &Value) -> Vec<(&'static str, String)> {
    vec![
        ("room", room.name.clone()),
        (
            "category",
            room.category
                .as_ref()
                .and_then(|c| c["name"].as_str())
                .map(|n| t(n).to_string())
                .unwrap_or_default(),
        ),
        ("rounds", s["stats"]["rounds"].as_i64().unwrap_or_default().to_string()),
        ("tracks", s["stats"]["tracks"].as_i64().unwrap_or_default().to_string()),
        ("players", s["stats"]["players_online"].as_i64().unwrap_or_default().to_string()),
    ]
}

fn resolved_intro(room: &RoomConfig, s: &Value, secondary: bool) -> String {
    let field = if secondary { "intro_secondary" } else { "intro" };
    if crate::i18n::locale() == "fr" {
        if let Some(txt) = s["content"][field].as_str() {
            return txt.to_string();
        }
    }
    let key = if secondary { "Room page intro secondary" } else { "Room page intro" };
    tr(key, &seo_tokens(room, s))
}

fn resolved_faq(room: &RoomConfig, s: &Value) -> Vec<(String, String)> {
    let rep = seo_tokens(room, s);
    vec![
        (tr("Room page FAQ q1", &rep), tr("Room page FAQ a1", &rep)),
        (tr("Room page FAQ q2", &rep), tr("Room page FAQ a2", &rep)),
    ]
}

#[component]
fn RoomSeoPanel(room: RwSignal<RoomConfig>, seo: RwSignal<Option<Value>>) -> impl IntoView {
    view! {
        <Show when=move || seo.get().is_some()>
            {move || {
                let s = seo.get().unwrap();
                let crumbs = s["breadcrumbs"].as_array().cloned().unwrap_or_default();
                let stats = s["stats"].clone();
                let similar = s["similar_rooms"].as_array().cloned().unwrap_or_default();
                let r = room.get();
                let intro = resolved_intro(&r, &s, false);
                let intro_secondary = resolved_intro(&r, &s, true);
                let faq = resolved_faq(&r, &s);
                view! {
                    <section id="room-seo" class="room-seo-panel retro-card mt-8">
                        <main class="retro-card__body space-y-6">
                            <nav class="text-sm text-white/50" aria-label="Breadcrumb">
                                <ol class="flex flex-wrap items-center gap-2">
                                    {crumbs.iter().enumerate().map(|(i, c)| {
                                        let label = t(c["label"].as_str().unwrap_or_default()).to_string();
                                        let href = c["href"].as_str().map(str::to_string);
                                        let last = i + 1 == crumbs.len();
                                        view! {
                                            <li class="flex items-center gap-2">
                                                {match &href {
                                                    Some(h) => view! {
                                                        <A href=h.clone() attr:class="transition-colors hover:text-brand-secondary">{label.clone()}</A>
                                                    }.into_any(),
                                                    None => view! {
                                                        <span class="text-white/80">{label.clone()}</span>
                                                    }.into_any(),
                                                }}
                                                {(!last).then(|| view! { <span aria-hidden="true">"/"</span> })}
                                            </li>
                                        }
                                    }).collect_view()}
                                </ol>
                            </nav>

                            <header class="space-y-3">
                                <h2 class="text-lg font-bold text-white md:text-xl">
                                    {tr("About this room", &[("room", room.get().name.clone())])}
                                </h2>
                                <p class="text-sm leading-relaxed text-white/80 md:text-base">{intro}</p>
                                <p class="text-sm leading-relaxed text-white/60">{intro_secondary}</p>
                            </header>

                            <dl class="grid grid-cols-2 gap-3 sm:grid-cols-3">
                                <div class="rounded-lg border border-white/10 bg-brand-midnight/80 px-3 py-2.5">
                                    <dt class="text-xs uppercase tracking-wide text-white/50">{t("Tracks")}</dt>
                                    <dd class="text-lg font-bold text-white">{stats["tracks"].as_i64().unwrap_or_default()}</dd>
                                </div>
                                <div class="rounded-lg border border-white/10 bg-brand-midnight/80 px-3 py-2.5">
                                    <dt class="text-xs uppercase tracking-wide text-white/50">{t("Rounds played")}</dt>
                                    <dd class="text-lg font-bold text-white">{stats["rounds"].as_i64().unwrap_or_default()}</dd>
                                </div>
                                <div class="col-span-2 rounded-lg border border-white/10 bg-brand-midnight/80 px-3 py-2.5 sm:col-span-1">
                                    <dt class="text-xs uppercase tracking-wide text-white/50">{t("Players online")}</dt>
                                    <dd class="text-lg font-bold text-white">{stats["players_online"].as_i64().unwrap_or_default()}</dd>
                                </div>
                            </dl>

                            {(!similar.is_empty()).then(|| view! {
                                <section class="space-y-3" aria-labelledby="similar-rooms-heading">
                                    <h3 id="similar-rooms-heading" class="text-sm font-medium uppercase tracking-wider text-brand-secondary">
                                        {t("Similar official rooms")}
                                    </h3>
                                    <div class="grid grid-cols-2 gap-3 md:grid-cols-4">
                                        {similar.iter().map(|sr| {
                                            let slug = sr["slug"].as_str().unwrap_or_default().to_string();
                                            let name = sr["name"].as_str().unwrap_or_default().to_string();
                                            let photo = sr["photo"].as_str().map(str::to_string);
                                            view! {
                                                <A href=format!("/rooms/{slug}") attr:class="retro-card group flex flex-col overflow-hidden transition-transform hover:scale-[1.02]">
                                                    {photo.map(|p| view! {
                                                        <img src=p alt=name.clone() class="aspect-video w-full object-cover"/>
                                                    })}
                                                    <span class="px-3 py-2 text-sm font-medium text-white/90 group-hover:text-brand-secondary">{name}</span>
                                                </A>
                                            }
                                        }).collect_view()}
                                    </div>
                                </section>
                            })}

                            <section class="rounded-xl border border-white/10 bg-brand-midnight/40 p-4" aria-labelledby="room-faq-heading">
                                <h3 id="room-faq-heading" class="mb-3 text-base font-bold text-white">
                                    {tr("Room page FAQ title", &[("room", room.get().name.clone())])}
                                </h3>
                                <dl class="space-y-3">
                                    {faq.iter().map(|(q, a)| view! {
                                        <div>
                                            <dt class="text-sm font-semibold text-white">{q.clone()}</dt>
                                            <dd class="mt-1 text-sm leading-relaxed text-white/60">{a.clone()}</dd>
                                        </div>
                                    }).collect_view()}
                                </dl>
                            </section>
                        </main>
                    </section>
                }
            }}
        </Show>
    }
}

#[component]
pub fn RoomPage() -> impl IntoView {
    let auth = use_auth();
    let navigate = use_navigate();
    let params = use_params::<RoomParams>();
    let param = params.get().ok().and_then(|p| p.id).unwrap_or_default();
    let room_id = RwSignal::new(param.parse::<i64>().unwrap_or(0));

    // ---- state ------------------------------------------------------------
    let room = RwSignal::new(RoomConfig::default());
    let round = RwSignal::new(Option::<Round>::None);
    let track = RwSignal::new(Option::<Track>::None);
    let start_offset = RwSignal::new(0.0_f64);
    let users = RwSignal::new(Vec::<RoomUser>::new());
    let scores = RwSignal::new(HashMap::<i64, f64>::new());
    let round_id = RwSignal::new(Option::<i64>::None);
    let played_tracks = RwSignal::new(Vec::<Value>::new());
    let users_podium = RwSignal::new(Vec::<Value>::new());
    let teams_podium = RwSignal::new(Vec::<Value>::new());
    let round_finished = RwSignal::new(false);
    let finalizing = RwSignal::new(false);
    let joined = RwSignal::new(false);
    let current_time = RwSignal::new(0.0_f64);
    let next_track_at = RwSignal::new(Option::<String>::None);
    let next_audio_url = RwSignal::new(Option::<(i64, String)>::None);
    let paused = RwSignal::new(false);
    let answers_by_user = RwSignal::new(HashMap::<i64, Vec<Value>>::new());
    let found_all_markers = RwSignal::new(Vec::<Value>::new());

    // ---- view state (Show.vue layout) --------------------------------------
    let volume = RwSignal::new(crate::components::player::saved_volume());
    let display_chat = RwSignal::new(true);
    let podium_open = RwSignal::new(false);
    let share_open = RwSignal::new(false);
    let mobile_tab = RwSignal::new("playlist");
    let public_rooms = RwSignal::new(Vec::<Value>::new());
    let suggestion_open = RwSignal::new(false);

    let nav = navigate.clone();
    Effect::new(move |_| {
        if auth.ready.get() && !auth.is_authed() && api::stored_token().is_none() {
            leptos::task::spawn_local({
                let nav = nav.clone();
                async move {
                    match api::post_empty::<crate::types::AuthResponse>("/api/v1/auth/guest").await {
                        Ok(resp) => auth.complete_oauth(resp),
                        Err(_) => nav("/login", Default::default()),
                    }
                }
            });
        }
    });

    let rt = RwSignal::new_local(Option::<Realtime>::None);
    let shared_rt = use_context::<crate::app::SharedRealtime>();

    // ---- hydrate + wire events once we have a token -----------------------
    let hydrate = Rc::new(move |room_id: i64| {
        leptos::task::spawn_local(async move {
            let Ok(data) = api::get::<JoinedPayload>(&format!("/api/v1/rooms/{room_id}/joined")).await else {
                return;
            };
            if round_id.is_disposed() {
                return;
            }
            round.set(data.round);
            track.set(data.track);
            start_offset.set(data.start_time);
            scores.set(parse_scores(&data.scores));
            round_id.set(data.round_id);
            played_tracks.set(data.played_tracks);
            let parsed: Vec<RoomUser> = data
                .users
                .iter()
                .filter_map(|x| serde_json::from_value::<RoomUser>(x.clone()).ok())
                .collect();
            if !parsed.is_empty() {
                users.set(parsed);
            }
            if let Some(r) = data.room {
                if let Some(p) = r["is_playing"].as_bool() {
                    room.update(|c| c.is_playing = p);
                }
            }
            joined.set(true);
            if let (Some(rid), Some(tid)) = (round_id.get(), track.get().map(|t| t.id)) {
                let _ = api::post_empty::<Value>(&format!(
                    "/api/v1/rounds/{rid}/tracks/{tid}/listened"
                ))
                .await;
            }
        });
    });

    let needs_password = RwSignal::new(false);
    let password_wrong = RwSignal::new(false);
    let not_found = RwSignal::new(false);
    let password_input = RwSignal::new(String::new());
    let room_password = RwSignal::new(String::new());
    let show_done = RwSignal::new(false);
    let seo_data = RwSignal::new(Option::<Value>::None);
    Effect::new(move |_| {
        let _ = auth.user.get();
        let pwd = room_password.get();
        if api::stored_token().is_none() || show_done.get() {
            return;
        }
        let param = param.clone();
        leptos::task::spawn_local(async move {
            #[derive(serde::Deserialize)]
            struct ShowResp {
                room: RoomConfig,
                #[serde(default)]
                public_rooms: Vec<Value>,
                #[serde(default)]
                seo: Option<Value>,
            }
            let url = if pwd.is_empty() {
                format!("/api/v1/rooms/{param}")
            } else {
                format!(
                    "/api/v1/rooms/{param}?password={}",
                    js_sys::encode_uri_component(&pwd)
                )
            };
            match api::get::<ShowResp>(&url).await {
                _ if show_done.is_disposed() => {}
                Ok(resp) => {
                    show_done.set(true);
                    needs_password.set(false);
                    room_id.set(resp.room.id);
                    crate::i18n::set_title(Some(&format!(
                        "{} — {}",
                        resp.room.name,
                        t("Room title suffix")
                    )));
                    apply_room_head(&resp.room, resp.seo.as_ref(), !pwd.is_empty());
                    seo_data.set(resp.seo);
                    room.set(resp.room);
                    public_rooms.set(resp.public_rooms);
                }
                Err(e) if e.status == 403 => {
                    if let Some(name) = e.body["room"]["name"].as_str() {
                        room.update(|c| c.name = name.to_string());
                        crate::i18n::set_title(Some(&format!(
                            "{} — {}",
                            name,
                            t("Room title suffix")
                        )));
                    }
                    crate::seo::set_noindex();
                    password_wrong.set(!pwd.is_empty());
                    needs_password.set(true);
                }
                Err(e) if e.status == 404 => not_found.set(true),
                Err(_) => {}
            }
        });
    });

    Effect::new(move |_| {
        let _ = auth.user.get();
        let rid = room_id.get();
        if rid == 0 || rt.get().is_some() {
            return;
        }
        let channel = format!("room:{rid}");
        let shared = shared_rt.unwrap();
        let realtime = match shared.get() {
            Some(r) => r,
            None => {
                let Some(token) = api::stored_token() else {
                    return;
                };
                let r = Realtime::connect(&token);
                shared.set(Some(r.clone()));
                r
            }
        };
        rt.set(Some(realtime.clone()));

        let ch = channel.clone();
        realtime.on_presence(&ch, Rc::new(move |kind, list| {
            let parsed: Vec<RoomUser> = list
                .iter()
                .filter_map(|v| serde_json::from_value(v.clone()).ok())
                .collect();
            match kind {
                "sync" => users.set(parsed),
                "join" => {
                    for u in parsed {
                        users.update(|us| {
                            if !us.iter().any(|x| x.id == u.id) {
                                us.push(u.clone());
                            }
                        });
                    }
                }
                "leave" => {
                    let ids: Vec<i64> = parsed.iter().map(|u| u.id).collect();
                    users.update(|us| us.retain(|x| !ids.contains(&x.id)));
                }
                _ => {}
            }
        }));

        macro_rules! on {
            ($event:literal, $body:expr) => {{
                let cb: Rc<dyn Fn(&Value)> = Rc::new($body);
                realtime.listen(&channel, $event, cb);
            }};
        }

        on!("RoomState", move |e| {
            scores.set(parse_scores(&e["scores"]));
            if let Some(id) = e["roundId"].as_i64() {
                round_id.set(Some(id));
            }
        });
        on!("NewScore", move |e| {
            let score = if e["score"].is_object() { &e["score"] } else { e };
            if let Some(uid) = score["user_id"].as_i64() {
                let total = score["total"].as_f64().unwrap_or(0.0);
                scores.update(|m| {
                    m.insert(uid, total);
                });
                if let Some(ans) = score["answers"].as_array() {
                    answers_by_user.update(|m| {
                        m.entry(uid).or_default().extend(ans.iter().cloned());
                    });
                }
            }
        });
        on!("RoundStarted", move |e| {
            if let Ok(r) = serde_json::from_value::<Round>(e["round"].clone()) {
                round_id.set(Some(r.id));
                round.set(Some(r));
            }
            played_tracks.set(vec![]);
            teams_podium.set(vec![]);
            round_finished.set(false);
            finalizing.set(false);
            scores.set(HashMap::new());
            answers_by_user.set(HashMap::new());
            found_all_markers.set(vec![]);
        });
        on!("RoundFinalizing", move |_| {
            finalizing.set(true);
        });
        on!("RoundFinished", move |e| {
            if let Ok(r) = serde_json::from_value::<Round>(e["round"].clone()) {
                round.set(Some(r));
            }
            users_podium.set(e["users_podium"].as_array().cloned().unwrap_or_default());
            teams_podium.set(e["teams_podium"].as_array().cloned().unwrap_or_default());
            finalizing.set(false);
            round_finished.set(true);
        });
        on!("TrackPlayed", move |e| {
            if let Ok(r) = serde_json::from_value::<Round>(e["round"].clone()) {
                round.set(Some(r));
            }
            if let Ok(tr) = serde_json::from_value::<Track>(e["track"].clone()) {
                track.set(Some(tr.clone()));
                if let (Some(rid), Some(trk)) = (round_id.get(), track.get()) {
                    let _ = leptos::task::spawn_local(async move {
                        let _ = api::post_empty::<Value>(&format!(
                            "/api/v1/rounds/{rid}/tracks/{}/listened",
                            trk.id
                        ))
                        .await;
                    });
                }
            }
            answers_by_user.set(HashMap::new());
            found_all_markers.set(vec![]);
        });
        on!("TrackEnded", move |e| {
            track.set(None);
            next_track_at.set(e["next_track_at"].as_str().map(str::to_string));
            let url = e["next_track_audio_url"]
                .as_str()
                .or(e["next_track_preview_url"].as_str())
                .map(str::to_string);
            next_audio_url.set(e["next_track_id"].as_i64().zip(url));
            if let Some(tp) = e.get("track").cloned() {
                played_tracks.update(|v| v.push(tp));
            }
        });
        on!("TrackVoted", move |e| {
            apply_track_voted(played_tracks, &e["track"]);
        });
        on!("TrackPaused", move |_| {
            room.update(|c| c.is_playing = false);
            paused.set(true);
        });
        on!("TrackResumed", move |_| {
            room.update(|c| c.is_playing = true);
        });
        on!("UserEloUpdated", move |e| {
            if let (Some(uid), Some(elo)) = (e["user_id"].as_i64(), e["elo"].as_i64()) {
                users.update(|us| {
                    if let Some(u) = us.iter_mut().find(|x| x.id == uid) {
                        u.elo = elo;
                    }
                });
            }
        });
        on!("UserHasFoundAllTheAnswers", move |e| {
            found_all_markers.update(|v| {
                let u = if e["user"].is_object() { e["user"].clone() } else { e.clone() };
                if !v.iter().any(|x: &Value| x["id"].as_i64() == u["id"].as_i64()) {
                    v.push(u);
                }
            });
        });
        on!("UserLevelUpdated", move |_| {});

        realtime.join(&channel);
        {
            let hb = SendWrapper::new(realtime.start_heartbeat());
            on_cleanup(move || drop(hb));
        }

        let hydrate2 = hydrate.clone();
        leptos::task::spawn_local(async move {
            hydrate2(rid);
            let _ = api::post_empty::<Value>(&format!(
                "/api/v1/rooms/{rid}/presence-joined"
            ))
            .await;
        });
        {
            let rt2 = realtime.clone();
            let iv = SendWrapper::new(gloo_timers::callback::Interval::new(25_000, move || {
                let rt2 = rt2.clone();
                if joined.try_get_untracked().unwrap_or(false) {
                    leptos::task::spawn_local(async move {
                        let _ = api::post_empty::<Value>(&format!(
                            "/api/v1/rooms/{rid}/presence-joined"
                        ))
                        .await;
                    });
                }
                let _ = rt2;
            }));
            on_cleanup(move || drop(iv));
        }
        {
            let hydrate = hydrate.clone();
            realtime.on_reconnect(Rc::new(move || hydrate(rid)));
        }
    });

    {
        on_cleanup(move || {
            let rid = room_id.get_untracked();
            if rid == 0 {
                return;
            }
            if let Some(realtime) = rt.get_untracked() {
                realtime.leave(&format!("room:{rid}"));
            }
            leptos::task::spawn_local(async move {
                let _ = api::post_empty::<Value>(&format!(
                    "/api/v1/rooms/{rid}/presence-left"
                ))
                .await;
            });
        });
    }

    let conn = move || {
        rt.get()
            .map(|r| r.conn_state().get())
            .unwrap_or("reconnecting")
    };
    let _me_id = auth.user.get_untracked().map(|u| u.id);
    let can_moderate_chat = auth
        .user
        .get_untracked()
        .map(|u| u.is_administrator || u.is_public_moderator)
        .unwrap_or(false);

    let chat_inited = RwSignal::new(false);
    Effect::new(move |_| {
        let r = room.get();
        if r.id != 0 && !chat_inited.get_untracked() {
            chat_inited.set(true);
            display_chat.set(r.is_chat_active);
        }
    });

    let is_moderator = Memo::new(move |_| {
        let Some(me) = auth.user.get().map(|u| u.id) else {
            return false;
        };
        room.get()
            .moderators
            .iter()
            .any(|m| m["id"].as_i64() == Some(me))
    });
    let can_vote = Signal::derive(move || auth.user.get().is_some());
    let me_sig = Signal::derive(move || auth.user.get().map(|u| u.id));
    let is_guest_sig = Signal::derive(move || auth.user.get().map(|u| u.is_guest).unwrap_or(true));
    let moderator_ids = Signal::derive(move || {
        room.get()
            .moderators
            .iter()
            .filter_map(|m| m["id"].as_i64())
            .collect::<std::collections::HashSet<i64>>()
    });

    view! {
        <div class="flex h-full min-h-[calc(100vh-3.5rem)] flex-col">
            <Show
                when=move || joined.get()
                fallback=move || view! {
                    <Show
                        when=move || not_found.get()
                        fallback=move || view! {
                            <Show
                                when=move || needs_password.get()
                                fallback=move || view! {
                                    <div class="flex flex-1 items-center justify-center gap-4">
                                        <Spinner/>
                                        <div>
                                            <h2 class="text-xl font-medium text-white">{t("Loading")}"…"</h2>
                                            <p class="mt-1 text-sm text-white/60">{t("Connecting to the room")}</p>
                                        </div>
                                    </div>
                                }
                            >
                        <div class="flex flex-1 items-center justify-center">
                            <form
                                class="w-full max-w-sm space-y-4 rounded-xl border border-white/10 bg-surface-raised/60 p-6"
                                on:submit=move |e| {
                                    e.prevent_default();
                                    room_password.set(password_input.get_untracked());
                                }
                            >
                                <h1 class="text-xl font-medium uppercase text-white">
                                    {move || room.get().name.clone()}
                                </h1>
                                <p class="flex items-center text-sm text-orange-400">
                                    {t("This room is secured by a password")}
                                </p>
                                <Show when=move || password_wrong.get()>
                                    <p class="form-error">{t("Incorrect password")}</p>
                                </Show>
                                <div>
                                    <label class="form-label">{t("Password")}</label>
                                    <input bind:value=password_input type="password" required class="form-input"/>
                                </div>
                                <button type="submit" class="btn-primary w-full">
                                    {t("Login")}
                                </button>
                                <p class="text-center text-xs text-white/50">
                                    {t("If you are the owner of the room and have forgotten the password, you can change it on")}" "
                                    <a href="/rooms" class="underline hover:text-white/80">
                                        {t("the room's editing page")}
                                    </a>
                                </p>
                            </form>
                        </div>
                    </Show>
                }
            >
                <div class="flex flex-1 items-center justify-center">
                    <div class="rounded-xl border border-white/10 bg-surface-raised/60 px-8 py-10 text-center">
                        <h1 class="mb-2 text-2xl font-bold text-white">404</h1>
                        <p class="text-white/60">{t("Page not found")}</p>
                    </div>
                </div>
            </Show>
                }
            >
                <div class="room-show-root h-full min-w-0 overflow-x-hidden md:flex"
                    style="height:calc(100dvh - 3.5rem)">
                    <div class="room-show-scroll relative min-w-0 flex-1 overflow-y-auto overflow-x-hidden p-4 md:px-12 md:py-8">
                        <article class="room-show-header flex flex-col gap-2 sm:flex-row sm:flex-wrap sm:items-center sm:justify-between md:mb-6 md:gap-3">
                            <div class="flex min-w-0 items-center gap-2 sm:gap-3">
                                <h1 class="retro-title retro-title--white min-w-0 truncate text-xl sm:text-2xl">
                                    {move || if room.get().name.is_empty() { format!("Room #{}", room_id.get()) } else { room.get().name } }
                                </h1>
                                <span class=move || format!(
                                    "flex shrink-0 items-center gap-1.5 text-xs font-medium sm:text-sm {}",
                                    if conn() == "reconnecting" { "text-brand-secondary" } else { "text-white/60" }
                                )>
                                    <span class=move || format!(
                                        "h-2 w-2 shrink-0 rounded-full {}",
                                        if conn() == "connected" { "bg-brand-accent" } else { "animate-pulse bg-brand-secondary" }
                                    )></span>
                                    {move || if conn() == "connected" { t("Connected") } else { t("Reconnecting") }}
                                </span>
                            </div>
                            <div class="w-full min-w-0 sm:w-auto">
                                <RoomActions
                                    room=room
                                    me=auth.user
                                    _round=round
                                    room_id=room_id
                                    display_chat=display_chat
                                    podium_open=podium_open
                                    share_open=share_open
                                    is_moderator=is_moderator
                                />
                            </div>
                        </article>

                        <Show when=move || {
                            let r = room.get();
                            !r.is_autostart && !r.is_playing && round.get().map(|x| x.is_playing).unwrap_or(false) == false
                        }>
                            <div class="mb-6 flex items-center gap-2 rounded-lg border border-brand-secondary/30 bg-brand-secondary/10 px-4 py-3 text-white/80">
                                <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5 shrink-0" viewBox="0 0 20 20" fill="currentColor">
                                    <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7-4a1 1 0 11-2 0 1 1 0 012 0zM9 9a1 1 0 000 2v3a1 1 0 001 1h1a1 1 0 100-2v-3a1 1 0 00-1-1H9z" clip-rule="evenodd"/>
                                </svg>
                                <div>
                                    <span class="font-bold">{t("This room is in manual start mode")}</span>
                                    <p class="text-sm">{t("The person responsible for the room (moderators) must be present to start the game")}</p>
                                </div>
                            </div>
                        </Show>

                        <div class="room-show-player mb-2 min-w-0 md:mb-8">
                            <Player
                                track=track
                                track_duration=room.get().track_duration.max(1.0)
                                start_offset=start_offset
                                next_track_at=next_track_at
                                next_audio_url=next_audio_url
                                paused=paused
                                pause_between_tracks=room.get().pause_between_tracks.max(1.0)
                                current_time=current_time
                                found_all=found_all_markers
                                volume=volume
                            />
                            <AnswerInput round=round track=track current_time=current_time.read_only() volume=volume/>
                        </div>

                        <div class="room-mobile-shell mb-4 md:hidden">
                            <div class="mb-3 flex gap-2">
                                {["playlist", "ranking", "chat"].iter().map(|tab| {
                                    let tab = *tab;
                                    let active = move || mobile_tab.get() == tab;
                                    let label = match tab {
                                        "playlist" => t("Playlist"),
                                        "ranking" => t("Ranking"),
                                        _ => t("Chat"),
                                    };
                                    view! {
                                        <button type="button"
                                            class=move || format!(
                                                "flex-1 rounded-lg border px-3 py-2 text-xs font-semibold uppercase tracking-wider transition-colors {}",
                                                if active() {
                                                    "border-brand-accent/50 bg-brand-accent/15 text-brand-accent"
                                                } else {
                                                    "border-white/10 bg-brand-deep text-white/60"
                                                }
                                            )
                                            on:click=move |_| mobile_tab.set(tab)>
                                            {label}
                                        </button>
                                    }
                                }).collect_view()}
                            </div>
                            <Show when=move || mobile_tab.get() == "playlist">
                                <PlaylistPanel played_tracks=played_tracks round=round room_id=room_id.get() can_vote=can_vote/>
                            </Show>
                            <Show when=move || mobile_tab.get() == "ranking">
                                <RankingPanel users=users scores=scores answers_by_user=answers_by_user track=track room_id=room_id.get() me_id=me_sig podium_open=podium_open/>
                            </Show>
                        </div>

                        <div class="hidden gap-8 md:grid md:grid-cols-2">
                            <PlaylistPanel played_tracks=played_tracks round=round room_id=room_id.get() can_vote=can_vote/>
                            <RankingPanel users=users scores=scores answers_by_user=answers_by_user track=track room_id=room_id.get() me_id=me_sig podium_open=podium_open/>
                        </div>

                        <section class="retro-card mt-8 hidden md:block">
                            <Show when=move || room.get().description.is_some()>
                                <header class="retro-card__header">
                                    <h3 class="text-sm">{move || room.get().description.unwrap_or_default()}</h3>
                                </header>
                            </Show>
                            <main class="retro-card__body">
                                <div class="flex flex-col gap-6 text-sm lg:flex-row lg:justify-between">
                                    <div class="w-full">
                                        <div class="flex flex-wrap items-center gap-4">
                                            <span class="text-sm font-medium uppercase tracking-wider text-brand-secondary">{t("Moderators")}</span>
                                            <div class="flex flex-wrap gap-3">
                                                <For
                                                    each=move || room.get().moderators
                                                    key=|m| m["id"].as_i64().unwrap_or_default()
                                                    children=move |m: Value| {
                                                        let mid = m["id"].as_i64().unwrap_or_default();
                                                        let name = m["name"].as_str().unwrap_or("?").to_string();
                                                        let photo = m["photo"].as_str().map(str::to_string);
                                                        let online = move || users.get().iter().any(|u| u.id == mid);
                                                        view! {
                                                            <span class=move || format!(
                                                                "flex items-center space-x-2 border border-white/10 bg-brand-midnight px-3 py-1.5 {}",
                                                                if online() { "ring-2 ring-brand-accent" } else { "" }
                                                            )>
                                                                {photo.map(|p| view! {
                                                                    <img src=p alt=name.clone() title=name.clone() class="h-6 w-6 rounded-full ring-1 ring-white/20"/>
                                                                })}
                                                                <span class=move || format!(
                                                                    "font-medium {}",
                                                                    if online() { "text-brand-accent" } else { "text-white/80" }
                                                                )>{name}</span>
                                                            </span>
                                                        }
                                                    }
                                                />
                                            </div>
                                        </div>
                                    </div>
                                    <div class="flex flex-wrap items-center gap-2 sm:gap-3 md:flex-nowrap lg:gap-4">
                                        <button type="button" class="btn-secondary btn-sm shrink-0 whitespace-nowrap">
                                            <span class="font-medium">
                                                {move || room.get().tracks_count.map(|n| n.to_string()).unwrap_or_else(|| "-".into())}
                                            </span>
                                            " "{t("Tracks")}
                                        </button>
                                        <Show when=move || room.get().rounds_count.unwrap_or(0).gt(&0)>
                                            <button type="button" class="btn-secondary btn-sm shrink-0 whitespace-nowrap">
                                                <span class="font-medium">{move || room.get().rounds_count.unwrap_or(0)}</span>
                                                " "{t("Rounds played")}
                                            </button>
                                        </Show>
                                        <button type="button" class="btn-secondary btn-sm shrink-0 whitespace-nowrap"
                                            on:click=move |_| suggestion_open.set(true)>
                                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="h-5 w-5">
                                                <path stroke-linecap="round" stroke-linejoin="round" d="M12 18v-5.25m0 0a6.01 6.01 0 001.5-.189m-1.5.189a6.01 6.01 0 01-1.5-.189m3.75 7.478a12.06 12.06 0 01-4.5 0m3.75 2.383a14.406 14.406 0 01-3 0M14.25 18v-.192c0-.983.658-1.823 1.508-2.316a7.5 7.5 0 10-7.517 0c.85.493 1.509 1.333 1.509 2.316V18" />
                                            </svg>
                                            <span>{t("Send a suggestion")}</span>
                                        </button>
                                    </div>
                                </div>
                            </main>
                        </section>

                        <Show when=move || !public_rooms.get().is_empty()>
                            <section class="retro-card mt-8 hidden md:block">
                                <main class="retro-card__body">
                                    <div class="flex flex-col gap-3">
                                        <span class="text-sm font-medium uppercase tracking-wider text-brand-secondary">{t("Public Rooms")}</span>
                                        <div class="-mx-1 flex gap-2 overflow-x-auto px-1 pb-1 sm:flex-wrap sm:overflow-visible">
                                            <For
                                                each=move || public_rooms.get()
                                                key=|p| p["slug"].as_str().unwrap_or_default().to_string()
                                                children=|p: Value| {
                                                    let slug = p["slug"].as_str().unwrap_or_default().to_string();
                                                    let name = p["name"].as_str().unwrap_or_default().to_string();
                                                    let photo = p["photo"].as_str().map(str::to_string);
                                                    view! {
                                                        <A href=format!("/rooms/{slug}")
                                                            attr:class="flex shrink-0 items-center space-x-2 border border-white/10 bg-brand-midnight px-3 py-1.5 transition-colors hover:border-white/20 hover:bg-brand-deep-hover">
                                                            {photo.map(|ph| view! {
                                                                <img src=ph alt=name.clone() class="h-5 w-5 rounded-full ring-1 ring-white/20"/>
                                                            })}
                                                            <span class="whitespace-nowrap font-medium text-white/80">{name}</span>
                                                        </A>
                                                    }
                                                }
                                            />
                                        </div>
                                    </div>
                                </main>
                            </section>
                        </Show>

                        <RoomSeoPanel room=room seo=seo_data/>

                        <Show when=move || {
                            joined.get()
                                && !round.get().map(|r| r.is_playing).unwrap_or(false)
                                && !room.get().is_playing
                        }>
                            <crate::ads::Adsense force_serve=true wrapper_class="mt-8"/>
                        </Show>
                    </div>

                    {move || {
                        let mobile = mobile_tab.get() == "chat";
                        let show = room.get().is_chat_active
                            && rt.get().is_some()
                            && (display_chat.get() || mobile);
                        show.then(|| {
                            let r = rt.get().unwrap();
                            let cls = if mobile {
                                "chat-panel flex h-[48dvh] max-h-[32rem] min-h-[24rem] w-full shrink-0 flex-col md:h-full md:max-h-none md:min-h-0 md:w-1/5"
                            } else {
                                "chat-panel hidden h-full w-1/5 shrink-0 flex-col md:flex"
                            };
                            view! {
                                <div class=cls>
                                    <RoomChat room_id=room_id.get() rt=r can_delete=can_moderate_chat is_guest=is_guest_sig moderator_ids=moderator_ids/>
                                </div>
                            }
                        })
                    }}
                </div>

                <Show when=move || finalizing.get() && !round_finished.get()>
                    <div class="round-finalizing-overlay">
                        <div class="round-finalizing-overlay__panel">
                            <div class="round-finalizing-overlay__pulse"></div>
                            <span class="relative h-10 w-10 animate-spin rounded-full border-2 border-brand-accent border-t-transparent"></span>
                            <div class="round-finalizing-overlay__title">{t("Calculating scores")}</div>
                        </div>
                    </div>
                </Show>
                <crate::components::finished_round::FinishedRoundModal
                    show=round_finished
                    users_podium=users_podium
                    teams_podium=teams_podium
                    room=room
                />

                <ShareModal open=share_open url=move || {
                    room.get().url.clone().unwrap_or_else(|| format!("/rooms/{}", room_id.get()))
                }/>
                <SuggestionModal open=suggestion_open room_id=room_id/>
            </Show>
        </div>
    }
}

// ---- RoomActions ---------------------------------------------------------------

#[component]
fn RoomActions(
    room: RwSignal<RoomConfig>,
    me: RwSignal<Option<crate::types::User>>,
    _round: RwSignal<Option<Round>>,
    room_id: RwSignal<i64>,
    display_chat: RwSignal<bool>,
    podium_open: RwSignal<bool>,
    share_open: RwSignal<bool>,
    is_moderator: Memo<bool>,
) -> impl IntoView {
    let start_round = move |_| {
        let rid = room_id.get_untracked();
        leptos::task::spawn_local(async move {
            let _ = api::post_empty::<Value>(&format!("/api/v1/rooms/{rid}/start")).await;
        });
    };
    let stop_round = move |_| {
        let rid = room_id.get_untracked();
        leptos::task::spawn_local(async move {
            let _ = api::post_empty::<Value>(&format!("/api/v1/rooms/{rid}/stop")).await;
        });
    };
    let toggle_bookmark = move |_| {
        let rid = room_id.get_untracked();
        let saved = room.get_untracked().is_bookmarked;
        leptos::task::spawn_local(async move {
            let ok = if saved {
                api::delete::<Value>(&format!("/api/v1/rooms/{rid}/bookmark")).await.is_ok()
            } else {
                api::post_empty::<Value>(&format!("/api/v1/rooms/{rid}/bookmark")).await.is_ok()
            };
            if ok {
                room.try_update(|r| r.is_bookmarked = !saved);
            }
        });
    };
    let options_open = RwSignal::new(false);

    view! {
        <div class="room-actions-bar">
            <RoomOptionsModal open=options_open room=room room_id=room_id me=me/>
            <div class="room-actions">
                <button type="button" class="room-action-btn" aria-label=t("Share this room")
                    on:click=move |_| share_open.set(true)>
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="h-4 w-4 shrink-0">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M7.217 10.907a2.25 2.25 0 100 2.186m0-2.186c.18.324.283.696.283 1.093s-.103.77-.283 1.093m0-2.186l9.566-5.314m-9.566 7.5l9.566 5.314m0 0a2.25 2.25 0 103.935 2.186 2.25 2.25 0 00-3.935-2.186zm0-12.814a2.25 2.25 0 103.933-2.185 2.25 2.25 0 00-3.933 2.185z" />
                    </svg>
                    <span class="hidden md:inline">{t("Share")}</span>
                </button>

                <Show when=move || is_moderator.get()>
                    <div class="room-actions__group flex items-center gap-2">
                        <button type="button" class="room-action-btn" on:click=move |_| options_open.set(true)>
                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="h-4 w-4 shrink-0">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M6 13.5V3.75m0 9.75a1.5 1.5 0 010 3m0-3a1.5 1.5 0 000 3m0 3.75V16.5m12-3V3.75m0 9.75a1.5 1.5 0 010 3m0-3a1.5 1.5 0 000 3m0 3.75V16.5m-6-9V3.75m0 3.75a1.5 1.5 0 010 3m0-3a1.5 1.5 0 000 3m0 9.75V10.5" />
                            </svg>
                            <span class="hidden md:inline">{t("Options")}</span>
                        </button>
                        <Show when=move || !room.get().is_autostart>
                            <Show when=move || room.get().is_playing fallback=move || view! {
                                <button type="button" class="room-action-btn room-action-btn--play" on:click=start_round>
                                    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="h-4 w-4 shrink-0">
                                        <path fill-rule="evenodd" d="M4.5 5.653c0-1.426 1.529-2.33 2.779-1.643l11.54 6.348c1.295.712 1.295 2.573 0 3.285L7.28 19.991c-1.25.687-2.779-.217-2.779-1.643V5.653z" clip-rule="evenodd"/>
                                    </svg>
                                    <span class="hidden md:inline">{t("Start round")}</span>
                                </button>
                            }>
                                <button type="button" class="room-action-btn room-action-btn--danger" on:click=stop_round>
                                    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="h-4 w-4 shrink-0">
                                        <path fill-rule="evenodd" d="M4.5 7.5a3 3 0 013-3h9a3 3 0 013 3v9a3 3 0 01-3 3h-9a3 3 0 01-3-3v-9z" clip-rule="evenodd"/>
                                    </svg>
                                    <span class="hidden md:inline">{t("Stop round")}</span>
                                </button>
                            </Show>
                        </Show>
                    </div>
                </Show>

                <Show when=move || me.get().is_some()>
                    <button type="button" class="room-action-btn room-action-btn--highlight"
                        aria-label=t("Show rankings for this room")
                        on:click=move |_| podium_open.set(true)>
                        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="h-4 w-4 shrink-0">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M2.25 18L9 11.25l4.306 4.307a11.95 11.95 0 015.814-5.519l2.74-1.22m0 0l-5.94-2.28m5.94 2.28l-2.28 5.941" />
                        </svg>
                        <span class="hidden md:inline">{t("Rankings")}</span>
                    </button>

                    <button type="button"
                        class=move || if room.get().is_bookmarked {
                            "room-action-btn room-action-btn--saved"
                        } else {
                            "room-action-btn"
                        }
                        on:click=toggle_bookmark>
                        {move || if room.get().is_bookmarked {
                            view! {
                                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="h-4 w-4 shrink-0">
                                    <path fill-rule="evenodd" d="M10.788 3.21c.448-1.077 1.976-1.077 2.424 0l2.082 5.007 5.404.433c1.164.093 1.636 1.545.749 2.305l-4.117 3.527 1.257 5.273c.271 1.136-.964 2.033-1.96 1.425L12 18.354 7.373 21.18c-.996.608-2.231-.29-1.96-1.425l1.257-5.273-4.117-3.527c-.887-.76-.415-2.212.749-2.305l5.404-.433 2.082-5.006z" clip-rule="evenodd"/>
                                </svg>
                            }.into_any()
                        } else {
                            view! {
                                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="h-4 w-4 shrink-0">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M11.48 3.499a.562.562 0 011.04 0l2.125 5.111a.563.563 0 00.475.345l5.518.442c.499.04.701.663.321.988l-4.204 3.602a.563.563 0 00-.182.557l1.285 5.385a.562.562 0 01-.84.61l-4.725-2.885a.563.563 0 00-.586 0L6.982 20.54a.562.562 0 01-.84-.61l1.285-5.386a.562.562 0 00-.182-.557l-4.204-3.602a.563.563 0 01.321-.988l5.518-.442a.563.563 0 00.475-.345L11.48 3.5z" />
                                </svg>
                            }.into_any()
                        }}
                        <span class="hidden md:inline">{t("Bookmark")}</span>
                    </button>
                </Show>

                <Show when=move || me.get().is_some() && room.get().is_chat_active>
                    <button type="button"
                        class=move || if display_chat.get() {
                            "room-action-btn room-action-btn--active"
                        } else {
                            "room-action-btn"
                        }
                        aria-label=move || if display_chat.get() { t("Hide chatbox") } else { t("Show chatbox") }
                        aria-pressed=move || display_chat.get()
                        on:click=move |_| display_chat.update(|v| *v = !*v)>
                        <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z" />
                        </svg>
                        <span class="hidden md:inline">{t("Chat")}</span>
                    </button>
                </Show>
            </div>
        </div>
    }
}

// ---- ShareModal ----------------------------------------------------------------

#[component]
fn ShareModal(
    open: RwSignal<bool>,
    url: impl Fn() -> String + 'static + Copy,
) -> impl IntoView {
    let copied = RwSignal::new(false);
    let share_url = format!("{}{}", web_sys::window()
        .and_then(|w| w.location().origin().ok())
        .unwrap_or_default(), url());
    let copy_url = share_url.clone();
    let copy = Callback::new(move |_: web_sys::MouseEvent| {
        if let Some(w) = web_sys::window() {
            let _ = w.navigator().clipboard().write_text(&copy_url);
        }
        copied.set(true);
        gloo_timers::callback::Timeout::new(1_500, move || { let _ = copied.try_set(false); }).forget();
    });
    view! {
        <Show when=move || open.get()>
            <div class="fixed inset-0 z-50 flex items-center justify-center px-4">
                <div class="retro-modal-overlay" on:click=move |_| open.set(false)></div>
                <div class="retro-modal-panel relative max-w-md">
                    <div class="flex items-center justify-between border-b border-white/10 px-6 py-4">
                        <h3 class="retro-title retro-title--accent text-lg">{t("Share this room")}</h3>
                        <button type="button" class="retro-icon-btn !h-8 !w-8" on:click=move |_| open.set(false)>
                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="h-5 w-5">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                            </svg>
                        </button>
                    </div>
                    <div class="p-6">
                        <div class="flex gap-2">
                            <input type="text" readonly class="form-input flex-1" prop:value=share_url.clone()/>
                            <button type="button" class="btn-primary btn-sm" on:click=move |e| copy.run(e)>
                                {move || if copied.get() { t("Copied!") } else { t("Copy") }}
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </Show>
    }
}

// ---- SuggestionModal -----------------------------------------------------------

#[component]
fn SuggestionModal(
    open: RwSignal<bool>,
    room_id: RwSignal<i64>,
) -> impl IntoView {
    let body = RwSignal::new(String::new());
    let sent = RwSignal::new(false);
    let sending = RwSignal::new(false);
    let submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let text = body.get().trim().to_string();
        if text.is_empty() || sending.get() {
            return;
        }
        sending.set(true);
        let rid = room_id.get_untracked();
        #[derive(serde::Serialize)]
        struct B {
            message: String,
        }
        leptos::task::spawn_local(async move {
            if api::post::<Value, _>(
                &format!("/api/v1/rooms/{rid}/suggestion"),
                &B { message: text },
            )
            .await
            .is_ok()
            {
                sent.try_set(true);
                body.try_set(String::new());
            }
            sending.try_set(false);
        });
    };
    view! {
        <Show when=move || open.get()>
            <div class="fixed inset-0 z-50 flex items-center justify-center px-4">
                <div class="retro-modal-overlay" on:click=move |_| { open.set(false); sent.set(false); }></div>
                <div class="retro-modal-panel relative max-w-md">
                    <div class="flex items-center justify-between border-b border-white/10 px-6 py-4">
                        <h3 class="retro-title retro-title--accent text-lg">{t("Send a suggestion")}</h3>
                        <button type="button" class="retro-icon-btn !h-8 !w-8" on:click=move |_| { open.set(false); sent.set(false); }>
                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="h-5 w-5">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                            </svg>
                        </button>
                    </div>
                    <form class="p-6" on:submit=submit>
                        {move || if sent.get() {
                            view! { <p class="text-sm text-brand-accent">{t("Suggestion sent, thank you!")}</p> }.into_any()
                        } else {
                            view! {
                                <div>
                                    <textarea bind:value=body rows="4" maxlength="500"
                                        class="form-input w-full"
                                        placeholder=t("Suggest a track, an artist, a playlist…")></textarea>
                                    <button type="submit" class="btn-primary mt-3" disabled=move || sending.get()>
                                        {t("Send")}
                                    </button>
                                </div>
                            }.into_any()
                        }}
                    </form>
                </div>
            </div>
        </Show>
    }
}

// ---- RoomOptionsModal ---------------------------------------------------------

#[component]
fn RoomOptionsModal(
    open: RwSignal<bool>,
    room: RwSignal<RoomConfig>,
    room_id: RwSignal<i64>,
    me: RwSignal<Option<crate::types::User>>,
) -> impl IntoView {
    let tbr = RwSignal::new(10i64);
    let dur = RwSignal::new(30i64);
    let pt = RwSignal::new(5i64);
    let pr = RwSignal::new(30i64);
    let chat_active = RwSignal::new(true);
    let random = RwSignal::new(false);
    let autostart = RwSignal::new(false);
    let featured = RwSignal::new(false);
    let has_password = RwSignal::new(false);
    let password = RwSignal::new(String::new());
    let saving = RwSignal::new(false);

    Effect::new(move |_| {
        if open.get() {
            let r = room.get();
            tbr.set(r.tracks_by_round);
            dur.set(r.track_duration as i64);
            pt.set(r.pause_between_tracks as i64);
            pr.set(r.pause_between_rounds as i64);
            chat_active.set(r.is_chat_active);
            random.set(r.is_random);
            autostart.set(r.is_autostart);
            featured.set(r.is_featured);
            has_password.set(r.has_password);
            password.set(String::new());
        }
    });

    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if saving.get_untracked() {
            return;
        }
        saving.set(true);
        let rid = room_id.get_untracked();
        let body = serde_json::json!({
            "tracks_by_round": tbr.get_untracked(),
            "track_duration": dur.get_untracked(),
            "pause_between_tracks": pt.get_untracked(),
            "pause_between_rounds": pr.get_untracked(),
            "is_chat_active": chat_active.get_untracked(),
            "is_random": random.get_untracked(),
            "is_autostart": autostart.get_untracked(),
            "is_featured": featured.get_untracked(),
            "has_password": has_password.get_untracked(),
            "password": password.get_untracked(),
        });
        leptos::task::spawn_local(async move {
            if api::put::<Value, _>(&format!("/api/v1/rooms/{rid}/options"), &body)
                .await
                .is_ok()
            {
                room.try_update(|r| {
                    r.tracks_by_round = body["tracks_by_round"].as_i64().unwrap_or(r.tracks_by_round);
                    r.track_duration = body["track_duration"].as_f64().unwrap_or(r.track_duration);
                    r.pause_between_tracks = body["pause_between_tracks"].as_f64().unwrap_or(r.pause_between_tracks);
                    r.pause_between_rounds = body["pause_between_rounds"].as_f64().unwrap_or(r.pause_between_rounds);
                    r.is_chat_active = body["is_chat_active"].as_bool().unwrap_or(r.is_chat_active);
                    r.is_random = body["is_random"].as_bool().unwrap_or(r.is_random);
                    r.is_autostart = body["is_autostart"].as_bool().unwrap_or(r.is_autostart);
                    r.is_featured = body["is_featured"].as_bool().unwrap_or(r.is_featured);
                    r.has_password = body["has_password"].as_bool().unwrap_or(r.has_password);
                });
                open.try_set(false);
            }
            saving.try_set(false);
        });
    };

    let slider = move |label: &'static str, val: RwSignal<i64>, min: i64, max: i64, suffix: &'static str| {
        view! {
            <div>
                <div class="mb-2 flex justify-between">
                    <label class="text-sm font-medium text-neutral-300">{t(label)}</label>
                    <span class="rounded bg-purple-900/50 px-2 py-0.5 text-xs font-bold text-purple-300">
                        {val} " " {t(suffix)}
                    </span>
                </div>
                <input type="range" min=min max=max step="1"
                    class="h-2 w-full cursor-pointer appearance-none rounded-lg bg-neutral-700 accent-purple-500"
                    prop:value=val
                    on:input=move |ev| val.set(event_target_value(&ev).parse().unwrap_or(val.get_untracked()))/>
                <div class="mt-1 flex justify-between text-xs text-neutral-500">
                    <span>{min}</span><span>{max}</span>
                </div>
            </div>
        }
    };

    let toggle = move |label: &'static str, val: RwSignal<bool>| {
        view! {
            <label class="flex cursor-pointer items-center gap-2 rounded-lg bg-neutral-700/50 p-3 text-sm text-neutral-200 transition-colors duration-200 hover:bg-neutral-700">
                <input type="checkbox" class="accent-purple-500" prop:checked=val
                    on:change=move |ev| val.set(event_target_checked(&ev))/>
                <span>{t(label)}</span>
            </label>
        }
    };

    view! {
        <Show when=move || open.get()>
            <div class="fixed inset-0 z-50 flex items-center justify-center px-4">
                <div class="retro-modal-overlay" on:click=move |_| open.set(false)></div>
                <div class="retro-modal-panel relative max-w-lg">
                    <div class="flex items-center justify-between border-b border-white/10 px-6 py-4">
                        <h3 class="retro-title retro-title--accent text-lg">{t("Room options")}</h3>
                        <button type="button" class="retro-icon-btn !h-8 !w-8" on:click=move |_| open.set(false)>
                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="h-5 w-5">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                            </svg>
                        </button>
                    </div>
                    <form class="max-h-[75vh] space-y-5 overflow-y-auto p-6" on:submit=save>
                        {slider("Tracks per round", tbr, 1, 50, "tracks")}
                        {slider("Track duration", dur, 5, 30, "seconds")}
                        {slider("Pause between tracks", pt, 0, 30, "seconds")}
                        {slider("Pause between rounds", pr, 0, 60, "seconds")}
                        <div class="grid grid-cols-1 gap-3 md:grid-cols-2">
                            {move || (me.get().map(|u| u.is_administrator).unwrap_or(false)).then(|| toggle("Featured", featured))}
                            {toggle("Chatbox", chat_active)}
                            {toggle("Random", random)}
                            {toggle("Autostart", autostart)}
                            {toggle("Password", has_password)}
                        </div>
                        <Show when=move || has_password.get()>
                            <input type="password" autocomplete="new-password"
                                class="form-input w-full"
                                placeholder=t("Password")
                                prop:value=password
                                on:input=move |ev| password.set(event_target_value(&ev))/>
                        </Show>
                        <div class="flex justify-end gap-3 pt-2">
                            <button type="button"
                                class="rounded-lg border border-neutral-600 bg-neutral-800 px-4 py-2 text-neutral-300 transition-colors duration-200 hover:bg-neutral-700"
                                on:click=move |_| open.set(false)>
                                {t("Close")}
                            </button>
                            <button type="submit" class="btn-primary !px-4 !py-2" disabled=move || saving.get()>
                                {t("Update")}
                            </button>
                        </div>
                    </form>
                </div>
            </div>
        </Show>
    }
}
