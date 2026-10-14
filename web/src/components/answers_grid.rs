use crate::api;
use crate::i18n::t;
use crate::types::Round;
use leptos::portal::Portal;
use leptos::prelude::*;
use send_wrapper::SendWrapper;
use serde::Serialize;
use serde_json::Value;
use wasm_bindgen::JsCast;

const DOWNVOTE_REASONS: &[(&str, &str)] = &[
    ("sound_quality", "Poor sound quality"),
    ("difficulty", "Too difficult"),
    ("passage_choice", "Bad passage choice"),
    ("personal_taste", "Not my taste"),
    ("controversial_artist", "Controversial artist"),
    ("other", "Other reason"),
];

#[derive(Serialize)]
struct DownvoteBody<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<&'a str>,
}

fn apply_vote_payload(played_tracks: RwSignal<Vec<Value>>, track_id: i64, data: &Value) {
    played_tracks.update(|list| {
        if let Some(tp) = list
            .iter_mut()
            .find(|x| x["id"].as_i64() == Some(track_id))
        {
            for k in ["upvotes", "downvotes", "user_voted_up", "user_voted_down"] {
                if !data[k].is_null() {
                    tp[k] = data[k].clone();
                }
            }
        }
    });
}

#[component]
pub fn PlaylistPanel(
    played_tracks: RwSignal<Vec<Value>>,
    round: RwSignal<Option<Round>>,
    room_id: i64,
    can_vote: Signal<bool>,
) -> impl IntoView {
    let track_progress = move || {
        let r = round.get();
        let current = r.as_ref().map(|x| x.current).unwrap_or(0);
        let total = r.as_ref().map(|x| x.tracks.len() as i64).unwrap_or(0);
        (current, total)
    };
    let pct = move || {
        let (c, total) = track_progress();
        if total == 0 {
            0
        } else {
            (c * 100 / total).min(100)
        }
    };
    let menu_for = RwSignal::new(Option::<(i64, f64, f64)>::None);

    {
        let h = SendWrapper::new(window_event_listener(leptos::ev::keydown, move |ev| {
            if ev.key() == "Escape" {
                menu_for.set(None);
            }
        }));
        on_cleanup(move || drop(h));
    }

    let vote = move |track_id: i64, up: bool, reason: Option<&'static str>| {
        menu_for.set(None);
        leptos::task::spawn_local(async move {
            let path = if up {
                format!("/api/v1/rooms/{room_id}/tracks/{track_id}/upvote")
            } else {
                format!("/api/v1/rooms/{room_id}/tracks/{track_id}/downvote")
            };
            if up {
                if let Ok(d) = api::post_empty::<Value>(&path).await {
                    apply_vote_payload(played_tracks, track_id, &d);
                }
            } else if let Ok(d) =
                api::post::<Value, _>(&path, &DownvoteBody { reason }).await
            {
                apply_vote_payload(played_tracks, track_id, &d);
            }
        });
    };

    view! {
        <section class="retro-card">
            <header class="retro-card__header">
                <div class="flex w-full flex-wrap items-center justify-between gap-2">
                    <h3 class="retro-title retro-title--accent flex items-center text-base md:text-xl">
                        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="mr-2 h-4 w-4 md:h-5 md:w-5" aria-hidden="true">
                            <path stroke-linecap="round" stroke-linejoin="round" d="M9 9l10.5-3m0 6.553v3.75a2.25 2.25 0 01-1.632 2.163l-1.32.377a1.803 1.803 0 11-.99-3.467l2.31-.66a2.25 2.25 0 001.632-2.163zm0 0V4.846c0-1.062-.746-1.953-1.776-2.154l-2.04-.397A2.25 2.25 0 0012 4.5v12.75a2.25 2.25 0 01-1.632 2.163l-1.32.377a1.803 1.803 0 11-.99-3.467l2.31-.66A2.25 2.25 0 009 15.553V9z" />
                        </svg>
                        {t("Playlist")}
                    </h3>
                    <Show when=move || round.get().is_some()>
                        <div class="flex items-center gap-2">
                            <div class="text-xs font-medium text-white/60">
                                {t("Track")}" "{move || format!("{} / {}", track_progress().0, track_progress().1)}
                            </div>
                            <div class="h-1.5 w-16 bg-brand-midnight md:w-32">
                                <div class="h-1.5 bg-brand-accent transition-all duration-300" style=move || format!("width:{}%", pct())></div>
                            </div>
                        </div>
                    </Show>
                </div>
            </header>
            <main class="retro-card__body">
                <div class="h-64 overflow-y-auto pr-2 md:h-80 2xl:h-96"
                    style="scrollbar-width:thin;scrollbar-color:rgb(0 173 181 / 0.5) rgb(26 26 46 / 0.5)">
                    <ul class="space-y-3 md:space-y-4">
                        {move || {
                            let list = played_tracks.get();
                            if list.is_empty() {
                                return view! {
                                    <li class="flex items-center justify-center py-8 text-white/50 md:py-12">
                                        <div class="text-center">
                                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="mx-auto mb-2 h-8 w-8 opacity-50 md:h-12 md:w-12">
                                                <path stroke-linecap="round" stroke-linejoin="round" d="M9 9l10.5-3m0 6.553v3.75a2.25 2.25 0 01-1.632 2.163l-1.32.377a1.803 1.803 0 11-.99-3.467l2.31-.66a2.25 2.25 0 001.632-2.163zm0 0V4.846c0-1.062-.746-1.953-1.776-2.154l-2.04-.397A2.25 2.25 0 0012 4.5v12.75a2.25 2.25 0 01-1.632 2.163l-1.32.377a1.803 1.803 0 11-.99-3.467l2.31-.66A2.25 2.25 0 009 15.553V9z" />
                                            </svg>
                                            <p>{t("No tracks played yet")}</p>
                                        </div>
                                    </li>
                                }.into_any();
                            }
                            list.iter().rev().cloned().collect::<Vec<_>>().into_iter().map(|tp| {
                                let tid = tp["id"].as_i64().unwrap_or_default();
                                let artwork = tp["artwork_url"].as_str().map(str::to_string);
                                let answers = tp["answers"].as_array().cloned().unwrap_or_default();
                                let up_s = display_votes(&tp["upvotes"]);
                                let down_a = display_votes(&tp["downvotes"]);
                                let voted_up = tp["user_voted_up"].as_bool().unwrap_or(false);
                                let voted_down = tp["user_voted_down"].as_bool().unwrap_or(false);
                                view! {
                                    <li class="room-track-card">
                                        <div class="flex flex-row gap-0">
                                            <div class="relative h-28 w-28 shrink-0">
                                                <div class="absolute inset-0 z-0 m-2 rounded-full bg-black/50"></div>
                                                <div class="absolute inset-0 z-10 m-3 flex items-center justify-center overflow-hidden rounded-full bg-brand-deep">
                                                    <div class="absolute inset-0 z-0 bg-gradient-to-br from-brand-midnight to-brand-deep opacity-50">
                                                        <div class="bg-repeating-radial-gradient absolute inset-0 opacity-20"></div>
                                                    </div>
                                                    <div class="absolute inset-0 z-20 flex items-center justify-center">
                                                        <div class="h-4 w-4 rounded-full border-2 border-white/20 bg-brand-midnight"></div>
                                                    </div>
                                                    <div class="absolute inset-0 z-10">
                                                        {artwork.map(|a| view! {
                                                            <img src=a class="h-full w-full rounded-full object-cover" alt=""/>
                                                        })}
                                                    </div>
                                                    <div class="absolute left-1/4 top-0 z-20 h-3 w-12 -rotate-45 transform rounded-full bg-white opacity-20"></div>
                                                    <div class="rotate-30 absolute bottom-1/4 right-1/3 z-20 h-2 w-8 transform rounded-full bg-white opacity-10"></div>
                                                </div>
                                            </div>
                                            <div class="min-w-0 flex-grow p-3">
                                                <ul class="space-y-2">
                                                    {answers.into_iter().map(|a| {
                                                        let type_name = a["type"]["name"].as_str()
                                                            .or(a["name"].as_str())
                                                            .unwrap_or_default()
                                                            .to_string();
                                                        let value = a["value"].as_str().unwrap_or_default().to_string();
                                                        view! {
                                                            <li class="flex items-start gap-2 text-sm">
                                                                <div class="flex min-w-0 flex-wrap items-start gap-2">
                                                                    <div class="room-answer-badge--ghost shrink-0">
                                                                        <span class="max-w-none truncate" title=type_name.clone()>{t(type_name.as_str()).to_string()}</span>
                                                                    </div>
                                                                    <span class="min-w-0 flex-1 whitespace-normal font-medium leading-relaxed text-white/80">{value}</span>
                                                                </div>
                                                            </li>
                                                        }
                                                    }).collect_view()}
                                                </ul>
                                            </div>
                                            <Show when=move || can_vote.get()>
                                                <div class="flex flex-col items-center justify-center p-3">
                                                    <div class="room-track-votes room-track-votes--desktop">
                                                        <button type="button"
                                                            class=move || if voted_up {
                                                                "room-track-vote room-track-vote--up room-track-vote--up-active"
                                                            } else {
                                                                "room-track-vote room-track-vote--up"
                                                            }
                                                            title=t("Upvote this track")
                                                            on:click=move |_| vote(tid, true, None)>
                                                            <span class="room-track-vote__icon" aria-hidden="true">
                                                                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="h-3.5 w-3.5">
                                                                    <path d="M1 8.25a1.25 1.25 0 112.5 0v7.5a1.25 1.25 0 11-2.5 0v-7.5zM11 3V1.7c0-.268.14-.526.395-.607A2 2 0 0114 3c0 .995-.182 1.948-.514 2.826-.204.54.166 1.174.744 1.174h3.52c1.242 0 2.26 1.01 2.146 2.247a10.003 10.003 0 01-1.341 4.786l-3.866 6.516c-.49.826-1.428 1.326-2.451 1.236l-5.5-.5c-1.032-.093-1.738-1.013-1.738-2.03V9.75c0-.462.21-.9.57-1.187l2.53-2.03C9.3 5.66 10.5 4.5 11 3z"/>
                                                                </svg>
                                                            </span>
                                                            <span class="room-track-vote__count">{up_s.clone()}</span>
                                                        </button>
                                                        <div class="relative">
                                                            <button type="button"
                                                                class=if voted_down {
                                                                    "room-track-vote room-track-vote--down room-track-vote--down-active"
                                                                } else {
                                                                    "room-track-vote room-track-vote--down"
                                                                }
                                                                title=t(if voted_down { "Remove downvote" } else { "Downvote this track" })
                                                                on:click=move |ev| {
                                                                    if voted_down {
                                                                        vote(tid, false, None);
                                                                    } else if menu_for.get_untracked().map(|m| m.0) == Some(tid) {
                                                                        menu_for.set(None);
                                                                    } else if let Some(el) = ev
                                                                        .current_target()
                                                                        .and_then(|tg| tg.dyn_into::<web_sys::Element>().ok())
                                                                    {
                                                                        let r = el.get_bounding_client_rect();
                                                                        if r.width() == 0.0 {
                                                                            return;
                                                                        }
                                                                        let (w, h) = web_sys::window()
                                                                            .map(|win| {
                                                                                (
                                                                                    win.inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(0.0),
                                                                                    win.inner_height().ok().and_then(|v| v.as_f64()).unwrap_or(0.0),
                                                                                )
                                                                            })
                                                                            .unwrap_or((0.0, 0.0));
                                                                        menu_for.set(Some((
                                                                            tid,
                                                                            (w - r.right()).max(0.0),
                                                                            h - r.top() + 4.0,
                                                                        )));
                                                                    }
                                                                }>
                                                                <span class="room-track-vote__icon" aria-hidden="true">
                                                                    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="h-3.5 w-3.5">
                                                                        <path d="M18.905 12.75a1.25 1.25 0 01-2.5 0v-7.5a1.25 1.25 0 012.5 0v7.5zM8.905 17v1.3c0 .268-.14.526-.395.607A2 2 0 015.905 17c0-.995.182-1.948.514-2.826.204-.54-.166-1.174-.744-1.174h-3.52c-1.242 0-2.26-1.01-2.146-2.247.307-2.348.83-4.57 1.341-4.786l3.866-6.516c.49-.826 1.428-1.326 2.451-1.236l5.5.5c1.032.093 1.738 1.013 1.738 2.03v8.5c0 .462-.21.9-.57 1.187l-2.53 2.03c-.627.503-1.637 1.533-1.4 3.542z" transform="rotate(180 10 10)"/>
                                                                    </svg>
                                                                </span>
                                                                <span class="room-track-vote__count">{down_a.clone()}</span>
                                                            </button>
                                                        </div>
                                                    </div>
                                                </div>
                                            </Show>
                                        </div>
                                    </li>
                                }
                            }).collect_view().into_any()
                        }}
                    </ul>
                </div>
            </main>
        </section>

        {move || menu_for.get().map(|(mtid, right, bottom)| view! {
            <Portal>
                <div class="fixed inset-0 z-[99998] bg-transparent"
                    on:click=move |_| menu_for.set(None)></div>
                <div class="retro-dropdown-panel fixed z-[99999]"
                    style=format!("right:{right:.0}px;bottom:{bottom:.0}px")>
                    <div class="room-downvote-menu">
                        <p class="room-downvote-menu__title">{t("Why are you downvoting?")}</p>
                        <ul class="room-downvote-menu__list" role="menu">
                            {DOWNVOTE_REASONS.iter().map(|(val, label)| {
                                let val = *val;
                                let label = *label;
                                view! {
                                    <li role="none">
                                        <button type="button" class="room-downvote-menu__option" role="menuitem"
                                            on:click=move |_| vote(mtid, false, Some(val))>
                                            {t(label)}
                                        </button>
                                    </li>
                                }
                            }).collect_view()}
                        </ul>
                    </div>
                </div>
            </Portal>
        })}
    }
}

fn display_votes(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        _ => v.as_i64().map(|n| n.to_string()).unwrap_or_else(|| "0".into()),
    }
}

pub fn apply_track_voted(played_tracks: RwSignal<Vec<Value>>, event_track: &Value) {
    let tid = event_track["id"].as_i64().unwrap_or_default();
    played_tracks.update(|list| {
        if let Some(tp) = list.iter_mut().find(|x| x["id"].as_i64() == Some(tid)) {
            if !event_track["upvotes"].is_null() {
                tp["upvotes"] = event_track["upvotes"].clone();
            }
            if !event_track["downvotes"].is_null() {
                tp["downvotes"] = event_track["downvotes"].clone();
            }
        }
    });
}
