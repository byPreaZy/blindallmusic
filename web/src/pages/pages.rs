use crate::api;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::hooks::use_params_map;
use serde_json::Value;

#[component]
pub fn StaticPage() -> impl IntoView {
    let params = use_params_map();
    let page = RwSignal::new(Value::Null);
    let loading = RwSignal::new(true);
    let not_found = RwSignal::new(false);

    leptos::task::spawn_local(async move {
        let slug = params.get().get("slug").unwrap_or_default();
        match api::get::<Value>(&format!("/api/v1/pages/{slug}")).await {
            Ok(v) => {
                crate::i18n::set_title(v["page"]["title"].as_str());
                crate::seo::set_canonical(&crate::seo::canonical_url(&format!("/pages/{slug}")));
                crate::seo::apply_alternates(&format!("/pages/{slug}"));
                page.set(v["page"].clone());
            }
            Err(e) if e.status == 404 => not_found.set(true),
            Err(_) => {}
        }
        loading.set(false);
    });

    view! {
        <div class="mx-auto max-w-3xl px-4 py-8">
            <Show
                when=move || !loading.get()
                fallback=move || view! { <div class="text-white/60">{t("Loading")}"…"</div> }
            >
                <Show
                    when=move || !not_found.get() && !page.get().is_null()
                    fallback=move || view! {
                        <div class="py-24 text-center text-white/60">{t("Page not found")}</div>
                    }
                >
                    <article class="rounded-xl border border-white/10 bg-surface-raised/60">
                        <div class="flex w-full items-center justify-between gap-4 border-b border-white/10 px-5 py-4">
                            <h1 class="text-xl font-bold text-white">
                                {move || page.get()["title"].as_str().unwrap_or("").to_string()}
                            </h1>
                            <small class="shrink-0 text-xs text-white/40">
                                {t("Last revision")} " : "
                                {move || {
                                    let p = page.get();
                                    p["date"].as_str().unwrap_or("").to_string()
                                }}
                            </small>
                        </div>
                        <div class="prose-invert max-w-none p-5 text-white/80"
                            inner_html=move || page.get()["content"].as_str().unwrap_or("").to_string()/>
                    </article>
                </Show>
            </Show>
        </div>
    }
}
