//! Room chat — `Chat.vue` + `Message.vue` + `Moderation.vue` ports:
//! `.chat-panel` with header (title + alert moderators + FAQ link), avatar
//! messages newest-first (`flex-col-reverse`), composer, guest banner,
//! per-message reactions (`chat.message.{id}` channel), report button,
//! team badge, reports counter, moderation modal with BanForm.

use crate::api;
use crate::auth::use_auth;
use crate::i18n::t;
use crate::ws::Realtime;
use leptos::ev;
use leptos::prelude::*;
use leptos_router::components::A;
use send_wrapper::SendWrapper;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::rc::Rc;

/// `chat_reactions` + `publicModerators` shared props — fetched once per
/// chat mount (`GET /api/v1/chat/config`).
#[derive(Clone, Default)]
struct ChatConfig {
    standard_emojis: Vec<String>,
    supporter_emojis: Vec<String>,
    public_moderator_ids: HashSet<i64>,
}

/// `userHasSupporterReactions` — donor perk gate.
fn can_use_supporter_reactions(perks: &[String], is_supporter: bool) -> bool {
    perks.iter().any(|p| p == "supporter_reactions")
        || (!perks.is_empty() && is_supporter)
        || (perks.is_empty() && is_supporter)
}

#[component]
pub fn RoomChat(
    room_id: i64,
    rt: Realtime,
    /// Public moderator / admin — can delete messages.
    can_delete: bool,
    /// Guest users get the register/login banner instead of the composer.
    /// Signal — the guest account can be created after mount.
    is_guest: Signal<bool>,
    /// Room moderator ids — their messages get the accent styling + badge.
    /// Signal — `show` payload can land after the first render.
    moderator_ids: Signal<HashSet<i64>>,
) -> impl IntoView {
    let auth = use_auth();
    let messages = RwSignal::new(Vec::<Value>::new());
    let draft = RwSignal::new(String::new());
    let sending = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);
    let reported = RwSignal::new(false);
    let config = RwSignal::new(ChatConfig::default());
    let channel = format!("chat-room.{room_id}");

    // History once.
    {
        let messages = messages.clone();
        leptos::task::spawn_local(async move {
            if let Ok(v) = api::get::<Value>(&format!("/api/v1/rooms/{room_id}/messages")).await {
                if let Some(arr) = v["messages"].as_array() {
                    messages.set(arr.clone());
                }
            }
        });
    }

    // Emoji catalog + public moderator ids (Inertia shared props port).
    {
        let config = config;
        leptos::task::spawn_local(async move {
            if let Ok(v) = api::get::<Value>("/api/v1/chat/config").await {
                config.set(ChatConfig {
                    standard_emojis: v["reactions"]["standard"]
                        .as_array()
                        .map(|a| a.iter().filter_map(|e| e.as_str().map(str::to_string)).collect())
                        .unwrap_or_default(),
                    supporter_emojis: v["reactions"]["supporter"]
                        .as_array()
                        .map(|a| a.iter().filter_map(|e| e.as_str().map(str::to_string)).collect())
                        .unwrap_or_default(),
                    public_moderator_ids: v["public_moderator_ids"]
                        .as_array()
                        .map(|a| a.iter().filter_map(|e| e.as_i64()).collect())
                        .unwrap_or_default(),
                });
            }
        });
    }

    // Live events.
    {
        let rt = rt.clone();
        rt.subscribe(&channel);
        let ch = channel.clone();
        rt.listen(&ch, "NewMessage", Rc::new(move |data| {
            if let Some(m) = data.get("message") {
                messages.update(|msgs| {
                    // Dedupe by id (our own POST returns 204 — broadcast is source).
                    let id = m["id"].as_i64().unwrap_or_default();
                    if !msgs.iter().any(|x| x["id"].as_i64() == Some(id)) {
                        msgs.push(m.clone());
                        if msgs.len() > 50 {
                            msgs.remove(0);
                        }
                    }
                });
            }
        }));
        let ch = channel.clone();
        let messages2 = messages;
        // `MessageReported` — replace the message in place (reports counter).
        rt.listen(&ch, "MessageReported", Rc::new(move |data| {
            if let Some(m) = data.get("message") {
                let id = m["id"].as_i64().unwrap_or_default();
                messages2.update(|msgs| {
                    if let Some(x) = msgs.iter_mut().find(|x| x["id"].as_i64() == Some(id)) {
                        *x = m.clone();
                    }
                });
            }
        }));
        let ch = channel.clone();
        rt.listen(&ch, "MessageDeleted", Rc::new(move |data| {
            let id = data["message"]["id"].as_i64().unwrap_or_default();
            messages.update(|msgs| {
                msgs.retain(|x| x["id"].as_i64() != Some(id));
            });
        }));
        let rtw = SendWrapper::new(rt);
        on_cleanup(move || rtw.leave(&channel));
    }

    let send = move |ev: ev::SubmitEvent| {
        ev.prevent_default();
        let body = draft.get().trim().to_string();
        if body.is_empty() || sending.get() {
            return;
        }
        sending.set(true);
        error.set(None);
        draft.set(String::new());
        #[derive(Serialize)]
        struct B {
            body: String,
        }
        // `chat::store` answers 204 — no JSON body to parse.
        leptos::task::spawn_local(async move {
            match api::post_no_content(
                &format!("/api/v1/rooms/{room_id}/messages"),
                &B { body },
            )
            .await
            {
                Ok(_) => {}
                Err(e) => {
                    error.set(Some(e.message));
                }
            }
            sending.set(false);
        });
    };

    // `AlertModeratorsModal.vue` — the button opens a modal with an optional
    // comment, not a direct POST.
    let alerting = RwSignal::new(false);
    let alert_message = RwSignal::new(String::new());
    let alert = move |_| {
        let msg = alert_message.get_untracked();
        leptos::task::spawn_local(async move {
            if api::post::<Value, _>(
                &format!("/api/v1/rooms/{room_id}/alert"),
                &json!({ "message": msg }),
            )
            .await
            .is_ok()
            {
                reported.set(true);
                alerting.set(false);
            }
        });
    };

    view! {
        <div class="chat-panel">
            <div class="chat-header">
                <span class="text-[10px] font-bold uppercase tracking-[0.2em] text-brand-accent">{t("Chat")}</span>
                {move || if reported.get() {
                    view! {
                        <div class="flex items-center text-xs text-brand-accent">
                            <svg xmlns="http://www.w3.org/2000/svg" class="mr-1 h-3.5 w-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" />
                            </svg>
                            {t("Alert sent")}
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <button type="button" class="room-action-btn !py-1.5 !text-[10px]"
                            title=t("Report a problem on the chat")
                            on:click=move |_| alerting.set(true)>
                            <svg xmlns="http://www.w3.org/2000/svg" class="h-3.5 w-3.5 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M15 17h5l-1.405-1.405A2.032 2.032 0 0118 14.158V11a6.002 6.002 0 00-4-5.659V5a2 2 0 10-4 0v.341C7.67 6.165 6 8.388 6 11v3.159c0 .538-.214 1.055-.595 1.436L4 17h5m6 0v1a3 3 0 11-6 0v-1m6 0H9" />
                            </svg>
                            <span class="hidden lg:inline">{t("Alert moderators")}</span>
                        </button>
                    }.into_any()
                }}
                <a href="/docs/faq" title="FAQ" target="_blank" rel="noopener noreferrer" class="chat-icon-btn !h-7 !w-7">
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="h-4 w-4">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M9.879 7.519c1.171-1.025 3.071-1.025 4.242 0 1.172 1.025 1.172 2.687 0 3.712-.203.179-.43.326-.67.442-.745.361-1.45.999-1.45 1.827v.75M21 12a9 9 0 11-18 0 9 9 0 0121 12zm-9 5.25h.008v.008H12v-.008z" />
                    </svg>
                </a>
            </div>

            <div class="chat-messages flex flex-1 flex-col-reverse gap-1 overflow-y-auto px-2 py-2">
                // flex-col-reverse renders first-in-DOM at the bottom — feed
                // messages newest-first so latest sits at the composer edge.
                <For
                    each=move || {
                        let mut list = messages.get();
                        list.reverse();
                        list
                    }
                    key=|m| m["id"].as_i64().unwrap_or_default()
                    children=move |m: Value| {
                        view! {
                            <ChatMessage
                                message=m
                                can_delete=can_delete
                                moderator_ids=moderator_ids
                                config=config
                                viewer=Signal::derive(move || auth.user.get())
                            />
                        }
                    }
                />
            </div>

            {move || error.get().map(|e| view! {
                <p class="px-3 pb-1 text-xs text-brand-danger">{e}</p>
            })}

            {move || if is_guest.get() {
                view! {
                    <div class="chat-composer flex w-full">
                        <div class="chat-guest-banner text-center">
                            <A href="/register" attr:class="font-medium text-brand-accent underline hover:text-brand-accent-hover">
                                {t("Register")}
                            </A>
                            {t(" or ")}
                            <A href="/login" attr:class="font-medium text-brand-accent underline hover:text-brand-accent-hover">
                                {t("Login")}
                            </A>
                            {t("to chat with other players")}
                        </div>
                    </div>
                }.into_any()
            } else {
                view! {
                    <div class="chat-composer relative flex w-full">
                        <form class="relative flex w-full text-sm" on:submit=send aria-label="Message form">
                            <div class="chat-input-wrap">
                                <input bind:value=draft type="text" autocomplete="off" maxlength="500"
                                    class="w-full border-0 bg-transparent py-3 pl-4 pr-12 text-base focus:outline-none focus:ring-0"
                                    placeholder=t("Type your message...")/>
                            </div>
                            <button type="submit" class="chat-send-btn" aria-label="Send message"
                                disabled=move || draft.get().trim().is_empty() || sending.get()>
                                <svg xmlns="http://www.w3.org/2000/svg" class="size-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                                    <path d="M3.714 3.048a.498.498 0 0 0-.683.627l2.843 7.627a2 2 0 0 1 0 1.396l-2.842 7.627a.498.498 0 0 0 .682.627l18-8.5a.5.5 0 0 0 0-.904z" />
                                    <path d="M6 12h16" />
                                </svg>
                            </button>
                        </form>
                    </div>
                }.into_any()
            }}
        </div>
        // `AlertModeratorsModal.vue` — optional comment + danger submit.
        <Show when=move || alerting.get()>
            <div class="fixed inset-0 z-[60] flex items-center justify-center bg-black/70 p-4"
                on:click=move |_| alerting.set(false)>
                <div class="w-full max-w-md rounded-xl border border-white/10 bg-surface-raised p-6 shadow-2xl"
                    on:click=|ev| ev.stop_propagation()>
                    <h2 class="mb-4 text-lg font-bold text-white">{t("Alerting moderators")}</h2>
                    <p class="mb-4 rounded-lg border border-amber-400/30 bg-amber-400/10 px-3 py-2 text-sm text-amber-200">
                        {t("Warning, this feature should only be used in case of rule breaking in the chat or a technical issue in the game")}
                    </p>
                    <label class="form-label">{t("Comment (optional)")}</label>
                    <textarea bind:value=alert_message rows="3"
                        class="form-input w-full resize-y"></textarea>
                    <div class="mt-4 flex justify-end gap-2">
                        <button type="button" class="btn-secondary" on:click=move |_| alerting.set(false)>
                            {t("Close")}
                        </button>
                        <button type="button" class="btn-danger" on:click=alert>
                            {t("Alerting moderators")}
                        </button>
                    </div>
                </div>
            </div>
        </Show>
    }
}

/// `Message.vue` port — one chat row: avatar (+mod crown), name (click →
/// moderation modal for room/public mods), team badge, reports counter,
/// time, body, reactions row, hover actions (react / report / delete).
#[component]
fn ChatMessage(
    message: Value,
    can_delete: bool,
    moderator_ids: Signal<HashSet<i64>>,
    config: RwSignal<ChatConfig>,
    viewer: Signal<Option<crate::types::User>>,
) -> impl IntoView {
    // `Realtime` is `Rc`-backed — pull it from context instead of threading
    // it through the `<For>` `children` closure (which must be `Send`).
    let rt = use_context::<crate::app::SharedRealtime>()
        .expect("SharedRealtime")
        .get_untracked()
        .expect("ws ready");
    let msg_id = message["id"].as_i64().unwrap_or_default();
    let uid = message["user"]["id"].as_i64().unwrap_or_default();
    let name = StoredValue::new(message["user"]["name"].as_str().unwrap_or("?").to_string());
    let body = StoredValue::new(message["body"].as_str().unwrap_or_default().to_string());
    let time = StoredValue::new(message["time"].as_str().unwrap_or_default().to_string());
    let photo = message["user"]["photo"]
        .as_str()
        .or_else(|| message["user"]["photo_path"].as_str())
        .map(str::to_string);
    let team_name = message["user"]["team"]["name"].as_str().map(str::to_string);
    let reports = RwSignal::new(message["reports"].as_i64().unwrap_or(0).abs());
    let reactions = RwSignal::new(message["reactions"].as_array().cloned().unwrap_or_default());
    let reactions_hydrated = message["reactions"].is_array();
    let user_reaction = RwSignal::new(message["user_reaction"].as_str().map(str::to_string));
    let show_picker = RwSignal::new(false);
    let reporting = RwSignal::new(false);
    let reported_once = RwSignal::new(false);
    let moderate = RwSignal::new(false);
    // Keep the latest message value for `MessageReported` updates handled by
    // the parent; local signals only track what the parent doesn't resend.
    let _ = &message;

    // Hydrate reactions when the payload didn't carry them (`hasHydratedReactions`).
    if !reactions_hydrated {
        let reactions = reactions;
        let user_reaction = user_reaction;
        leptos::task::spawn_local(async move {
            if let Ok(v) = api::get::<Value>(&format!("/api/v1/messages/{msg_id}/reactions")).await {
                if let Some(arr) = v["reactions"].as_array() {
                    reactions.set(arr.clone());
                }
                user_reaction.set(v["userReaction"].as_str().map(str::to_string));
            }
        });
    }

    // `chat.message.{id}` — `MessageReactionUpdated` broadcast.
    {
        let rt = rt.clone();
        let ch = format!("chat.message.{msg_id}");
        rt.subscribe(&ch);
        let reactions = reactions;
        let user_reaction = user_reaction;
        rt.listen(&ch, "MessageReactionUpdated", Rc::new(move |data| {
            if let Some(arr) = data["reactions"].as_array() {
                reactions.set(arr.clone());
            }
            user_reaction.set(data["userReaction"].as_str().map(str::to_string));
        }));
        let rtw = SendWrapper::new(rt);
        on_cleanup(move || rtw.leave(&ch));
    }

    let toggle_reaction = move |emoji: String| {
        let msg_id = msg_id;
        let prev = user_reaction.get_untracked();
        leptos::task::spawn_local(async move {
            #[derive(Serialize)]
            struct B<'a> {
                emoji: &'a str,
            }
            if prev.as_deref() == Some(emoji.as_str()) {
                let _ = api::post::<Value, _>(
                    &format!("/api/v1/messages/{msg_id}/reactions"),
                    &B { emoji: &emoji },
                )
                .await;
            } else {
                if let Some(old) = prev {
                    let _ = api::post::<Value, _>(
                        &format!("/api/v1/messages/{msg_id}/reactions"),
                        &B { emoji: &old },
                    )
                    .await;
                }
                let _ = api::post::<Value, _>(
                    &format!("/api/v1/messages/{msg_id}/reactions"),
                    &B { emoji: &emoji },
                )
                .await;
            }
            // State updates come via the `MessageReactionUpdated` broadcast.
        });
    };

    let report = move |_| {
        if reporting.get() || reported_once.get() {
            return;
        }
        reporting.set(true);
        leptos::task::spawn_local(async move {
            if api::post_no_content(&format!("/api/v1/messages/{msg_id}/report"), &json!({}))
                .await
                .is_ok()
            {
                reported_once.set(true);
            }
            reporting.set(false);
        });
    };

    let delete_msg = move |_| {
        leptos::task::spawn_local(async move {
            let _ = api::delete_no_content(&format!("/api/v1/messages/{msg_id}")).await;
        });
    };

    let viewer_is_room_mod = move || {
        viewer
            .get()
            .map(|u| moderator_ids.get().contains(&u.id))
            .unwrap_or(false)
    };
    let viewer_is_public_mod = move || {
        viewer
            .get()
            .map(|u| u.is_public_moderator || config.get().public_moderator_ids.contains(&u.id))
            .unwrap_or(false)
    };
    // `isModerator || userIsPublicModerator` — opens the Moderation modal.
    let viewer_can_moderate = move || viewer_is_room_mod() || viewer_is_public_mod();
    // `shouldShowReportButton` — hidden on public-moderator messages.
    let author_is_public_mod = config.get().public_moderator_ids.contains(&uid);
    let from_mod = moderator_ids.get().contains(&uid);
    // Supporter emoji gate — `userHasSupporterReactions`.
    let can_supporter = move || {
        viewer
            .get()
            .map(|u| can_use_supporter_reactions(&u.donor_perks, u.is_supporter))
            .unwrap_or(false)
    };

    view! {
        <div class="chat-message group">
            // Hover actions — react + report (top-right).
            <div class="absolute right-2 top-2 z-20 hidden flex-row gap-1 group-hover:flex">
                <button type="button" class="chat-icon-btn text-base" title="Réagir"
                    on:click=move |_| show_picker.update(|v| *v = !*v)>"😊"</button>
                {(!author_is_public_mod).then(|| view! {
                    <button type="button" class="chat-icon-btn disabled:opacity-50"
                        disabled=move || reporting.get()
                        title=move || if reported_once.get() { t("Reported") } else { t("Report this message") }
                        on:click=report>
                        {move || if reported_once.get() {
                            view! {
                                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="size-4 text-brand-accent">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M9 12.75L11.25 15 15 9.75M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
                                </svg>
                            }.into_any()
                        } else {
                            view! {
                                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="size-4 text-brand-primary">
                                    <path stroke-linecap="round" stroke-linejoin="round" d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126ZM12 15.75h.007v.008H12v-.008Z" />
                                </svg>
                            }.into_any()
                        }}
                    </button>
                })}
            </div>

            <div class="flex items-start gap-3">
                <div class="relative">
                    {match photo.clone() {
                        Some(p) => view! {
                            // `user.photo` is a full URL (accessor); raw
                            // storage paths from older payloads get the
                            // /storage/ prefix.
                            <img src=if p.starts_with("http") { p.clone() } else { format!("/storage/{p}") } alt=name.get_value()
                                class=format!(
                                    "h-10 w-10 rounded-full border-2 object-cover {}",
                                    if from_mod { "border-brand-accent" } else { "border-white/20" }
                                )/>
                        }.into_any(),
                        None => view! {
                            <div class=format!(
                                "flex h-10 w-10 items-center justify-center rounded-full border-2 bg-brand-deep text-sm font-bold text-white/60 {}",
                                if from_mod { "border-brand-accent" } else { "border-white/20" }
                            )>
                                {name.get_value().chars().next().unwrap_or('?').to_uppercase().to_string()}
                            </div>
                        }.into_any(),
                    }}
                    {from_mod.then(|| view! {
                        <div class="absolute -bottom-1 -right-1 bg-brand-accent p-0.5" title="Room Moderator">
                            <svg xmlns="http://www.w3.org/2000/svg" class="h-3 w-3 text-white" viewBox="0 0 20 20" fill="currentColor">
                                <path fill-rule="evenodd" d="M2.166 4.999A11.954 11.954 0 0010 1.944 11.954 11.954 0 0017.834 5c.11.65.166 1.32.166 2.001 0 5.225-3.34 9.67-8 11.317C5.34 16.67 2 12.225 2 7c0-.682.057-1.35.166-2.001zm11.541 3.708a1 1 0 00-1.414-1.414L9 10.586 7.707 9.293a1 1 0 00-1.414 1.414l2 2a1 1 0 001.414 0l4-4z" clip-rule="evenodd"/>
                            </svg>
                        </div>
                    })}
                </div>
                <div class="min-w-0 flex-1">
                    <div class="relative mb-1 flex items-center justify-between">
                        <div class="flex items-center gap-2 overflow-hidden">
                            // Username — opens the moderation modal when the
                            // viewer is a room/public moderator.
                            {move || if viewer_can_moderate() {
                                let n = name.get_value();
                                view! {
                                    <button type="button"
                                        class=format!("truncate text-sm font-bold hover:underline {}", if from_mod { "text-brand-accent" } else { "text-white" })
                                        on:click=move |_| moderate.set(true)>
                                        {n}
                                    </button>
                                }.into_any()
                            } else {
                                let n = name.get_value();
                                view! {
                                    <span class=format!("truncate text-sm font-bold {}", if from_mod { "text-brand-accent" } else { "text-white" })>
                                        {n}
                                    </span>
                                }.into_any()
                            }}
                            {team_name.clone().map(|tn| view! {
                                <span class="truncate border border-white/10 bg-brand-midnight px-1.5 py-0.5 text-xs text-white/60">
                                    {tn}
                                </span>
                            })}
                        </div>
                        <div class="flex items-center gap-1">
                            {move || (reports.get() >= 1).then(|| view! {
                                <span class="ml-2 whitespace-nowrap text-xs text-brand-secondary" title=t("Reports")>
                                    " " {reports.get()} " " {if reports.get() == 1 { t("Report") } else { t("Reports") }}
                                </span>
                            })}
                            <span class="ml-2 shrink-0 whitespace-nowrap text-xs text-white/40">{time.get_value()}</span>
                            {can_delete.then(|| view! {
                                <button type="button"
                                    class="chat-icon-btn hidden !h-6 !w-6 text-brand-primary group-hover:flex"
                                    title=t("Delete this message")
                                    on:click=delete_msg>
                                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="h-3.5 w-3.5">
                                        <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                                    </svg>
                                </button>
                            })}
                        </div>
                    </div>
                    <p class="whitespace-pre-wrap break-words text-sm leading-relaxed text-white/90">{body.get_value()}</p>

                    // Reactions row.
                    <div class="relative mt-2 flex flex-wrap items-center gap-1">
                        <For
                            each=move || reactions.get()
                            key=|r| r["emoji"].as_str().unwrap_or_default().to_string()
                            children=move |r: Value| {
                                let emoji = r["emoji"].as_str().unwrap_or_default().to_string();
                                let count = r["count"].as_i64().unwrap_or(0);
                                let users: Vec<String> = r["users"].as_array()
                                    .map(|a| a.iter().filter_map(|u| u["name"].as_str().map(str::to_string)).collect())
                                    .unwrap_or_default();
                                let e = emoji.clone();
                                let e2 = emoji.clone();
                                view! {
                                    <div class="group/emoji relative">
                                        <button type="button"
                                            class=move || {
                                                let mut c = "chat-reaction".to_string();
                                                if user_reaction.get().as_deref() == Some(e.as_str()) {
                                                    c.push_str(" chat-reaction--mine");
                                                }
                                                if config.get().supporter_emojis.iter().any(|s| s == &e) {
                                                    c.push_str(" chat-reaction--supporter");
                                                }
                                                c
                                            }
                                            on:click=move |_| toggle_reaction(e2.clone())>
                                            <span>{emoji.clone()}</span>
                                            <span class="text-xs font-medium">{count}</span>
                                        </button>
                                        {(!users.is_empty()).then(|| view! {
                                            <span class="pointer-events-none absolute bottom-full left-1/2 z-20 mb-2 -translate-x-1/2 whitespace-nowrap border border-white/10 bg-brand-deep px-2 py-1 text-xs text-white/80 opacity-0 shadow-lg transition-opacity group-hover/emoji:opacity-100">
                                                {users.join(", ")}
                                            </span>
                                        })}
                                    </div>
                                }
                            }
                        />
                    </div>
                </div>
            </div>

            // Emoji picker — `retro-dropdown-panel` floating above the message.
            {move || show_picker.get().then(|| {
                let cfg = config.get();
                let can_sup = can_supporter();
                view! {
                    <div class="retro-dropdown-panel absolute right-2 top-10 z-40 flex flex-wrap gap-1 border border-white/10 bg-brand-deep p-2 shadow-lg">
                        {cfg.standard_emojis.iter().map(|e| {
                            let e = e.clone();
                            let e2 = e.clone();
                            view! {
                                <button type="button" class="text-xl transition-transform hover:scale-125"
                                    on:click=move |_| { toggle_reaction(e2.clone()); show_picker.set(false); }>
                                    {e}
                                </button>
                            }
                        }).collect::<Vec<_>>()}
                        {(!cfg.supporter_emojis.is_empty()).then(|| view! {
                            <div class="my-1 w-full border-t border-white/10" />
                            <p class=format!("w-full px-1 text-[10px] font-semibold uppercase tracking-wide {}", if can_sup { "text-amber-300/90" } else { "text-white/35" })>
                                {t("Supporter reactions")}
                            </p>
                            {cfg.supporter_emojis.iter().map(|e| {
                                let e = e.clone();
                                let e2 = e.clone();
                                view! {
                                    <button type="button"
                                        class=format!("text-xl transition-transform {}", if can_sup { "hover:scale-125" } else { "cursor-not-allowed opacity-40" })
                                        disabled={!can_sup}
                                        title={if can_sup { String::new() } else { t("Supporter reaction emoji locked").to_string() }}
                                        on:click=move |_| { if can_sup { toggle_reaction(e2.clone()); show_picker.set(false); } }>
                                        {e}
                                    </button>
                                }
                            }).collect::<Vec<_>>()}
                        })}
                    </div>
                }
            })}

            // `Moderation.vue` modal — room/public moderators only.
            {move || moderate.get().then(|| {
                let author_is_room_mod = moderator_ids.get().contains(&uid);
                view! {
                    <ChatModerationModal
                        name=name.get_value()
                        body=body.get_value()
                        time=time.get_value()
                        user_id=uid
                        message_id=msg_id
                        show_ban=move || !author_is_room_mod && viewer_is_public_mod()
                        on_close=move || moderate.set(false)
                    />
                }
            })}
        </div>
    }
}

/// `Moderation.vue` port — modal over a message: quote, delete, BanForm.
#[component]
fn ChatModerationModal(
    #[prop(into)] name: String,
    #[prop(into)] body: String,
    #[prop(into)] time: String,
    user_id: i64,
    message_id: i64,
    show_ban: impl Fn() -> bool + Copy + Send + 'static,
    on_close: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let name = StoredValue::new(name);
    let body = StoredValue::new(body);
    let time = StoredValue::new(time);
    let show_ban_form = RwSignal::new(false);
    let banned = RwSignal::new(false);
    let delete = move |_| {
        leptos::task::spawn_local(async move {
            let _ = api::delete_no_content(&format!("/api/v1/messages/{message_id}")).await;
            on_close();
        });
    };
    view! {
        <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/70 p-4"
            on:click=move |_| on_close()>
            <div class="w-full max-w-md rounded-xl border border-white/10 bg-surface-raised shadow-2xl"
                on:click=move |e| e.stop_propagation()>
                <div class="flex w-full items-center justify-between border-b border-white/10 px-4 py-3">
                    <h2 class="retro-title retro-title--primary">{t("Moderation")}</h2>
                    <span class="pr-2 text-sm italic text-white/60">{name.get_value()}</span>
                </div>
                <div class="p-4">
                    <blockquote class="my-2 border-l border-brand-accent/40 px-4 py-2 font-mono text-white/70">
                        <span class="mr-1 text-xs">{time.get_value()}</span>
                        <br/>
                        {body.get_value()}
                    </blockquote>
                    <div class="my-4 flex items-center gap-4">
                        {move || if banned.get() {
                            view! { <span class="text-sm text-brand-accent">{t("User banned")}</span> }.into_any()
                        } else {
                            let n = name.get_value();
                            view! {
                                <button type="button" class="btn-danger btn-sm" on:click=delete>{t("Delete message")}</button>
                                {show_ban().then(|| view! {
                                    <button type="button" class="btn-danger btn-sm"
                                        on:click=move |_| show_ban_form.update(|v| *v = !*v)>
                                        {t("Ban")} " " {n.clone()}
                                    </button>
                                })}
                                <button type="button" class="btn-secondary btn-sm" on:click=move |_| on_close()>{t("Cancel")}</button>
                            }.into_any()
                        }}
                    </div>
                    {move || (show_ban_form.get() && !banned.get()).then(|| view! {
                        <ChatBanForm user_id=user_id name=name.get_value() on_banned=move || banned.set(true)/>
                    })}
                </div>
            </div>
        </div>
    }
}

/// `BanForm.vue` port — duration + reason selects → `POST moderation/users/{id}/ban`.
#[component]
fn ChatBanForm(
    user_id: i64,
    name: String,
    on_banned: impl Fn() + Copy + Send + 'static,
) -> impl IntoView {
    let duration = RwSignal::new(Option::<i64>::None);
    let reason = RwSignal::new(String::new());
    let busy = RwSignal::new(false);
    // `reasons` — same list as BanForm.vue.
    let reasons = [
        "Pseudonyme inapproprié.",
        "Langage inapproprié.",
        "Propos injurieux, sexistes ou racistes.",
        "Menace ou harcèle d'autres joueurs.",
        "Donne les réponses dans le chat.",
        "Utilise un nouveau compte alors que le joueur a déjà été banni.",
        "Troll, spam.",
        "Triche.",
    ];
    let submit = move |ev: ev::SubmitEvent| {
        ev.prevent_default();
        if reason.get().is_empty() || busy.get() {
            return;
        }
        busy.set(true);
        let r = reason.get();
        let d = duration.get();
        leptos::task::spawn_local(async move {
            // `user.ban` — PHP sends `expired_at` ("+1 day"…); the API takes
            // minutes (`BanUserRequest` duration).
            if api::post::<Value, _>(
                &format!("/api/v1/moderation/users/{user_id}/ban"),
                &json!({"reason": r, "duration": d}),
            )
            .await
            .is_ok()
            {
                on_banned();
            }
            busy.set(false);
        });
    };
    view! {
        <form class="m-4 rounded border border-white/10 p-4 text-sm" on:submit=submit>
            <label class="mb-1 block text-xs text-white/50">{t("Duration")}</label>
            <select class="mb-2 w-full rounded border border-white/10 bg-surface-base px-2 py-1.5 text-white"
                on:change=move |ev| {
                    duration.set(match event_target_value(&ev).as_str() {
                        "+1 day" => Some(1440),
                        "+1 week" => Some(10080),
                        "+1 month" => Some(43200),
                        _ => None,
                    });
                }>
                <option value="+1 day" class="bg-surface-raised">{t("One day")}</option>
                <option value="+1 week" class="bg-surface-raised">{t("One week")}</option>
                <option value="+1 month" class="bg-surface-raised">{t("One month")}</option>
                <option value="" class="bg-surface-raised">{t("Unlimited")}</option>
            </select>
            <label class="mb-1 block text-xs text-white/50">{t("Reason")}</label>
            <select class="w-full rounded border border-white/10 bg-surface-base px-2 py-1.5 text-white"
                on:change=move |ev| reason.set(event_target_value(&ev))>
                <option value="" class="bg-surface-raised"></option>
                {reasons.iter().map(|r| view! {
                    <option value=*r class="bg-surface-raised">{t(r)}</option>
                }).collect::<Vec<_>>()}
            </select>
            <button type="submit" class="btn-danger btn-sm my-4" disabled=move || busy.get()>
                {t("Confirm and ban")} " " {name}
            </button>
        </form>
    }
}
