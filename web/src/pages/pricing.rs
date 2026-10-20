use crate::api;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use serde_json::Value;

#[component]
pub fn PricingPage() -> impl IntoView {
    crate::i18n::set_title_key("Pricing");
    let data = RwSignal::new(Value::Null);
    let loading = RwSignal::new(true);
    let subscribed = leptos_router::hooks::use_query_map()
        .get_untracked()
        .get("subscribed")
        .filter(|p| ["plus", "pro"].contains(&p.as_str()));

    leptos::task::spawn_local(async move {
        if let Ok(v) = api::get::<Value>("/api/v1/billing/plans").await {
            data.set(v);
        }
        loading.set(false);
    });

    let is_plus = move || data.get()["is_plus"].as_bool().unwrap_or(false);
    let is_pro = move || data.get()["is_pro"].as_bool().unwrap_or(false);
    let plus_url = move || data.get()["plus_payment_url"].as_str().map(str::to_string);
    let pro_url = move || data.get()["pro_payment_url"].as_str().map(str::to_string);
    let plus_avail = move || data.get()["plus_available"].as_bool().unwrap_or(false);
    let pro_avail = move || data.get()["pro_available"].as_bool().unwrap_or(false);

    let cta = move |url: Option<String>, avail: bool, active: bool| {
        if active {
            view! {
                <span class="inline-flex items-center gap-1.5 rounded-full border border-teal-400/40 bg-teal-500/15 px-4 py-2 text-sm font-semibold text-teal-200">
                    "✓ "{t("Active plan")}
                </span>
            }
            .into_any()
        } else if !avail {
            view! {
                <span class="rounded-lg border border-white/10 px-4 py-2 text-sm text-white/40">
                    {t("Available soon")}
                </span>
            }
            .into_any()
        } else if let Some(u) = url {
            view! {
                <a href=u class="btn-primary inline-block" target="_blank" rel="noopener">
                    {t("Subscribe")}
                </a>
            }
            .into_any()
        } else {
            view! {
                <A href="/login" attr:class="btn-primary inline-block">
                    {t("Log in to subscribe")}
                </A>
            }
            .into_any()
        }
    };

    let perk = |icon: &'static str, key: &'static str| {
        view! {
            <li class="flex items-start gap-2 text-sm text-white/70">
                <span class="mt-0.5 text-brand-accent">{icon}</span>
                {t(key)}
            </li>
        }
    };

    view! {
        <div class="mx-auto max-w-4xl px-4 py-8">
            <div class="mb-8 text-center">
                <h1 class="retro-title retro-title--white mb-2 text-3xl">{t("Pricing")}</h1>
                <p class="text-white/60">{t("Pricing intro")}</p>
            </div>

            {subscribed.map(|p| view! {
                <div class="mb-6 rounded-xl border border-teal-400/40 bg-teal-500/15 px-4 py-3 text-center text-sm font-semibold text-teal-200">
                    {if p == "pro" { t("Pro subscribed thanks") } else { t("Plus subscribed thanks") }}
                </div>
            })}

            <Show
                when=move || !loading.get()
                fallback=move || view! { <div class="text-white/60">{t("Loading")}"…"</div> }
            >
                <div class="grid gap-6 md:grid-cols-2">
                    <section class="flex flex-col rounded-2xl border border-white/10 bg-gradient-to-br from-brand-deep via-brand-midnight to-neutral-900 p-6 shadow-lg">
                        <div class="mb-4">
                            <h2 class="text-xl font-bold text-white">{"Blind'All Plus"}</h2>
                            <p class="text-sm text-white/50">{t("For players")}</p>
                        </div>
                        <p class="mb-4">
                            <span class="text-3xl font-bold text-white">"4,99 €"</span>
                            <span class="text-sm text-white/50">{t("/month")}</span>
                        </p>
                        <ul class="mb-6 flex-1 space-y-2">
                            {perk("★", "Plus perk badge")}
                            {perk("🏷", "Plus perk custom tag")}
                            {perk("↻", "Plus perk revive")}
                            {perk("🏠", "Plus perk rooms")}
                            {perk("📊", "Plus perk stats")}
                        </ul>
                        {move || cta(plus_url(), plus_avail(), is_plus())}
                    </section>

                    <section class="flex flex-col rounded-2xl border border-amber-400/20 bg-gradient-to-br from-neutral-900 via-brand-midnight to-brand-deep p-6 shadow-lg">
                        <div class="mb-4 flex items-center justify-between">
                            <div>
                                <h2 class="text-xl font-bold text-white">{"Blind'All Pro"}</h2>
                                <p class="text-sm text-white/50">{t("For bars & events")}</p>
                            </div>
                            <span class="rounded-full border border-amber-400/40 bg-amber-500/15 px-2.5 py-1 text-[10px] font-bold uppercase tracking-wider text-amber-200">
                                "B2B"
                            </span>
                        </div>
                        <p class="mb-4">
                            <span class="text-3xl font-bold text-white">"29 €"</span>
                            <span class="text-sm text-white/50">{t("/month per venue")}</span>
                        </p>
                        <ul class="mb-6 flex-1 space-y-2">
                            {perk("📺", "Pro perk tv")}
                            {perk("🏷", "Pro perk branding")}
                            {perk("🎮", "Pro perk host")}
                            {perk("📱", "Pro perk qr")}
                        </ul>
                        {move || cta(pro_url(), pro_avail(), is_pro())}
                    </section>
                </div>

                <p class="mt-8 text-center text-xs text-white/40">
                    {t("Pricing fairness note")}
                </p>
            </Show>
        </div>
    }
}
