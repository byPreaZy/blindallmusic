use crate::api;
use crate::auth::use_auth;
use crate::i18n::t;
use leptos::prelude::*;
use serde_json::Value;

#[component]
pub fn FaqPage() -> impl IntoView {
    crate::i18n::set_title_key("FAQ");
    crate::seo::apply_page("/docs/faq", t("FAQ meta description"));
    let auth = use_auth();
    let faqs = RwSignal::new(Vec::<Value>::new());
    let total = RwSignal::new(0i64);
    let search = RwSignal::new(String::new());
    let page = RwSignal::new(1i64);
    let open_id = RwSignal::new(Option::<i64>::None);
    let loading = RwSignal::new(true);
    let voting = RwSignal::new(Option::<i64>::None);
    let refresh = RwSignal::new(0i64);

    Effect::new(move |_| {
        let (s, p) = (search.get(), page.get());
        refresh.track();
        loading.set(true);
        leptos::task::spawn_local(async move {
            let url = format!("/api/v1/faqs?page={p}&search={}", encode(&s));
            match api::get::<Value>(&url).await {
                Ok(v) => {
                    faqs.set(v["data"].as_array().cloned().unwrap_or_default());
                    total.set(v["total"].as_i64().unwrap_or(0));
                }
                Err(_) => {}
            }
            loading.set(false);
        });
    });

    let vote = move |id: i64, dir: &'static str| {
        if voting.get().is_some() {
            return;
        }
        voting.set(Some(id));
        leptos::task::spawn_local(async move {
            if api::post_empty::<Value>(&format!("/api/v1/faqs/{id}/vote/{dir}"))
                .await
                .is_ok()
            {
                refresh.update(|r| *r += 1);
            }
            voting.set(None);
        });
    };

    let last_page = move || (total.get() + 5) / 6;

    view! {
        <div class="mx-auto max-w-3xl px-4 py-8">
            <h1 class="retro-title retro-title--white mb-6 text-2xl">{t("FAQ")}</h1>
            <input type="text" placeholder={t("Search a question…")}
                class="mb-6 w-full rounded-lg border border-white/10 bg-surface-raised px-4 py-3 text-white outline-none focus:border-brand-accent/50"
                prop:value=search
                on:input=move |ev| { search.set(event_target_value(&ev)); page.set(1); }/>

            <Show
                when=move || !loading.get()
                fallback=move || view! { <div class="text-white/60">{t("Loading")}"…"</div> }
            >
                <Show
                    when=move || !faqs.get().is_empty()
                    fallback=move || view! {
                        <div class="rounded-xl border border-white/10 bg-surface-raised/60 p-8 text-center text-white/60">
                            {t("No questions found")}
                        </div>
                    }
                >
                    <div class="space-y-3">
                        <For
                            each=move || faqs.get()
                            key=|f| f["id"].as_i64().unwrap_or(0)
                            children=move |f: Value| {
                                let id = f["id"].as_i64().unwrap_or(0);
                                let up = f["upvotes"].as_i64().unwrap_or(0);
                                let down = f["downvotes"].as_i64().unwrap_or(0);
                                view! {
                                    <div class="overflow-hidden rounded-xl border border-white/10 bg-surface-raised/60">
                                        <button type="button"
                                            class="flex w-full items-center justify-between gap-3 px-5 py-4 text-left font-medium text-white hover:bg-white/[0.03]"
                                            on:click=move |_| open_id.update(|o| *o = if *o == Some(id) { None } else { Some(id) })>
                                            <span>{f["question"].as_str().unwrap_or("").to_string()}</span>
                                            <span class=move || if open_id.get() == Some(id) { "text-white/40 transition rotate-180" } else { "text-white/40 transition" }>
                                                "▾"
                                            </span>
                                        </button>
                                        <Show when=move || open_id.get() == Some(id)>
                                            <div class="border-t border-white/5 px-5 py-4">
                                                <div class="prose-sm whitespace-pre-wrap text-white/70"
                                                    inner_html=f["answer"].as_str().unwrap_or("").to_string()/>
                                                <div class="mt-4 flex items-center gap-3">
                                                    <span class="text-xs text-white/40">{t("Was this helpful?")}</span>
                                                    <Show
                                                        when=move || auth.user.get().is_some()
                                                        fallback=move || view! {
                                                            <span class="text-xs text-white/30">
                                                                "+"{up} " / -"{down} " · " {t("Log in to vote")}
                                                            </span>
                                                        }
                                                    >
                                                        <button type="button"
                                                            class="rounded border border-white/10 px-2 py-1 text-xs text-teal-400 hover:bg-teal-500/10 disabled:opacity-40"
                                                            disabled=move || voting.get() == Some(id)
                                                            on:click=move |_| vote(id, "up")>
                                                            "▲ " {up}
                                                        </button>
                                                        <button type="button"
                                                            class="rounded border border-white/10 px-2 py-1 text-xs text-red-400 hover:bg-red-500/10 disabled:opacity-40"
                                                            disabled=move || voting.get() == Some(id)
                                                            on:click=move |_| vote(id, "down")>
                                                            "▼ " {down}
                                                        </button>
                                                    </Show>
                                                </div>
                                            </div>
                                        </Show>
                                    </div>
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

fn encode(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || "-_.~".contains(c) {
                c.to_string()
            } else {
                format!("%{:02X}", c as u32)
            }
        })
        .collect()
}
