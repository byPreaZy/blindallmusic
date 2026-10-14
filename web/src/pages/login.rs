//! Login / register — forms → Sanctum token + redirect. Also forgot/reset
//! password, email verification, and OAuth (Socialite) flows.

use crate::api;
use crate::auth::use_auth;
use crate::i18n::t;
use leptos::ev;
use leptos::prelude::*;
use leptos_router::components::A;
use leptos_router::hooks::{use_navigate, use_params_map, use_query_map};
use serde_json::{json, Value};

/// `Head::…->robots('noindex, nofollow')` on every auth page.
fn auth_noindex() {
    crate::seo::set_meta("name", "robots", "noindex, nofollow");
}

#[component]
pub fn LoginPage() -> impl IntoView {
    crate::i18n::set_title_key("Login");
    auth_noindex();
    let auth = use_auth();
    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let error = RwSignal::new(Option::<String>::None);
    let busy = RwSignal::new(false);
    // PHP `LoginForm` — `remember: true` by default.
    let remember = RwSignal::new(true);
    let navigate = use_navigate();

    let submit = move |ev: ev::SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        error.set(None);
        let navigate = navigate.clone();
        leptos::task::spawn_local(async move {
            match auth.login(&email.get(), &password.get(), remember.get()).await {
                Ok(()) => navigate("/", Default::default()),
                Err(e) => error.set(Some(
                    e.first("email").or(e.first("password")).unwrap_or(e.message),
                )),
            }
            busy.set(false);
        });
    };

    view! {
        <div class="mx-auto flex max-w-md flex-col px-4 py-16">
            <h1 class="retro-title retro-title--white mb-8 text-2xl">{t("Login")}</h1>
            <form class="space-y-4 rounded-xl border border-white/10 bg-surface-raised/60 p-6" on:submit=submit>
                {move || error.get().map(|e| view! {
                    <p class="form-error">{e}</p>
                })}
                <div>
                    <label class="form-label">{t("Email")}</label>
                    <input bind:value=email type="email" required class="form-input" autocomplete="email"/>
                </div>
                <div>
                    <label class="form-label">{t("Password")}</label>
                    <input bind:value=password type="password" required class="form-input" autocomplete="current-password"/>
                </div>
                <label class="flex select-none items-center gap-1.5" for="remember">
                    <input id="remember" type="checkbox" prop:checked=remember
                        on:change=move |ev| remember.set(event_target_checked(&ev))
                        class="accent-brand-accent"/>
                    <span class="text-sm">{t("Remember Me")}</span>
                </label>
                <button type="submit" class="btn-primary w-full" disabled=move || busy.get()>
                    {t("Login")}
                </button>
                <div class="flex items-center justify-between text-sm">
                    <A href="/forgot-password" attr:class="text-white/50 hover:text-white hover:underline">
                        {t("Forgot your password?")}
                    </A>
                    <A href="/register" attr:class="text-brand-accent hover:underline">{t("Register")}</A>
                </div>
                <div class="text-center">
                    <A href="/" attr:class="text-sm text-white/50 underline hover:text-white">
                        {t("Return to Homepage")}
                    </A>
                </div>
            </form>
            <SocialButtons/>
        </div>
    }
}

/// Socialite provider buttons — `GET /auth/oauth/{provider}/redirect`
/// returns the authorization URL; unconfigured providers surface a clean error.
/// Blind'All Music keeps Google + Discord only.
#[component]
pub fn SocialButtons() -> impl IntoView {
    let error = RwSignal::new(Option::<String>::None);
    const PROVIDERS: &[(&str, &str)] = &[
        ("google", "Google"),
        ("discord", "Discord"),
    ];
    view! {
        <div class="mt-4 rounded-xl border border-white/10 bg-surface-raised/60 p-6">
            <div class="mb-3 text-center text-xs uppercase tracking-wider text-white/40">
                {t("Or continue with")}
            </div>
            {move || error.get().map(|e| view! {
                <p class="form-error mb-3 text-center">{e}</p>
            })}
            <div class="grid grid-cols-2 gap-2">
                {PROVIDERS.iter().map(|(id, label)| {
                    let id = *id;
                    view! {
                        <button type="button"
                            class="rounded-lg border border-white/10 px-3 py-2 text-sm text-white/80 transition hover:bg-white/10"
                            on:click=move |_| {
                                error.set(None);
                                leptos::task::spawn_local(async move {
                                    match api::get::<Value>(&format!("/api/v1/auth/oauth/{id}/redirect")).await {
                                        Ok(v) => {
                                            if let Some(url) = v["url"].as_str() {
                                                let _ = web_sys::window().map(|w| w.location().set_href(url));
                                            }
                                        }
                                        Err(e) => {
                                            if e.status == 503 {
                                                error.set(Some(t("This provider is not configured.").to_string()));
                                            } else {
                                                error.set(Some(e.message));
                                            }
                                        }
                                    }
                                });
                            }>
                            {*label}
                        </button>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}

#[component]
pub fn RegisterPage() -> impl IntoView {
    crate::i18n::set_title_key("Register");
    auth_noindex();
    let auth = use_auth();
    let name = RwSignal::new(String::new());
    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let password_confirmation = RwSignal::new(String::new());
    let errors = RwSignal::new(Vec::<String>::new());
    let busy = RwSignal::new(false);
    let navigate = use_navigate();

    let submit = move |ev: ev::SubmitEvent| {
        ev.prevent_default();
        busy.set(true);
        errors.set(vec![]);
        let navigate = navigate.clone();
        leptos::task::spawn_local(async move {
            match auth
                .register(
                    &name.get(),
                    &email.get(),
                    &password.get(),
                    &password_confirmation.get(),
                )
                .await
            {
                Ok(()) => navigate("/", Default::default()),
                Err(e) => {
                    let mut errs: Vec<String> =
                        e.errors.values().flatten().cloned().collect();
                    if errs.is_empty() {
                        errs.push(e.message);
                    }
                    errors.set(errs);
                }
            }
            busy.set(false);
        });
    };

    view! {
        <div class="mx-auto flex max-w-md flex-col px-4 py-16">
            <h1 class="retro-title retro-title--white mb-8 text-2xl">{t("Register")}</h1>
            <form class="space-y-4 rounded-xl border border-white/10 bg-surface-raised/60 p-6" on:submit=submit>
                {move || {
                    let errs = errors.get();
                    (!errs.is_empty()).then(|| view! {
                        <ul class="form-error space-y-1">
                            {errs.into_iter().map(|e| view! { <li>{e}</li> }).collect_view()}
                        </ul>
                    })
                }}
                <div>
                    <label class="form-label">{t("Name")} " / " {t("Nickname")}</label>
                    <input bind:value=name type="text" required maxlength="25" class="form-input"/>
                </div>
                <div>
                    <label class="form-label">{t("Email")}</label>
                    <input bind:value=email type="email" required class="form-input" autocomplete="email"/>
                </div>
                <div>
                    <label class="form-label">{t("Password")}</label>
                    <input bind:value=password type="password" required minlength="8" class="form-input" autocomplete="new-password"/>
                </div>
                <div>
                    <label class="form-label">{t("Confirm password")}</label>
                    <input bind:value=password_confirmation type="password" required minlength="8" class="form-input" autocomplete="new-password"/>
                </div>
                <button type="submit" class="btn-primary w-full" disabled=move || busy.get()>
                    {t("Register")}
                </button>
            </form>
            <SocialButtons/>
        </div>
    }
}

// ---- forgot / reset password -------------------------------------------------

/// `Auth/ForgotPassword` — email → `POST /auth/forgot-password`.
#[component]
pub fn ForgotPasswordPage() -> impl IntoView {
    crate::i18n::set_title_key("Forgot your password?");
    auth_noindex();
    let email = RwSignal::new(String::new());
    let error = RwSignal::new(Option::<String>::None);
    let sent = RwSignal::new(false);
    let busy = RwSignal::new(false);

    view! {
        <div class="mx-auto flex max-w-md flex-col px-4 py-16">
            <h1 class="retro-title retro-title--white mb-4 text-2xl">{t("Forgot your password?")}</h1>
            <p class="mb-8 text-sm text-white/60">
                {t("Forgot your password? No problem. Just let us know your email address and we will email you a password reset link that will allow you to choose a new one")}
            </p>
            <form class="space-y-4 rounded-xl border border-white/10 bg-surface-raised/60 p-6" on:submit=move |ev| {
                ev.prevent_default();
                busy.set(true); error.set(None);
                leptos::task::spawn_local(async move {
                    match api::post::<Value,_>("/api/v1/auth/forgot-password", &json!({"email": email.get_untracked()})).await {
                        Ok(_) => sent.set(true),
                        Err(e) => error.set(Some(e.first("email").unwrap_or(e.message))),
                    }
                    busy.set(false);
                });
            }>
                {move || error.get().map(|e| view! { <p class="form-error">{e}</p> })}
                {move || sent.get().then(|| view! {
                    <p class="rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-300">
                        {t("We have emailed your password reset link.")}
                    </p>
                })}
                <div>
                    <label class="form-label">{t("Email")}</label>
                    <input bind:value=email type="email" required class="form-input" autocomplete="email"/>
                </div>
                <button type="submit" class="btn-primary w-full" disabled=move || busy.get()>
                    {t("Email Password Reset Link")}
                </button>
            </form>
        </div>
    }
}

/// `Auth/ResetPassword` — `/reset-password/:token?email=` (link from the mail).
#[component]
pub fn ResetPasswordPage() -> impl IntoView {
    crate::i18n::set_title_key("Reset Password");
    auth_noindex();
    let params = use_params_map();
    let query = use_query_map();
    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let confirm = RwSignal::new(String::new());
    let error = RwSignal::new(Option::<String>::None);
    let done = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let navigate = StoredValue::new(use_navigate());

    Effect::new(move |_| {
        if let Some(e) = query.get().get("email") {
            email.set(e);
        }
    });

    view! {
        <div class="mx-auto flex max-w-md flex-col px-4 py-16">
            <h1 class="retro-title retro-title--white mb-8 text-2xl">{t("Reset password")}</h1>
            <form class="space-y-4 rounded-xl border border-white/10 bg-surface-raised/60 p-6" on:submit=move |ev| {
                ev.prevent_default();
                busy.set(true); error.set(None);
                let token = params.get().get("token").unwrap_or_default();
                leptos::task::spawn_local(async move {
                    let body = json!({
                        "token": token,
                        "email": email.get_untracked(),
                        "password": password.get_untracked(),
                        "password_confirmation": confirm.get_untracked(),
                    });
                    match api::post::<Value,_>("/api/v1/auth/reset-password", &body).await {
                        Ok(_) => {
                            done.set(true);
                            let nav = navigate.get_value();
                            nav("/login", Default::default());
                        }
                        Err(e) => error.set(Some(
                            e.first("email").or(e.first("password")).unwrap_or(e.message))),
                    }
                    busy.set(false);
                });
            }>
                {move || error.get().map(|e| view! { <p class="form-error">{e}</p> })}
                <div>
                    <label class="form-label">{t("Email")}</label>
                    <input bind:value=email type="email" required class="form-input" autocomplete="email"/>
                </div>
                <div>
                    <label class="form-label">{t("Password")}</label>
                    <input bind:value=password type="password" required minlength="8" class="form-input" autocomplete="new-password"/>
                </div>
                <div>
                    <label class="form-label">{t("Confirm password")}</label>
                    <input bind:value=confirm type="password" required minlength="8" class="form-input" autocomplete="new-password"/>
                </div>
                <button type="submit" class="btn-primary w-full" disabled=move || busy.get() || done.get()>
                    {t("Reset password")}
                </button>
            </form>
        </div>
    }
}

// ---- email verification ---------------------------------------------------------

/// `Auth/VerifyEmail` — prompt + resend button (requires auth).
#[component]
pub fn VerifyEmailPage() -> impl IntoView {
    crate::i18n::set_title_key("Verify Email");
    auth_noindex();
    let auth = use_auth();
    let sent = RwSignal::new(false);
    let error = RwSignal::new(Option::<String>::None);
    view! {
        <div class="mx-auto flex max-w-md flex-col px-4 py-16">
            <h1 class="retro-title retro-title--white mb-8 text-2xl">{t("Verify your email")}</h1>
            <div class="space-y-4 rounded-xl border border-white/10 bg-surface-raised/60 p-6">
                <Show when=move || auth.user.get().is_some()
                    fallback=move || view! {
                        <p class="text-sm text-white/60">
                            <A href="/login" attr:class="text-brand-accent hover:underline">{t("Login")}</A>
                            " " {t("to verify your email address.")}
                        </p>
                    }>
                    <p class="text-sm text-white/70">
                        {t("Thanks for signing up! Before getting started, could you verify your email address by clicking on the link we just emailed to you?")}
                    </p>
                    {move || sent.get().then(|| view! {
                        <p class="rounded-lg border border-emerald-500/30 bg-emerald-500/10 px-4 py-3 text-sm text-emerald-300">
                            {t("A new verification link has been sent to your email address.")}
                        </p>
                    })}
                    {move || error.get().map(|e| view! { <p class="form-error">{e}</p> })}
                    <button type="button" class="btn-primary w-full"
                        on:click=move |_| {
                            error.set(None);
                            leptos::task::spawn_local(async move {
                                match api::post_empty::<Value>("/api/v1/auth/email/verification-notification").await {
                                    Ok(_) => sent.set(true),
                                    Err(e) => error.set(Some(e.message)),
                                }
                            });
                        }>
                        {t("Resend verification email")}
                    </button>
                </Show>
            </div>
        </div>
    }
}

/// `/verify-email/:id/:hash?expires&signature` — the mailed link hits the API
/// directly for `Accept: json`, but for an HTML accept the API 302s here;
/// this page also handles being opened with the signed query itself.
#[component]
pub fn VerifyEmailConfirmPage() -> impl IntoView {
    let params = use_params_map();
    let query = use_query_map();
    let status = RwSignal::new("loading".to_string());

    Effect::new(move |_| {
        let p = params.get();
        let q = query.get();
        let (Some(id), Some(hash), Some(expires), Some(signature)) = (
            p.get("id").map(|x| x.to_string()),
            p.get("hash").map(|x| x.to_string()),
            q.get("expires").map(|x| x.to_string()),
            q.get("signature").map(|x| x.to_string()),
        ) else {
            status.set("invalid".to_string());
            return;
        };
        leptos::task::spawn_local(async move {
            let url = format!(
                "/api/v1/auth/email/verify/{id}/{hash}?expires={expires}&signature={signature}"
            );
            match api::get::<Value>(&url).await {
                Ok(_) => status.set("verified".to_string()),
                Err(e) if e.status == 403 => status.set("invalid".to_string()),
                Err(e) => status.set(format!("error:{}", e.message)),
            }
        });
    });

    view! {
        <div class="mx-auto flex max-w-md flex-col px-4 py-16">
            <div class="rounded-xl border border-white/10 bg-surface-raised/60 p-8 text-center">
                {move || match status.get().as_str() {
                    "verified" => view! {
                        <h1 class="retro-title retro-title--white mb-4 text-2xl">{t("Email verified!")}</h1>
                        <p class="mb-6 text-sm text-white/60">{t("Your email address has been verified.")}</p>
                        <A href="/" attr:class="btn-primary">{t("Continue")}</A>
                    }.into_any(),
                    "invalid" => view! {
                        <h1 class="retro-title retro-title--white mb-4 text-2xl">"403"</h1>
                        <p class="mb-6 text-sm text-white/60">{t("This verification link is invalid or has expired.")}</p>
                        <A href="/verify-email" attr:class="text-brand-accent hover:underline">{t("Resend verification email")}</A>
                    }.into_any(),
                    s if s.starts_with("error:") => view! {
                        <p class="form-error">{s.trim_start_matches("error:").to_string()}</p>
                    }.into_any(),
                    _ => view! {
                        <p class="text-white/60">{t("Verifying")}"…"</p>
                    }.into_any(),
                }}
            </div>
        </div>
    }
}

// ---- oauth callback ---------------------------------------------------------

/// `/oauth/:provider/callback?code&state` — forwards the provider response
/// to `GET /auth/oauth/{provider}/callback`, stores the returned token.
#[component]
pub fn OAuthCallbackPage() -> impl IntoView {
    let params = use_params_map();
    let query = use_query_map();
    let auth = use_auth();
    let status = RwSignal::new("loading".to_string());
    let navigate = StoredValue::new(use_navigate());

    Effect::new(move |_| {
        let provider = params.get().get("provider").unwrap_or_default();
        let q = query.get();
        if q.get("error").is_some() {
            status.set("denied".to_string());
            return;
        }
        let (Some(code), Some(oauth_state)) = (
            q.get("code").map(|x| x.to_string()),
            q.get("state").map(|x| x.to_string()),
        ) else {
            status.set("invalid".to_string());
            return;
        };
        leptos::task::spawn_local(async move {
            let url = format!(
                "/api/v1/auth/oauth/{provider}/callback?code={}&state={}",
                code, oauth_state
            );
            match api::get::<crate::types::AuthResponse>(&url).await {
                Ok(resp) => {
                    auth.complete_oauth(resp);
                    let nav = navigate.get_value();
                    nav("/", Default::default());
                }
                Err(e) => status.set(format!("error:{}", e.message)),
            }
        });
    });

    view! {
        <div class="mx-auto flex max-w-md flex-col px-4 py-16">
            <div class="rounded-xl border border-white/10 bg-surface-raised/60 p-8 text-center">
                {move || match status.get().as_str() {
                    "denied" => view! {
                        <p class="mb-6 text-sm text-white/60">{t("Sign-in was cancelled.")}</p>
                        <A href="/login" attr:class="text-brand-accent hover:underline">{t("Back to login")}</A>
                    }.into_any(),
                    "invalid" => view! {
                        <p class="mb-6 text-sm text-white/60">{t("Invalid OAuth response.")}</p>
                        <A href="/login" attr:class="text-brand-accent hover:underline">{t("Back to login")}</A>
                    }.into_any(),
                    s if s.starts_with("error:") => view! {
                        <div>
                            <p class="form-error mb-4">{s.trim_start_matches("error:").to_string()}</p>
                            <A href="/login" attr:class="text-brand-accent hover:underline">{t("Back to login")}</A>
                        </div>
                    }.into_any(),
                    _ => view! { <p class="text-white/60">{t("Signing in")}"…"</p> }.into_any(),
                }}
            </div>
        </div>
    }
}
