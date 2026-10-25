use crate::api;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use serde_json::Value;

// Pro subscription CTA: links straight to the Stripe payment page when the
// server exposes `pro_url` (same source as the donations page), otherwise it
// falls back to /donations where the subscribe card lives.
fn pro_cta(pro_url: RwSignal<String>, class: &'static str) -> impl IntoView {
    view! {
        <a
            href=move || {
                let u = pro_url.get();
                if u.is_empty() {
                    "/donations".to_string()
                } else {
                    u
                }
            }
            class=format!("btn-primary {class}")
            target=move || (!pro_url.get().is_empty()).then_some("_blank")
            rel=move || (!pro_url.get().is_empty()).then_some("noopener")
        >
            {t("Passer à Pro")}
        </a>
    }
}

fn eq_bars(cls: &'static str) -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 60 24" class=cls fill="currentColor" aria-hidden="true">
            <rect x="0" y="8" width="4" height="8" rx="1"><animate attributeName="height" values="8;16;8" dur="0.9s" repeatCount="indefinite"/><animate attributeName="y" values="8;4;8" dur="0.9s" repeatCount="indefinite"/></rect>
            <rect x="8" y="4" width="4" height="16" rx="1"><animate attributeName="height" values="16;6;16" dur="0.7s" repeatCount="indefinite"/><animate attributeName="y" values="4;9;4" dur="0.7s" repeatCount="indefinite"/></rect>
            <rect x="16" y="10" width="4" height="6" rx="1"><animate attributeName="height" values="6;14;6" dur="0.8s" repeatCount="indefinite"/><animate attributeName="y" values="10;6;10" dur="0.8s" repeatCount="indefinite"/></rect>
            <rect x="24" y="6" width="4" height="12" rx="1"><animate attributeName="height" values="12;18;12" dur="1s" repeatCount="indefinite"/><animate attributeName="y" values="6;3;6" dur="1s" repeatCount="indefinite"/></rect>
            <rect x="32" y="9" width="4" height="8" rx="1"><animate attributeName="height" values="8;15;8" dur="0.75s" repeatCount="indefinite"/><animate attributeName="y" values="9;5;9" dur="0.75s" repeatCount="indefinite"/></rect>
            <rect x="40" y="5" width="4" height="14" rx="1"><animate attributeName="height" values="14;7;14" dur="0.85s" repeatCount="indefinite"/><animate attributeName="y" values="5;9;5" dur="0.85s" repeatCount="indefinite"/></rect>
            <rect x="48" y="7" width="4" height="10" rx="1"><animate attributeName="height" values="10;17;10" dur="0.95s" repeatCount="indefinite"/><animate attributeName="y" values="7;4;7" dur="0.95s" repeatCount="indefinite"/></rect>
        </svg>
    }
}

// Decorative QR-code mock for the TV-screen illustration.
fn qr_mock(cls: &'static str) -> impl IntoView {
    view! {
        <svg viewBox="0 0 100 100" class=cls aria-hidden="true">
            <rect width="100" height="100" rx="6" fill="#ffffff"/>
            <rect x="10" y="10" width="22" height="22" fill="#111827"/>
            <rect x="14" y="14" width="14" height="14" fill="#ffffff"/>
            <rect x="18" y="18" width="6" height="6" fill="#111827"/>
            <rect x="68" y="10" width="22" height="22" fill="#111827"/>
            <rect x="72" y="14" width="14" height="14" fill="#ffffff"/>
            <rect x="76" y="18" width="6" height="6" fill="#111827"/>
            <rect x="10" y="68" width="22" height="22" fill="#111827"/>
            <rect x="14" y="72" width="14" height="14" fill="#ffffff"/>
            <rect x="18" y="76" width="6" height="6" fill="#111827"/>
            <rect x="42" y="10" width="7" height="7" fill="#111827"/>
            <rect x="54" y="16" width="7" height="7" fill="#111827"/>
            <rect x="42" y="24" width="7" height="7" fill="#111827"/>
            <rect x="10" y="42" width="7" height="7" fill="#111827"/>
            <rect x="22" y="52" width="7" height="7" fill="#111827"/>
            <rect x="36" y="42" width="7" height="7" fill="#111827"/>
            <rect x="48" y="54" width="7" height="7" fill="#111827"/>
            <rect x="60" y="42" width="7" height="7" fill="#111827"/>
            <rect x="72" y="52" width="7" height="7" fill="#111827"/>
            <rect x="84" y="42" width="7" height="7" fill="#111827"/>
            <rect x="42" y="70" width="7" height="7" fill="#111827"/>
            <rect x="56" y="80" width="7" height="7" fill="#111827"/>
            <rect x="70" y="70" width="7" height="7" fill="#111827"/>
            <rect x="82" y="82" width="7" height="7" fill="#111827"/>
        </svg>
    }
}

// Fake TV-mode screen shown in the hero: live status, QR join panel and a
// mini scoreboard — what customers actually see on the bar's TV.
fn tv_mock() -> impl IntoView {
    view! {
        <div class="retro-panel p-3">
            <div class="relative flex aspect-[16/10] flex-col overflow-hidden rounded-lg border border-white/10 bg-gradient-to-br from-brand-deep to-brand-midnight p-4 sm:p-5">
                <div class="pointer-events-none absolute inset-0 bg-gradient-to-b from-brand-deep/40 via-transparent to-brand-midnight/80" aria-hidden="true"/>
                <div class="relative flex items-center justify-between gap-3">
                    <span class="inline-flex items-center gap-2 text-[10px] font-bold uppercase tracking-[0.28em] text-brand-secondary sm:text-xs">
                        <span class="h-2 w-2 animate-pulse rounded-full bg-brand-primary" aria-hidden="true"/>
                        {t("En direct dans votre bar")}
                    </span>
                    {eq_bars("h-6 w-12 text-brand-primary sm:h-8 sm:w-14")}
                </div>
                <div class="relative mt-3 flex flex-1 items-center gap-4 sm:mt-4 sm:gap-5">
                    <div class="shrink-0">
                        {qr_mock("h-20 w-20 rounded-md sm:h-28 sm:w-28")}
                    </div>
                    <div class="min-w-0">
                        <p class="text-sm font-bold text-white sm:text-lg">{t("Scannez pour buzzer")}</p>
                        <p class="mt-1 text-[11px] leading-snug text-white/55 sm:text-sm">
                            {t("Chacun joue depuis son téléphone — aucune application à installer.")}
                        </p>
                    </div>
                </div>
                <div class="relative mt-3 space-y-1.5 sm:mt-4">
                    {[("Léa", "w-[86%]", "1 240"), ("Marco", "w-[72%]", "1 040"), ("Team Comptoir", "w-[58%]", "830")]
                        .into_iter()
                        .map(|(name, width, pts)| {
                            view! {
                                <div class="relative flex items-center gap-2 overflow-hidden rounded-md border border-white/5 bg-white/[0.04] px-2.5 py-1.5">
                                    <div class=format!("absolute inset-y-0 left-0 bg-brand-primary/15 {width}") aria-hidden="true"/>
                                    <span class="relative h-2 w-2 shrink-0 rounded-full bg-brand-secondary" aria-hidden="true"/>
                                    <span class="relative min-w-0 flex-1 truncate text-[11px] font-medium text-white/85 sm:text-xs">{name}</span>
                                    <span class="relative text-[11px] font-bold text-brand-secondary sm:text-xs">{pts} " pts"</span>
                                </div>
                            }
                        })
                        .collect_view()}
                </div>
            </div>
            <div class="mx-auto mt-1.5 h-3 w-24 rounded-b-xl bg-brand-deep-hover" aria-hidden="true"/>
        </div>
    }
}

fn step_card(n: &'static str, title: &'static str, desc: &'static str) -> impl IntoView {
    view! {
        <div class="relative rounded-xl border border-white/10 bg-surface-raised/60 p-5">
            <span class="flex h-9 w-9 items-center justify-center rounded-full bg-brand-primary text-sm font-black text-brand-midnight">
                {n}
            </span>
            <h3 class="mt-4 font-bold text-white">{title}</h3>
            <p class="mt-1.5 text-sm leading-relaxed text-white/60">{desc}</p>
        </div>
    }
}

fn feature_card(icon: &'static str, title: &'static str, desc: &'static str) -> impl IntoView {
    view! {
        <div class="rounded-xl border border-white/10 bg-surface-raised/60 p-5 transition-colors hover:border-white/20">
            <span class="text-2xl" aria-hidden="true">{icon}</span>
            <h3 class="mt-3 font-bold text-white">{title}</h3>
            <p class="mt-1.5 text-sm leading-relaxed text-white/60">{desc}</p>
        </div>
    }
}

fn faq_item(question: &'static str, answer: &'static str) -> impl IntoView {
    view! {
        <details class="group overflow-hidden rounded-xl border border-white/10 bg-surface-raised/60">
            <summary class="flex cursor-pointer items-center justify-between gap-3 px-5 py-4 font-medium text-white hover:bg-white/[0.03]">
                <span>{question}</span>
                <span class="text-white/40 transition group-open:rotate-180">"▾"</span>
            </summary>
            <p class="border-t border-white/5 px-5 py-4 text-sm leading-relaxed text-white/60">{answer}</p>
        </details>
    }
}

#[component]
pub fn BarsPage() -> impl IntoView {
    crate::i18n::set_title_key("Blind test pour bars & pubs");
    crate::seo::apply_page("/bars", t("Transformez votre TV en blind test géant : vos clients buzzent depuis leur téléphone, le score s'affiche en direct. 29 €/mois, sans engagement."));

    // Same source as the donations page for the Pro Stripe payment link.
    let pro_url = RwSignal::new(String::new());
    leptos::task::spawn_local(async move {
        if let Ok(v) = api::get::<Value>("/api/v1/donations/progress").await {
            if let Some(u) = v["pro_url"].as_str() {
                pro_url.set(u.to_string());
            }
        }
    });

    view! {
        <div class="mx-auto max-w-6xl px-4 py-8 sm:py-12">

            // ── Hero ──────────────────────────────────────────────────
            <section class="grid items-center gap-10 lg:grid-cols-2">
                <div>
                    <p class="game-section-kicker">{t("Pour les bars & pubs")}</p>
                    <h1 class="mt-3 text-3xl font-black leading-tight text-white sm:text-4xl lg:text-5xl">
                        {t("Animez vos soirées avec le blind test nouvelle génération")}
                    </h1>
                    <p class="mt-4 max-w-xl text-base leading-relaxed text-white/60 sm:text-lg">
                        {t("Votre écran TV affiche le jeu et le QR code, vos clients buzzent depuis leur téléphone. Aucune application à installer, aucun matériel à acheter.")}
                    </p>
                    <div class="mt-8 flex flex-wrap items-center gap-3">
                        {pro_cta(pro_url, "!px-8 !py-3")}
                        <A href="/rooms/create" attr:class="game-btn-secondary !min-h-[46px] !px-8">
                            {t("Essayer gratuitement")}
                        </A>
                    </div>
                    <ul class="mt-6 flex flex-wrap gap-x-5 gap-y-2 text-sm text-white/50">
                        <li class="flex items-center gap-1.5">
                            <span class="text-brand-accent" aria-hidden="true">"✓"</span>
                            {t("Prêt en 5 minutes")}
                        </li>
                        <li class="flex items-center gap-1.5">
                            <span class="text-brand-accent" aria-hidden="true">"✓"</span>
                            {t("Le son sort sur votre sono")}
                        </li>
                        <li class="flex items-center gap-1.5">
                            <span class="text-brand-accent" aria-hidden="true">"✓"</span>
                            {t("Sans engagement")}
                        </li>
                    </ul>
                </div>
                <div class="mx-auto w-full max-w-lg lg:max-w-none">
                    {tv_mock()}
                </div>
            </section>

            // ── How it works ──────────────────────────────────────────
            <section class="mt-16 sm:mt-24">
                <div class="text-center">
                    <p class="game-section-kicker">{t("Simple comme bonjour")}</p>
                    <h2 class="retro-title retro-title--white mt-2 text-2xl sm:text-3xl">{t("Comment ça marche")}</h2>
                </div>
                <div class="mt-8 grid gap-4 md:grid-cols-3">
                    {step_card("1", t("Créez votre room"), t("Choisissez parmi les playlists officielles incluses ou créez les vôtres : Années 80, Rap, Disney, Rock… il y en a pour tous vos clients."))}
                    {step_card("2", t("Affichez sur votre TV"), t("Ouvrez le mode TV sur n'importe quel écran : le jeu tourne en grand format et le QR code s'affiche pour que tout le bar rejoigne."))}
                    {step_card("3", t("Vos clients buzzent"), t("Chacun répond depuis son téléphone, sans application et sans son sur le mobile. Le scoreboard se met à jour en direct.") )}
                </div>
            </section>

            // ── Features ──────────────────────────────────────────────
            <section class="mt-16 sm:mt-24">
                <div class="text-center">
                    <p class="game-section-kicker">{t("Tout est inclus")}</p>
                    <h2 class="retro-title retro-title--white mt-2 text-2xl sm:text-3xl">{t("Pensé pour l'animation en bar")}</h2>
                </div>
                <div class="mt-8 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
                    {feature_card("📺", t("Mode TV grand écran"), t("Une interface conçue pour être lue de loin : piste en cours, QR code et scores bien visibles depuis tout le bar."))}
                    {feature_card("📱", t("Pad sur téléphone"), t("Vos clients répondent depuis leur navigateur, sans rien installer et sans son sur leur téléphone : la musique sort sur votre sono."))}
                    {feature_card("🎵", t("Playlists officielles & perso"), t("Des dizaines de playlists prêtes à l'emploi, plus vos propres playlists pour coller à l'ambiance de votre établissement."))}
                    {feature_card("🏆", t("Scoreboard en temps réel"), t("Le classement défile en direct à l'écran : la compétition monte, le bar s'enflamme."))}
                    {feature_card("🧰", t("Aucun matériel à acheter"), t("Une TV ou un projecteur avec un navigateur suffit. Pas de buzzers, pas de boîtier, pas d'installation."))}
                    {feature_card("✨", t("Votre image sur grand écran"), t("Avec Pro, affichez le nom et la photo de votre établissement sur l'écran TV pendant toute la partie."))}
                </div>
            </section>

            // ── Pricing ───────────────────────────────────────────────
            <section class="mt-16 sm:mt-24">
                <div class="rounded-2xl border border-purple-400/30 bg-gradient-to-br from-purple-500/10 to-transparent p-6 sm:p-8">
                    <div class="grid items-center gap-8 lg:grid-cols-2">
                        <div>
                            <p class="game-section-kicker">{t("Offre Pro")}</p>
                            <h2 class="mt-2 text-2xl font-bold text-purple-300 sm:text-3xl">"🏢 Blind'All Pro"</h2>
                            <p class="mt-2 flex items-baseline gap-2">
                                <span class="text-4xl font-black text-white">{t("29 €")}</span>
                                <span class="text-sm font-semibold text-purple-200">{t("/mois TTC")}</span>
                            </p>
                            <p class="mt-1 text-sm text-white/60">{t("Sans engagement — résiliable à tout moment.")}</p>
                            <ul class="mt-5 space-y-2 text-sm text-white/70">
                                <li>"📍 " {t("Image de votre établissement sur l'écran TV")}</li>
                                <li>"📺 " {t("Mode TV grand écran avec QR code de participation")}</li>
                                <li>"🎵 " {t("Playlists officielles & playlists personnalisées")}</li>
                                <li>"👑 " {t("Tous les avantages supporter")}</li>
                            </ul>
                        </div>
                        <div class="rounded-xl border border-white/10 bg-brand-midnight/60 p-6 text-center">
                            <p class="text-sm font-semibold uppercase tracking-wider text-white/40">
                                {t("Prêt à transformer vos soirées ?")}
                            </p>
                            <div class="mt-5 flex flex-col items-center gap-3">
                                {pro_cta(pro_url, "!px-10 !py-3 !text-sm")}
                                <A href="/rooms/create" attr:class="text-sm font-medium text-white/60 underline-offset-4 hover:text-white hover:underline">
                                    {t("ou essayez gratuitement dès ce soir")}
                                </A>
                            </div>
                            <p class="mt-5 text-xs leading-relaxed text-white/40">
                                {t("En vous abonnant, vous acceptez nos")}
                                " "
                                <A href="/pages/conditions-generales-vente" attr:class="underline hover:text-white/70">{t("Conditions Générales de Vente")}</A>
                                "."
                            </p>
                        </div>
                    </div>
                </div>
            </section>

            // ── FAQ ───────────────────────────────────────────────────
            <section class="mt-16 sm:mt-24">
                <div class="text-center">
                    <p class="game-section-kicker">{t("On répond à tout")}</p>
                    <h2 class="retro-title retro-title--white mt-2 text-2xl sm:text-3xl">{t("Questions fréquentes")}</h2>
                </div>
                <div class="mx-auto mt-8 max-w-3xl space-y-3">
                    {faq_item(
                        t("Faut-il du matériel spécial ?"),
                        t("Non. Une TV ou un projecteur avec un navigateur web suffit pour le mode TV, et vos clients jouent sur leur propre téléphone. Aucun buzzer ni boîtier à acheter."),
                    )}
                    {faq_item(
                        t("Une smart TV suffit-elle ?"),
                        t("Oui : ouvrez le navigateur de votre TV et affichez la room en mode live. Un ordinateur branché en HDMI fonctionne tout aussi bien."),
                    )}
                    {faq_item(
                        t("Combien de joueurs peuvent participer ?"),
                        t("Le format est pensé pour tout un bar : les joueurs scannent le QR code affiché à l'écran et répondent chacun depuis leur téléphone."),
                    )}
                    {faq_item(
                        t("Puis-je résilier quand je veux ?"),
                        t("Oui, l'abonnement Pro est sans engagement : vous pouvez l'arrêter à tout moment et il reste actif jusqu'à la fin du mois en cours."),
                    )}
                </div>
            </section>

            // ── Final CTA ─────────────────────────────────────────────
            <section class="mt-16 sm:mt-24">
                <div class="rounded-2xl border border-white/10 bg-gradient-to-br from-brand-deep to-brand-midnight p-8 text-center sm:p-10">
                    <h2 class="text-2xl font-black text-white sm:text-3xl">{t("Prêt à remplir vos soirées creuses ?")}</h2>
                    <p class="mx-auto mt-3 max-w-xl text-white/60">
                        {t("Lancez votre premier blind test dès ce soir — c'est gratuit, et ça ne prend que quelques minutes.")}
                    </p>
                    <div class="mt-7 flex flex-wrap items-center justify-center gap-3">
                        <A href="/rooms/create" attr:class="game-btn-secondary !min-h-[46px] !px-8">
                            {t("Essayer gratuitement")}
                        </A>
                        {pro_cta(pro_url, "!px-8 !py-3")}
                    </div>
                </div>
            </section>
        </div>
    }
}
