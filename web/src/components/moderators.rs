//! `ModeratorsManager.vue` ×2 (Rooms + Playlists share the same UI):
//! current moderators list + a debounced `/api/v1/users?search=` dropdown
//! to attach by name. `attach_url`/`detach_url` take `{user_id}` JSON —
//! playlist uses one URL for both verbs, room uses `/attach` + `/detach`.

use crate::{api, i18n::t, types::IdName};
use leptos::prelude::*;
use serde_json::{json, Value};

#[component]
pub fn ModeratorsSection(
    moderators: Vec<IdName>,
    attach_url: String,
    detach_url: String,
    can_manage: bool,
    on_changed: impl Fn(()) + 'static + Copy + Send + Sync,
    on_err: impl Fn(String) + 'static + Copy + Send + Sync,
) -> impl IntoView {
    let attach_url = StoredValue::new(attach_url);
    let detach_url = StoredValue::new(detach_url);
    let busy = RwSignal::new(false);

    let do_attach = move |uid: i64| {
        busy.set(true);
        let url = attach_url.get_value();
        leptos::task::spawn_local(async move {
            match api::post::<Value, _>(&url, &json!({"user_id": uid})).await {
                Ok(_) => on_changed(()),
                Err(e) => on_err(e.message),
            }
            busy.set(false);
        });
    };
    let do_detach = move |uid: i64| {
        busy.set(true);
        let url = detach_url.get_value();
        leptos::task::spawn_local(async move {
            match api::delete_with_body::<Value, _>(&url, &json!({"user_id": uid})).await {
                Ok(_) => on_changed(()),
                Err(e) => on_err(e.message),
            }
            busy.set(false);
        });
    };

    view! {
        <section class="mb-6 rounded-xl border border-white/10 bg-surface-raised/60 p-5">
            <h2 class="mb-3 text-sm font-semibold uppercase tracking-wider text-white/60">
                {t("Moderators")}
            </h2>
            <ul class="mb-4 space-y-1">
                {move || {
                    if moderators.is_empty() {
                        view! { <li class="text-sm text-white/40">{t("No moderator")}</li> }.into_any()
                    } else {
                        moderators.iter().map(|m| {
                            let mid = m.id;
                            let mname = m.name.clone();
                            view! {
                                <li class="flex items-center gap-3 text-sm">
                                    <span class="text-white/80">{mname}</span>
                                    <span class="text-white/30">"#{mid}"</span>
                                    {can_manage.then(|| view! {
                                        <button type="button"
                                            class="ml-auto text-xs text-red-400 hover:underline"
                                            disabled=move || busy.get()
                                            on:click=move |_| do_detach(mid)>
                                            {t("Remove")}
                                        </button>
                                    })}
                                </li>
                            }
                        }).collect::<Vec<_>>().into_any()
                    }
                }}
            </ul>
            {can_manage.then(|| view! {
                <UserSearchPicker on_pick=move |u| do_attach(u.id)/>
            })}
        </section>
    }
}

/// Debounced user autocomplete — `GET /api/v1/users?search=` → `{users.data}`.
#[component]
fn UserSearchPicker(
    on_pick: impl Fn(IdName) + 'static + Copy + Send + Sync,
) -> impl IntoView {
    let term = RwSignal::new(String::new());
    let results = RwSignal::new(Vec::<IdName>::new());
    let searching = RwSignal::new(false);
    let open = RwSignal::new(false);

    // 300ms debounce — last-write-wins on the term.
    Effect::new(move |_| {
        let q = term.get();
        if q.trim().len() < 2 {
            results.set(vec![]);
            open.set(false);
            return;
        }
        searching.set(true);
        leptos::task::spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(300).await;
            if term.get_untracked() != q {
                return; // a newer keystroke superseded us
            }
            match api::get::<Value>(&format!(
                "/api/v1/users?search={}",
                js_sys::encode_uri_component(&q)
            ))
            .await
            {
                Ok(v) => {
                    let list: Vec<IdName> = v["users"]["data"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(|u| serde_json::from_value(u.clone()).ok())
                                .collect()
                        })
                        .unwrap_or_default();
                    results.set(list);
                    open.set(true);
                }
                Err(_) => results.set(vec![]),
            }
            searching.set(false);
        });
    });

    view! {
        <div class="relative">
            <input type="text" placeholder={t("Add a moderator")}
                class="form-input w-full"
                prop:value=term on:input=move |ev| term.set(event_target_value(&ev))/>
            {move || searching.get().then(|| view! {
                <span class="absolute right-3 top-2.5 text-xs text-white/40">"…"</span>
            })}
            <Show when=move || open.get() && !results.get().is_empty()>
                <ul class="absolute z-20 mt-1 max-h-60 w-full overflow-y-auto rounded-lg border border-white/10 bg-surface-raised shadow-xl">
                    <For each=move || results.get() key=|u| u.id
                        children=move |u: IdName| {
                            let u2 = u.clone();
                            view! {
                                <li>
                                    <button type="button"
                                        class="flex w-full items-center gap-2 px-3 py-2 text-left text-sm text-white/80 hover:bg-white/10"
                                        on:click=move |_| {
                                            on_pick(u2.clone());
                                            term.set(String::new());
                                            results.set(vec![]);
                                            open.set(false);
                                        }>
                                        {u.name.clone()}
                                        <span class="ml-auto text-xs text-white/30">"#{u.id}"</span>
                                    </button>
                                </li>
                            }
                        }
                    />
                </ul>
            </Show>
        </div>
    }
}
