use crate::api;
use crate::components::feedback_sounds::play_feedback;
use crate::i18n::t;
use crate::types::{CheckResponse, GoodAnswer, Round, Track};
use leptos::ev;
use leptos::html::Input;
use leptos::prelude::*;
use serde::Serialize;
use std::cell::{Cell, RefCell};
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;

#[derive(Serialize)]
struct CheckBody<'a> {
    text: &'a str,
    words: &'a [String],
    #[serde(rename = "currentTime")]
    current_time: f64,
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

fn stored_flag(key: &str, default: bool) -> bool {
    storage()
        .and_then(|s| s.get_item(key).ok().flatten())
        .map(|v| v != "false")
        .unwrap_or(default)
}

fn store_flag(key: &str, value: bool) {
    if let Some(s) = storage() {
        let _ = s.set_item(key, if value { "true" } else { "false" });
    }
}

struct QueueItem {
    text: String,
    round_id: i64,
    track_id: i64,
    generation: u64,
    current_time: f64,
}

#[component]
pub fn AnswerInput(
    round: RwSignal<Option<Round>>,
    track: RwSignal<Option<Track>>,
    current_time: ReadSignal<f64>,
    volume: RwSignal<f64>,
) -> impl IntoView {
    let text = RwSignal::new(String::new());
    let words = RwSignal::new(Vec::<String>::new());
    let found = RwSignal::new(Vec::<GoodAnswer>::new());
    let flash = RwSignal::new(Option::<String>::None);
    let hint = RwSignal::new(Option::<String>::None);
    let queue = Rc::new(RefCell::new(VecDeque::<QueueItem>::new()));
    let processing = Rc::new(Cell::new(false));
    let generation = Rc::new(Cell::new(0_u64));
    let input_ref = NodeRef::<Input>::new();
    let settings_open = RwSignal::new(false);
    let auto_focus = RwSignal::new(stored_flag("autoFocus", true));
    let answer_sounds = RwSignal::new(stored_flag("answerSounds", true));
    let found_ids = Memo::new(move |_| found.get().iter().map(|a| a.id).collect::<HashSet<i64>>());

    let blurred = RwSignal::new(false);

    let disabled =
        Memo::new(move |_| round.get().is_none() || track.get().is_none() || blurred.get());

    // Anti-triche : masquer la page (autre onglet, minimise, switch d'app)
    // pendant une piste bloque la saisie jusqu'à la piste suivante. Les
    // réponses déjà soumises restent valides. La simple perte de focus
    // (second écran, Discord à côté, devtools, notif OS) ne bloque PAS :
    // trop de faux positifs pour des joueurs qui voient encore la page.
    {
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;
        if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
            let cb = Closure::wrap(Box::new({
                let doc = doc.clone();
                move || {
                    if doc.hidden()
                        && round.get_untracked().is_some()
                        && track.get_untracked().is_some()
                        && !blurred.get_untracked()
                    {
                        blurred.set(true);
                        text.set(String::new());
                    }
                }
            }) as Box<dyn FnMut()>);
            let _ = doc
                .add_event_listener_with_callback("visibilitychange", cb.as_ref().unchecked_ref());
            let cb = send_wrapper::SendWrapper::new(cb);
            on_cleanup(move || {
                let _ = doc.remove_event_listener_with_callback(
                    "visibilitychange",
                    cb.as_ref().unchecked_ref(),
                );
            });
        }
    }

    let last_track_id = RwSignal::new(Option::<i64>::None);
    let queue_reset = queue.clone();
    let generation_reset = generation.clone();
    Effect::new(move |_| {
        let id = track.get().map(|t| t.id);
        if id != last_track_id.get() {
            last_track_id.set(id);
            generation_reset.set(generation_reset.get() + 1);
            queue_reset.borrow_mut().clear();
            found.set(vec![]);
            words.set(vec![]);
            text.set(String::new());
            flash.set(None);
            hint.set(None);
            blurred.set(false);
            if id.is_some() && auto_focus.get_untracked() {
                if let Some(el) = input_ref.get() {
                    let _ = el.focus();
                }
            }
        }
    });

    let flash_for = move |kind: &str| {
        let kind = kind.to_string();
        flash.set(Some(kind.clone()));
        play_feedback(&kind, answer_sounds.get_untracked(), volume.get_untracked());
        let ms = match kind.as_str() {
            "good" => 700,
            "almost" => 1000,
            "bad" => 550,
            _ => 1500,
        };
        gloo_timers::callback::Timeout::new(ms, move || {
            // try_* — the timeout outlives the component when the track
            // ends or the page unmounts → plain set() panics on disposed.
            if flash.try_get().flatten().as_deref() == Some(kind.as_str()) {
                let _ = flash.try_set(None);
            }
        })
        .forget();
    };

    let flashing_ids = RwSignal::new(HashSet::<i64>::new());
    let flash_chips = move |ids: Vec<i64>| {
        flashing_ids.update(|s| s.extend(ids.iter().copied()));
        gloo_timers::callback::Timeout::new(650, move || {
            let _ = flashing_ids.try_update(|s| {
                for id in &ids {
                    s.remove(id);
                }
            });
        })
        .forget();
    };

    let pending_count = RwSignal::new(0_usize);

    let process_queue = {
        let queue = queue.clone();
        let processing = processing.clone();
        let generation = generation.clone();
        let flash_for = Rc::new(flash_for);
        let flash_chips = Rc::new(flash_chips);
        move || {
            if processing.replace(true) {
                return;
            }
            let queue = queue.clone();
            let processing = processing.clone();
            let generation = generation.clone();
            let flash_for = flash_for.clone();
            let flash_chips = flash_chips.clone();
            leptos::task::spawn_local(async move {
                loop {
                    let item = {
                        let mut q = queue.borrow_mut();
                        loop {
                            match q.front() {
                                None => break None,
                                Some(it) if it.generation != generation.get() => {
                                    q.pop_front();
                                    pending_count.set(q.len() + 1);
                                }
                                Some(it) => break Some(it.text.clone()),
                            }
                        }
                        .map(|txt| {
                            let it = q.pop_front().unwrap();
                            pending_count.set(q.len() + 1);
                            (it, txt)
                        })
                    };
                    let Some((item, item_text)) = item else {
                        break;
                    };
                    if item.generation != generation.get() {
                        continue;
                    }
                    let path = format!(
                        "/api/v1/rounds/{}/tracks/{}/check",
                        item.round_id, item.track_id
                    );
                    let body_words = words.get_untracked();
                    match api::post::<CheckResponse, _>(
                        &path,
                        &CheckBody {
                            text: &item_text,
                            words: &body_words,
                            current_time: item.current_time,
                        },
                    )
                    .await
                    {
                        Ok(_) if item.generation != generation.get() => {}
                        Ok(resp) => {
                            if words.is_disposed() {
                                break;
                            }
                            words.set(resp.words);
                            if !resp.good_answers.is_empty() {
                                flash_chips(resp.good_answers.iter().map(|a| a.id).collect());
                                found.update(|f| f.extend(resp.good_answers));
                            }
                            if let Some(msg) = resp.message {
                                match msg.kind.as_str() {
                                    "hint" => {
                                        hint.set(Some(msg.body));
                                        gloo_timers::callback::Timeout::new(5_000, move || {
                                            let _ = hint.try_set(None);
                                        })
                                        .forget();
                                    }
                                    k => flash_for(k),
                                }
                            }
                        }
                        Err(e) => match e.status {
                            409 => {}
                            _ => flash_for("bad"),
                        },
                    }
                    if words.is_disposed() {
                        break;
                    }
                }
                processing.set(false);
                pending_count.set(queue.borrow().len());
            })
        }
    };

    let wrap_class = move || {
        let base = "room-input-wrap";
        match flash.get().as_deref() {
            Some("good") => format!("{base} room-input-wrap--flash-good"),
            Some("almost") => format!("{base} room-input-wrap--flash-almost"),
            Some("bad") => format!("{base} room-input-wrap--flash-bad"),
            _ => base.to_string(),
        }
    };

    let is_composing = RwSignal::new(false);
    let do_submit = {
        let queue = queue.clone();
        let generation = generation.clone();
        let process_queue = process_queue.clone();
        move || {
            if disabled.get() || is_composing.get() {
                return;
            }
            let submitted = text.get().trim().to_string();
            if submitted.is_empty() {
                return;
            }
            let (Some(r), Some(tr)) = (round.get(), track.get()) else {
                return;
            };
            text.set(String::new());
            let gen = generation.get();
            {
                let mut q = queue.borrow_mut();
                if q.iter()
                    .any(|it| it.text == submitted && it.generation == gen && it.track_id == tr.id)
                {
                    return;
                }
                q.push_back(QueueItem {
                    text: submitted,
                    round_id: r.id,
                    track_id: tr.id,
                    generation: gen,
                    current_time: current_time.get(),
                });
                pending_count.set(q.len() + if processing.get() { 1 } else { 0 });
            }
            play_feedback(
                "send",
                answer_sounds.get_untracked(),
                volume.get_untracked(),
            );
            process_queue();
            if let Some(el) = input_ref.get() {
                let _ = el.focus();
            }
        }
    };
    let submit_form = {
        let do_submit = do_submit.clone();
        move |ev: ev::SubmitEvent| {
            ev.prevent_default();
            do_submit();
        }
    };
    let submit_form_from_key = do_submit.clone();

    view! {
        <div class="room-user-input w-full space-y-2">
            <form class="m-0 flex w-full items-center justify-center p-0" on:submit=submit_form>
                <div class="relative flex w-full flex-col">
                    <span class="sr-only" aria-live="polite" aria-atomic="true">
                        {move || match flash.get().as_deref() {
                            Some("good") => t("Correct answer").to_string(),
                            Some("almost") => t("Almost!").to_string(),
                            Some("bad") => t("Wrong!").to_string(),
                            _ => String::new(),
                        }}
                    </span>
                    <div class=wrap_class>
                        <div class="room-input-field">
                            <input node_ref=input_ref
                                bind:value=text
                                type="text"
                                class="room-input"
                                placeholder=t("Any idea?")
                                aria-label=t("Any idea?")
                                tabindex="0"
                                autocomplete="off"
                                maxlength="255"
                                on:paste=move |ev| { ev.prevent_default(); flash_for("bad"); }
                                on:drop=move |ev| { ev.prevent_default(); flash_for("bad"); }
                                on:compositionstart=move |_| is_composing.set(true)
                                on:compositionend=move |_| is_composing.set(false)
                                on:keydown=move |ev: ev::KeyboardEvent| {
                                    if ev.key() == "Enter" {
                                        ev.prevent_default();
                                        submit_form_from_key();
                                    }
                                }
                                disabled=move || disabled.get()
                            />
                        </div>
                        <div class="room-input-actions">
                            <div class="relative">
                                <button type="button" class="room-input-settings" aria-label=t("Settings")
                                    on:click=move |_| settings_open.update(|o| *o = !*o)>
                                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="h-5 w-5" aria-hidden="true">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M9.594 3.94c.09-.542.56-.94 1.11-.94h2.593c.55 0 1.02.398 1.11.94l.213 1.281c.063.374.313.686.645.87.074.04.147.083.22.127.324.196.72.257 1.075.124l1.217-.456a1.125 1.125 0 011.37.49l1.296 2.247a1.125 1.125 0 01-.26 1.431l-1.003.827c-.293.24-.438.613-.431.992a6.759 6.759 0 010 .255c-.007.378.138.75.43.99l1.005.828c.424.35.534.954.26 1.43l-1.298 2.247a1.125 1.125 0 01-1.369.491l-1.217-.456c-.355-.133-.75-.072-1.076.124a6.57 6.57 0 01-.22.128c-.331.183-.581.495-.644.869l-.213 1.28c-.09.543-.56.941-1.11.941h-2.594c-.55 0-1.02-.398-1.11-.94l-.213-1.281c-.062-.374-.312-.686-.644-.87a6.52 6.52 0 01-.22-.127c-.325-.196-.72-.257-1.076-.124l-1.217.456a1.125 1.125 0 01-1.369-.49l-1.297-2.247a1.125 1.125 0 01.26-1.431l1.004-.827c.292-.24.437-.613.43-.992a6.932 6.932 0 010-.255c.007-.378-.138-.75-.43-.99l-1.004-.828a1.125 1.125 0 01-.26-1.43l1.297-2.247a1.125 1.125 0 011.37-.491l1.216.456c.356.133.751.072 1.076-.124.072-.044.146-.087.22-.128.332-.183.582-.495.644-.869l.214-1.281z" />
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                                    </svg>
                                </button>
                                <Show when=move || settings_open.get()>
                                    <div class="fixed inset-0 z-40" on:click=move |_| settings_open.set(false)></div>
                                    <div class="retro-dropdown-panel absolute bottom-full right-0 z-50 mb-2 w-64 py-2">
                                        <div class="space-y-3 px-4 py-2">
                                            <div class="flex items-center justify-between">
                                                <label class="text-sm text-white/70">{t("Auto-focus")}</label>
                                                <button type="button" aria-pressed=move || auto_focus.get()
                                                    class=move || format!(
                                                        "relative inline-flex h-6 w-11 items-center rounded-full transition-colors duration-200 {}",
                                                        if auto_focus.get() { "bg-brand-accent-dark" } else { "bg-brand-midnight" }
                                                    )
                                                    on:click=move |_| {
                                                        auto_focus.update(|v| *v = !*v);
                                                        store_flag("autoFocus", auto_focus.get_untracked());
                                                    }>
                                                    <span class=move || format!(
                                                        "inline-block h-4 w-4 transform rounded-full bg-white transition-transform duration-200 {}",
                                                        if auto_focus.get() { "translate-x-6" } else { "translate-x-1" }
                                                    )></span>
                                                </button>
                                            </div>
                                            <div class="flex items-center justify-between">
                                                <label class="text-sm text-white/70">{t("Answer sounds")}</label>
                                                <button type="button" aria-pressed=move || answer_sounds.get()
                                                    class=move || format!(
                                                        "relative inline-flex h-6 w-11 items-center rounded-full transition-colors duration-200 {}",
                                                        if answer_sounds.get() { "bg-brand-accent-dark" } else { "bg-brand-midnight" }
                                                    )
                                                    on:click=move |_| {
                                                        answer_sounds.update(|v| *v = !*v);
                                                        store_flag("answerSounds", answer_sounds.get_untracked());
                                                        if answer_sounds.get_untracked() {
                                                            play_feedback("bad", true, volume.get_untracked());
                                                        }
                                                    }>
                                                    <span class=move || format!(
                                                        "inline-block h-4 w-4 transform rounded-full bg-white transition-transform duration-200 {}",
                                                        if answer_sounds.get() { "translate-x-6" } else { "translate-x-1" }
                                                    )></span>
                                                </button>
                                            </div>
                                            <div>
                                                <label class="mb-1 block text-sm text-white/70">{t("Volume")}</label>
                                                <input type="range" min="0" max="1" step="0.05"
                                                    class="w-full cursor-pointer accent-brand-accent"
                                                    prop:value=move || volume.get().to_string()
                                                    on:input=move |ev| {
                                                        let v: f64 = event_target_value(&ev).parse().unwrap_or(1.0);
                                                        volume.set(v);
                                                    }/>
                                            </div>
                                        </div>
                                    </div>
                                </Show>
                            </div>

                            <button type="submit"
                                class=move || {
                                    if pending_count.get() > 0 {
                                        "room-input-submit room-input-submit--busy"
                                    } else {
                                        "room-input-submit"
                                    }
                                }
                                disabled=move || disabled.get() || text.get().trim().is_empty()
                                aria-busy=move || pending_count.get() != 0
                                aria-label=t("Send")>
                                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2.5" stroke="currentColor" class="h-5 w-5" aria-hidden="true">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5" />
                                </svg>
                            </button>
                        </div>
                    </div>
                    {move || blurred.get().then(|| view! {
                        <p class="room-input-hint mt-1 flex items-center gap-1 text-xs text-red-400">
                            {t("Answer locked — you left the tab. Next track will unlock it.")}
                        </p>
                    })}
                    {move || hint.get().map(|h| view! {
                        <p class="room-input-hint mt-1 flex items-center gap-1 text-xs text-brand-secondary">
                            <span>{h}</span>
                        </p>
                    })}
                </div>
            </form>

            <div class="room-answer-chips-row relative min-w-0">
                <ul class="room-answer-chips flex flex-wrap gap-2 text-sm sm:gap-4">
                    {move || track.get().map(|tr| {
                        let fids = found_ids.get();
                        let f = found.get();
                        let flashing = flashing_ids.get();
                        tr.answers.into_iter().map(|a| {
                            let is_found = fids.contains(&a.id);
                            let good = f.iter().find(|g| g.id == a.id);
                            let mut cls = String::from("room-answer-chip");
                            if is_found {
                                cls.push_str(" room-answer-chip--found");
                            }
                            if flashing.contains(&a.id) {
                                cls.push_str(" room-answer-chip--flash");
                            }
                            let label = if is_found {
                                good.and_then(|g| g.value.clone()).or(a.value.clone()).unwrap_or_default()
                            } else {
                                format!("{} ?", t(&a.name))
                            };
                            let icon = good
                                .and_then(|g| g.answer_type.as_ref()?.svg_icon.clone())
                                .unwrap_or_default();
                            view! { <li class=cls>
                                {move || (!icon.is_empty()).then(|| view! {
                                    <span class="mr-2 text-white/90" inner_html=icon.clone()></span>
                                })}
                                <span class="font-semibold text-white">{label}</span>
                            </li> }
                        }).collect_view()
                    })}
                </ul>
            </div>
        </div>
    }
}
