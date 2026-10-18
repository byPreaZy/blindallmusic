use crate::api;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use serde_json::Value;

const CARD: &str = "rounded-2xl border border-slate-700 bg-gradient-to-br from-slate-800 to-slate-900 p-6";
const H2: &str = "mb-4 text-2xl font-bold";
const TXT: &str = "text-neutral-300";

fn section(children: impl IntoView + 'static) -> impl IntoView {
    view! { <div class=CARD>{children}</div> }
}

fn bullet_list(items: &[&'static str]) -> impl IntoView {
    items
        .iter()
        .map(|i| {
            view! {
                <li class="flex items-start gap-2">
                    <span class="mt-1 text-blue-400">"•"</span>
                    <span>{*i}</span>
                </li>
            }
        })
        .collect::<Vec<_>>()
}

fn link_card(href: &'static str, title: &'static str, sub: &'static str, color: &'static str) -> impl IntoView {
    let (icon_bg, icon_fg) = match color {
        "blue" => ("bg-blue-500/20 group-hover:bg-blue-500/30", "text-blue-400"),
        "purple" => ("bg-purple-500/20 group-hover:bg-purple-500/30", "text-purple-400"),
        "green" => ("bg-green-500/20 group-hover:bg-green-500/30", "text-green-400"),
        "orange" => ("bg-orange-500/20 group-hover:bg-orange-500/30", "text-orange-400"),
        "teal" => ("bg-teal-500/20 group-hover:bg-teal-500/30", "text-teal-400"),
        "pink" => ("bg-pink-500/20 group-hover:bg-pink-500/30", "text-pink-400"),
        _ => ("bg-slate-500/20 group-hover:bg-slate-500/30", "text-slate-400"),
    };
    view! {
        <A href=href attr:class="group flex items-center gap-4 rounded-lg border border-slate-700 bg-slate-800/50 p-4 transition-all hover:bg-slate-800/70">
            <div class=format!("flex h-12 w-12 flex-shrink-0 items-center justify-center rounded-lg transition-colors {icon_bg}")>
                {match color {
                    "blue" => view! { <crate::components::level::LevelBadge level=10 current_xp=500 xp_for_next_level=1000 size=28/> }.into_any(),
                    "purple" => view! { <crate::components::elo::EloBadge elo=2000 size="sm" variant="compact"/> }.into_any(),
                    _ => view! {
                        <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" class=format!("h-6 w-6 {icon_fg}")>
                            <path stroke-linecap="round" stroke-linejoin="round" d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
                        </svg>
                    }.into_any(),
                }}
            </div>
            <div class="flex-1">
                <h3 class=format!("font-semibold text-white transition-colors group-hover:text-{color}-400")>{title}</h3>
                <p class="mt-1 text-xs text-neutral-400">{sub}</p>
            </div>
            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="2" stroke="currentColor" class="h-5 w-5 text-neutral-400 transition-colors group-hover:text-blue-400">
                <path stroke-linecap="round" stroke-linejoin="round" d="M13.5 4.5L21 12m0 0l-7.5 7.5M21 12H3" />
            </svg>
        </A>
    }
}

// ---- /docs : progression systems overview -------------------------------------

#[component]
pub fn DocsIndexPage() -> impl IntoView {
    crate::i18n::set_title_key("Docs overview title");
    crate::seo::apply_page("/docs", t("Docs overview description"));
    view! {
        <div class="mx-auto max-w-5xl space-y-8 px-4 py-8">
            <div class="space-y-4 text-center">
                <h1 class="text-4xl font-bold">"Les systèmes de progression"</h1>
                <p class="text-lg text-neutral-400">"Vue d'ensemble des systèmes de progression : Scores, Levels et ELO"</p>
                <p class="text-sm text-neutral-500">"Cette page présente une synthèse des trois systèmes de progression sur Blind'All Music. Pour des guides détaillés, consultez les sections spécifiques ci-dessous."</p>
            </div>

            {section(view! {
                <h2 class=H2>"Pourquoi 3 systèmes ?"</h2>
                <p class=format!("mb-4 {TXT}")>"On a créé trois systèmes différents pour récompenser différents aspects de ton expérience sur Blind'All Music :"</p>
                <ul class=format!("ml-4 list-inside list-disc space-y-2 {TXT}")>
                    <li><strong>"Les Scores"</strong>" : Pour mesurer ton activité et ta participation"</li>
                    <li><strong>"Les Levels"</strong>" : Pour récompenser ton engagement global sur la plateforme"</li>
                    <li><strong>"L'ELO"</strong>" : Pour évaluer ta compétence réelle dans les parties compétitives"</li>
                </ul>
                <p class=format!("mt-4 {TXT}")>"Chaque système a son rôle et ils se complètent bien. Ensemble, ça donne une progression plus riche et équilibrée."</p>
            })}

            <div class=format!("overflow-x-auto {CARD}")>
                <h2 class="mb-6 text-2xl font-bold">"Comparaison des systèmes"</h2>
                <table class="w-full border-collapse text-sm">
                    <thead>
                        <tr class="border-b border-slate-700">
                            <th class="bg-slate-800/50 px-4 py-3 text-left font-semibold">"Caractéristique"</th>
                            <th class="bg-slate-800/50 px-4 py-3 text-left font-semibold">"Scores"</th>
                            <th class="bg-slate-800/50 px-4 py-3 text-left font-semibold">"Levels"</th>
                            <th class="bg-slate-800/50 px-4 py-3 text-left font-semibold">"ELO"</th>
                        </tr>
                    </thead>
                    <tbody>
                        {[
                            ("Objectif", "Mesurer l'activité", "Récompenser l'engagement", "Évaluer la compétence"),
                            ("Évolution", "Toujours croissante", "Toujours croissante", "Peut augmenter ou diminuer"),
                            ("Où ça compte", "Toutes les rooms", "Rooms publiques uniquement", "Rooms publiques, 3+ joueurs"),
                            ("Affichage pendant les parties", "✅ Oui", "❌ Non (dans les profils)", "✅ Oui"),
                            ("Influence des adversaires", "Aucune", "Aucune", "✅ Oui (plus fort = plus de points)"),
                            ("Valeur initiale", "0", "Level 1", "1500"),
                        ].into_iter().map(|(a, b, c, d)| view! {
                            <tr class="border-b border-slate-700/50 hover:bg-slate-800/30">
                                <td class="px-4 py-3 font-semibold">{a}</td>
                                <td class="px-4 py-3">{b}</td>
                                <td class="px-4 py-3">{c}</td>
                                <td class="px-4 py-3">{d}</td>
                            </tr>
                        }).collect::<Vec<_>>()}
                    </tbody>
                </table>
            </div>

            <div class="rounded-2xl border border-yellow-700/50 bg-gradient-to-br from-yellow-900/20 to-yellow-800/20 p-6">
                <h2 class="mb-2 text-2xl font-bold text-yellow-400">"Scores"</h2>
                <p class=format!("mb-4 {TXT}")>"Les scores, c'est simple : chaque bonne réponse te rapporte des points. Plus tu joues, plus tu accumules de points."</p>
                <ul class=format!("ml-4 list-inside list-disc space-y-1 text-sm {TXT}")>
                    <li>"Ça compte dans "<strong>"toutes les rooms"</strong>" (publiques et privées)"</li>
                    <li>"Ça ne peut "<strong>"jamais baisser"</strong>" - ça s'accumule"</li>
                    <li>"Ça mesure ton "<strong>"activité totale"</strong>" sur Blind'All Music"</li>
                    <li>"C'est affiché pendant les parties pour voir qui mène"</li>
                </ul>
            </div>

            <div class="rounded-2xl border border-blue-700/50 bg-gradient-to-br from-blue-900/20 to-blue-800/20 p-6">
                <h2 class="mb-2 text-2xl font-bold text-blue-400">"Levels"</h2>
                <p class=format!("mb-4 {TXT}")>"Les levels récompensent ton engagement global sur Blind'All Music. Tu gagnes de l'XP (Experience Points) en participant activement à la communauté."</p>
                <ul class=format!("ml-4 list-inside list-disc space-y-1 text-sm {TXT}")>
                    <li>"Ça compte uniquement dans les "<strong>"rooms publiques"</strong></li>
                    <li>"C'est basé sur plusieurs trucs : scores, ancienneté, rooms créées, playlists, etc."</li>
                    <li>"Ça ne peut "<strong>"jamais baisser"</strong>" - progression continue"</li>
                    <li>"C'est affiché dans les "<strong>"profils"</strong>" pour montrer ton expérience"</li>
                    <li>"Chaque level demande plus d'XP que le précédent"</li>
                </ul>
                <div class="mt-4">
                    <A href="/docs/level" attr:class="inline-flex items-center gap-2 text-sm font-semibold text-blue-400 transition-colors hover:text-blue-300">"Tout savoir sur les Levels →"</A>
                </div>
            </div>

            <div class="rounded-2xl border border-purple-700/50 bg-gradient-to-br from-purple-900/20 to-purple-800/20 p-6">
                <h2 class="mb-2 text-2xl font-bold text-purple-400">"ELO"</h2>
                <p class=format!("mb-4 {TXT}")>"L'ELO, c'est un système de classement compétitif qui évalue ton niveau réel. Contrairement aux scores et levels, l'ELO peut monter ou descendre selon tes performances."</p>
                <ul class=format!("ml-4 list-inside list-disc space-y-1 text-sm {TXT}")>
                    <li>"Ça compte uniquement dans les "<strong>"rooms publiques avec 3+ joueurs"</strong></li>
                    <li>"Ça peut "<strong>"monter ou descendre"</strong>" selon tes performances"</li>
                    <li>"Ça prend en compte le "<strong>"niveau de tes adversaires"</strong></li>
                    <li>"C'est affiché "<strong>"pendant les parties"</strong>" pour évaluer la compétence"</li>
                    <li>"Système de "<strong>"placement"</strong>" pour les nouveaux joueurs (10 premiers rounds)"</li>
                    <li>"K-factor variable selon ton niveau d'expérience"</li>
                    <li><strong>"Calcul proportionnel"</strong>" : Si tu arrives en milieu de partie, ton ELO évolue proportionnellement aux tracks jouées (minimum 80%)"</li>
                </ul>
                <div class="mt-4">
                    <A href="/docs/elo" attr:class="inline-flex items-center gap-2 text-sm font-semibold text-purple-400 transition-colors hover:text-purple-300">"Tout savoir sur l'ELO →"</A>
                </div>
            </div>

            <div class="rounded-2xl border border-green-700/50 bg-gradient-to-br from-green-900/20 to-green-800/20 p-6">
                <h2 class="mb-4 text-2xl font-bold text-green-400">"Où les voir ?"</h2>
                <div class="space-y-4">
                    <div class="rounded-lg bg-slate-800/50 p-4">
                        <h3 class="mb-2 font-semibold text-white">"Pendant une partie"</h3>
                        <p class="text-sm text-neutral-300">"Tu verras les "<strong>"scores"</strong>" (pour voir qui mène) et l'"<strong>"ELO"</strong>" (pour évaluer la compétence des joueurs). Le level n'est pas affiché pendant les parties car il représente l'expérience globale, pas la compétence dans cette partie spécifique."</p>
                    </div>
                    <div class="rounded-lg bg-slate-800/50 p-4">
                        <h3 class="mb-2 font-semibold text-white">"Dans un profil"</h3>
                        <p class="text-sm text-neutral-300">"Tu verras les "<strong>"scores totaux"</strong>", le "<strong>"level"</strong>" (expérience) et l'"<strong>"ELO"</strong>" (compétence). Ça donne une vue complète de l'activité et du niveau du joueur."</p>
                    </div>
                    <div class="rounded-lg bg-slate-800/50 p-4">
                        <h3 class="mb-2 font-semibold text-white">"Dans les classements"</h3>
                        <p class="text-sm text-neutral-300">"Les classements peuvent être triés par "<strong>"score"</strong>" (activité), "<strong>"level"</strong>" (expérience) ou "<strong>"ELO"</strong>" (compétence). Chaque classement montre un aspect différent de la progression."</p>
                    </div>
                </div>
            </div>

            {section(view! {
                <h2 class=H2>"Pour aller plus loin"</h2>
                <div class="grid gap-4 md:grid-cols-2">
                    {link_card("/docs/level", "Système de Levels", "Comment gagner de l'XP et progresser", "blue")}
                    {link_card("/docs/elo", "Système ELO", "Tout savoir sur le classement compétitif", "purple")}
                    {link_card("/docs/glossary", "Glossaire", "Définitions des termes techniques", "green")}
                    {link_card("/docs/howto", "Guide de démarrage", "Comment jouer à un blind test", "orange")}
                    {link_card("/docs/create-content", "Créer du contenu", "Rooms, playlists, tracks et réponses", "teal")}
                    {link_card("/docs/support", "Soutenir Blind'All Music", "Objectif mensuel et dons", "pink")}
                    {link_card("/docs/faq", "FAQ", "Questions fréquentes", "blue")}
                </div>
            })}

            <div class="rounded-2xl border border-indigo-700/50 bg-gradient-to-br from-indigo-900/20 to-indigo-800/20 p-6">
                <h2 class="mb-4 text-2xl font-bold text-indigo-400">"En bref"</h2>
                <div class=format!("space-y-3 {TXT}")>
                    <p><strong>"Scores"</strong>" = Ton activité totale, toujours croissante, visible partout"</p>
                    <p><strong>"Levels"</strong>" = Ton expérience et engagement, toujours croissant, visible dans les profils"</p>
                    <p><strong>"ELO"</strong>" = Ta compétence réelle, peut varier, visible pendant les parties compétitives"</p>
                    <p class="mt-4 font-semibold text-white">"Ces trois systèmes se complètent pour offrir une progression plus riche et équilibrée, qui récompense à la fois l'activité, l'engagement et la compétence."</p>
                </div>
            </div>

            <div class="pt-4 text-center">
                <A href="/" attr:class="text-blue-400 transition-colors hover:text-blue-300">" ← Retour à l'accueil "</A>
            </div>
        </div>
    }
}

// ---- /docs/level -----------------------------------------------------------------

#[component]
pub fn DocsLevelPage() -> impl IntoView {
    crate::i18n::set_title_key("Level System");
    crate::seo::apply_page("/docs/level", t("Docs level description"));
    let metrics: &[(&str, &str, &str, &str, &str)] = &[
        ("Score in public rooms", "Your total score accumulated in public rooms. This is the main source of XP.", "1 point = 1 XP", "No limit", "text-yellow-500"),
        ("Seniority", "Reward for your loyalty to Blind'All Music. Based on the number of months since your registration.", "50 XP per month", "600 XP (12 months maximum)", "text-blue-500"),
        ("Rooms created", "Reward for creating and sharing rooms with the community.", "100 XP per room", "1000 XP (10 rooms maximum)", "text-green-500"),
        ("Team membership", "Reward for joining a team and playing together.", "200 XP", "200 XP (one-time bonus)", "text-purple-500"),
        ("Playlists created", "Reward for creating playlists and contributing to the game content.", "20 XP per playlist", "2000 XP (100 playlists maximum)", "text-indigo-500"),
        ("Tracks liked", "Reward for engaging with the community by liking tracks.", "5 XP per track liked", "1000 XP (200 tracks maximum)", "text-red-500"),
        ("Mini-games score", "Reward for playing solo mini-games (Quiz, Who sang?). Your total points from mini-games count as XP.", "1 point = 1 XP", "1000 XP (maximum)", "text-teal-500"),
        ("Consecutive days streak", "Reward for daily connection. Your streak resets if you miss a day.", "10 XP per day", "300 XP (30 days maximum)", "text-amber-500"),
    ];
    let progression: &[(i64, i64, &str)] = &[
        (1, 100, "0-99"), (2, 150, "100-249"), (3, 200, "250-449"),
        (4, 250, "450-699"), (5, 300, "700-999"), (10, 500, "~3,000-3,499"),
        (20, 1000, "~15,000-15,999"), (30, 1500, "~40,000-40,999"),
        (50, 2500, "~100,000-100,999"), (100, 5000, "~400,000+"),
    ];
    view! {
        <div class="mx-auto max-w-4xl space-y-8 px-4 py-8">
            <div class="space-y-4 text-center">
                <h1 class="text-4xl font-bold">{t("Level System")}</h1>
                <p class="text-lg text-neutral-400">{t("Understand how the level system works and how to earn XP to progress.")}</p>
            </div>

            {section(view! {
                <h2 class=H2>{t("How it works")}</h2>
                <div class=format!("space-y-3 {TXT}")>
                    <p>{t("The level system rewards your activity and engagement on Blind'All Music. Your level is calculated based on your total XP (Experience Points), which you earn through various activities.")}</p>
                    <p>{t("Your level increases as you accumulate XP. The higher your level, the more XP you need to reach the next level.")}</p>
                    <p class="font-semibold text-white">{t("All XP calculations are updated in real-time when you play in public rooms.")}</p>
                    <p class="font-semibold text-green-400">{t("There is no maximum level - you can progress infinitely! The XP requirement increases by 50 XP per level.")}</p>
                </div>
            })}

            {section(view! {
                <h2 class=H2>{t("Level Progression")}</h2>
                <p class="mb-4 text-neutral-400">{t("The XP required to reach the next level increases progressively. Here are some examples:")}</p>
                <div class="overflow-x-auto">
                    <table class="w-full text-sm">
                        <thead>
                            <tr class="border-b border-slate-700">
                                <th class="px-4 py-3 text-left font-semibold">{t("Level")}</th>
                                <th class="px-4 py-3 text-left font-semibold">{t("XP needed for next level")}</th>
                                <th class="px-4 py-3 text-left font-semibold">{t("Total XP range")}</th>
                            </tr>
                        </thead>
                        <tbody>
                            {progression.iter().enumerate().map(|(i, (lvl, need, total))| view! {
                                <tr class:if=i % 2 == 0 class="bg-slate-800/50">
                                    <td class="px-4 py-3 font-bold">{*lvl}</td>
                                    <td class="px-4 py-3">{*need}" "{t("XP")}</td>
                                    <td class="px-4 py-3">{*total}" "{t("XP")}</td>
                                </tr>
                            }).collect::<Vec<_>>()}
                        </tbody>
                    </table>
                </div>
                <p class="mt-4 text-xs text-neutral-500">{t("Note: The XP requirement increases by 50 XP per level. There is no maximum level - you can progress infinitely!")}</p>
            })}

            <div class="space-y-6">
                <h2 class="text-2xl font-bold">{t("How to earn XP")}</h2>
                <p class="text-neutral-400">{t("Here are all the ways you can earn XP to increase your level:")}</p>
                <div class="grid gap-4 md:grid-cols-2">
                    {metrics.iter().map(|(title, desc, formula, max, color)| view! {
                        <div class="rounded-xl border border-slate-700 bg-slate-800/40 p-5">
                            <h3 class=format!("text-lg font-bold {color}")>{t(title)}</h3>
                            <p class="mt-1 text-sm text-neutral-300">{t(desc)}</p>
                            <div class="mt-3 flex flex-wrap gap-2">
                                <span class="rounded bg-slate-700 px-2 py-1 text-xs font-semibold">{t(formula)}</span>
                                <span class="px-2 py-1 text-xs text-neutral-400">{t("Max")}": "{t(max)}</span>
                            </div>
                        </div>
                    }).collect::<Vec<_>>()}
                </div>
            </div>

            <div class="rounded-2xl border border-blue-700/50 bg-gradient-to-br from-blue-900/20 to-blue-800/20 p-6">
                <h2 class="mb-4 text-2xl font-bold text-blue-400">{t("When are levels updated?")}</h2>
                <ul class="space-y-2 text-neutral-300">
                    {bullet_list(&[
                        "In real-time when you score points in public rooms",
                        "When you create a room or playlist",
                        "When you like or unlike a track",
                        "When you join a team",
                        "When you log in (if not updated in the last hour)",
                    ])}
                </ul>
            </div>

            <div class="rounded-2xl border border-amber-700/50 bg-gradient-to-br from-amber-900/20 to-amber-800/20 p-6">
                <h2 class="mb-4 text-2xl font-bold text-amber-400">{t("Important Notes")}</h2>
                <ul class="space-y-2 text-neutral-300">
                    {bullet_list(&[
                        "Only scores from public rooms (without password) count towards your level.",
                        "Each metric has a maximum XP cap to ensure balanced progression.",
                        "Your consecutive days streak resets if you don't log in for more than 24 hours.",
                        "Level updates are broadcast in real-time, so you'll see your level increase immediately.",
                        "There is no maximum level - the progression continues infinitely. The XP requirement increases by 50 XP per level (not by 10).",
                    ])}
                </ul>
            </div>

            <div class="pt-4 text-center">
                <A href="/docs" attr:class="text-blue-400 transition-colors hover:text-blue-300">" ← "{t("Back to docs")}</A>
            </div>
        </div>
    }
}

// ---- /docs/elo -------------------------------------------------------------------

fn elo_section(id: &'static str, title: &'static str, children: impl IntoView + 'static) -> impl IntoView {
    view! {
        <section id=id class=CARD>
            <h2 class=H2>{title}</h2>
            {children}
        </section>
    }
}

#[component]
pub fn DocsEloPage() -> impl IntoView {
    crate::i18n::set_title_key("Docs elo title");
    crate::seo::apply_page("/docs/elo", t("Docs elo description"));
    view! {
        <div class="mx-auto max-w-4xl space-y-8 px-4 py-8">
            <div class="space-y-4 text-center">
                <h1 class="text-4xl font-bold">"Système ELO - Documentation Complète"</h1>
                <p class="text-lg text-neutral-400">"Comprenez comment fonctionne le système ELO et comment votre classement évolue."</p>
            </div>

            <div class="rounded-2xl border border-blue-700/50 bg-gradient-to-br from-blue-900/20 to-blue-800/20 p-6">
                <h2 class="mb-4 text-2xl font-bold text-blue-400">"Table des matières"</h2>
                <ul class="space-y-2 text-neutral-300">
                    {[
                        ("introduction", "1. Introduction"),
                        ("quest-ce-que-lelo", "2. Qu'est-ce que l'ELO ?"),
                        ("comment-fonctionne", "3. Comment fonctionne le système ELO ?"),
                        ("rounds-placement", "4. Rounds de placement"),
                        ("calcul-changements", "5. Calcul des changements d'ELO"),
                        ("k-factor", "6. K-factor variable"),
                        ("conditions", "7. Conditions pour compter l'ELO"),
                        ("tracks-jouees", "8. Tracks jouées et calcul proportionnel"),
                        ("metriques", "9. Métriques de performance"),
                        ("win-streak", "10. Win Streak"),
                        ("protection", "11. Protection contre l'inflation/déflation"),
                        ("faq", "12. FAQ"),
                    ].into_iter().map(|(id, label)| view! {
                        <li><a href=format!("#{id}") class="text-blue-400 hover:text-blue-300">{label}</a></li>
                    }).collect::<Vec<_>>()}
                </ul>
            </div>

            {elo_section("introduction", "1. Introduction", view! {
                <p class=format!("mb-4 {TXT}")>"Le système ELO est un système de classement qui évalue votre niveau de compétence en fonction de vos performances dans les parties. Plus votre ELO est élevé, plus vous êtes considéré comme un joueur expérimenté."</p>
                <div class="rounded border-l-4 border-blue-500 bg-blue-900/20 p-4">
                    <p class="font-semibold text-white"><strong>"Votre ELO initial :"</strong>" Tous les nouveaux joueurs commencent avec un ELO de "<strong>"1500 points"</strong>"."</p>
                </div>
            })}

            {elo_section("quest-ce-que-lelo", "2. Qu'est-ce que l'ELO ?", view! {
                <p class=format!("mb-4 {TXT}")>"L'ELO est un nombre qui représente votre niveau de compétence. Il évolue après chaque partie selon :"</p>
                <ul class=format!("mb-4 ml-4 list-inside list-disc space-y-2 {TXT}")>
                    <li>"Votre position finale dans le round"</li>
                    <li>"Le niveau ELO de vos adversaires"</li>
                    <li>"Votre propre niveau ELO actuel"</li>
                </ul>
                <p class="font-semibold text-white"><strong>"Plus votre ELO est élevé, plus vous êtes considéré comme un bon joueur."</strong></p>
            })}

            {elo_section("comment-fonctionne", "3. Comment fonctionne le système ELO ?", view! {
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Principe de base"</h3>
                <p class=format!("mb-4 {TXT}")>"À la fin de chaque round, votre ELO peut augmenter ou diminuer selon votre performance. Le système compare :"</p>
                <ul class=format!("mb-4 ml-4 list-inside list-disc space-y-2 {TXT}")>
                    <li><strong>"Votre score réel"</strong>" : basé sur votre position finale (1er, 2ème, 3ème, etc.)"</li>
                    <li><strong>"Votre score attendu"</strong>" : basé sur votre ELO et celui de vos adversaires"</li>
                </ul>
                <div class="mb-4 rounded border-l-4 border-green-500 bg-green-900/20 p-4">
                    <p class="mb-2 font-semibold text-white"><strong>"Si vous faites mieux que prévu"</strong>" → Votre ELO augmente"</p>
                    <p class="font-semibold text-white"><strong>"Si vous faites moins bien que prévu"</strong>" → Votre ELO diminue"</p>
                </div>
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Exemple concret"</h3>
                <p class=format!("mb-2 {TXT}")>"Imaginez que vous avez un ELO de 1500 et que vous jouez contre 3 adversaires avec des ELO de 1600, 1500 et 1400."</p>
                <ul class=format!("ml-4 list-inside list-disc space-y-2 {TXT}")>
                    <li><strong>"Score attendu"</strong>" : Le système s'attend à ce que vous finissiez environ 2ème ou 3ème"</li>
                    <li><strong>"Si vous finissez 1er"</strong>" : Vous avez fait mieux que prévu → Votre ELO augmente"</li>
                    <li><strong>"Si vous finissez 4ème"</strong>" : Vous avez fait moins bien que prévu → Votre ELO diminue"</li>
                </ul>
            })}

            {elo_section("rounds-placement", "4. Rounds de placement", view! {
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Qu'est-ce que le placement ?"</h3>
                <p class=format!("mb-4 {TXT}")>"Les "<strong>"10 premiers rounds"</strong>" où votre ELO est compté sont appelés \"rounds de placement\". Pendant cette période :"</p>
                <ul class=format!("mb-4 ml-4 list-inside list-disc space-y-2 {TXT}")>
                    <li>"Votre ELO change "<strong>"plus rapidement"</strong>" pour s'ajuster à votre niveau réel"</li>
                    <li>"Le système utilise un "<strong>"K-factor plus élevé"</strong>" (de 50 à 32) pour vous placer rapidement"</li>
                </ul>
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Pourquoi un placement ?"</h3>
                <p class=format!("mb-4 {TXT}")>"Le placement permet au système de déterminer rapidement votre niveau réel. Après 10 rounds, votre ELO devrait être proche de votre niveau réel."</p>
                <div class="rounded border-l-4 border-amber-500 bg-amber-900/20 p-4">
                    <p class="font-semibold text-white"><strong>"Important :"</strong>" Seuls les rounds dans des "<strong>"rooms publiques avec au moins 3 joueurs"</strong>" comptent pour le placement et l'ELO."</p>
                </div>
            })}

            {elo_section("calcul-changements", "5. Calcul des changements d'ELO", view! {
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Formule utilisée"</h3>
                <p class=format!("mb-4 {TXT}")>"Le changement d'ELO se calcule ainsi :"</p>
                <div class="mb-4 rounded border border-purple-500/30 bg-purple-900/20 p-4 text-center">
                    <p class="font-mono text-lg font-semibold text-white"><strong>"Changement ELO = K × (Score réel - Score attendu)"</strong></p>
                </div>
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Score réel"</h3>
                <p class=format!("mb-4 {TXT}")>"Votre score réel dépend de votre position finale :"</p>
                <div class="mb-4 overflow-x-auto">
                    <table class="w-full border-collapse text-sm">
                        <thead>
                            <tr class="border-b border-slate-700">
                                <th class="bg-slate-800/50 px-4 py-3 text-left font-semibold">"Position"</th>
                                <th class="bg-slate-800/50 px-4 py-3 text-left font-semibold">"Score réel (exemple avec 4 joueurs)"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {[("1er", "1.0 (100%)"), ("2ème", "0.75 (75%)"), ("3ème", "0.5 (50%)"), ("4ème", "0.25 (25%)")]
                                .into_iter().map(|(p, s)| view! {
                                    <tr class="border-b border-slate-700/50 hover:bg-slate-800/30">
                                        <td class="px-4 py-3">{p}</td><td class="px-4 py-3">{s}</td>
                                    </tr>
                                }).collect::<Vec<_>>()}
                        </tbody>
                    </table>
                </div>
                <p class=format!("mb-4 {TXT}")><strong>"Formule :"</strong>" "<code class="rounded bg-purple-900/20 px-2 py-1">"(Nombre de joueurs - Position + 1) / Nombre de joueurs"</code></p>
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Score attendu"</h3>
                <p class=TXT>"Le score attendu est calculé en comparant votre ELO "<strong>"individuellement avec chaque adversaire"</strong>", puis en faisant la moyenne. Plus vos adversaires ont un ELO élevé, plus votre score attendu est bas."</p>
                <p class=format!("mt-2 {TXT}")><strong>"Exemple :"</strong>" Si vous avez 1500 ELO et que tous vos adversaires ont 1600 ELO, votre score attendu sera d'environ 0.36 (36%), ce qui signifie que le système s'attend à ce que vous finissiez plutôt en bas du classement."</p>
            })}

            {elo_section("k-factor", "6. K-factor variable", view! {
                <p class=format!("mb-4 {TXT}")>"Le K-factor détermine "<strong>"l'ampleur des changements"</strong>" d'ELO. Plus le K-factor est élevé, plus votre ELO peut changer rapidement."</p>
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Pendant le placement (10 premiers rounds)"</h3>
                <ul class=format!("mb-4 ml-4 list-inside list-disc space-y-2 {TXT}")>
                    <li><strong>"1er round"</strong>" : K = 50"</li>
                    <li><strong>"2ème round"</strong>" : K = 48"</li>
                    <li><strong>"3ème round"</strong>" : K = 45"</li>
                    <li>"..."</li>
                    <li><strong>"10ème round"</strong>" : K = 32"</li>
                </ul>
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Après le placement"</h3>
                <p class=format!("mb-4 {TXT}")>"Le K-factor dépend de votre ELO actuel :"</p>
                <div class="mb-4 overflow-x-auto">
                    <table class="w-full border-collapse text-sm">
                        <thead>
                            <tr class="border-b border-slate-700">
                                <th class="bg-slate-800/50 px-4 py-3 text-left font-semibold">"Votre ELO"</th>
                                <th class="bg-slate-800/50 px-4 py-3 text-left font-semibold">"K-factor"</th>
                                <th class="bg-slate-800/50 px-4 py-3 text-left font-semibold">"Explication"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {[
                                ("< 1200", "40", "Débutants : changements plus importants pour progresser rapidement"),
                                ("1200-1600", "32", "Intermédiaires : changements standard"),
                                ("1600-2000", "24", "Avancés : changements plus petits pour plus de stabilité"),
                                ("> 2000", "16", "Experts : changements très petits pour maintenir la précision"),
                            ].into_iter().map(|(e, k, x)| view! {
                                <tr class="border-b border-slate-700/50 hover:bg-slate-800/30">
                                    <td class="px-4 py-3">{e}</td><td class="px-4 py-3">{k}</td><td class="px-4 py-3">{x}</td>
                                </tr>
                            }).collect::<Vec<_>>()}
                        </tbody>
                    </table>
                </div>
                <p class=TXT><strong>"Pourquoi cette différence ?"</strong>" Les joueurs expérimentés ont un niveau plus stable, donc leur ELO doit changer moins rapidement pour rester précis."</p>
            })}

            {elo_section("conditions", "7. Conditions pour compter l'ELO", view! {
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Votre ELO est mis à jour uniquement si :"</h3>
                <ul class=format!("mb-4 space-y-2 {TXT}")>
                    {[
                        "La room est publique (les rooms privées ne comptent pas)",
                        "Il y a au moins 3 joueurs dans le round",
                        "Le round est terminé (vous avez participé jusqu'à la fin)",
                        "Vous avez joué au moins 80% des tracks du round",
                    ].into_iter().map(|i| view! {
                        <li class="flex items-start gap-2"><span class="mt-1 text-green-400">"✅"</span><span><strong>{i}</strong></span></li>
                    }).collect::<Vec<_>>()}
                </ul>
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Votre ELO n'est PAS mis à jour si :"</h3>
                <ul class=format!("mb-4 space-y-2 {TXT}")>
                    {["La room est privée", "Il y a moins de 3 joueurs", "Vous avez joué moins de 80% des tracks du round"]
                        .into_iter().map(|i| view! {
                            <li class="flex items-start gap-2"><span class="mt-1 text-red-400">"❌"</span><span>{i}</span></li>
                        }).collect::<Vec<_>>()}
                </ul>
                <div class="rounded border-l-4 border-blue-500 bg-blue-900/20 p-4">
                    <p class="font-semibold text-white"><strong>"Important :"</strong>" Même si votre ELO n'est pas mis à jour, vos statistiques (score total, métriques de performance) sont toujours enregistrées pour l'affichage."</p>
                </div>
            })}

            {elo_section("tracks-jouees", "8. Tracks jouées et calcul proportionnel", view! {
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Comment ça fonctionne ?"</h3>
                <p class=format!("mb-4 {TXT}")>"Le système enregistre toutes les tracks que vous avez jouées dans un round, même si vous êtes arrivé en milieu de partie. Cela permet un calcul d'ELO plus juste et équitable."</p>
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Seuil minimum : 80% des tracks"</h3>
                <p class=format!("mb-4 {TXT}")>"Pour que votre ELO soit compté, vous devez avoir joué "<strong>"au moins 80% des tracks"</strong>" du round. Par exemple :"</p>
                <ul class=format!("mb-4 ml-4 list-inside list-disc space-y-2 {TXT}")>
                    <li><strong>"Round de 10 tracks"</strong>" : Vous devez avoir joué au moins 8 tracks"</li>
                    <li><strong>"Round de 5 tracks"</strong>" : Vous devez avoir joué au moins 4 tracks"</li>
                </ul>
                <div class="mb-4 rounded border-l-4 border-amber-500 bg-amber-900/20 p-4">
                    <p class="font-semibold text-white"><strong>"Pourquoi 80% ?"</strong>" Ce seuil garantit que vous avez participé à une partie significative du round, ce qui rend le calcul d'ELO plus fiable."</p>
                </div>
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Calcul proportionnel du changement d'ELO"</h3>
                <p class=format!("mb-4 {TXT}")>"Si vous avez joué au moins 80% des tracks, votre changement d'ELO est "<strong>"proportionnel au nombre de tracks jouées"</strong>" :"</p>
                <div class="mb-4 rounded border border-purple-500/30 bg-purple-900/20 p-4">
                    <p class="text-center font-mono text-lg font-semibold text-white"><strong>"Changement ELO final = Changement ELO calculé × (Tracks jouées / Tracks totales)"</strong></p>
                </div>
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Exemples concrets"</h3>
                <div class="mb-4 space-y-4">
                    {[
                        ("Exemple 1 : Round complet (10/10 tracks)", "Vous avez joué toutes les 10 tracks d'un round. Le système calcule un changement d'ELO de +20 points.", "+20 points"),
                        ("Exemple 2 : Arrivée en milieu de partie (9/10 tracks)", "Vous êtes arrivé après le début et avez joué 9 tracks sur 10. Le système calcule un changement d'ELO de +20 points.", "+18 points (20 × 0.9)"),
                        ("Exemple 3 : Arrivée tardive (8/10 tracks)", "Vous êtes arrivé tard et avez joué 8 tracks sur 10 (exactement le minimum). Le système calcule un changement d'ELO de +20 points.", "+16 points (20 × 0.8)"),
                        ("Exemple 4 : Pas assez de tracks (7/10 tracks)", "Vous avez joué seulement 7 tracks sur 10 (70% < 80%).", "Votre ELO n'est pas compté pour ce round"),
                    ].into_iter().map(|(t_, d, r)| view! {
                        <div class="rounded-lg bg-slate-800/50 p-4">
                            <p class="mb-2 font-semibold text-white">{t_}</p>
                            <p class="text-sm text-neutral-300">{d}<br/><strong>"Résultat :"</strong>" "<strong>{r}</strong></p>
                        </div>
                    }).collect::<Vec<_>>()}
                </div>
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Avantages de ce système"</h3>
                <ul class=format!("mb-4 ml-4 list-inside list-disc space-y-2 {TXT}")>
                    <li><strong>"Équité"</strong>" : Les joueurs arrivés en milieu de partie peuvent quand même faire évoluer leur ELO"</li>
                    <li><strong>"Proportionnalité"</strong>" : Plus vous jouez de tracks, plus votre ELO peut évoluer"</li>
                    <li><strong>"Fiabilité"</strong>" : Le seuil de 80% garantit que seuls les joueurs ayant participé significativement comptent"</li>
                    <li><strong>"Flexibilité"</strong>" : Vous n'êtes pas pénalisé si vous rejoignez une partie en cours"</li>
                </ul>
                <div class="rounded border-l-4 border-green-500 bg-green-900/20 p-4">
                    <p class="font-semibold text-white"><strong>"En résumé :"</strong>" Si vous rejoignez une partie en cours et jouez au moins 80% des tracks restantes, votre ELO évoluera proportionnellement au nombre de tracks que vous avez réellement jouées."</p>
                </div>
            })}

            {elo_section("metriques", "9. Métriques de performance", view! {
                <p class=format!("mb-4 {TXT}")>"En plus de l'ELO, le système enregistre plusieurs métriques pour chaque round :"</p>
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Temps moyen de réponse"</h3>
                <p class=format!("mb-4 {TXT}")>"Le temps moyen (en secondes) que vous avez mis pour répondre correctement aux questions."</p>
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Nombre de réponses rapides"</h3>
                <p class=format!("mb-4 {TXT}")>"Le nombre de réponses que vous avez données rapidement (dans les 18% de la durée de la piste). Ces réponses rapides vous donnent un bonus de score."</p>
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Nombre total de réponses"</h3>
                <p class=format!("mb-4 {TXT}")>"Le nombre total de bonnes réponses que vous avez données dans le round."</p>
                <div class="rounded border-l-4 border-blue-500 bg-blue-900/20 p-4">
                    <p class="font-semibold text-white"><strong>"Ces métriques sont toujours enregistrées"</strong>", même si l'ELO n'est pas compté (rooms privées, moins de 3 joueurs)."</p>
                </div>
            })}

            {elo_section("win-streak", "10. Win Streak", view! {
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Qu'est-ce que le Win Streak ?"</h3>
                <p class=format!("mb-4 {TXT}")>"Le Win Streak (série de victoires) compte le nombre de "<strong>"victoires consécutives"</strong>" (finir 1er) dans une même room."</p>
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Comment ça fonctionne ?"</h3>
                <ul class=format!("mb-4 ml-4 list-inside list-disc space-y-2 {TXT}")>
                    <li>"Si vous finissez "<strong>"1er"</strong>" dans un round, votre Win Streak est calculé"</li>
                    <li>"Le système vérifie vos résultats dans les rounds précédents de la même room"</li>
                    <li>"Il compte combien de fois vous avez fini 1er consécutivement"</li>
                    <li>"Dès que vous ne finissez pas 1er, le streak est réinitialisé à 0"</li>
                </ul>
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Exemple"</h3>
                <ul class=format!("mb-4 ml-4 list-inside list-disc space-y-2 {TXT}")>
                    <li><strong>"Round 1"</strong>" : Vous finissez 1er → Win Streak = 1"</li>
                    <li><strong>"Round 2"</strong>" : Vous finissez 1er → Win Streak = 2"</li>
                    <li><strong>"Round 3"</strong>" : Vous finissez 2ème → Win Streak = 0 (réinitialisé)"</li>
                    <li><strong>"Round 4"</strong>" : Vous finissez 1er → Win Streak = 1 (nouveau streak)"</li>
                </ul>
                <div class="rounded border-l-4 border-amber-500 bg-amber-900/20 p-4">
                    <p class="font-semibold text-white"><strong>"Note :"</strong>" Le Win Streak est calculé "<strong>"par room"</strong>". Si vous changez de room, le streak recommence."</p>
                </div>
            })}

            {elo_section("protection", "11. Protection contre l'inflation/déflation", view! {
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Pourquoi cette protection ?"</h3>
                <p class=format!("mb-4 {TXT}")>"Sans protection, l'ELO moyen de tous les joueurs pourrait augmenter (inflation) ou diminuer (déflation) au fil du temps, ce qui rendrait le système moins précis."</p>
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Protection locale"</h3>
                <p class=format!("mb-2 {TXT}")>"Le système applique automatiquement des ajustements :"</p>
                <ul class=format!("mb-4 ml-4 list-inside list-disc space-y-2 {TXT}")>
                    <li><strong>"Si votre ELO > 2000"</strong>" et que vous gagnez des points : le gain est réduit de 10%"</li>
                    <li><strong>"Si votre ELO < 1000"</strong>" et que vous perdez des points : la perte est réduite de 10%"</li>
                </ul>
                <p class=format!("mb-4 {TXT}")>"Cela empêche les ELO extrêmes de devenir trop volatils."</p>
                <h3 class="mb-3 mt-6 text-xl font-semibold">"Protection globale"</h3>
                <p class=TXT>"L'équipe peut exécuter périodiquement une commande pour ajuster tous les ELO et maintenir une moyenne stable autour de 1500. Ces ajustements sont rares et appliqués équitablement à tous les joueurs."</p>
            })}

            {elo_section("faq", "12. FAQ - Questions Fréquentes", view! {
                <div class="space-y-6">
                    {[
                        ("Mon ELO a diminué alors que j'ai bien joué, pourquoi ?",
                         "Plusieurs raisons possibles : vos adversaires avaient un ELO plus bas (le système s'attendait à mieux), vous êtes encore en placement (changements plus importants les 10 premiers rounds), ou vous avez un ELO très élevé (> 2000 gagne moins pour éviter l'inflation)."),
                        ("Pourquoi mon ELO ne change pas après un round ?",
                         "Votre ELO ne change que si : la room est publique, il y a au moins 3 joueurs, le round est terminé, et vous avez joué au moins 80% des tracks. Sinon votre ELO reste inchangé, mais vos statistiques sont enregistrées."),
                        ("Que se passe-t-il si j'arrive en milieu de partie ?",
                         "Si vous jouez au moins 80% des tracks restantes, votre ELO sera compté, de manière proportionnelle aux tracks jouées. Exemple : changement calculé de +20, vous avez joué 9/10 tracks → +18 points."),
                        ("Combien de points puis-je gagner ou perdre ?",
                         "Cela dépend de votre ELO actuel (K-factor), du nombre de rounds joués (placement ou non), de votre position finale et de l'ELO de vos adversaires. En général : ±20 à ±50 pendant le placement, ±5 à ±40 ensuite."),
                        ("Mon ELO peut-il devenir négatif ?",
                         "Non, l'ELO minimum est de 100 points. Si votre ELO descend très bas, il sera maintenu à 100 minimum."),
                        ("Que se passe-t-il si je joue dans une room privée ?",
                         "Votre ELO ne change pas, mais votre score total est enregistré, vos métriques de performance sont enregistrées, et vous pouvez voir votre classement dans cette room."),
                        ("Puis-je voir mon historique ELO ?",
                         "Oui, tous vos résultats sont enregistrés dans les \"standings\" de chaque round : votre ELO avant/après, le changement, votre position finale et toutes vos métriques de performance."),
                        ("Le système est-il équitable ?",
                         "Oui : même formule pour tous, calcul progressif (battre des joueurs forts rapporte plus), protection contre l'inflation, et transparence totale des résultats."),
                        ("Que faire si je pense qu'il y a une erreur ?",
                         "Vérifiez que le round était en room publique avec 3+ joueurs, votre position finale et l'ELO des adversaires, puis contactez l'équipe de modération avec les détails du round."),
                    ].into_iter().map(|(q, a)| view! {
                        <div class="rounded-lg bg-slate-800/50 p-5">
                            <h3 class="mb-3 text-lg font-semibold text-blue-400">{q}</h3>
                            <p class=TXT>{a}</p>
                        </div>
                    }).collect::<Vec<_>>()}
                </div>
            })}

            <div class="rounded-2xl border border-green-700/50 bg-gradient-to-br from-green-900/20 to-green-800/20 p-6">
                <h2 class="mb-4 text-2xl font-bold text-green-400">"Résumé des points clés"</h2>
                <ul class="space-y-2 text-neutral-300">
                    {[
                        "ELO initial : 1500 points pour tous les nouveaux joueurs",
                        "Placement : 10 premiers rounds avec changements plus rapides",
                        "K-factor variable : Plus vous êtes expérimenté, moins votre ELO change rapidement",
                        "Calcul progressif : Plus vous battez des joueurs forts, plus vous gagnez de points",
                        "Conditions : Seulement les rooms publiques avec 3+ joueurs et 80%+ de tracks jouées",
                        "Calcul proportionnel : L'ELO évolue proportionnellement au nombre de tracks jouées",
                        "Protection : Système anti-inflation/déflation pour maintenir la précision",
                        "Transparence : Tous vos résultats sont enregistrés et consultables",
                    ].into_iter().map(|i| view! {
                        <li class="flex items-start gap-2"><span class="mt-1 text-green-400">"✅"</span><span><strong>{i}</strong></span></li>
                    }).collect::<Vec<_>>()}
                </ul>
            </div>

            <div class="pt-4 text-center">
                <A href="/docs" attr:class="text-blue-400 transition-colors hover:text-blue-300">" ← Retour à la documentation "</A>
            </div>
        </div>
    }
}

// ---- /docs/glossary -----------------------------------------------------------

#[component]
pub fn DocsGlossaryPage() -> impl IntoView {
    crate::i18n::set_title_key("Docs glossary title");
    crate::seo::apply_page("/docs/glossary", t("Docs glossary description"));
    crate::seo::set_meta("name", "keywords", t("Docs glossary keywords"));
    let definitions: &[(&str, &str, &str, &[&str])] = &[
        ("Blind test", "Un blind test est un jeu musical où les participants doivent identifier une chanson en écoutant uniquement un extrait audio, sans voir le titre, l'artiste ou la pochette d'album.", "Sur Blind'All Music, les blind tests sont organisés en rooms (salles de jeu) multijoueurs où plusieurs joueurs s'affrontent en temps réel.", &["Quiz musical", "Room", "Track"]),
        ("Quiz musical", "Un quiz musical est un jeu de questions-réponses basé sur la musique. Sur Blind'All Music, les quiz musicaux prennent la forme de blind tests multijoueurs en ligne.", "Les quiz musicaux sur Blind'All Music sont organisés par catégories (Années 2000, Disney, Chanson française, Rock, Pop, etc.) et permettent aux joueurs de tester leurs connaissances musicales.", &["Blind test", "Room", "Category"]),
        ("Room", "Une room (salle de jeu) est un espace de jeu multijoueur sur Blind'All Music où les participants jouent ensemble à un blind test.", "Les rooms peuvent être publiques (officielles, gérées par les modérateurs de Blind'All Music) ou privées (créées par les joueurs, avec ou sans mot de passe). Chaque room contient des playlists de musiques et organise des rounds de jeu.", &["Playlist", "Round", "Public room", "Private room"]),
        ("Round", "Un round est une manche de jeu dans une room. Chaque round contient plusieurs tracks (morceaux de musique) que les joueurs doivent identifier.", "Sur Blind'All Music, un round se termine lorsque tous les tracks ont été joués. Les scores sont calculés en fonction des bonnes réponses et du temps de réponse.", &["Track", "Score", "ELO"]),
        ("Track", "Un track est un morceau de musique joué pendant un round. Les joueurs doivent identifier le titre et/ou l'artiste du track.", "Les tracks sont organisés en playlists et peuvent provenir de différents fournisseurs musicaux (Deezer, YouTube, Apple Music, etc.).", &["Playlist", "Music provider"]),
        ("Score", "Le score est le nombre total de points accumulés par un joueur sur Blind'All Music. Chaque bonne réponse rapporte des points.", "Le score est toujours croissant et mesure l'activité totale d'un joueur. Il est affiché pendant les parties pour voir qui mène.", &["Level", "ELO", "Total score"]),
        ("Level", "Le level (niveau) est un système de progression basé sur l'XP (Experience Points) qui récompense l'engagement global d'un joueur sur Blind'All Music.", "Le level augmente en fonction de plusieurs facteurs : scores dans les rooms publiques, ancienneté, rooms créées, playlists créées, appartenance à une équipe. Le level est toujours croissant.", &["XP", "Score", "ELO"]),
        ("ELO", "L'ELO est un système de classement compétitif qui évalue la compétence réelle d'un joueur. Contrairement au score et au level, l'ELO peut augmenter ou diminuer selon les performances.", "L'ELO sur Blind'All Music fonctionne uniquement dans les rooms publiques avec 3 joueurs ou plus. Il prend en compte le niveau des adversaires : battre un joueur avec un ELO élevé rapporte plus de points.", &["Score", "Level", "Competitive ranking"]),
        ("XP (Experience Points)", "Les XP (Experience Points) sont les points d'expérience qui déterminent le level d'un joueur. Ils sont gagnés par diverses activités sur la plateforme.", "Sur Blind'All Music, les XP sont gagnés par : 1 point de score = 1 XP, 50 XP par mois d'ancienneté (max 600 XP), 100 XP par room créée (max 1000 XP), 200 XP pour l'appartenance à une équipe, 20 XP par playlist créée (max 2000 XP).", &["Level", "Score"]),
        ("Playlist", "Une playlist est une collection de tracks organisés par thème, genre ou catégorie. Les playlists sont utilisées pour créer des rooms de jeu.", "Sur Blind'All Music, les utilisateurs peuvent créer leurs propres playlists ou utiliser des playlists publiques. Les playlists peuvent être importées depuis Deezer ou créées manuellement.", &["Track", "Room", "Music provider"]),
        ("Public room", "Une room publique est une room officielle gérée par les modérateurs de Blind'All Music. Les scores, levels et ELO comptent uniquement dans les rooms publiques.", "Les rooms publiques sont créées et maintenues par l'équipe Blind'All Music. Elles permettent de jouer avec des joueurs du monde entier et de progresser dans les classements globaux.", &["Private room", "Room", "Score"]),
        ("Private room", "Une room privée est une room de la communauté créée et gérée par les joueurs. Elle peut être protégée par un mot de passe ou accessible sans mot de passe.", "Les rooms privées permettent aux joueurs de créer leurs propres espaces de jeu. Les scores dans les rooms privées ne comptent pas pour le level ou l'ELO, mais comptent pour le score total.", &["Public room", "Room"]),
        ("Category", "Une catégorie est un classement thématique des rooms et playlists (ex: Années 2000, Disney, Chanson française, Rock, Pop).", "Les catégories permettent aux joueurs de trouver facilement des rooms correspondant à leurs goûts musicaux.", &["Room", "Playlist"]),
        ("Music provider", "Un music provider (fournisseur musical) est un service qui fournit les extraits audio pour les tracks (Deezer, YouTube, Apple Music, etc.).", "Blind'All Music supporte plusieurs fournisseurs musicaux pour permettre une grande variété de contenu musical.", &["Track", "Playlist"]),
    ];
    view! {
        <div class="mx-auto max-w-5xl space-y-8 px-4 py-8">
            <div class="space-y-4 text-center">
                <h1 class="text-4xl font-bold text-white">"Glossaire Blind'All Music"</h1>
                <p class="text-lg text-neutral-400">"Définitions claires et précises des termes utilisés sur la plateforme"</p>
            </div>

            {section(view! {
                <h2 class="mb-4 text-2xl font-bold text-white">"À propos de ce glossaire"</h2>
                <p class=format!("mb-4 {TXT}")>"Ce glossaire définit de manière claire et assertive tous les termes techniques et concepts utilisés sur Blind'All Music. Chaque définition est structurée pour être facilement compréhensible et extractible par les moteurs de recherche génératifs."</p>
                <p class=TXT><strong>"Blind'All Music est une plateforme de quiz musicaux multijoueurs en ligne"</strong>" où les joueurs s'affrontent dans des blind tests organisés en rooms (salles de jeu). La plateforme utilise trois systèmes de progression distincts : les scores (activité), les levels (engagement) et l'ELO (compétence)."</p>
            })}

            <div class="space-y-6">
                {definitions.iter().map(|(term, def, ctx, related)| view! {
                    <div class=CARD>
                        <h2 class="mb-3 text-2xl font-bold text-white">{*term}</h2>
                        <p class="mb-3 text-lg text-neutral-300">{*def}</p>
                        <div class="mb-3 rounded-lg bg-slate-800/50 p-4">
                            <p class="text-sm text-neutral-300"><strong class="text-white">"Contexte Blind'All Music :"</strong>" "{*ctx}</p>
                        </div>
                        <div class="flex flex-wrap items-center gap-2">
                            <span class="text-xs text-neutral-400">"Voir aussi :"</span>
                            {related.iter().map(|r| view! {
                                <span class="rounded bg-slate-700/50 px-2 py-1 text-xs text-neutral-300">{*r}</span>
                            }).collect::<Vec<_>>()}
                        </div>
                    </div>
                }).collect::<Vec<_>>()}
            </div>

            <div class="rounded-2xl border border-indigo-700/50 bg-gradient-to-br from-indigo-900/20 to-indigo-800/20 p-6">
                <h2 class="mb-4 text-2xl font-bold text-indigo-400">"Facts clés sur Blind'All Music"</h2>
                <ul class="space-y-3 text-neutral-300">
                    <li><strong>"Blind'All Music est gratuit"</strong>" : Tous les quiz musicaux sont accessibles sans abonnement ni paiement."</li>
                    <li><strong>"Multijoueur en temps réel"</strong>" : Les parties se déroulent en direct avec plusieurs joueurs simultanés."</li>
                    <li><strong>"Trois systèmes de progression"</strong>" : Score (activité), Level (engagement), ELO (compétence)."</li>
                    <li><strong>"Rooms publiques et privées"</strong>" : Les rooms publiques sont officielles et gérées par Blind'All Music. Les rooms privées sont créées par les joueurs et peuvent être protégées par un mot de passe ou non."</li>
                    <li><strong>"Plusieurs fournisseurs musicaux"</strong>" : Deezer, YouTube, Apple Music, Audius et plus encore."</li>
                </ul>
            </div>

            <div class="pt-4 text-center">
                <A href="/docs" attr:class="text-blue-400 transition-colors hover:text-blue-300">" ← Retour à la documentation "</A>
            </div>
        </div>
    }
}

// ---- /docs/howto ---------------------------------------------------------------

#[component]
pub fn DocsHowtoPage() -> impl IntoView {
    crate::i18n::set_title_key("Docs howto title");
    crate::seo::apply_page("/docs/howto", t("Docs howto description"));
    crate::seo::set_meta("name", "keywords", t("Docs howto keywords"));
    let steps: &[(&str, &str, &[&str])] = &[
        ("Créer un compte ou se connecter", "Créez un compte gratuit sur Blind'All Music ou connectez-vous avec votre compte existant.",
         &["Vous pouvez utiliser Google ou Discord pour vous connecter rapidement", "Aucune carte bancaire requise - Blind'All Music est entièrement gratuit", "La création de compte prend moins d'une minute"]),
        ("Choisir une room", "Parcourez les rooms disponibles sur la page d'accueil.",
         &["Les rooms sont organisées par catégories : Années 2000, Disney, Chanson française, Rock, Pop, etc.", "Chaque room affiche le nombre de joueurs actifs et le nombre de rounds joués", "Cliquez sur une room pour voir ses détails et la rejoindre"]),
        ("Rejoindre la room", "Entrez dans la room et attendez que la partie commence.",
         &["Si la room est privée, entrez le mot de passe fourni par le créateur", "Les rooms publiques sont accessibles à tous sans mot de passe", "Une fois dans la room, vous verrez la liste des joueurs et les paramètres de la partie"]),
        ("Écouter et identifier", "Un extrait audio sera joué - écoutez attentivement pour identifier la chanson.",
         &["La durée par défaut est de 30 secondes par track", "Vous pouvez voir le temps restant pendant l'écoute", "Concentrez-vous sur la mélodie, les paroles et les instruments pour identifier la chanson"]),
        ("Saisir votre réponse", "Tapez votre réponse dans le champ de saisie.",
         &["Vous pouvez indiquer le titre, l'artiste, ou les deux", "Plus votre réponse est précise et rapide, plus vous gagnez de points", "Les réponses partielles peuvent aussi rapporter des points selon la précision"]),
        ("Voir les résultats", "Après chaque track, les bonnes réponses sont affichées.",
         &["Vous verrez votre score et votre position dans le classement en temps réel", "Les autres joueurs qui ont trouvé la bonne réponse sont également affichés", "Votre progression est mise à jour automatiquement"]),
        ("Continuer le round", "Le round continue avec plusieurs tracks jusqu'à la fin.",
         &["Un round contient généralement plusieurs tracks (configurable par le créateur de la room)", "À la fin du round, un podium affiche les trois meilleurs joueurs", "Votre score total, votre level et votre ELO sont mis à jour automatiquement"]),
    ];
    view! {
        <div class="mx-auto max-w-5xl space-y-8 px-4 py-8">
            <div class="space-y-4 text-center">
                <h1 class="text-4xl font-bold text-white">"Comment jouer à un blind test sur Blind'All Music"</h1>
                <p class="text-lg text-neutral-400">"Guide étape par étape pour débuter sur la plateforme de quiz musicaux multijoueurs"</p>
            </div>

            {section(view! {
                <h2 class="mb-4 text-2xl font-bold text-white">"À propos de ce guide"</h2>
                <p class=format!("mb-4 {TXT}")>"Ce guide explique comment jouer à un blind test sur Blind'All Music, étape par étape. "<strong>"Blind'All Music est une plateforme gratuite de quiz musicaux multijoueurs en ligne"</strong>" où vous pouvez tester vos connaissances musicales en temps réel avec d'autres joueurs."</p>
                <div class="rounded-lg bg-slate-800/50 p-4">
                    <p class="text-sm text-neutral-300">
                        <strong class="text-white">"Temps estimé :"</strong>" 5 minutes pour compléter votre première partie"<br/>
                        <strong class="text-white">"Coût :"</strong>" Gratuit - Aucun abonnement requis"<br/>
                        <strong class="text-white">"Prérequis :"</strong>" Compte Blind'All Music, connexion Internet, haut-parleurs ou casque audio"
                    </p>
                </div>
            })}

            <div class="space-y-6">
                {steps.iter().enumerate().map(|(i, (title, desc, details))| view! {
                    <div class=CARD>
                        <div class="flex items-start gap-4">
                            <div class="flex h-12 w-12 flex-shrink-0 items-center justify-center rounded-full border-2 border-blue-500/50 bg-blue-500/20">
                                <span class="text-xl font-bold text-blue-400">{i + 1}</span>
                            </div>
                            <div class="flex-1">
                                <h2 class="mb-3 text-2xl font-bold text-white">{*title}</h2>
                                <p class="mb-4 text-lg text-neutral-300">{*desc}</p>
                                <ul class="space-y-2">
                                    {details.iter().map(|d| view! {
                                        <li class="flex items-start gap-2 text-sm text-neutral-300">
                                            <span class="mt-1 text-blue-400">"•"</span><span>{*d}</span>
                                        </li>
                                    }).collect::<Vec<_>>()}
                                </ul>
                            </div>
                        </div>
                    </div>
                }).collect::<Vec<_>>()}
            </div>

            <div class="rounded-2xl border border-green-700/50 bg-gradient-to-br from-green-900/20 to-green-800/20 p-6">
                <h2 class="mb-4 text-2xl font-bold text-green-400">"Conseils pour améliorer votre score"</h2>
                <ul class="space-y-3 text-neutral-300">
                    <li><strong>"Répondez rapidement"</strong>" : Plus vous répondez vite, plus vous gagnez de points. La vitesse est un facteur clé dans le calcul du score."</li>
                    <li><strong>"Soyez précis"</strong>" : Indiquez à la fois le titre et l'artiste pour maximiser vos points. Les réponses partielles rapportent moins de points."</li>
                    <li><strong>"Jouez régulièrement"</strong>" : Plus vous jouez, plus vous progressez dans les levels et l'ELO. Les rooms publiques comptent pour votre progression."</li>
                    <li><strong>"Créez vos propres rooms"</strong>" : Créer des rooms et des playlists vous rapporte de l'XP et améliore votre level."</li>
                </ul>
            </div>

            {section(view! {
                <h2 class=H2>"Pour aller plus loin"</h2>
                <div class="grid gap-4 md:grid-cols-2">
                    {link_card("/docs", "Documentation complète", "Tout savoir sur les systèmes de progression", "blue")}
                    {link_card("/docs/glossary", "Glossaire", "Définitions des termes techniques", "green")}
                </div>
            })}

            <div class="pt-4 text-center">
                <A href="/docs" attr:class="text-blue-400 transition-colors hover:text-blue-300">" ← Retour à la documentation "</A>
            </div>
        </div>
    }
}

// ---- /docs/create-content -----------------------------------------------------------

#[component]
pub fn DocsCreateContentPage() -> impl IntoView {
    crate::i18n::set_title_key("Docs create content title");
    crate::seo::apply_page("/docs/create-content", t("Docs create content description"));
    crate::seo::set_meta("name", "keywords", t("Docs create content keywords"));
    view! {
        <div class="mx-auto max-w-5xl space-y-8 px-4 py-8">
            <div class="space-y-4 text-center">
                <h1 class="text-4xl font-bold text-white">"Créer des rooms et playlists"</h1>
                <p class="text-lg text-neutral-400">"Guide complet pour créer et configurer vos contenus sur Blind'All Music"</p>
            </div>

            {section(view! {
                <h2 class="mb-4 text-2xl font-bold text-white">"Comprendre la différence entre rooms et playlists"</h2>
                <div class=format!("space-y-4 {TXT}")>
                    <p><strong>"Une playlist est une collection de tracks (morceaux de musique)"</strong>" organisés par thème, genre ou catégorie. Les playlists sont réutilisables et peuvent être attachées à plusieurs rooms."</p>
                    <p><strong>"Une room est un espace de jeu"</strong>" qui utilise une ou plusieurs playlists pour organiser des parties de blind test. La room définit les règles de jeu (durée des tracks, nombre de tracks par round, etc.)."</p>
                    <div class="mt-4 rounded-lg bg-slate-800/50 p-4">
                        <p class="text-sm text-neutral-300"><strong class="text-white">"Relation :"</strong>" Une room peut utiliser plusieurs playlists, et une playlist peut être utilisée par plusieurs rooms. Les tracks appartiennent à une playlist unique, mais cette playlist peut être partagée entre plusieurs rooms."</p>
                    </div>
                </div>
            })}

            <div class="rounded-2xl border border-blue-700/50 bg-gradient-to-br from-blue-900/20 to-blue-800/20 p-6">
                <h2 class="mb-6 text-3xl font-bold text-blue-400">"Créer une playlist"</h2>
                <div class="space-y-6">
                    <div class="rounded-lg bg-slate-800/50 p-5">
                        <h3 class="mb-3 text-xl font-bold text-white">"Étape 1 : Créer la playlist"</h3>
                        <p class=format!("mb-4 {TXT}")>"Pour créer une playlist, vous devez fournir :"</p>
                        <ul class=format!("space-y-2 {TXT}")>
                            {bullet_list(&[
                                "Nom : Maximum 50 caractères, doit être unique sur la plateforme",
                                "Description : Optionnelle, permet de décrire le thème ou le contenu de la playlist",
                                "Visibilité : Vous pouvez rendre la playlist publique (accessible à tous) ou privée (uniquement pour vous)",
                            ])}
                        </ul>
                        <p class="mt-4 text-sm text-neutral-300"><strong>"Note :"</strong>" Vous êtes automatiquement ajouté comme modérateur de votre playlist. Vous pouvez ajouter d'autres modérateurs pour gérer la playlist ensemble."</p>
                    </div>
                    <div class="rounded-lg bg-slate-800/50 p-5">
                        <h3 class="mb-3 text-xl font-bold text-white">"Étape 2 : Ajouter des tracks à la playlist"</h3>
                        <p class=format!("mb-4 {TXT}")>"Une fois la playlist créée, vous pouvez y ajouter des morceaux de musique. Pour cela :"</p>
                        <ol class=format!("mb-4 ml-4 list-inside list-decimal space-y-3 {TXT}")>
                            <li><strong>"Utilisez la barre de recherche"</strong>" dans la section \"Rechercher des morceaux\" de votre playlist. Tapez le nom d'une chanson ou d'un artiste pour rechercher sur YouTube, Apple Music, Deezer et Blind'All Music."</li>
                            <li><strong>"Parcourez les résultats"</strong>" qui affichent le nom de l'artiste, le titre de la chanson, l'album et la pochette. Vous pouvez écouter un extrait avant d'ajouter le morceau."</li>
                            <li><strong>"Cliquez sur \"Ajouter\""</strong>" pour inclure le morceau dans votre playlist."</li>
                            <li><strong>"Importez depuis une autre plateforme"</strong>" : Vous pouvez également importer une playlist complète depuis Deezer si vous avez le lien."</li>
                            <li><strong>"Uploadez vos propres fichiers"</strong>" : Pour les morceaux non disponibles sur les plateformes, vous pouvez uploader vos propres fichiers audio."</li>
                        </ol>
                        <div class="mt-4 rounded-lg bg-blue-900/30 p-4">
                            <p class="text-sm text-neutral-300"><strong class="text-blue-400">"Astuce :"</strong>" Vous pouvez filtrer les résultats par fournisseur musical (YouTube, Apple Music, Deezer, Blind'All Music) pour trouver plus facilement ce que vous cherchez."</p>
                        </div>
                    </div>
                    <div class="rounded-lg bg-slate-800/50 p-5">
                        <h3 class="mb-3 text-xl font-bold text-white">"Étape 3 : Configurer les réponses et les points"</h3>
                        <p class=format!("mb-4 {TXT}")>"Après avoir ajouté un morceau à votre playlist, vous devez configurer les réponses que les joueurs devront trouver. Chaque morceau doit avoir au minimum deux réponses :"</p>
                        <ul class=format!("mb-4 space-y-3 {TXT}")>
                            <li class="flex items-start gap-2"><span class="mt-1 text-blue-400">"•"</span>
                                <div><span><strong>"Réponse Artiste"</strong>" : Le nom de l'artiste ou du groupe"</span>
                                    <p class="mt-1 text-sm text-neutral-400">"Cette réponse est obligatoire pour tous les morceaux."</p></div></li>
                            <li class="flex items-start gap-2"><span class="mt-1 text-blue-400">"•"</span>
                                <div><span><strong>"Réponse Titre"</strong>" : Le titre de la chanson"</span>
                                    <p class="mt-1 text-sm text-neutral-400">"Cette réponse est obligatoire pour tous les morceaux."</p></div></li>
                        </ul>
                        <p class=format!("mb-4 {TXT}")><strong>"Pour ajouter une réponse :"</strong></p>
                        <ol class=format!("mb-4 ml-4 list-inside list-decimal space-y-2 {TXT}")>
                            <li>"Cliquez sur le bouton "<strong>"\"Ajouter une réponse\""</strong>" sous le morceau dans votre playlist"</li>
                            <li>"Sélectionnez le "<strong>"type de réponse"</strong>" (Artiste, Titre, ou autre type disponible)"</li>
                            <li>"Entrez la "<strong>"réponse exacte"</strong>" que les joueurs devront trouver"</li>
                            <li>"Définissez le "<strong>"nombre de points"</strong>" attribués pour cette réponse (entre 0 et 99)"</li>
                            <li>"Cliquez sur "<strong>"\"Enregistrer\""</strong>" pour valider"</li>
                        </ol>
                        <p class=format!("mb-4 {TXT}")><strong>"Gestion des réponses :"</strong>" Vous pouvez modifier ou supprimer une réponse existante en cliquant dessus. Les réponses s'affichent sous chaque morceau avec leur type et leur nombre de points."</p>
                        <div class="mt-4 rounded-lg bg-blue-900/30 p-4">
                            <p class="text-sm text-neutral-300"><strong class="text-blue-400">"Système de points :"</strong>" Le score final d'un joueur pour une réponse = score de base de la réponse + bonus ordre (0.5 points si dans le top 3) + bonus vitesse (0.5 points si réponse donnée dans les 18% de la durée du morceau)."</p>
                        </div>
                    </div>
                </div>
            </div>

            <div class="rounded-2xl border border-purple-700/50 bg-gradient-to-br from-purple-900/20 to-purple-800/20 p-6">
                <h2 class="mb-6 text-3xl font-bold text-purple-400">"Créer une room"</h2>
                <div class="space-y-6">
                    <div class="rounded-lg bg-slate-800/50 p-5">
                        <h3 class="mb-3 text-xl font-bold text-white">"Étape 1 : Créer la room"</h3>
                        <p class=format!("mb-4 {TXT}")>"Pour créer une room, vous devez fournir :"</p>
                        <ul class=format!("space-y-2 {TXT}")>
                            {bullet_list(&[
                                "Nom : Maximum 25 caractères, doit être unique sur la plateforme",
                                "Catégorie : Obligatoire, détermine dans quelle section la room apparaît (Années 2000, Disney, Chanson française, etc.)",
                                "Description : Optionnelle, permet de décrire le thème ou les règles spéciales de la room",
                            ])}
                        </ul>
                        <p class="mt-4 text-sm text-neutral-300"><strong>"Note :"</strong>" Vous êtes automatiquement ajouté comme modérateur de votre room. Vous pouvez ajouter d'autres modérateurs pour gérer la room ensemble."</p>
                    </div>
                    <div class="rounded-lg bg-slate-800/50 p-5">
                        <h3 class="mb-3 text-xl font-bold text-white">"Étape 2 : Attacher des playlists à la room"</h3>
                        <p class=format!("mb-4 {TXT}")>"Une room peut utiliser plusieurs playlists. Les tracks de toutes les playlists attachées seront disponibles pour les parties."</p>
                        <ul class=format!("space-y-2 {TXT}")>
                            {bullet_list(&[
                                "Vous pouvez attacher vos propres playlists (publiques ou privées)",
                                "Vous pouvez attacher des playlists publiques créées par d'autres utilisateurs",
                                "Vous pouvez attacher des playlists dont vous êtes modérateur",
                            ])}
                        </ul>
                        <div class="mt-4 rounded-lg bg-purple-900/30 p-4">
                            <p class="text-sm text-neutral-300"><strong class="text-purple-400">"Important :"</strong>" Une room sans playlist attachée ne peut pas démarrer de partie car il n'y a pas de tracks disponibles."</p>
                        </div>
                    </div>
                    <div class="rounded-lg bg-slate-800/50 p-5">
                        <h3 class="mb-3 text-xl font-bold text-white">"Étape 3 : Configurer les options de jeu"</h3>
                        <p class=format!("mb-4 {TXT}")>"Les options de jeu déterminent comment se déroulent les parties dans votre room :"</p>
                        <div class="space-y-4">
                            {[
                                ("Tracks par round", "Nombre de tracks joués dans chaque round.", "Minimum : 1 track · Maximum : 50 tracks · Valeur par défaut : 15 tracks"),
                                ("Durée d'un track", "Durée en secondes de l'extrait audio joué pour chaque track.", "Minimum : 5 secondes · Maximum : 30 secondes · Valeur par défaut : 30 secondes"),
                                ("Pause entre les tracks", "Temps d'attente en secondes entre chaque track dans un round.", "Minimum : 0 secondes · Maximum : 60 secondes · Valeur par défaut : 0 secondes"),
                                ("Pause entre les rounds", "Temps d'attente en secondes entre la fin d'un round et le début du suivant.", "Minimum : 0 secondes · Maximum : 60 secondes · Valeur par défaut : 10 secondes"),
                            ].into_iter().map(|(h, d, v)| view! {
                                <div class="border-l-4 border-purple-500 pl-4">
                                    <h4 class="mb-2 font-bold text-white">{h}</h4>
                                    <p class="mb-2 text-sm text-neutral-300">{d}</p>
                                    <p class="text-sm text-neutral-300">"• "{v}</p>
                                </div>
                            }).collect::<Vec<_>>()}
                            <div class="border-l-4 border-purple-500 pl-4">
                                <h4 class="mb-2 font-bold text-white">"Options booléennes"</h4>
                                <ul class="space-y-2 text-sm text-neutral-300">
                                    <li><strong>"Publique"</strong>" : Si activée, la room est accessible à tous. Si désactivée, la room est privée et nécessite un mot de passe."</li>
                                    <li><strong>"Autostart"</strong>" : Si activée, les rounds démarrent automatiquement après la pause entre rounds. Si désactivée, un modérateur doit démarrer manuellement chaque round."</li>
                                    <li><strong>"Aléatoire"</strong>" : Si activée, les tracks sont joués dans un ordre aléatoire. Si désactivée, les tracks sont joués dans l'ordre de la playlist."</li>
                                    <li><strong>"Chat actif"</strong>" : Si activée, les joueurs peuvent utiliser le chat pendant les parties. Si désactivée, le chat est désactivé."</li>
                                </ul>
                            </div>
                        </div>
                    </div>
                </div>
            </div>

            <div class="rounded-2xl border border-green-700/50 bg-gradient-to-br from-green-900/20 to-green-800/20 p-6">
                <h2 class="mb-6 text-3xl font-bold text-green-400">"Workflow recommandé"</h2>
                <div class=format!("space-y-4 {TXT}")>
                    {[
                        ("Créer une ou plusieurs playlists", "Organisez vos tracks par thème ou genre dans des playlists réutilisables."),
                        ("Ajouter des tracks aux playlists", "Pour chaque track, configurez les réponses (Artiste et Titre) et les points associés."),
                        ("Créer une room", "Définissez le nom, la catégorie et la description de votre room."),
                        ("Attacher des playlists à la room", "Sélectionnez une ou plusieurs playlists qui seront utilisées pour les parties."),
                        ("Configurer les options de jeu", "Ajustez le nombre de tracks par round, la durée, les pauses et les options booléennes selon vos préférences."),
                    ].into_iter().enumerate().map(|(i, (h, d))| view! {
                        <div class="flex items-start gap-3">
                            <span class="text-xl font-bold text-green-400">{i + 1}"."</span>
                            <div>
                                <p class="font-semibold text-white">{h}</p>
                                <p class="text-sm">{d}</p>
                            </div>
                        </div>
                    }).collect::<Vec<_>>()}
                </div>
            </div>

            <div class="rounded-2xl border border-indigo-700/50 bg-gradient-to-br from-indigo-900/20 to-indigo-800/20 p-6">
                <h2 class="mb-4 text-2xl font-bold text-indigo-400">"Points importants à retenir"</h2>
                <ul class="space-y-3 text-neutral-300">
                    <li><strong>"Les playlists sont réutilisables"</strong>" : Une même playlist peut être utilisée par plusieurs rooms différentes."</li>
                    <li><strong>"Les tracks appartiennent à une seule playlist"</strong>" : Chaque track est lié à une playlist unique, mais cette playlist peut être partagée."</li>
                    <li><strong>"Les réponses sont obligatoires"</strong>" : Chaque track doit avoir au minimum une réponse Artiste et une réponse Titre."</li>
                    <li><strong>"Les points sont configurables"</strong>" : Vous pouvez définir le score de chaque réponse pour équilibrer la difficulté."</li>
                    <li><strong>"Les modérateurs peuvent gérer"</strong>" : Vous pouvez ajouter des modérateurs pour gérer vos playlists et rooms ensemble."</li>
                </ul>
            </div>

            <div class="pt-4 text-center">
                <A href="/docs" attr:class="text-blue-400 transition-colors hover:text-blue-300">" ← Retour à la documentation "</A>
            </div>
        </div>
    }
}

// ---- /docs/support : donation-backed -----------------------------------------------

fn euros(cents: i64) -> String {
    format!("{:.2} €", cents as f64 / 100.0)
}

fn month_label(key: &str) -> String {
    const MONTHS: &[&str] = &[
        "janvier", "février", "mars", "avril", "mai", "juin",
        "juillet", "août", "septembre", "octobre", "novembre", "décembre",
    ];
    let mut it = key.split('-');
    let year = it.next().unwrap_or("");
    let m = it.next().and_then(|m| m.parse::<usize>().ok()).unwrap_or(0);
    if m == 0 || m > 12 { return key.to_string(); }
    format!("{} {}", MONTHS[m - 1], year)
}

#[component]
pub fn DocsSupportPage() -> impl IntoView {
    crate::i18n::set_title_key("Docs support title");
    crate::seo::apply_page("/docs/support", t("Docs support description"));
    let history = RwSignal::new(Vec::<Value>::new());
    let recent = RwSignal::new(Vec::<Value>::new());
    let progress = RwSignal::new(Value::Null);

    leptos::task::spawn_local(async move {
        if let Ok(v) = api::get::<Value>("/api/v1/donations/progress").await {
            progress.set(v);
        }
        if let Ok(v) = api::get::<Value>("/api/v1/donations/history?months=12").await {
            history.set(v["history"].as_array().cloned().unwrap_or_default());
        }
        if let Ok(v) = api::get::<Value>("/api/v1/donations/recent?limit=30").await {
            recent.set(v["donations"].as_array().cloned().unwrap_or_default());
        }
    });

    view! {
        <div class="mx-auto max-w-5xl space-y-8 px-4 py-8">
            <div class="space-y-4 text-center">
                <h1 class="text-4xl font-bold text-white">{t("Support Blind'All Music")}</h1>
                <p class="mx-auto max-w-2xl text-lg text-neutral-400">{move || crate::i18n::tr("Docs support intro", &[("goal", crate::ads::goal_or(&progress.get()["goal_cents"].as_i64().map(crate::ads::euros).unwrap_or_default()))])}</p>
            </div>

            {move || {
                let p = progress.get();
                let raised = p["raised_cents"].as_i64().unwrap_or(0);
                let goal = p["goal_cents"].as_i64().unwrap_or(1).max(1);
                let pct = (raised as f64 / goal as f64 * 100.0).min(100.0);
                view! {
                    <div class=CARD>
                        <h2 class="mb-3 text-2xl font-bold text-white">{t("Monthly goal")}</h2>
                        <div class="mb-2 h-3 overflow-hidden rounded-full bg-slate-700">
                            <div class="h-full rounded-full bg-brand-accent" style=format!("width:{pct:.0}%")></div>
                        </div>
                        <p class="text-sm text-neutral-300">
                            {format!("{} / {} : {:.0}%", euros(raised), euros(goal), raised as f64 / goal as f64 * 100.0)}
                        </p>
                        <div class="mt-4">
                            <A href="/donations" attr:class="btn-primary !px-4 !py-2 text-sm">{t("Donate")}</A>
                        </div>
                    </div>
                }
            }}

            {section(view! {
                <h2 class="mb-4 text-2xl font-bold text-white">{t("How donations work")}</h2>
                <ul class=format!("ml-4 list-inside list-disc space-y-2 {TXT}")>
                    <li>{move || crate::i18n::tr("Donation howto goal", &[("goal", crate::ads::goal_or(&progress.get()["goal_cents"].as_i64().map(crate::ads::euros).unwrap_or_default()))])}</li>
                    <li>{t("Donation howto ads")}</li>
                    <li>{move || crate::i18n::tr("Donation howto surplus", &[("goal", crate::ads::goal_or(&progress.get()["goal_cents"].as_i64().map(crate::ads::euros).unwrap_or_default()))])}</li>
                    <li>{t("Donation howto transparency")}</li>
                </ul>
            })}

            {section(view! {
                <h2 class="mb-4 text-2xl font-bold text-white">{t("Recent donations")}</h2>
                <p class="mb-4 text-sm text-neutral-400">{t("Recent donations intro")}</p>
                <div class="space-y-2">
                    {move || {
                        let list = recent.get();
                        if list.is_empty() {
                            view! { <p class="text-sm text-neutral-500">{t("No donations yet this month.")}</p> }.into_any()
                        } else {
                            list.into_iter().map(|d| view! {
                                <div class="flex items-center justify-between rounded-lg bg-slate-800/50 px-4 py-2 text-sm">
                                    <span class="text-neutral-200">
                                        {d["user"]["name"].as_str().unwrap_or("Anonyme").to_string()}
                                    </span>
                                    <span class="font-medium text-emerald-300">
                                        {euros(d["amount_cents"].as_i64().unwrap_or(0))}
                                    </span>
                                </div>
                            }).collect::<Vec<_>>().into_any()
                        }
                    }}
                </div>
            })}

            <div class=format!("overflow-x-auto {CARD}")>
                <h2 class="mb-4 text-2xl font-bold text-white">{t("Monthly history")}</h2>
                <table class="w-full min-w-[40rem] border-collapse text-sm">
                    <thead>
                        <tr class="border-b border-slate-700 text-left text-neutral-400">
                            <th class="px-3 py-2 font-semibold">{t("Month")}</th>
                            <th class="px-3 py-2 font-semibold">{t("Raised")}</th>
                            <th class="px-3 py-2 font-semibold">{t("Carryover")}</th>
                            <th class="px-3 py-2 font-semibold">{t("Effective")}</th>
                            <th class="px-3 py-2 font-semibold">{t("Surplus")}</th>
                            <th class="px-3 py-2 font-semibold">{t("Status")}</th>
                        </tr>
                    </thead>
                    <tbody>
                        {move || history.get().into_iter().map(|row| {
                            let key = row["month_key"].as_str().unwrap_or_default().to_string();
                            let carry = row["carryover_cents"].as_i64().unwrap_or(0);
                            let surplus = row["surplus_cents"].as_i64().unwrap_or(0);
                            let reached = row["goal_reached"].as_bool().unwrap_or(false);
                            view! {
                                <tr class="border-b border-slate-700/50 text-neutral-200">
                                    <td class="px-3 py-3 capitalize">{month_label(&key)}</td>
                                    <td class="px-3 py-3">{euros(row["raised_cents"].as_i64().unwrap_or(0))}</td>
                                    <td class="px-3 py-3 text-amber-200/90">{if carry > 0 { euros(carry) } else { "—".into() }}</td>
                                    <td class="px-3 py-3 font-medium">{euros(row["effective_cents"].as_i64().unwrap_or(0))}</td>
                                    <td class="px-3 py-3 text-emerald-300/90">{if surplus > 0 { euros(surplus) } else { "—".into() }}</td>
                                    <td class="px-3 py-3">
                                        {if reached {
                                            view! { <span class="text-emerald-400">{t("Reached")}</span> }.into_any()
                                        } else {
                                            view! { <span class="text-neutral-400">{t("In progress")}</span> }.into_any()
                                        }}
                                    </td>
                                </tr>
                            }
                        }).collect::<Vec<_>>()}
                    </tbody>
                </table>
            </div>

            <div class="text-center">
                <A href="/docs" attr:class="text-sm text-neutral-400 underline-offset-2 hover:text-white hover:underline">{t("Back to docs")}</A>
            </div>
        </div>
    }
}

// ---- /blind-test-{slug} : category landing (`CategoryController::show`) --------------

#[component]
pub fn CategoryLandingPage() -> impl IntoView {
    let params = leptos_router::hooks::use_params_map();
    let slug = move || {
        params
            .get()
            .get("cat")
            .and_then(|c| c.strip_prefix("blind-test-").map(str::to_string))
    };
    let data = RwSignal::new(Option::<Value>::None);
    let not_found = RwSignal::new(false);

    leptos::task::spawn_local(async move {
        match slug() {
            Some(s) => match api::get::<Value>(&format!("/api/v1/categories/{s}/landing")).await {
                Ok(v) => {
                    crate::i18n::set_title(v["content"]["meta_title"].as_str());
                    if let Some(desc) = v["content"]["meta_description"].as_str() {
                        crate::seo::apply_page(&format!("/blind-test-{s}"), desc);
                    }
                    data.set(Some(v));
                }
                Err(_) => not_found.set(true),
            },
            None => not_found.set(true),
        }
    });

    view! {
        <div class="mx-auto max-w-5xl space-y-8 px-4 py-8">
            <Show when=move || not_found.get()>
                <div class="rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-6 text-center text-red-300">
                    "404 : "{t("Page not found")}
                </div>
            </Show>
            <Show when=move || data.get().is_none() && !not_found.get()>
                <div class="text-white/60">{t("Loading")}"…"</div>
            </Show>
            {move || data.get().map(|d| {
                let cat_name = d["category"]["name"].as_str().unwrap_or_default().to_string();
                let heading = d["content"]["heading"].as_str().unwrap_or_default().to_string();
                let intro = d["content"]["intro"].as_str().unwrap_or_default().to_string();
                let intro2 = d["content"]["intro_secondary"].as_str().unwrap_or_default().to_string();
                let rooms_heading = d["content"]["rooms_heading"].as_str().unwrap_or_default().to_string();
                let faq_title = d["content"]["faq_title"].as_str().unwrap_or_default().to_string();
                let rooms_count = d["roomsCount"].as_i64()
                    .or_else(|| d["rooms"].as_array().map(|a| a.len() as i64))
                    .unwrap_or(0);
                let faq: Vec<(String, String)> = d["content"]["faq"].as_array().map(|a| {
                    a.iter().map(|f| (
                        f["question"].as_str().unwrap_or_default().to_string(),
                        f["answer"].as_str().unwrap_or_default().to_string(),
                    )).collect()
                }).unwrap_or_default();
                let rooms: Vec<Value> = d["rooms"].as_array().cloned().unwrap_or_default();
                view! {
                    <nav class="text-sm text-neutral-500" aria-label="Breadcrumb">
                        <ol class="flex flex-wrap items-center gap-2">
                            <li><A href="/" attr:class="transition-colors hover:text-yellow-400">{t("Home")}</A></li>
                            <li aria-hidden="true">"/"</li>
                            <li class="text-neutral-300">{t(cat_name.as_str()).to_string()}</li>
                        </ol>
                    </nav>
                    <div class="space-y-3">
                        <h1 class="text-3xl font-extrabold text-white sm:text-4xl">{heading}</h1>
                        <p class="max-w-3xl text-base leading-relaxed text-neutral-300 sm:text-lg">{intro}</p>
                        <p class="max-w-3xl text-sm leading-relaxed text-neutral-400 sm:text-base">{intro2}</p>
                    </div>

                    <div class=CARD>
                        <div class="mb-4 flex flex-wrap items-end justify-between gap-3">
                            <h2 class=H2>{rooms_heading}</h2>
                            <p class="text-sm text-neutral-500">
                                {crate::i18n::tr(":count rooms", &[(":count", rooms_count.to_string())])}
                            </p>
                        </div>
                        <div class="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
                            {rooms.into_iter().map(|r| {
                                let href = format!("/rooms/{}", r["slug"].as_str().unwrap_or(&r["id"].as_i64().unwrap_or(0).to_string()));
                                let name = r["name"].as_str().unwrap_or_default().to_string();
                                let members = r["member_count"].as_i64().unwrap_or(0);
                                let playing = r["is_playing"].as_bool().unwrap_or(false);
                                let photo = r["photo_path"].as_str().map(|p| format!("/storage/{p}"));
                                view! {
                                    <A href=href attr:class="group flex items-center gap-4 rounded-xl border border-slate-700 bg-slate-800/50 p-4 transition hover:bg-slate-800/70">
                                        {match photo {
                                            Some(src) => view! { <img src=src class="h-14 w-14 rounded-lg object-cover" alt=""/> }.into_any(),
                                            None => view! { <div class="flex h-14 w-14 items-center justify-center rounded-lg bg-slate-700 text-2xl">"🎵"</div> }.into_any(),
                                        }}
                                        <div class="min-w-0 flex-1">
                                            <h3 class="truncate font-semibold text-white group-hover:text-brand-accent">{name}</h3>
                                            <p class="text-xs text-neutral-400">
                                                {members}" "{t("players")}
                                                {playing.then(|| " · 🟢 Live")}
                                            </p>
                                        </div>
                                    </A>
                                }
                            }).collect::<Vec<_>>()}
                        </div>
                    </div>

                    <div class=CARD>
                        <h2 class=H2>{faq_title}</h2>
                        <div class="space-y-4">
                            {faq.into_iter().map(|(q, a)| view! {
                                <div class="rounded-lg bg-slate-800/50 p-5">
                                    <h3 class="mb-2 font-semibold text-blue-400">{q}</h3>
                                    <p class="text-sm text-neutral-300">{a}</p>
                                </div>
                            }).collect::<Vec<_>>()}
                        </div>
                    </div>

                    <div class="rounded-2xl border border-indigo-700/50 bg-gradient-to-br from-indigo-900/20 to-indigo-800/20 p-6 text-center">
                        <h2 class="mb-2 text-2xl font-bold text-indigo-400">{t("Category page rankings CTA title")}</h2>
                        <p class="mb-4 text-sm text-neutral-300">{t("Category page rankings CTA body")}</p>
                        <A href="/rankings" attr:class="btn-primary !px-5 !py-2">{t("Rankings")}</A>
                    </div>
                }
            })}
        </div>
    }
}

// ---- /contact -------------------------------------------------------------------------

#[component]
pub fn ContactPage() -> impl IntoView {
    crate::i18n::set_title_key("Contact");
    crate::seo::apply_page("/contact", t("Contact meta description"));
    let email = RwSignal::new(String::new());
    let message = RwSignal::new(String::new());
    let sent = RwSignal::new(false);
    let err = RwSignal::new(Option::<String>::None);
    let busy = RwSignal::new(false);

    view! {
        <div class="mx-auto max-w-2xl px-4 py-8">
            <Show when=move || sent.get() fallback=move || view! {
                <div class="rounded-xl border border-white/10 bg-surface-raised/60">
                    <div class="border-b border-white/10 px-5 py-4">
                        <h3 class="text-xl font-bold text-white">{t("Send a message to")} " Blind'All Music"</h3>
                    </div>
                    <div class="p-5">
                        <p class="mb-3 rounded-lg bg-blue-900/40 px-4 py-3 text-sm text-blue-200">
                            {t("For any request to add or modify tracks in public playlists, please contact the moderators directly")}
                        </p>
                        <p class="mb-4 text-sm text-white/60">
                            {t("To report a bug or suggest a site improvement, please use the following link:")}
                            " "
                            <a href="https://github.com/byPreaZy/blindallmusic/issues" target="_blank" rel="noopener noreferrer" class="underline">
                                {t("Report a bug")}
                            </a>
                        </p>
                        <form class="space-y-4"
                            on:submit=move |ev| {
                                ev.prevent_default();
                                busy.set(true); err.set(None);
                                leptos::task::spawn_local(async move {
                                    let body = serde_json::json!({
                                        "email": email.get_untracked(),
                                        "message": message.get_untracked(),
                                    });
                                    match api::post::<Value, _>("/api/v1/contact", &body).await {
                                        Ok(_) => sent.set(true),
                                        Err(e) => err.set(Some(e.message)),
                                    }
                                    busy.set(false);
                                });
                            }>
                            {move || err.get().map(|e| view! { <p class="form-error">{e}</p> })}
                            <div>
                                <label class="form-label">{t("Email")}</label>
                                <input bind:value=email type="email" required maxlength="255" class="form-input"/>
                            </div>
                            <div>
                                <label class="form-label">{t("Message")}</label>
                                <textarea bind:value=message required minlength="10" maxlength="5000" rows="10" class="form-input"></textarea>
                            </div>
                            <div class="flex justify-end">
                                <button type="submit" class="btn-primary" disabled=move || busy.get()>
                                    {t("Submit")}
                                </button>
                            </div>
                        </form>
                    </div>
                </div>
            }>
                <div class="rounded-xl border border-white/10 bg-surface-raised/60">
                    <div class="border-b border-white/10 px-5 py-4">
                        <h3 class="text-xl font-bold text-white">{t("Message sent")} "!"</h3>
                    </div>
                    <div class="p-5">
                        <p class="mb-4 text-white/70">{t("Thank you, the message has been successfully sent")}</p>
                        <A href="/" attr:class="btn-primary inline-block">{t("Return to Homepage")}</A>
                    </div>
                </div>
            </Show>
        </div>
    }
}

// ---- legacy redirects -------------------------------------------------------------

#[component]
pub fn FaqRedirect() -> impl IntoView {
    let navigate = leptos_router::hooks::use_navigate();
    Effect::new(move |_| {
        navigate("/docs/faq", Default::default());
    });
    view! { <div class="px-4 py-16 text-center text-white/60">{t("Loading")}"…"</div> }
}

#[component]
pub fn RankingSortRedirect() -> impl IntoView {
    let params = leptos_router::hooks::use_params_map();
    let navigate = leptos_router::hooks::use_navigate();
    Effect::new(move |_| {
        let s = params.get_untracked().get("sort").unwrap_or_default();
        let sort = match s.as_str() {
            "level" | "score" | "elo" | "week" => s,
            _ => "elo".to_string(),
        };
        navigate(&format!("/rankings?sort={sort}"), Default::default());
    });
    view! { <div class="px-4 py-16 text-center text-white/60">{t("Loading")}"…"</div> }
}

#[component]
pub fn LocaleRedirect() -> impl IntoView {
    let location = leptos_router::hooks::use_location();
    let navigate = leptos_router::hooks::use_navigate();
    Effect::new(move |_| {
        let path = location.pathname.get_untracked();
        let (rest, prefixed) = if path == "/en" || path == "/es" {
            ("/", true)
        } else if let Some(r) = path.strip_prefix("/en/") {
            (r, true)
        } else if let Some(r) = path.strip_prefix("/es/") {
            (r, true)
        } else {
            ("", false)
        };
        if !prefixed {
            return;
        }
        crate::i18n::set_locale(if path.starts_with("/en") { "en" } else { "es" });
        let search = location.search.get_untracked();
        let rest = rest.trim_start_matches('/');
        let target = if rest.is_empty() {
            format!("/{search}")
        } else {
            format!("/{rest}{search}")
        };
        navigate(&target, Default::default());
    });
    view! { <div class="px-4 py-16 text-center text-white/60">{t("Loading")}"…"</div> }
}

#[component]
pub fn LanguageSwitch() -> impl IntoView {
    let params = leptos_router::hooks::use_params_map();
    let navigate = leptos_router::hooks::use_navigate();
    Effect::new(move |_| {
        let lang = params.get_untracked().get("lang").unwrap_or_default();
        crate::i18n::set_locale(&lang);
        navigate("/", Default::default());
    });
    view! { <div class="px-4 py-16 text-center text-white/60">{t("Loading")}"…"</div> }
}

// ---- /banned : `Pages/Banned.vue` -----------------------------------------------------

#[component]
pub fn BannedPage() -> impl IntoView {
    crate::i18n::set_title_key("You have been banned!");
    crate::seo::set_meta("name", "robots", "noindex, nofollow");
    let ban = RwSignal::new(Option::<Value>::None);
    let loaded = RwSignal::new(false);
    leptos::task::spawn_local(async move {
        if let Ok(v) = api::get::<Value>("/api/v1/user/banned").await {
            ban.set(v["ban"].as_object().map(|_| v["ban"].clone()));
        }
        loaded.set(true);
    });
    view! {
        <div class="mx-auto max-w-xl px-4 py-16">
            <div class=CARD>
                <h1 class="my-4 text-xl text-red-500">{t("You have been banned from blindallmusic.com!")}</h1>
                {move || {
                    if !loaded.get() {
                        view! { <p class="text-white/60">{t("Loading")}"…"</p> }.into_any()
                    } else if let Some(b) = ban.get() {
                        let comment = b["comment"].as_str().unwrap_or_default().to_string();
                        let exp = b["expired_at"].as_str().unwrap_or_default().to_string();
                        view! {
                            <div class="space-y-3 text-neutral-300">
                                <p><b>{t("Reason for ban:")}</b><br/>{comment}</p>
                                <p><b>{t("The eviction from the site ends on:")}</b><br/>{exp}</p>
                                <p class="text-orange-500">{t("In some cases, a report can be made to the relevant authorities")}</p>
                                <p class="text-orange-500">{t("You can always access your account management to comply with data protection rules, but in case of reporting we reserve the right to retain evidence as provided by law")}</p>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="space-y-3 text-neutral-300">
                                <p>{t("The IP address you are browsing from has been blocked")}</p>
                                <p class="text-orange-500">{t("The IP address is blocked because a user with the same address was banned or suspicious activity was detected on that IP")}</p>
                            </div>
                        }.into_any()
                    }
                }}
            </div>
        </div>
    }
}
