use crate::api;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use serde_json::Value;

const DISCORD_SVG: &str = "M20.317 4.3698a19.7913 19.7913 0 00-4.8851-1.5152.0741.0741 0 00-.0785.0371c-.211.3753-.4447.8648-.6083 1.2495-1.8447-.2762-3.68-.2762-5.4868 0-.1636-.3933-.4058-.8742-.6177-1.2495a.077.077 0 00-.0785-.037 19.7363 19.7363 0 00-4.8852 1.515.0699.0699 0 00-.0321.0277C.5334 9.0458-.319 13.5799.0992 18.0578a.0824.0824 0 00.0312.0561c2.0528 1.5076 4.0413 2.4228 5.9929 3.0294a.0777.0777 0 00.0842-.0276c.4616-.6304.8731-1.2952 1.226-1.9942a.076.076 0 00-.0416-.1057c-.6528-.2476-1.2743-.5495-1.8722-.8923a.077.077 0 01-.0076-.1277c.1258-.0943.2517-.1923.3718-.2914a.0743.0743 0 01.0776-.0105c3.9278 1.7933 8.18 1.7933 12.0614 0a.0739.0739 0 01.0785.0095c.1202.099.246.1981.3728.2924a.077.077 0 01-.0066.1276 12.2986 12.2986 0 01-1.873.8914.0766.0766 0 00-.0407.1067c.3604.698.7719 1.3628 1.225 1.9932a.076.076 0 00.0842.0286c1.961-.6067 3.9495-1.5219 6.0023-3.0294a.077.077 0 00.0313-.0552c.5004-5.177-.8382-9.6739-3.5485-13.6604a.061.061 0 00-.0312-.0286z";

const GITHUB_SVG: &str = "M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z";

#[derive(Clone, Copy)]
struct Partner {
    name: &'static str,
    url: &'static str,
    tagline: &'static str,
}

const PARTNERS: &[Partner] = &[
    Partner {
        name: "Clere'ly Design",
        url: "https://clerely-design.fr",
        tagline: "Clere'ly Design tagline",
    },
    Partner {
        name: "MARIA",
        url: "https://maria.clerely.fr",
        tagline: "MARIA tagline",
    },
];

// App semver + deploy build sha, fetched once from /api/v1/version.
#[component]
fn AppVersion() -> impl IntoView {
    let version = RwSignal::new(String::new());
    let build = RwSignal::new(String::new());
    leptos::task::spawn_local(async move {
        if let Ok(v) = api::get::<Value>("/api/v1/version").await {
            if let Some(s) = v["version"].as_str() {
                version.set(s.to_string());
            }
            if let Some(s) = v["build"].as_str() {
                build.set(s.chars().take(8).collect());
            }
        }
    });
    view! {
        <span class="rounded border border-white/10 bg-white/5 px-1.5 py-0.5 font-mono text-[10px] text-white/40" title=move || format!("{} {}", t("Deployed version"), build.get())>
            {move || if version.get().is_empty() { "v…".to_string() } else { format!("v{}", version.get()) }}
        </span>
    }
}

#[component]
fn PartnersCarousel() -> impl IntoView {
    let idx = RwSignal::new(0usize);
    let handle = send_wrapper::SendWrapper::new(gloo_timers::callback::Interval::new(
        5000,
        move || {
            idx.update(|i| *i = (*i + 1) % PARTNERS.len());
        },
    ));
    on_cleanup(move || drop(handle));

    view! {
        <div class="relative h-[72px]">
            {PARTNERS
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    let p = *p;
                    view! {
                        <a
                            href=p.url
                            target="_blank"
                            rel="external nofollow sponsored"
                            class="group absolute inset-0 flex flex-col justify-center rounded-lg border border-white/10 bg-white/[0.03] px-3 transition-opacity duration-500 hover:border-white/25"
                            class=("opacity-0", move || idx.get() != i)
                            class=("pointer-events-none", move || idx.get() != i)
                            tabindex=move || if idx.get() == i { "0" } else { "-1" }
                        >
                            <span class="block text-sm font-bold text-white transition-colors group-hover:text-brand-accent">{p.name}</span>
                            <span class="mt-1 block text-xs leading-tight text-white/50">{t(p.tagline)}</span>
                        </a>
                    }
                })
                .collect_view()}
        </div>
        <div class="mt-2 flex gap-1.5">
            {PARTNERS
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    view! {
                        <button
                            type="button"
                            aria-label=format!("Partner {}", i + 1)
                            class="h-1.5 w-4 rounded-full transition-colors"
                            class=("bg-brand-accent", move || idx.get() == i)
                            class=("bg-white/20", move || idx.get() != i)
                            on:click=move |_| idx.set(i)
                        />
                    }
                })
                .collect_view()}
        </div>
    }
}

#[component]
fn FooterDonationGoal() -> impl IntoView {
    let data = RwSignal::new(Option::<Value>::None);
    leptos::task::spawn_local(async move {
        if let Ok(v) = api::get::<Value>("/api/v1/donations/progress").await {
            data.set(Some(v));
        }
    });
    view! {
        <Show when=move || data.get().is_some() fallback=|| ()>
            {move || {
                let d = data.get().unwrap_or_default();
                let raised = d["raised_cents"].as_f64().unwrap_or(0.0) / 100.0;
                let goal = d["goal_cents"].as_f64().unwrap_or(0.0) / 100.0;
                let pct = d["percent"].as_f64().unwrap_or(0.0).min(100.0);
                view! {
                    <A href="/donations" attr:class="group block">
                        <div class="mb-2 flex items-baseline justify-between">
                            <span class="text-sm font-bold text-white">{format!("{raised:.0} €")}</span>
                            <span class="text-xs text-white/50">{format!("{} {goal:.0} €", t("of"))}</span>
                        </div>
                        <div class="retro-progress">
                            <div class="retro-progress__fill" style=format!("width:{pct}%") />
                        </div>
                        <p class="mt-2 text-xs text-white/50 transition group-hover:text-white/70">
                            {t("Offrir un café")}
                        </p>
                    </A>
                }.into_any()
            }}
        </Show>
    }
}

#[component]
pub fn Footer() -> impl IntoView {
    view! {
        <footer class="surface-footer mt-16 py-12">
            <div class="mx-auto max-w-[1600px] px-4 sm:px-6 lg:px-8">
                <div class="grid grid-cols-1 gap-8 pb-8 sm:grid-cols-2 lg:grid-cols-5">
                    <div class="col-span-1">
                        <div class="mb-4 flex items-center">
                            <span class="text-xl font-bold uppercase tracking-[0.12em] text-brand-primary">"Blind'All Music"</span>
                        </div>
                        <p class="mb-4 text-white/70">{t("Free multiplayer music quizzes for everyone. Challenge friends and discover new music.")}</p>
                        <div class="flex space-x-4">
                            <a href=api::DISCORD_INVITE_URL target="_blank" rel="external nofollow" class="text-white/70 transition-colors hover:text-brand-accent">
                                <span class="sr-only">"Discord"</span>
                                <svg class="h-6 w-6" fill="currentColor" viewBox="0 0 24 24" aria-hidden="true"><path d=DISCORD_SVG /></svg>
                            </a>
                            <a href="https://github.com/byPreaZy/blindallmusic" target="_blank" rel="external nofollow" class="text-white/70 transition-colors hover:text-brand-accent">
                                <span class="sr-only">"GitHub"</span>
                                <svg class="h-6 w-6" fill="currentColor" viewBox="0 0 24 24" aria-hidden="true"><path fill-rule="evenodd" d=GITHUB_SVG clip-rule="evenodd" /></svg>
                            </a>
                        </div>
                    </div>

                    <div class="col-span-1">
                        <h3 class="retro-footer-title">{t("Quick Links")}</h3>
                        <ul class="space-y-2">
                            <li><A href="/" attr:class="text-white/70 transition-colors hover:text-white">{t("Home")}</A></li>
                            <li><A href="/docs" attr:class="text-white/70 transition-colors hover:text-white">{t("Documentation")}</A></li>
                            <li><A href="/docs/faq" attr:class="text-white/70 transition-colors hover:text-white">{t("FAQ")}</A></li>
                            <li><A href="/contact" attr:class="text-white/70 transition-colors hover:text-white">{t("Contact")}</A></li>
                            <li><a href="https://github.com/byPreaZy/blindallmusic/issues" target="_blank" rel="noopener noreferrer" class="text-white/70 transition-colors hover:text-white">{t("Report a bug")}</a></li>
                        </ul>
                    </div>

                    <div class="col-span-1">
                        <h3 class="retro-footer-title">{t("Legal")}</h3>
                        <ul class="space-y-2">
                            <li><A href="/pages/conditions-generales" attr:class="text-white/70 transition-colors hover:text-white">{t("Terms and Conditions")}</A></li>
                            <li><A href="/pages/mentions-legales" attr:class="text-white/70 transition-colors hover:text-white">{t("Legal Notice")}</A></li>
                            <li><A href="/pages/politique-de-confidentialite" attr:class="text-white/70 transition-colors hover:text-white">{t("Privacy Policy")}</A></li>
                        </ul>
                    </div>

                    <div class="col-span-1">
                        <h3 class="retro-footer-title">{t("Support Blind'All Music")}</h3>
                        <FooterDonationGoal />
                    </div>

                    <div class="col-span-1">
                        <h3 class="retro-footer-title">{t("Partners")}</h3>
                        <PartnersCarousel />
                    </div>
                </div>

                <div class="flex flex-col items-center justify-between border-t border-white/10 pt-8 md:flex-row">
                    <div class="mb-4 text-white/50 md:mb-0">
                        {format!("© {} Blind'All Music. {}", js_sys::Date::new_0().get_full_year(), t("All rights reserved."))}
                    </div>
                    <div class="flex items-center gap-3 text-white/50">
                        <span>{t("Site personnel fait bénévolement avec")}" ❤️ "{t("en France")}</span>
                        <AppVersion/>
                    </div>
                </div>
            </div>
        </footer>
    }
}
