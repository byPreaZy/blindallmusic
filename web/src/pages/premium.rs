use crate::api;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::use_query_map;
use serde_json::Value;

#[component]
pub fn PremiumPage() -> impl IntoView {
    crate::i18n::set_title_key("Blind'All Premium");
    let status = RwSignal::new(Value::Null);
    let loading = RwSignal::new(true);
    // Art. L221-28 13° C. conso: the 14-day withdrawal right is waived only
    // by an express request — the CGV wording ("en cochant la case")
    // requires a real checkbox gating the Stripe redirect.
    let consent = RwSignal::new(false);
    // Stripe Payment Link after_completion redirects here with
    // ?subscribed=plus|pro — show a confirmation banner.
    let query = use_query_map();
    let just_subscribed = move || {
        matches!(
            query.get().get("subscribed").as_deref(),
            Some("plus") | Some("pro")
        )
    };

    leptos::task::spawn_local(async move {
        if let Ok(v) = api::get::<Value>("/api/v1/billing/status").await {
            status.set(v);
        }
        loading.set(false);
    });

    view! {
        <div class="mx-auto max-w-4xl px-4 py-8">
            <h1 class="retro-title retro-title--white mb-2 text-2xl">"⭐ Blind'All Premium"</h1>
            <p class="mb-8 text-white/60">
                {t("Go further: no ads, daily revive, custom tag — and venue branding for bars & events.")}
            </p>
            {move || {
                just_subscribed().then(|| view! {
                    <div class="mb-6 rounded-xl border border-emerald-400/30 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-200">
                        {t("Subscription activated — welcome aboard!")}
                    </div>
                })
            }}

            <Show
                when=move || !loading.get()
                fallback=move || view! { <div class="text-white/60">{t("Loading")}"…"</div> }
            >
                {move || {
                    let p = status.get();
                    if p.is_null() {
                        return view! { <div/> }.into_any();
                    }
                    let plus_url = p["plus_url"].as_str().unwrap_or("").to_string();
                    let pro_url = p["pro_url"].as_str().unwrap_or("").to_string();
                    let plan = p["plan"].as_str().unwrap_or("").to_string();
                    // Show the cards whenever Plus is offered or the
                    // viewer is subscribed — even anonymous (CTA -> /login).
                    let offered = p["plus_offered"].as_bool().unwrap_or(false);
                    if !offered && plan.is_empty() {
                        return view! {
                            <div class="rounded-xl border border-white/10 bg-surface-raised/60 p-6 text-center text-white/50">
                                {t("Subscriptions are not available yet.")}
                            </div>
                        }.into_any();
                    }
                    let sub_btn = move |url: String, label: &'static str| {
                        if !consent.get() {
                            return view! {
                                <span class="btn-primary cursor-not-allowed !px-5 !py-2 whitespace-nowrap opacity-40">
                                    {t(label)}
                                </span>
                            }
                            .into_any();
                        }
                        // Anonymous/guest gets /login — the
                        // subscription needs a logged account
                        // to attach to.
                        let (href, external) = if url.is_empty() {
                            ("/login".to_string(), false)
                        } else {
                            (url, true)
                        };
                        view! {
                            <a href=href class="btn-primary !px-5 !py-2 whitespace-nowrap"
                                target=external.then_some("_blank")
                                rel=external.then_some("noopener")>
                                {t(label)}
                            </a>
                        }
                        .into_any()
                    };
                    view! {
                        <label class="mb-4 flex cursor-pointer items-start gap-2 text-xs leading-relaxed text-white/60">
                            <input type="checkbox" class="mt-0.5"
                                prop:checked=move || consent.get()
                                on:change=move |ev| consent.set(event_target_checked(&ev)) />
                            <span>
                                {t("I accept the Terms of Sale, request immediate access and waive my 14-day withdrawal right (art. L221-28).")}
                            </span>
                        </label>
                        <div class="grid gap-4 sm:grid-cols-2">
                            <div class="rounded-2xl border border-amber-400/30 bg-gradient-to-br from-amber-500/10 to-transparent p-5">
                                <div>
                                    <h3 class="text-lg font-bold text-amber-300">
                                        "⭐ Blind'All Plus"
                                        {(plan == "plus").then(|| view! {
                                            <span class="ml-2 rounded bg-amber-400/20 px-2 py-0.5 text-xs uppercase text-amber-200">
                                                {t("Active plan")}
                                            </span>
                                        })}
                                    </h3>
                                    <p class="mt-1 text-sm font-semibold text-amber-200">
                                        {t("4.99 €/month — cancel anytime")}
                                    </p>
                                    <ul class="mt-2 space-y-1 text-sm text-white/70">
                                        <li>"🚫 " {t("No ads, forever")}</li>
                                        <li>"💛 " {t("Daily-challenge revive")}</li>
                                        <li>"🏷️ " {t("Custom tag")}</li>
                                        <li>"👑 " {t("All supporter perks")}</li>
                                    </ul>
                                </div>
                                <div class="mt-4">
                                    {(plan.is_empty()).then(|| sub_btn(plus_url.clone(), "Subscribe"))}
                                </div>
                            </div>
                            <div class="rounded-2xl border border-purple-400/30 bg-gradient-to-br from-purple-500/10 to-transparent p-5">
                                <div>
                                    <h3 class="text-lg font-bold text-purple-300">
                                        "🏢 Blind'All Pro"
                                        {(plan == "pro").then(|| view! {
                                            <span class="ml-2 rounded bg-purple-400/20 px-2 py-0.5 text-xs uppercase text-purple-200">
                                                {t("Active plan")}
                                            </span>
                                        })}
                                    </h3>
                                    <p class="mt-1 text-sm font-semibold text-purple-200">
                                        {t("29.00 €/month — cancel anytime")}
                                    </p>
                                    <ul class="mt-2 space-y-1 text-sm text-white/70">
                                        <li>"📍 " {t("Venue branding on the TV screen")}</li>
                                        <li>"📺 " {t("Room photo for your venue")}</li>
                                        <li>"👑 " {t("All supporter perks")}</li>
                                    </ul>
                                    <p class="mt-2 text-xs text-purple-200/60">
                                        {t("For bars & events")}
                                    </p>
                                </div>
                                <div class="mt-4">
                                    {(plan.is_empty()).then(|| sub_btn(pro_url.clone(), "Subscribe"))}
                                </div>
                            </div>
                        </div>
                        // Art. L221-28 13° C. conso: immediate execution
                        // of digital content requires express waiver of
                        // the 14-day withdrawal right — stated before the
                        // Stripe redirect.
                        <p class="mt-3 text-xs leading-relaxed text-white/40">
                            {t("By subscribing you accept our")}
                            " "
                            <A href="/pages/conditions-generales-vente" attr:class="underline hover:text-white/70">{t("Terms of Sale")}</A>
                            ". "
                            {t("You expressly request immediate access and waive your 14-day withdrawal right (art. L221-28 French Consumer Code).")}
                        </p>
                    }.into_any()
                }}
            </Show>
        </div>
    }
}
