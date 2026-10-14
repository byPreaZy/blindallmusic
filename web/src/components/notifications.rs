use crate::api;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use serde_json::{json, Value};

#[component]
pub fn Notifications() -> impl IntoView {
    let items = use_context::<crate::app::NotificationFeed>()
        .unwrap_or_else(|| RwSignal::new(Vec::new()));
    let open = RwSignal::new(false);
    let busy = RwSignal::new(false);

    leptos::task::spawn_local(async move {
        if let Ok(v) = api::get::<Value>("/api/v1/users/notifications").await {
            items.set(v["notifications"].as_array().cloned().unwrap_or_default());
        }
    });

    let remove = move |id: &str| {
        items.update(|l| l.retain(|n| n["id"].as_str() != Some(id)));
    };

    let mark_read = move |id: String| {
        remove(&id);
        leptos::task::spawn_local(async move {
            let _ = api::post_empty::<Value>(
                &format!("/api/v1/users/notifications/{id}/read"),
            ).await;
        });
    };

    let mark_done = move |id: String| {
        remove(&id);
        leptos::task::spawn_local(async move {
            let _ = api::post_empty::<Value>(
                &format!("/api/v1/users/notifications/{id}/done"),
            ).await;
        });
    };

    let team_request_action = move |request_id: i64, notif_id: String, action: &'static str| {
        if busy.get() {
            return;
        }
        busy.set(true);
        leptos::task::spawn_local(async move {
            let body = json!({"notification_id": notif_id});
            let _ = api::post::<Value, _>(
                &format!("/api/v1/teams/requests/{request_id}/{action}"),
                &body,
            ).await;
            items.update(|l| l.retain(|n| n["id"].as_str() != Some(notif_id.as_str())));
            busy.set(false);
        });
    };

    let count = move || items.get().len();

    let popup = use_context::<crate::app::NotificationPopup>()
        .unwrap_or_else(|| RwSignal::new(None));

    view! {
        <div class="relative">
            {move || {
                let p = popup.get()?;
                let data = p["data"].clone();
                let message = data["message"].as_str().unwrap_or("").to_string();
                let room_slug = data["room"]["slug"].as_str().unwrap_or("").to_string();
                let nid = p["id"].as_str().unwrap_or("").to_string();
                let nid_done = nid.clone();
                Some(view! {
                    <div class="fixed left-0 right-0 top-0 z-30 flex w-full justify-center px-4">
                        <div class="retro-flash retro-flash--success my-2 max-w-2xl p-2">
                            <div class="mb-1 flex items-center justify-between border-b border-white/10 pb-1">
                                <h4 class="flex items-center font-bold uppercase text-red-400">
                                    {t("Moderation alert")}
                                </h4>
                                <span class="text-xs text-neutral-400">
                                    {data["created_at"].as_str().unwrap_or("").to_string()}
                                </span>
                            </div>
                            <div class="my-2 text-sm font-medium">
                                {(!room_slug.is_empty()).then(|| view! {
                                    <A href=format!("/rooms/{room_slug}")>{message}</A>
                                })}
                            </div>
                            <button type="button" class="btn-primary !px-2 !py-1 !text-xs ml-auto mt-2 flex items-center"
                                on:click=move |_| { mark_done(nid_done.clone()); popup.set(None); }>
                                "✓ " {t("Done")}
                            </button>
                        </div>
                    </div>
                })
            }}
            <button type="button" class="retro-icon-btn relative cursor-pointer" title="Notifications"
                on:click=move |_| open.update(|o| *o = !*o)>
                {move || (count() > 0).then(|| view! {
                    <span class="absolute -right-1 -top-1 z-10 flex h-5 w-5 items-center justify-center truncate rounded-full border-2 border-brand-midnight bg-brand-primary text-[10px] font-bold text-white shadow-lg">
                        {if count() > 99 { "99+".to_string() } else { count().to_string() }}
                    </span>
                })}
                <svg xmlns="http://www.w3.org/2000/svg" class="h-5 w-5 text-white/70 transition-colors hover:text-white"
                    fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"
                    stroke-linecap="round" stroke-linejoin="round">
                    <path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"/>
                    <path d="M13.73 21a2 2 0 0 1-3.46 0"/>
                </svg>
            </button>

            <Show when=move || open.get()>
                <div class="fixed inset-0 z-40" on:click=move |_| open.set(false)/>
                <div class="absolute right-0 z-50 mt-2 w-96 max-w-[90vw] rounded-xl border border-white/10 bg-surface-raised shadow-2xl">
                    <div class="max-h-96 overflow-y-auto p-2">
                        {move || {
                            let list = items.get();
                            if list.is_empty() {
                                return vec![view! {
                                    <div class="p-4 text-center text-sm text-white/50">{t("No notification")}</div>
                                }.into_any()];
                            }
                            list.into_iter().map(|n| {
                                let id = n["id"].as_str().unwrap_or("").to_string();
                                let footer_id = id.clone();
                                let ty = n["type"].as_str().unwrap_or("");
                                let data = n["data"].clone();
                                let message = data["message"].as_str().unwrap_or("").to_string();
                                let room_slug = data["room"]["slug"].as_str().unwrap_or("").to_string();
                                let short_ty = ty.rsplit('\\').next().unwrap_or(ty);
                                view! {
                                    <div class="my-2 rounded-lg border border-white/10 bg-brand-midnight p-3">
                                        {match short_ty {
                                            "NewTeamRequest" => view! {
                                                <div>
                                                    <h4 class="mb-1 border-b border-white/10 pb-1 text-xs uppercase text-white/40">{t("Team")}</h4>
                                                    <p class="my-2 text-sm font-medium text-white/90">{message}</p>
                                                    <div class="mt-1 flex items-center justify-end gap-2">
                                                        {data["teamRequest"]["id"].as_i64().map(|rid| {
                                                            let nid = id.clone();
                                                            let nid2 = id.clone();
                                                            view! {
                                                                <button type="button"
                                                                    class="rounded border border-red-500/40 px-2 py-1 text-xs text-red-400 disabled:opacity-40"
                                                                    disabled=move || busy.get()
                                                                    on:click=move |_| team_request_action(rid, nid.clone(), "decline")>
                                                                    {t("Decline")}
                                                                </button>
                                                                <button type="button"
                                                                    class="btn-primary !px-2 !py-1 !text-xs disabled:opacity-40"
                                                                    disabled=move || busy.get()
                                                                    on:click=move |_| team_request_action(rid, nid2.clone(), "accept")>
                                                                    {t("Accept")}
                                                                </button>
                                                            }
                                                        })}
                                                    </div>
                                                </div>
                                            }.into_any(),
                                            "NewSuggestion" => view! {
                                                <div>
                                                    <h4 class="mb-1 border-b border-white/10 pb-1 text-xs font-bold uppercase text-white/70">{t("Suggestion")}</h4>
                                                    <p class="my-2 whitespace-pre-line text-sm text-white/90">{message}</p>
                                                    <p class="text-xs text-white/40">
                                                        {t("Sent by")} " " {data["user"]["name"].as_str().unwrap_or("").to_string()}
                                                        " " {t("on")} " " {data["room"]["name"].as_str().unwrap_or("").to_string()}
                                                    </p>
                                                    {data["room"]["playlists"].as_str().filter(|s| !s.is_empty()).map(|p| view! {
                                                        <p class="text-xs text-white/40">"Playlists : " {p.to_string()}</p>
                                                    })}
                                                    <button type="button" class="btn-primary !px-2 !py-1 !text-xs ml-auto mt-2 block"
                                                        on:click=move |_| mark_done(id.clone())>
                                                        "✓ " {t("Done")}
                                                    </button>
                                                </div>
                                            }.into_any(),
                                            "NewRoomAlert" => view! {
                                                <div>
                                                    <h4 class="mb-1 border-b border-white/10 pb-1 text-xs font-bold uppercase text-red-400">
                                                        {t("Moderation alert")}
                                                    </h4>
                                                    <p class="my-2 text-sm font-medium text-white/90">
                                                        {(!room_slug.is_empty()).then(|| view! {
                                                            <A href=format!("/rooms/{room_slug}") attr:class="hover:text-brand-accent-light"
                                                                on:click=move |_| open.set(false)>
                                                                {message}
                                                            </A>
                                                        })}
                                                    </p>
                                                    <button type="button" class="btn-primary !px-2 !py-1 !text-xs ml-auto mt-2 block"
                                                        on:click=move |_| mark_done(id.clone())>
                                                        "✓ " {t("Done")}
                                                    </button>
                                                </div>
                                            }.into_any(),
                                            "TeamRequestApproved" | "TeamRequestRejected" => {
                                                let text = if short_ty == "TeamRequestApproved" {
                                                    t("Your request has been accepted!").to_string()
                                                } else {
                                                    message.clone()
                                                };
                                                view! {
                                                    <div>
                                                        <h4 class="mb-1 border-b border-white/10 pb-1 text-xs uppercase text-white/40">
                                                            {t("Team")} " " {data["team"]["name"].as_str().unwrap_or("").to_string()}
                                                        </h4>
                                                        <p class="my-2 text-sm font-medium text-white/90">{text}</p>
                                                    </div>
                                                }.into_any()
                                            }
                                            _ => view! {
                                                <div>
                                                    <p class="my-1 text-sm text-white/90">{message}</p>
                                                </div>
                                            }.into_any(),
                                        }}
                                        <div class="mt-1 flex justify-end">
                                            <button type="button" class="text-white/40 hover:text-white"
                                                title={t("Mark as read")}
                                                on:click=move |_| mark_read(footer_id.clone())>
                                                <svg xmlns="http://www.w3.org/2000/svg" class="h-4 w-4" fill="none"
                                                    viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
                                                    <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12"/>
                                                </svg>
                                            </button>
                                        </div>
                                    </div>
                                }.into_any()
                            }).collect::<Vec<_>>()
                        }}
                    </div>
                </div>
            </Show>
        </div>
    }
}
