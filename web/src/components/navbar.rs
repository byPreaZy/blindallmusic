use crate::auth::use_auth;
use crate::components::level::LevelDisplay;
use crate::components::notifications::Notifications;
use crate::i18n::{locale, set_locale, t};
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::{use_location, use_navigate, use_query_map};
use send_wrapper::SendWrapper;
use wasm_bindgen::JsCast;

#[component]
fn SearchRooms() -> impl IntoView {
    let query = use_query_map();
    let navigate = StoredValue::new(use_navigate());
    let input_ref = NodeRef::<leptos::html::Input>::new();
    let value = RwSignal::new(String::new());
    let focused = RwSignal::new(false);
    let timeout = StoredValue::new(SendWrapper::new(
        Option::<gloo_timers::callback::Timeout>::None,
    ));

    {
        let input_ref = input_ref;
        leptos::task::spawn_local(async move {
            let cb = wasm_bindgen::closure::Closure::<dyn Fn(web_sys::KeyboardEvent)>::new(
                move |ev: web_sys::KeyboardEvent| {
                    if ev.key() == "/" {
                        let tag = ev
                            .target()
                            .and_then(|t| {
                                wasm_bindgen::JsCast::dyn_ref::<web_sys::HtmlElement>(&t)
                                    .map(|e| e.tag_name())
                            })
                            .unwrap_or_default();
                        if tag != "INPUT" && tag != "TEXTAREA" {
                            ev.prevent_default();
                            if let Some(i) = input_ref.get() {
                                let _ = i.focus();
                            }
                        }
                    }
                },
            );
            let _ = web_sys::window()
                .unwrap()
                .add_event_listener_with_callback("keydown", cb.as_ref().unchecked_ref());
            cb.forget();
        });
    }

    let debounced = move |v: String| {
        timeout.update_value(|t| **t = None); // dropping cancels pending nav
        let url = if v.trim().is_empty() {
            "/".to_string()
        } else {
            format!("/?search={}", js_sys::encode_uri_component(&v))
        };
        timeout.update_value(|t| {
            **t = Some(gloo_timers::callback::Timeout::new(150, move || {
                navigate.with_value(|n| n(&url, Default::default()));
            }))
        });
    };

    Effect::new(move |_| {
        let url_q = query.get().get("search").unwrap_or_default();
        if url_q != value.get_untracked() {
            value.set(url_q);
        }
    });

    let clear = move || {
        value.set(String::new());
        navigate.with_value(|n| n("/", Default::default()));
        if let Some(i) = input_ref.get() {
            let _ = i.focus();
        }
    };

    view! {
        <form class="relative w-full" on:submit=move |ev| ev.prevent_default()>
            <div class=move || {
                if focused.get() { "retro-search group border-white/25 shadow-lg" } else { "retro-search group" }
            }>
                <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor"
                     class="h-5 w-5 text-white/50 transition-colors group-hover:text-white" class:text-white=move || focused.get()>
                    <path stroke-linecap="round" stroke-linejoin="round" d="M21 21l-5.197-5.197m0 0A7.5 7.5 0 105.196 5.196a7.5 7.5 0 0010.607 10.607z" />
                </svg>
                <input node_ref=input_ref
                    class="w-full bg-transparent px-3 py-1 text-sm text-white placeholder-white/60 focus:outline-none"
                    placeholder=format!("{}...", t("Search a room"))
                    spellcheck="false"
                    prop:value=move || value.get()
                    on:input=move |ev| { let v = event_target_value(&ev); value.set(v.clone()); debounced(v); }
                    on:focus=move |_| focused.set(true)
                    on:blur=move |_| focused.set(false)
                    on:keydown=move |ev| { if ev.key() == "Escape" { clear(); } } />
                <Show when=move || !value.get().is_empty() fallback=move || view! {
                    <kbd class="hidden h-5 w-5 items-center justify-center border border-white/20 bg-brand-midnight text-xs text-white/50 md:flex" title="Press / to search">"/"</kbd>
                }>
                    <button type="button" class="flex h-6 w-6 items-center justify-center bg-brand-midnight text-white/70 transition-colors hover:text-white" title="Clear search"
                        on:click=move |_| clear()>
                        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class="h-4 w-4">
                            <path fill-rule="evenodd" d="M5.47 5.47a.75.75 0 011.06 0L12 10.94l5.47-5.47a.75.75 0 111.06 1.06L13.06 12l5.47 5.47a.75.75 0 01-1.06-1.06L12 13.06l-5.47 5.47a.75.75 0 01-1.06 1.06L10.94 12 5.47 6.53a.75.75 0 010-1.06z" clip-rule="evenodd" />
                        </svg>
                    </button>
                </Show>
            </div>
        </form>
    }
}

#[component]
fn LanguageSwitcher() -> impl IntoView {
    view! {
        <label class="sr-only" for="language-select">{t("Language")}</label>
        <select id="language-select"
            class="cursor-pointer rounded-lg border border-white/10 bg-transparent px-1.5 py-1 text-xs text-white/70 hover:text-white"
            prop:value=move || locale()
            on:change=move |ev| {
                let v = event_target_value(&ev);
                set_locale(&v); // persists + reloads on change
            }>
            <option value="fr" class="bg-surface-raised text-white">"fr"</option>
            <option value="en" class="bg-surface-raised text-white">"en"</option>
            <option value="es" class="bg-surface-raised text-white">"es"</option>
        </select>
    }
}

fn gamepad_icon(cls: &'static str) -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class=cls aria-hidden="true">
            <path d="M7.5 6.5A4.5 4.5 0 003 11v3.5a4.5 4.5 0 008.6 1.9l.5-.9h-.2l.5.9a4.5 4.5 0 008.6-1.9V11a4.5 4.5 0 00-4.5-4.5h-9zM8 9.5a.75.75 0 01.75.75v.75h.75a.75.75 0 010 1.5h-.75v.75a.75.75 0 01-1.5 0v-.75H6.5a.75.75 0 010-1.5h.75v-.75A.75.75 0 018 9.5zm7.5 1a1 1 0 110 2 1 1 0 010-2zm2.5 2.5a1 1 0 110 2 1 1 0 010-2z"/>
        </svg>
    }
}

fn trophy_icon(cls: &'static str) -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class=cls aria-hidden="true">
            <path d="M5 3h14a1 1 0 011 1v1h2a1 1 0 011 1v2a4 4 0 01-4 4h-.42A6 6 0 0113 15.92V19h3a1 1 0 110 2H8a1 1 0 110-2h3v-3.08A6 6 0 015.42 12H5a4 4 0 01-4-4V6a1 1 0 011-1h2V4a1 1 0 011-1zm14 4v3a2 2 0 002-2V7h-2zM3 7v1a2 2 0 002 2V7H3z"/>
        </svg>
    }
}

fn help_icon(cls: &'static str) -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class=cls aria-hidden="true">
            <path fill-rule="evenodd" d="M2.25 12c0-5.385 4.365-9.75 9.75-9.75s9.75 4.365 9.75 9.75-4.365 9.75-9.75 9.75S2.25 17.385 2.25 12zm11.378-3.917c-.89-.777-2.366-.777-3.255 0a.75.75 0 01-.988-1.129c1.454-1.272 3.776-1.272 5.23 0 1.513 1.324 1.513 3.518 0 4.842a3.75 3.75 0 01-.837.552c-.676.328-1.028.774-1.028 1.152v.75a.75.75 0 01-1.5 0v-.75c0-1.279 1.06-2.107 1.907-2.496.393-.178.755-.322.97-.433.285-.148.345-.426.345-.604 0-.324-.11-.521-.344-.634zM12 15.75a.75.75 0 100 1.5.75.75 0 000-1.5z" clip-rule="evenodd"/>
        </svg>
    }
}

fn home_icon(cls: &'static str) -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class=cls aria-hidden="true">
            <path fill-rule="evenodd" d="M11.293 3.293a1 1 0 011.414 0l6 6a1 1 0 01.293.707V19a2 2 0 01-2 2h-3a1 1 0 01-1-1v-4h-2v4a1 1 0 01-1 1H7a2 2 0 01-2-2v-8.997a1 1 0 01.293-.707l6-6z" clip-rule="evenodd"/>
        </svg>
    }
}

fn user_icon(cls: &'static str) -> impl IntoView {
    view! {
        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="currentColor" class=cls aria-hidden="true">
            <path fill-rule="evenodd" d="M7.5 6a4.5 4.5 0 119 0 4.5 4.5 0 01-9 0zM3.751 20.105a8.25 8.25 0 0116.498 0 .75.75 0 01-.437.695A18.683 18.683 0 0112 22.5c-2.786 0-5.433-.608-7.812-1.7a.75.75 0 01-.437-.695z" clip-rule="evenodd"/>
        </svg>
    }
}

#[component]
fn HelpDropdown() -> impl IntoView {
    let open = RwSignal::new(false);
    let link = move |href: &'static str, label: &'static str| {
        view! {
            <A href=href attr:class="retro-menu-link" on:click=move |_| open.set(false)>
                {t(label)}
            </A>
        }
    };
    view! {
        <div class="relative">
            <button type="button" title=t("Help & Documentation")
                class="retro-icon-btn cursor-pointer text-white/70 hover:text-brand-accent"
                on:click=move |_| open.update(|o| *o = !*o)>
                {help_icon("h-5 w-5")}
            </button>
            <Show when=move || open.get()>
                <div class="fixed inset-0 z-40" on:click=move |_| open.set(false)/>
                <div class="absolute left-0 z-50 mt-2 w-56 rounded-xl border border-white/10 bg-surface-raised py-2 shadow-2xl">
                    <div class="retro-menu-label">{t("Guides")}</div>
                    {link("/docs/howto", "Comment jouer")}
                    {link("/docs/create-content", "Créer rooms & playlists")}
                    <div class="my-1 border-t border-white/10"></div>
                    <div class="retro-menu-label">{t("Référence")}</div>
                    {link("/docs/glossary", "Glossaire")}
                    {link("/docs/faq", "FAQ")}
                    <div class="my-1 border-t border-white/10"></div>
                    <div class="retro-menu-label">{t("Systèmes de progression")}</div>
                    {link("/docs", "Vue d'ensemble")}
                    {link("/docs/level", "Levels")}
                    {link("/docs/elo", "ELO")}
                </div>
            </Show>
        </div>
    }
}

#[component]
fn UserDropdown() -> impl IntoView {
    let auth = use_auth();
    let open = RwSignal::new(false);

    let avatar = move || {
        let u = auth.user.get().unwrap_or_default();
        let src = u.photo.clone().unwrap_or_else(|| {
            format!(
                "/api/v1/avatar?name={}",
                js_sys::encode_uri_component(&u.name)
            )
        });
        view! {
            <img src=src alt=u.name
                class="h-10 w-10 rounded-full border-2 border-white/20 object-cover transition-colors group-hover:border-brand-accent"
                loading="lazy"/>
        }
    };

    let link = move |href: String, label: &'static str| {
        view! {
            <li>
                <A href=href attr:class="retro-menu-link" on:click=move |_| open.set(false)>
                    {t(label)}
                </A>
            </li>
        }
    };

    view! {
        <div class="relative">
            <button type="button" class="group flex cursor-pointer select-none items-center"
                on:click=move |_| open.update(|o| *o = !*o)>
                <span class="mr-1 whitespace-nowrap">{avatar}</span>
            </button>
            <Show when=move || open.get()>
                <div class="fixed inset-0 z-40" on:click=move |_| open.set(false)/>
                <ul class="absolute right-0 z-50 mt-2 w-48 rounded-xl border border-white/10 bg-surface-raised py-1 shadow-2xl">
                    {link("/me".into(), "My account")}
                    {link("/teams".into(), "Teams")}
                    {link("/rooms".into(), "Rooms")}
                    {link("/playlists".into(), "Playlists")}
                    {move || auth.user.get().map(|u| u.is_administrator).unwrap_or(false).then(|| {
                        link("/admin".into(), "Administration")
                    })}
                    {move || auth.user.get().map(|u| u.is_public_moderator).unwrap_or(false).then(|| {
                        link("/moderation".into(), "Modération")
                    })}
                    <li class="my-1 border-t border-white/10">
                        <div class="flex items-center justify-between px-4 py-3">
                            <span class="text-sm text-white/70">{t("Language")}</span>
                            <LanguageSwitcher/>
                        </div>
                    </li>
                    <li>
                        <button type="button" class="retro-menu-link w-full text-left text-brand-primary"
                            on:click=move |_| { open.set(false); auth.logout(); }>
                            {t("Logout")}
                        </button>
                    </li>
                </ul>
            </Show>
        </div>
    }
}

#[component]
fn MobileBottomNav() -> impl IntoView {
    let auth = use_auth();
    let location = use_location();
    let tab_cls = move |active: bool| {
        if active {
            "flex min-w-0 flex-1 flex-col items-center justify-center gap-0.5 px-1 py-1.5 text-brand-accent"
        } else {
            "flex min-w-0 flex-1 flex-col items-center justify-center gap-0.5 px-1 py-1.5 text-white/50 transition-colors hover:text-white/80"
        }
    };
    view! {
        <nav class="surface-nav fixed bottom-0 left-0 right-0 z-50 border-t border-white/10 md:hidden" style="padding-bottom: env(safe-area-inset-bottom)">
            <div class="flex items-stretch justify-evenly">
                {move || {
                    let path = location.pathname.get();
                    let query_tab = location.query.get().get("tab").unwrap_or_default();
                    let games_tab = path == "/" && query_tab == "minigames";
                    let home_active = (path == "/" && !games_tab) || path.starts_with("/rooms");
                    let games_active = path.starts_with("/minigames")
                        || path.starts_with("/duels")
                        || path.starts_with("/daily")
                        || path.starts_with("/games")
                        || games_tab;
                    let rankings_active = path.starts_with("/rankings");
                    let profile_active = path.starts_with("/profile") || path.starts_with("/settings");
                    let u = auth.user.get();
                    let profile_href = u.as_ref().map(|u| format!("/profile/{}", u.id)).unwrap_or_else(|| "/login".to_string());
                    view! {
                        <A href="/" attr:class=tab_cls(home_active)>
                            {home_icon("h-5 w-5")}
                            <span class="text-[10px] font-semibold leading-tight">{t("Home")}</span>
                        </A>
                        <A href="/?tab=minigames" attr:class=tab_cls(games_active)>
                            {gamepad_icon("h-5 w-5")}
                            <span class="text-[10px] font-semibold leading-tight">{t("Games")}</span>
                        </A>
                        <A href="/rankings" attr:class=tab_cls(rankings_active)>
                            {trophy_icon("h-5 w-5")}
                            <span class="text-[10px] font-semibold leading-tight">{t("Rankings")}</span>
                        </A>
                        {move || match u.clone() {
                            Some(u) if !u.is_guest => view! {
                                <A href=profile_href.clone() attr:class=tab_cls(profile_active)>
                                    {user_icon("h-5 w-5")}
                                    <span class="text-[10px] font-semibold leading-tight">{t("Profile")}</span>
                                </A>
                                <div class="flex min-w-0 flex-1 items-center justify-center px-1 py-1.5 text-white/50">
                                    <UserDropdown/>
                                </div>
                            }.into_any(),
                            Some(_) => view! {
                                <button class=tab_cls(false) on:click=move |_| auth.guest_to("/login")>
                                    {user_icon("h-5 w-5")}
                                    <span class="text-[10px] font-semibold leading-tight">{t("Login")}</span>
                                </button>
                                <button class=tab_cls(false) on:click=move |_| auth.guest_to("/register")>
                                    <span class="text-[10px] font-semibold leading-tight text-brand-accent">{t("Register")}</span>
                                </button>
                            }.into_any(),
                            None => view! {
                                <A href="/login" attr:class=tab_cls(false)>
                                    {user_icon("h-5 w-5")}
                                    <span class="text-[10px] font-semibold leading-tight">{t("Login")}</span>
                                </A>
                                <A href="/register" attr:class=tab_cls(false)>
                                    <span class="text-[10px] font-semibold leading-tight text-brand-accent">{t("Register")}</span>
                                </A>
                            }.into_any(),
                        }}
                    }
                }}
            </div>
        </nav>
    }
}

#[component]
pub fn Navbar() -> impl IntoView {
    let auth = use_auth();

    let is_room_show = move || {
        leptos_router::hooks::use_location()
            .pathname
            .get()
            .strip_prefix("/rooms/")
            .map(|rest| !rest.is_empty() && rest != "create" && !rest.contains('/'))
            .unwrap_or(false)
    };

    view! {
        <Show when=move || !is_room_show()>
            <div class="surface-nav border-b border-white/10 px-4 pb-3 pt-2 md:hidden">
                <div class="mb-2 flex items-center justify-between">
                    <A href="/" attr:class="retro-title retro-title--accent text-lg tracking-wider">"Blind'All Music"</A>
                    <div class="flex items-center gap-2">
                        <Show when=move || auth.user.get().map(|u| !u.is_guest).unwrap_or(false)>
                            <Notifications/>
                            <LevelDisplay/>
                        </Show>
                    </div>
                </div>
                <SearchRooms/>
            </div>
        </Show>
        <nav class="surface-nav sticky top-0 z-40 border-b border-white/10">
            <div class="relative hidden items-center justify-between px-4 py-2 sm:px-6 md:flex md:px-12">
                <div class="relative z-10 w-full lg:w-1/4">
                    <A href="/" attr:title="Blind'All Music" attr:class="group inline-block transition-all duration-200 hover:scale-[102%]">
                        <span class="retro-title retro-title--accent text-2xl tracking-wider">"Blind'All Music"</span>
                        <p class="mt-1 hidden text-sm tracking-widest text-white/60 transition-colors duration-200 group-hover:text-white lg:block">
                            {t("Tune In, Test Out!")}
                        </p>
                    </A>
                </div>
                <div class="relative z-10 flex w-full items-center justify-between pl-2 pr-8 lg:w-3/4">
                    <div class="mt-1 flex items-center gap-3">
                        <div class="w-56 lg:w-72">
                            <SearchRooms/>
                        </div>
                        <HelpDropdown/>
                    </div>
                    <div class="flex items-center gap-3">
                        <A href="/?tab=minigames" attr:title=t("Mini-games")
                            attr:class="retro-icon-btn text-brand-secondary hover:text-brand-secondary">
                            {gamepad_icon("h-5 w-5 drop-shadow-[0_0_8px_rgb(242_201_76/0.5)]")}
                        </A>
                        <A href="/rankings" attr:title=t("Rankings")
                            attr:class="retro-icon-btn text-brand-secondary hover:text-brand-secondary">
                            {trophy_icon("h-5 w-5 drop-shadow-[0_0_8px_rgb(242_201_76/0.5)]")}
                        </A>
                        <Show
                            when=move || auth.user.get().map(|u| !u.is_guest).unwrap_or(false)
                            fallback=move || {
                                if auth.user.get().is_some() {
                                    view! {
                                        <div class="flex gap-3">
                                            <button title=t("Login") class="retro-nav-btn"
                                                on:click=move |_| auth.guest_to("/login")>
                                                {t("Login")}
                                            </button>
                                            <button title=t("Register") class="retro-nav-btn--primary hidden lg:block"
                                                on:click=move |_| auth.guest_to("/register")>
                                                {t("Register")}
                                            </button>
                                        </div>
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="flex gap-3">
                                            <A href="/login" attr:title=t("Login") attr:class="retro-nav-btn">
                                                {t("Login")}
                                            </A>
                                            <A href="/register" attr:title=t("Register") attr:class="retro-nav-btn--primary hidden lg:block">
                                                {t("Register")}
                                            </A>
                                        </div>
                                    }.into_any()
                                }
                            }
                        >
                            <Notifications/>
                            <LevelDisplay/>
                            <UserDropdown/>
                        </Show>
                    </div>
                </div>
            </div>
        </nav>
        <MobileBottomNav/>
    }
}

#[component]
pub fn Spinner() -> impl IntoView {
    view! {
        <svg class="h-8 w-8 animate-spin text-brand-accent" xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
            <path class="opacity-90" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z"></path>
        </svg>
    }
}
