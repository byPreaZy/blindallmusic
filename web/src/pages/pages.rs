use crate::api;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_params_map;
use serde_json::Value;

/// Cross-links shown under every static page — mirrors the footer "Legal" list.
const RELATED: &[(&str, &str)] = &[
    ("/pages/conditions-generales", "Terms and Conditions"),
    ("/pages/mentions-legales", "Legal Notice"),
    ("/pages/politique-de-confidentialite", "Privacy Policy"),
    ("/docs/faq", "FAQ"),
    ("/docs", "Documentation"),
    ("/contact", "Contact"),
];

fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

/// Extract `<h2>` sections for the table of contents and rewrite them with
/// anchors + numbering so the article reads like legal/docs content.
fn sections_and_html(raw: &str) -> (Vec<(String, String)>, String) {
    let mut sections = Vec::new();
    let mut out = String::with_capacity(raw.len() + 1024);
    let mut rest = raw;
    while let Some(start) = rest.find("<h2") {
        let (before, from_h2) = rest.split_at(start);
        out.push_str(before);
        let (Some(tag_end), Some(close)) = (from_h2.find('>'), from_h2.find("</h2>")) else {
            out.push_str(from_h2);
            rest = "";
            break;
        };
        let inner = &from_h2[tag_end + 1..close];
        let n = sections.len() + 1;
        let id = format!("section-{n}");
        sections.push((id.clone(), strip_tags(inner)));
        out.push_str(&format!(
            "<h2 id=\"{id}\" class=\"scroll-mt-24\"><span class=\"mr-2 text-blue-400\">{n}.</span>{inner}</h2>"
        ));
        rest = &from_h2[close + 5..];
    }
    out.push_str(rest);
    (sections, out)
}

fn scroll_to(id: &str) {
    if let Some(el) = document().get_element_by_id(id) {
        el.scroll_into_view();
    }
}

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

    let current_slug = move || params.get().get("slug").unwrap_or_default();

    view! {
        <div class="mx-auto max-w-4xl space-y-8 px-4 py-8">
            <Show
                when=move || !loading.get()
                fallback=move || view! { <div class="text-center text-white/60">{t("Loading")}"…"</div> }
            >
                <Show
                    when=move || !not_found.get() && !page.get().is_null()
                    fallback=move || view! {
                        <div class="py-24 text-center text-white/60">{t("Page not found")}</div>
                    }
                >
                    <div class="space-y-3 text-center">
                        <h1 class="text-4xl font-bold">
                            {move || page.get()["title"].as_str().unwrap_or("").to_string()}
                        </h1>
                        <span class="inline-block rounded-full border border-white/10 bg-white/5 px-3 py-1 text-xs text-white/40">
                            {t("Last revision")} " : "
                            {move || {
                                let p = page.get();
                                let raw = p["revised_at"]
                                    .as_str()
                                    .or_else(|| p["updated_at"].as_str())
                                    .unwrap_or("");
                                raw.chars().take(10).collect::<String>()
                            }}
                        </span>
                    </div>

                    {move || {
                        let raw = page.get()["content"].as_str().unwrap_or("").to_string();
                        let (sections, html) = sections_and_html(&raw);
                        let toc = (!sections.is_empty()).then(|| {
                            view! {
                                <nav class="rounded-2xl border border-slate-700 bg-gradient-to-br from-slate-800 to-slate-900 p-6">
                                    <h2 class="mb-4 text-lg font-bold">{t("Table of contents")}</h2>
                                    <ol class="grid gap-x-8 gap-y-2 sm:grid-cols-2">
                                        {sections
                                            .into_iter()
                                            .enumerate()
                                            .map(|(i, (id, title))| {
                                                view! {
                                                    <li class="flex items-baseline gap-2.5">
                                                        <span class="w-6 flex-shrink-0 text-right text-sm font-bold text-blue-400">
                                                            {i + 1} "."
                                                        </span>
                                                        <a
                                                            href=format!("#{id}")
                                                            class="text-sm text-neutral-300 transition-colors hover:text-white"
                                                            on:click=move |ev| {
                                                                ev.prevent_default();
                                                                scroll_to(&id);
                                                            }
                                                        >
                                                            {title}
                                                        </a>
                                                    </li>
                                                }
                                            })
                                            .collect_view()}
                                    </ol>
                                </nav>
                            }
                        });
                        view! {
                            {toc}
                            <article class="rounded-2xl border border-slate-700 bg-gradient-to-br from-slate-800 to-slate-900 p-6 md:p-10">
                                <div class="prose prose-invert max-w-none text-neutral-300 prose-headings:text-white prose-h2:mt-8 prose-h2:border-b prose-h2:border-slate-700/60 prose-h2:pb-2 prose-h2:text-2xl prose-h2:font-bold prose-h3:text-xl prose-a:text-blue-400 prose-a:no-underline hover:prose-a:text-blue-300 prose-strong:text-white"
                                    inner_html=html/>
                            </article>
                        }
                    }}

                    <div class="rounded-2xl border border-slate-700 bg-gradient-to-br from-slate-800 to-slate-900 p-6">
                        <h2 class="mb-4 text-lg font-bold">{t("See also")}</h2>
                        <div class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
                            {RELATED
                                .iter()
                                .filter(|(href, _)| href != &format!("/pages/{}", current_slug()))
                                .map(|(href, key)| {
                                    view! {
                                        <A href=*href attr:class="group flex items-center gap-3 rounded-lg border border-slate-700 bg-slate-800/50 px-4 py-3 transition-all hover:border-slate-600 hover:bg-slate-800/80">
                                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" class="h-4 w-4 flex-shrink-0 text-blue-400">
                                                <path stroke-linecap="round" stroke-linejoin="round" d="M19.5 14.25v-2.625a3.375 3.375 0 00-3.375-3.375h-1.5A1.125 1.125 0 0113.5 7.125v-1.5a3.375 3.375 0 00-3.375-3.375H8.25m0 12.75h7.5m-7.5 3H12M10.5 2.25H5.625c-.621 0-1.125.504-1.125 1.125v17.25c0 .621.504 1.125 1.125 1.125h12.75c.621 0 1.125-.504 1.125-1.125V11.25a9 9 0 00-9-9z" />
                                            </svg>
                                            <span class="text-sm font-semibold text-white/80 transition-colors group-hover:text-white">{t(*key)}</span>
                                        </A>
                                    }
                                })
                                .collect_view()}
                        </div>
                    </div>
                </Show>
            </Show>
        </div>
    }
}
