use crate::api::{self, ApiError};
use crate::auth::use_auth;
use crate::i18n::t;
use leptos::prelude::*;
use leptos_router::components::A;
use serde_json::{json, Value};
use wasm_bindgen::JsCast;

#[component]
pub fn SettingsPage() -> impl IntoView {
    crate::i18n::set_title_key("My account");
    let auth = use_auth();
    crate::auth::require_registered(auth);
    let account = RwSignal::new(Value::Null);
    let navigate = StoredValue::new(leptos_router::hooks::use_navigate());
    let loading = RwSignal::new(true);
    let name = RwSignal::new(String::new());
    let email = RwSignal::new(String::new());
    let tag = RwSignal::new(String::new());
    let cur_pw = RwSignal::new(String::new());
    let new_pw = RwSignal::new(String::new());
    let new_pw2 = RwSignal::new(String::new());
    let show_donations = RwSignal::new(true);
    let msg = RwSignal::new(Option::<String>::None);
    let err = RwSignal::new(Option::<String>::None);
    let confirm_delete = RwSignal::new(false);

    let reload = move || {
        leptos::task::spawn_local(async move {
            match api::get::<Value>("/api/v1/users/me").await {
                Ok(v) => {
                    let a = &v["account"];
                    name.set(a["name"].as_str().unwrap_or("").to_string());
                    email.set(a["email"].as_str().unwrap_or("").to_string());
                    tag.set(a["custom_tag"].as_str().unwrap_or("").to_string());
                    show_donations.set(
                        a["show_donation_history_on_profile"]
                            .as_bool()
                            .unwrap_or(true),
                    );
                    account.set(v);
                }
                Err(e) if e.status == 401 => {
                    navigate.get_value()("/login", Default::default());
                }
                Err(_) => {}
            }
            loading.set(false);
        });
    };
    reload();

    let user_id = move || account.get()["account"]["id"].as_i64().unwrap_or(0);
    let has_password = move || {
        account.get()["account"]["has_password"]
            .as_bool()
            .unwrap_or(false)
    };

    let save_profile = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        err.set(None);
        msg.set(None);
        leptos::task::spawn_local(async move {
            let body = json!({
                "name": name.get_untracked(),
                "email": email.get_untracked(),
                "custom_tag": tag.get_untracked(),
            });
            match api::post::<Value, _>(&format!("/api/v1/users/{}", user_id()), &body).await {
                Ok(_) => {
                    msg.set(Some(t("Profile updated").to_string()));
                    auth.user.update(|u| {
                        if let Some(u) = u.as_mut() {
                            u.name = name.get_untracked();
                            u.custom_tag = tag.get_untracked();
                        }
                    });
                }
                Err(e) => err.set(Some(
                    e.first("name").or(e.first("email")).unwrap_or(e.message),
                )),
            }
        });
    };

    let save_password = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        err.set(None);
        msg.set(None);
        leptos::task::spawn_local(async move {
            let body = json!({
                "current_password": if has_password() { Some(cur_pw.get_untracked()) } else { None },
                "password": new_pw.get_untracked(),
                "password_confirmation": new_pw2.get_untracked(),
            });
            match api::put::<Value, _>(&format!("/api/v1/users/{}/password", user_id()), &body)
                .await
            {
                Ok(_) => {
                    cur_pw.set(String::new());
                    new_pw.set(String::new());
                    new_pw2.set(String::new());
                    msg.set(Some(t("Password updated").to_string()));
                }
                Err(e) => err.set(Some(
                    e.first("password")
                        .or(e.first("current_password"))
                        .unwrap_or(e.message),
                )),
            }
        });
    };

    let upload_photo = move |ev: leptos::ev::Event| {
        let Some(input) = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        else {
            return;
        };
        let Some(file) = input.files().and_then(|f| f.get(0)) else {
            return;
        };
        err.set(None);
        leptos::task::spawn_local(async move {
            match upload(user_id(), file).await {
                Ok(_) => {
                    msg.set(Some(t("Photo updated").to_string()));
                    reload();
                }
                Err(e) => err.set(Some(e.message)),
            }
        });
    };

    let remove_photo = move |_| {
        err.set(None);
        leptos::task::spawn_local(async move {
            if api::delete::<Value>(&format!("/api/v1/users/{}/photo", user_id()))
                .await
                .is_ok()
            {
                msg.set(Some(t("Photo removed").to_string()));
                reload();
            }
        });
    };

    let save_donation_pref = move |v: bool| {
        show_donations.set(v);
        leptos::task::spawn_local(async move {
            let _ = api::patch::<Value, _>(
                &format!("/api/v1/users/{}/donation-preferences", user_id()),
                &json!({"show_donation_history_on_profile": v}),
            )
            .await;
        });
    };

    let delete_account = move |_| {
        leptos::task::spawn_local(async move {
            if api::delete::<Value>(&format!("/api/v1/users/{}", user_id()))
                .await
                .is_ok()
            {
                auth.logout();
                navigate.get_value()("/", Default::default());
            } else {
                err.set(Some(t("Could not delete the account").to_string()));
            }
        });
    };

    view! {
        <div class="mx-auto max-w-5xl space-y-6 px-4 py-6 sm:py-8">
            <div class="space-y-1">
                <h1 class="text-sm font-bold uppercase tracking-wider text-white/50">{t("My account")}</h1>
                <p class="text-sm text-white/45">{t("Me page intro")}</p>
            </div>
            <Show
                when=move || !loading.get()
                fallback=move || view! { <div class="text-white/60">{t("Loading")}"…"</div> }
            >
                <section class="overflow-hidden rounded-2xl border border-white/10 bg-gradient-to-br from-brand-deep via-brand-midnight to-neutral-900 shadow-lg">
                    <div class="flex flex-col gap-6 p-6 sm:flex-row sm:items-start sm:p-8">
                        <div class="flex flex-col items-center gap-3">
                            <img src=move || account.get()["account"]["photo"].as_str().unwrap_or("").to_string()
                                class="h-20 w-20 rounded-full border border-white/10" alt=""/>
                            <div class="flex gap-2">
                                <label class="btn-primary !px-3 !py-1.5 cursor-pointer text-xs">
                                    {t("Upload")}
                                    <input type="file" accept="image/*" class="hidden"
                                        on:change=upload_photo/>
                                </label>
                                <Show when=move || account.get()["account"]["has_custom_photo"].as_bool().unwrap_or(false)>
                                    <button type="button"
                                        class="rounded-lg border border-red-500/30 px-3 py-1.5 text-xs text-red-400 hover:bg-red-500/10"
                                        on:click=remove_photo>
                                        {t("Remove")}
                                    </button>
                                </Show>
                            </div>
                            <p class="text-[10px] text-white/40">{t("Max 1 MB : resized to 350×350")}</p>
                        </div>
                        <div class="min-w-0 flex-1 space-y-2 text-center sm:text-left">
                            <h2 class="flex flex-wrap items-center justify-center gap-2 text-2xl font-bold text-white sm:justify-start sm:text-3xl">
                                {move || account.get()["account"]["name"].as_str().unwrap_or("").to_string()}
                                {move || (account.get()["account"]["is_supporter"].as_bool().unwrap_or(false)
                                    && !account.get()["account"]["donor_perks"].as_array().map(|p| p.iter().any(|x| x.as_str() == Some("avatar_crown"))).unwrap_or(false))
                                    .then(|| view! {
                                        <span class="inline-flex items-center gap-1 rounded-full border border-rose-400/40 bg-rose-500/15 px-2 py-0.5 text-[10px] font-semibold text-rose-200" title="Supporter">
                                            <svg xmlns="http://www.w3.org/2000/svg" class="h-3 w-3 shrink-0" viewBox="0 0 20 20" fill="currentColor">
                                                <path fill-rule="evenodd" d="M3.172 5.172a4 4 0 015.656 0L10 6.343l1.172-1.171a4 4 0 115.656 5.656L10 17.657l-6.828-6.829a4 4 0 010-5.656z" clip-rule="evenodd"></path>
                                            </svg>
                                            {t("Supporter")}
                                        </span>
                                    })}
                            </h2>
                            <p class="text-sm text-white/55">{move || account.get()["account"]["email"].as_str().unwrap_or("").to_string()}</p>
                            <p class="text-xs text-white/40">
                                {move || format!("{} {} · #{}",
                                    t("Member for"),
                                    account.get()["account"]["created_at_from_now"].as_str().unwrap_or(""),
                                    account.get()["account"]["id"].as_i64().unwrap_or(0))}
                            </p>
                            <A href=move || format!("/profile/{}", account.get()["account"]["id"].as_i64().unwrap_or(0))
                                attr:class="game-btn-secondary mt-4 inline-flex whitespace-nowrap">
                                {t("View my public profile")}
                            </A>
                        </div>
                    </div>
                </section>

                {move || msg.get().map(|m| view! {
                    <div class="mb-4 rounded-lg border border-teal-500/30 bg-teal-500/10 px-4 py-3 text-sm text-teal-300">{m}</div>
                })}
                {move || err.get().map(|e| view! {
                    <div class="mb-4 rounded-lg border border-red-500/30 bg-red-500/10 px-4 py-3 text-sm text-red-300">{e}</div>
                })}

                <div class="grid gap-6 lg:grid-cols-12">
                <div class="space-y-6 lg:col-span-7">

                <section class="rounded-xl border border-white/10 bg-surface-raised/60 p-6">
                    <h2 class="mb-4 text-lg font-semibold text-white">{t("Profile")}</h2>
                    <form class="space-y-4" on:submit=save_profile>
                        <div>
                            <label class="mb-1 block text-sm text-white/60">{t("Name")}</label>
                            <input type="text" minlength="2" maxlength="25" required
                                class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none focus:border-brand-accent/50"
                                prop:value=name
                                on:input=move |ev| name.set(event_target_value(&ev))/>
                        </div>
                        <div>
                            <label class="mb-1 block text-sm text-white/60">{t("Email")}</label>
                            <input type="email" required
                                class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none focus:border-brand-accent/50"
                                prop:value=email
                                on:input=move |ev| email.set(event_target_value(&ev))/>
                        </div>
                        <div>
                            <label class="mb-1 block text-sm text-white/60">{t("Custom tag")}</label>
                            {move || {
                                let entitled = auth.user.get().map(|u| {
                                    u.is_supporter || u.plan.is_some() || u.is_administrator
                                }).unwrap_or(false);
                                if entitled {
                                    view! {
                                        <input type="text" maxlength="24"
                                            class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none focus:border-brand-accent/50"
                                            prop:value=tag
                                            on:input=move |ev| tag.set(event_target_value(&ev))/>
                                        <p class="mt-1 text-xs text-white/40">{t("Custom tag help")}</p>
                                    }.into_any()
                                } else {
                                    view! {
                                        <div class="w-full rounded-lg border border-amber-500/20 bg-amber-500/5 px-3 py-2 text-sm text-white/50">
                                            {t("Custom tag is a supporter perk — unlock it with a donation or Plus.")}
                                            " "
                                            <A href="/donations" attr:class="text-amber-300 underline underline-offset-2">{t("See options")}</A>
                                        </div>
                                    }.into_any()
                                }
                            }}
                        </div>
                        <button type="submit" class="btn-primary">{t("Save")}</button>
                    </form>
                </section>

                <section class="rounded-xl border border-white/10 bg-surface-raised/60 p-6">
                    <h2 class="mb-4 text-lg font-semibold text-white">{t("My subscription")}</h2>
                    {move || {
                        let plan = auth.user.get().and_then(|u| u.plan.clone());
                        if let Some(plan) = plan {
                            let name = if plan == "pro" { "Blind'All Pro" } else { "Blind'All Plus" };
                            let price = if plan == "pro" { "29,00 € TTC/mois" } else { "4,99 € TTC/mois" };
                            let portal = auth.user.get().map(|u| u.portal_url.clone()).unwrap_or_default();
                            view! {
                                <div class="flex items-center justify-between gap-3">
                                    <div>
                                        <p class="font-semibold text-amber-300">{name}</p>
                                        <p class="text-sm text-white/50">{price}" — "{t("monthly, cancel anytime")}</p>
                                    </div>
                                    {if portal.is_empty() {
                                        view! {
                                            <A href="/donations" attr:class="btn-secondary !px-4 !py-1.5 text-sm">{t("Manage subscription")}</A>
                                        }.into_any()
                                    } else {
                                        view! {
                                            <a href=portal target="_blank" rel="noopener"
                                                class="btn-primary !px-4 !py-1.5 text-sm">{t("Manage subscription")}</a>
                                        }.into_any()
                                    }}
                                </div>
                                <p class="mt-3 text-xs text-white/40">
                                    {t("To cancel: use the link in your Stripe receipt e-mail, or contact us. Access stays active until the end of the paid period.")}
                                </p>
                            }.into_any()
                        } else {
                            view! {
                                <div class="flex items-center justify-between gap-3">
                                    <p class="text-sm text-white/50">{t("No active subscription.")}</p>
                                    <A href="/donations" attr:class="btn-primary !px-4 !py-1.5 text-sm">{t("See options")}</A>
                                </div>
                            }.into_any()
                        }
                    }}
                </section>

                <section class="rounded-xl border border-white/10 bg-surface-raised/60 p-6">
                    <h2 class="mb-4 text-lg font-semibold text-white">{t("Password")}</h2>
                    <form class="space-y-4" on:submit=save_password>
                        <Show when=move || has_password()>
                            <div>
                                <label class="mb-1 block text-sm text-white/60">{t("Current password")}</label>
                                <input type="password" required autocomplete="current-password"
                                    class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none focus:border-brand-accent/50"
                                    prop:value=cur_pw
                                    on:input=move |ev| cur_pw.set(event_target_value(&ev))/>
                            </div>
                        </Show>
                        <div>
                            <label class="mb-1 block text-sm text-white/60">{t("New password")}</label>
                            <input type="password" minlength="8" required autocomplete="new-password"
                                class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none focus:border-brand-accent/50"
                                prop:value=new_pw
                                on:input=move |ev| new_pw.set(event_target_value(&ev))/>
                        </div>
                        <div>
                            <label class="mb-1 block text-sm text-white/60">{t("Confirm new password")}</label>
                            <input type="password" minlength="8" required autocomplete="new-password"
                                class="w-full rounded-lg border border-white/10 bg-surface-base px-3 py-2 text-white outline-none focus:border-brand-accent/50"
                                prop:value=new_pw2
                                on:input=move |ev| new_pw2.set(event_target_value(&ev))/>
                        </div>
                        <button type="submit" class="btn-primary">{t("Update password")}</button>
                    </form>
                </section>

                <section class="rounded-xl border border-red-500/30 bg-red-500/5 p-6">
                    <h2 class="mb-2 text-lg font-semibold text-red-300">{t("Danger zone")}</h2>
                    <p class="mb-4 text-sm text-white/50">
                        {t("Deleting your account is permanent: rooms, playlists and scores will be anonymized.")}
                    </p>
                    <Show
                        when=move || confirm_delete.get()
                        fallback=move || view! {
                            <button type="button"
                                class="rounded-lg border border-red-500/40 px-4 py-2 text-sm text-red-400 hover:bg-red-500/10"
                                on:click=move |_| confirm_delete.set(true)>
                                {t("Delete my account")}
                            </button>
                        }
                    >
                        <div class="flex items-center gap-3">
                            <button type="button"
                                class="rounded-lg bg-red-600 px-4 py-2 text-sm font-bold text-white hover:bg-red-500"
                                on:click=delete_account>
                                {t("Yes, delete permanently")}
                            </button>
                            <button type="button"
                                class="rounded-lg border border-white/10 px-4 py-2 text-sm text-white/70"
                                on:click=move |_| confirm_delete.set(false)>
                                {t("Cancel")}
                            </button>
                        </div>
                    </Show>
                </section>
                </div>
                <aside class="space-y-6 lg:col-span-5">

                {move || {
                    let a = account.get()["account"].clone();
                    let is_supporter = a["is_supporter"].as_bool().unwrap_or(false);
                    let ds = a["donation_summary"].clone();
                    let donation_count = ds["donation_count"].as_i64().unwrap_or(0);
                    (is_supporter || donation_count > 0).then(|| view! {
                        <section class="mb-6 space-y-4 rounded-xl border border-amber-500/20 bg-amber-500/5 p-4">
                            <div>
                                <h2 class="text-sm font-bold uppercase tracking-wider text-amber-200/90">{t("Me supporter perks title")}</h2>
                                <p class="mt-1 text-sm text-white/55">{t("Me supporter perks intro")}</p>
                            </div>
                            <div class="grid grid-cols-2 gap-2 sm:grid-cols-3">
                                <div class="rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-center">
                                    <p class="text-lg font-bold text-white">
                                        {format!("{:.2} €", ds["total_cents"].as_f64().unwrap_or(0.0) / 100.0)}
                                    </p>
                                    <p class="text-[10px] uppercase tracking-wide text-white/50">{t("Total donated")}</p>
                                </div>
                                <div class="rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-center">
                                    <p class="text-lg font-bold text-white">{ds["months_supported"].as_i64().unwrap_or(0)}</p>
                                    <p class="text-[10px] uppercase tracking-wide text-white/50">{t("Months supported")}</p>
                                </div>
                                <div class="col-span-2 rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-center sm:col-span-1">
                                    <p class="text-lg font-bold text-white">{donation_count}</p>
                                    <p class="text-[10px] uppercase tracking-wide text-white/50">{t("Donations")}</p>
                                </div>
                            </div>
                            {is_supporter.then(|| view! {
                                <ul class="space-y-2">
                                    {[
                                        (
                                            "circle-slash",
                                            "M4.9 4.9l14.2 14.2M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20z",
                                            "Donation perk ad free title",
                                            "Donation perk ad free desc",
                                        ),
                                        (
                                            "crown",
                                            "M11.6 3.3a.5.5 0 0 1 .8 0l3.3 5a1 1 0 0 0 1.5.2l4.1-3.4a.5.5 0 0 1 .8.5l-2.6 12.1a1 1 0 0 1-1 .8H5.5a1 1 0 0 1-1-.8L1.9 5.6a.5.5 0 0 1 .8-.5l4.1 3.4a1 1 0 0 0 1.5-.2zM5 21h14",
                                            "Donation perk crown title",
                                            "Donation perk crown desc",
                                        ),
                                        (
                                            "smile-plus",
                                            "M22 11v1a10 10 0 1 1-9-10M8 14s1.5 2 4 2 4-2 4-2M9 9h.01M15 9h.01M19 5v6M16 8h6",
                                            "Donation perk reactions title",
                                            "Donation perk reactions desc",
                                        ),
                                    ].into_iter().map(|(_name, path_d, title, desc)| view! {
                                        <li class="flex gap-3 rounded-lg border border-white/10 bg-white/5 p-3">
                                            <span class="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-amber-400/20 bg-amber-500/10 text-amber-300">
                                                <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="h-4 w-4">
                                                    <path d=path_d></path>
                                                </svg>
                                            </span>
                                            <div class="min-w-0">
                                                <p class="text-sm font-semibold text-white">{t(title)}</p>
                                                <p class="mt-0.5 text-xs leading-relaxed text-white/45">{t(desc)}</p>
                                            </div>
                                        </li>
                                    }).collect::<Vec<_>>()}
                                </ul>
                            })}
                            {(donation_count > 0).then(|| view! {
                                <label class="flex cursor-pointer items-start gap-3 rounded-lg border border-white/10 bg-white/5 p-3">
                                    <input type="checkbox" class="mt-0.5 h-4 w-4 accent-amber-400"
                                        prop:checked=move || show_donations.get()
                                        on:change=move |ev| {
                                            let checked = ev.target()
                                                .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
                                                .map(|i| i.checked())
                                                .unwrap_or(true);
                                            save_donation_pref(checked);
                                        }/>
                                    <span class="min-w-0">
                                        <span class="block text-sm font-semibold text-white">{t("Show donation history on profile")}</span>
                                        <span class="mt-0.5 block text-xs leading-relaxed text-white/45">{t("Show donation history on profile hint")}</span>
                                    </span>
                                </label>
                            })}
                        </section>
                    })
                }}

                <section class="mb-6 space-y-3">
                    <div>
                        <h2 class="text-sm font-bold uppercase tracking-wider text-white/70">{t("Shortcuts")}</h2>
                        <p class="mt-1 text-sm text-white/45">{t("Me shortcuts intro")}</p>
                    </div>
                    <div class="grid gap-3 sm:grid-cols-2">
                        {move || {
                            let a = account.get()["account"].clone();
                            let uid = a["id"].as_i64().unwrap_or(0);
                            let stats = a["stats"].clone();
                            let stat = |k: &str| stats[k].as_i64().unwrap_or(0);
                            [
                                (format!("/profile/{uid}"), "Me link public profile title", "Me link public profile description", None),
                                (format!("/profile/{uid}?tab=scores"), "Scores", "Me link scores description", None),
                                (format!("/profile/{uid}?tab=bookmarks"), "Bookmarks", "Me link bookmarks description", Some(stat("bookmarks"))),
                                (format!("/profile/{uid}?tab=likes"), "Likes", "Me link likes description", None),
                                (format!("/profile/{uid}?tab=minigames"), "Mini-games", "Me link minigames description", None),
                                ("/rooms".into(), "My Rooms", "Me link rooms description", Some(stat("rooms"))),
                                ("/playlists".into(), "My Playlists", "Me link playlists description", Some(stat("playlists"))),
                                ("/rankings".into(), "Rankings", "Me link rankings description", None),
                            ].into_iter().map(|(href, title, desc, st)| view! {
                                <A href=href attr:class="group rounded-xl border border-white/10 bg-brand-deep/50 p-4 transition hover:border-brand-primary/30 hover:bg-brand-deep/80">
                                    <div class="flex items-start justify-between gap-3">
                                        <div class="min-w-0">
                                            <p class="font-semibold text-white group-hover:text-brand-secondary">{t(title)}</p>
                                            <p class="mt-1 text-xs leading-relaxed text-white/45">{t(desc)}</p>
                                        </div>
                                        {st.map(|s| view! {
                                            <span class="shrink-0 rounded-full bg-white/10 px-2 py-0.5 text-xs font-bold tabular-nums text-white">{s}</span>
                                        })}
                                    </div>
                                </A>
                            }).collect::<Vec<_>>()
                        }}
                    </div>
                    {move || (!account.get()["account"]["is_supporter"].as_bool().unwrap_or(false)).then(|| view! {
                        <A href="/docs/support" attr:class="block rounded-xl border border-amber-500/20 bg-amber-500/5 p-4 text-sm text-amber-100/90 transition hover:border-amber-400/35">
                            {t("Me support CTA")}
                        </A>
                    })}
                </section>
                </aside>
                </div>
            </Show>
        </div>
    }
}

async fn upload(user_id: i64, file: web_sys::File) -> Result<(), ApiError> {
    let form = web_sys::FormData::new().map_err(|e| ApiError {
        status: 0,
        message: format!("{e:?}"),
        errors: Default::default(),
        body: serde_json::Value::Null,
    })?;
    form.append_with_blob("photo", &file)
        .map_err(|e| ApiError {
            status: 0,
            message: format!("{e:?}"),
            errors: Default::default(),
            body: serde_json::Value::Null,
        })?;
    let token = api::stored_token();
    let req = gloo_net::http::Request::post(&format!("/api/v1/users/{user_id}/photo"));
    let req = if let Some(t) = token {
        req.header("Authorization", &format!("Bearer {t}"))
    } else {
        req
    };
    let resp = req
        .body(form)
        .map_err(|e| ApiError {
            status: 0,
            message: e.to_string(),
            errors: Default::default(),
            body: serde_json::Value::Null,
        })?
        .send()
        .await;
    api::finish(resp).await?;
    Ok(())
}
