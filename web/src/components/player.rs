use crate::i18n::t;
use crate::types::Track;
use leptos::prelude::*;
use send_wrapper::SendWrapper;
use serde_json::Value;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::HtmlAudioElement;

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

pub fn saved_volume() -> f64 {
    storage()
        .and_then(|s| s.get_item("volume").ok().flatten())
        .and_then(|v| v.parse().ok())
        .unwrap_or(1.0)
}

fn youtube_id(url: &str) -> Option<String> {
    if url.contains("youtube.com") {
        web_sys::Url::new(url).ok().and_then(|u| {
            web_sys::UrlSearchParams::new_with_str(&u.search())
                .ok()
                .and_then(|p| p.get("v"))
        })
    } else if url.contains("youtu.be") {
        web_sys::Url::new(url)
            .ok()
            .map(|u| u.pathname().trim_start_matches('/').to_string())
    } else {
        Some(url.to_string())
    }
}

#[component]
pub fn Player(
    track: RwSignal<Option<Track>>,
    track_duration: f64,
    start_offset: RwSignal<f64>,
    next_track_at: RwSignal<Option<String>>,
    next_audio_url: RwSignal<Option<(i64, String)>>,
    paused: RwSignal<bool>,
    pause_between_tracks: f64,
    current_time: RwSignal<f64>,
    found_all: RwSignal<Vec<Value>>,
    volume: RwSignal<f64>,
) -> impl IntoView {
    let percent = RwSignal::new(0.0_f64);
    let loading = RwSignal::new(false);
    let is_playing = RwSignal::new(false);
    let waiting = RwSignal::new(false);
    let needs_gesture = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);
    let countdown = RwSignal::new(0_i64);

    let audio = SendWrapper::new(Rc::new(RefCell::new(Option::<HtmlAudioElement>::None)));
    let preloaded_id = SendWrapper::new(Rc::new(RefCell::new(Option::<i64>::None)));
    let pending_start = SendWrapper::new(Rc::new(RefCell::new(0.0_f64)));
    let applied_initial = SendWrapper::new(Rc::new(RefCell::new(false)));

    {
        let audio = audio.clone();
        {
            if audio.borrow().is_none() {
                if let Ok(a) = HtmlAudioElement::new() {
                    a.set_volume(saved_volume());
                    {
                        let audio = audio.clone();
                        let cb = Closure::<dyn FnMut()>::new(move || {
                            if let Some(a) = audio.borrow().as_ref() {
                                let t = a.current_time();
                                current_time.set(t);
                                percent.set(
                                    (100.0 / track_duration * (t + 0.25)).min(100.0).round(),
                                );
                            }
                        });
                        let _ = a.add_event_listener_with_callback(
                            "timeupdate",
                            cb.as_ref().unchecked_ref(),
                        );
                        cb.forget();
                    }
                    {
                        let audio = audio.clone();
                        let cb = Closure::<dyn FnMut()>::new(move || {
                            if audio.borrow().is_some() {
                                is_playing.set(false);
                                waiting.set(true);
                            }
                        });
                        let _ = a
                            .add_event_listener_with_callback("ended", cb.as_ref().unchecked_ref());
                        cb.forget();
                    }
                    {
                        let cb = Closure::<dyn FnMut()>::new(move || {
                            loading.set(false);
                        });
                        let _ = a.add_event_listener_with_callback(
                            "canplaythrough",
                            cb.as_ref().unchecked_ref(),
                        );
                        cb.forget();
                    }
                    {
                        let audio = audio.clone();
                        let cb = Closure::<dyn FnMut()>::new(move || {
                            if let Some(a) = audio.borrow().as_ref() {
                                let code = a.error().map(|e| e.code()).unwrap_or(0);
                                let retried = a.get_attribute("data-retried").is_some();
                                if code == 4 && !retried {
                                    let _ = a.set_attribute("data-retried", "true");
                                    a.load();
                                    return;
                                }
                            }
                            error.set(Some("audio_error".to_string()));
                            loading.set(false);
                            is_playing.set(false);
                        });
                        let _ = a
                            .add_event_listener_with_callback("error", cb.as_ref().unchecked_ref());
                        cb.forget();
                    }
                    *audio.borrow_mut() = Some(a);
                }
            }
        }
    }

    let play_now = {
        let audio = audio.clone();
        let pending_start = pending_start.clone();
        SendWrapper::new(Rc::new(move || {
            let Some(a) = audio.borrow().as_ref().cloned() else {
                return;
            };
            let start = *pending_start.borrow();
            if start > 0.0 {
                a.set_current_time(start);
                current_time.set(start);
                percent
                    .set((100.0 / track_duration * (start + 0.25)).min(100.0).round());
            }
            let promise = a.play();
            match promise {
                Ok(p) => {
                    let pending_start = pending_start.clone();
                    let _ = wasm_bindgen_futures::spawn_local(async move {
                        match wasm_bindgen_futures::JsFuture::from(p).await {
                            Ok(_) => {
                                is_playing.set(true);
                                needs_gesture.set(false);
                                let target = *pending_start.borrow();
                                if target > 0.0 {
                                    let a2 = a.clone();
                                    let pending_start = pending_start.clone();
                                    gloo_timers::callback::Timeout::new(50, move || {
                                        let diff = (a2.current_time() - target).abs();
                                        if diff > 0.2 {
                                            a2.set_current_time(target);
                                            current_time.set(target);
                                        }
                                        *pending_start.borrow_mut() = 0.0;
                                    })
                                    .forget();
                                }
                            }
                            Err(e) => {
                                let name = js_sys::Reflect::get(&e, &"name".into())
                                    .ok()
                                    .and_then(|n| n.as_string())
                                    .unwrap_or_default();
                                if name == "NotAllowedError" {
                                    needs_gesture.set(true);
                                } else {
                                    error.set(Some(format!("play failed: {name}")));
                                }
                                is_playing.set(false);
                            }
                        }
                    });
                }
                Err(e) => {
                    leptos::logging::error!("audio.play() threw {:?}", e);
                }
            }
        }))
    };

    {
        let audio = audio.clone();
        let pending_start = pending_start.clone();
        let applied_initial = applied_initial.clone();
        let preloaded_id = preloaded_id.clone();
        let play_now = play_now.clone();
        Effect::new(move |_| {
            let t = track.get();
            match t {
                Some(tr) => {
                    waiting.set(false);
                    error.set(None);
                    loading.set(true);
                    countdown.set(0);
                    next_track_at.set(None);
                    next_audio_url.set(None);
                    if !*applied_initial.borrow() {
                        *pending_start.borrow_mut() = start_offset.get_untracked();
                        *applied_initial.borrow_mut() = true;
                    } else {
                        *pending_start.borrow_mut() = 0.0;
                    }
                    if tr.provider == "youtube" {
                        loading.set(false);
                        is_playing.set(true);
                        return;
                    }
                    if let Some(a) = audio.borrow().as_ref() {
                        let buffered = *preloaded_id.borrow() == Some(tr.id)
                            && a.ready_state() >= 2;
                        if buffered {
                            *preloaded_id.borrow_mut() = None;
                            a.set_muted(false);
                            a.set_volume(volume.get_untracked());
                            play_now();
                            return;
                        }
                        let src = tr.audio.clone().or(tr.preview_url.clone());
                        match src {
                            Some(url) => {
                                a.set_src(&url);
                                a.set_muted(false);
                                a.set_volume(volume.get_untracked());
                                a.set_current_time(0.0);
                                a.load();
                                play_now();
                            }
                            None => {
                                error.set(Some("missing audio url".to_string()));
                                loading.set(false);
                            }
                        }
                    }
                }
                None => {
                    if let Some(a) = audio.borrow().as_ref() {
                        let _ = a.pause();
                    }
                    is_playing.set(false);
                    percent.set(0.0);
                }
            }
        });
    }

    {
        let audio = audio.clone();
        let preloaded_id = preloaded_id.clone();
        Effect::new(move |_| {
            if let Some((tid, url)) = next_audio_url.get() {
                if let Some(a) = audio.borrow().as_ref() {
                    *preloaded_id.borrow_mut() = Some(tid);
                    let _ = a.pause();
                    a.set_muted(true);
                    a.set_preload("auto");
                    a.set_src(&url);
                    a.load();
                }
            }
        });
    }

    {
        Effect::new(move |_| {
            let target = next_track_at.get();
            if target.is_none() {
                return;
            }
            let secs = target
                .as_deref()
                .map(|iso| {
                    let ms = js_sys::Date::parse(iso);
                    if ms.is_nan() {
                        pause_between_tracks as i64
                    } else {
                        ((ms - js_sys::Date::now()) / 1000.0).ceil().max(0.0) as i64
                    }
                })
                .unwrap_or(pause_between_tracks as i64);
            countdown.set(secs);
            waiting.set(true);
            let iv = gloo_timers::callback::Interval::new(1_000, move || {
                countdown.update(|c| {
                    if *c > 0 {
                        *c -= 1;
                    }
                });
            });
            let iv = SendWrapper::new(iv);
            on_cleanup(move || drop(iv));
        });
    }

    let yt_embed = Memo::new(move |_| {
        track.get().and_then(|t| {
            if t.provider == "youtube" {
                t.preview_url
                    .as_deref()
                    .and_then(youtube_id)
                    .map(|id| {
                        format!(
                            "https://www.youtube.com/embed/{id}?autoplay=1&controls=0&disablekb=1"
                        )
                    })
            } else {
                None
            }
        })
    });

    {
        let audio = audio.clone();
        Effect::new(move |_| {
            let p = paused.get();
            if let Some(a) = audio.borrow().as_ref() {
                if p {
                    let _ = a.pause();
                    is_playing.set(false);
                } else if track.get_untracked().is_some() && a.paused() {
                    let _ = a.play();
                    is_playing.set(true);
                }
            }
        });
    }

    {
        let audio = audio.clone();
        Effect::new(move |_| {
            let v = volume.get();
            if let Some(a) = audio.borrow().as_ref() {
                a.set_volume(v);
            }
            if let Some(s) = storage() {
                let _ = s.set_item("volume", &v.to_string());
            }
        });
    }

    {
        let audio = audio.clone();
        on_cleanup(move || {
            if let Some(el) = audio.borrow().as_ref().cloned() {
                let _ = el.pause();
                let _ = el.remove_attribute("src");
                el.load();
            }
        });
    }

    let gesture_play = play_now.clone();
    let markers_empty = move || found_all.get().is_empty();
    let countdown_pct = move || {
        (countdown.get() as f64 / pause_between_tracks.max(1.0) * 100.0).min(100.0)
    };

    view! {
        <div class="room-player-shell room-player-shell--mobile">
            {move || yt_embed.get().map(|src| view! {
                <iframe src=src class="pointer-events-none absolute h-px w-px opacity-0" allow="autoplay"></iframe>
            })}

            <ul class="room-player__markers" class:room-player__markers--empty=markers_empty>
                <For
                    each=move || found_all.get()
                    key=|u: &Value| u["id"].as_i64().unwrap_or_default()
                    children=move |u: Value| {
                        let pct = (100.0 / track_duration.max(1.0)
                            * u["time"].as_f64().unwrap_or(0.0))
                        .max(6.0)
                        .min(94.0);
                        let name = u["name"].as_str().unwrap_or_default().to_string();
                        let title_n = name.clone();
                        let alt_n = name.clone();
                        let photo = u["photo"].as_str().map(str::to_string);
                        view! {
                            <li class="room-player__marker" style=format!("left:{pct}%") title=title_n>
                                {photo.map(|p| view! {
                                    <img src=if p.starts_with("http") { p.clone() } else { format!("/storage/{p}") }
                                        class="room-player__marker-avatar" alt=alt_n/>
                                })}
                                <span class="room-player__marker-name">{name}</span>
                                <div class="room-player__marker-tail" aria-hidden="true"></div>
                            </li>
                        }
                    }
                />
            </ul>

            <div id="player" class="room-player">
                <div class="room-player__track overflow-hidden">
                    {move || {
                        if let Some(e) = error.get() {
                            view! {
                                <div class="flex h-auto w-full flex-col bg-brand-primary/10 p-3 text-white/90">
                                    <div class="flex items-center">
                                        <svg xmlns="http://www.w3.org/2000/svg" class="mr-2 h-5 w-5 text-brand-primary" viewBox="0 0 20 20" fill="currentColor">
                                            <path fill-rule="evenodd" d="M18 10a8 8 0 11-16 0 8 8 0 0116 0zm-7 4a1 1 0 11-2 0 1 1 0 012 0zm-1-9a1 1 0 00-1 1v4a1 1 0 102 0V6a1 1 0 00-1-1z" clip-rule="evenodd"/>
                                        </svg>
                                        <span class="font-medium text-brand-primary-light">{e}</span>
                                    </div>
                                </div>
                            }.into_any()
                        } else if loading.get() && !(waiting.get() && countdown.get() > 0) {
                            view! {
                                <div class="flex h-6 w-full items-center justify-center bg-brand-midnight/60">
                                    <div class="flex items-center space-x-2">
                                        <div class="h-4 w-4 animate-spin rounded-full border-2 border-brand-accent border-t-transparent"></div>
                                        <span class="text-sm font-medium text-white/80">{t("Loading")}</span>
                                    </div>
                                </div>
                            }.into_any()
                        } else if waiting.get() && countdown.get() > 0 {
                            view! {
                                <div class="flex max-w-full flex-grow flex-col">
                                    <div class="relative h-6 w-full overflow-hidden bg-brand-midnight">
                                        <div class="flex h-6 items-center justify-center bg-brand-accent/80 text-white transition-all duration-1000 ease-linear"
                                            style=move || format!("width:{}%", countdown_pct())></div>
                                        <span class="absolute inset-0 flex items-center justify-center text-sm font-medium text-white">
                                            {t("Next track in")}" "{countdown.get()}"s"
                                        </span>
                                    </div>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <div class="relative h-6 w-full">
                                    <div class="absolute left-0 top-0 z-10 h-6 bg-brand-primary/80 transition-all duration-500 ease-linear"
                                        style=move || format!("width:{}%", percent.get().min(18.0))></div>
                                    <div class="absolute left-0 top-0 h-6 bg-brand-accent/90 transition-all duration-500 ease-linear"
                                        style=move || format!("width:{}%", percent.get())>
                                        <div class="absolute inset-0 opacity-10">
                                            <div class="shine-wave"></div>
                                        </div>
                                    </div>
                                </div>
                            }.into_any()
                        }
                    }}
                </div>
            </div>

            {move || needs_gesture.get().then(|| {
                let play_now = gesture_play.clone();
                view! {
                <button type="button" on:click=move |_| {
                    needs_gesture.set(false);
                    play_now();
                }
                    class="absolute inset-0 z-40 flex items-center justify-center gap-2 rounded-lg bg-brand-deep/90 text-sm font-medium text-white backdrop-blur-sm">
                    <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="h-5 w-5">
                        <path d="M8 5v14l11-7z"/>
                    </svg>
                    {t("Click to enable audio")}
                </button>
                }
            })}
        </div>
    }
}
