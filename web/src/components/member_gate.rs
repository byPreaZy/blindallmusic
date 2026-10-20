use leptos::prelude::*;
use leptos_router::components::A;

use crate::auth::use_auth;
use crate::i18n::t;

/// Registration gate for member-only features (mini-games, duels, daily
/// challenge). Guests and anonymous visitors previously got bounced away
/// by `require_registered` — the navbar link looked dead. Instead we now
/// render the feature's gate screen explaining why and where to go.
#[component]
pub fn MemberGate(
    /// Feature name shown in the heading ("Mini-jeux", "Duels"…).
    title: &'static str,
    children: ChildrenFn,
) -> impl IntoView {
    let auth = use_auth();
    let allowed = move || {
        auth.ready.get()
            && auth
                .user
                .get()
                .map(|u| !u.is_guest)
                .unwrap_or(false)
    };
    view! {
        <Show when=allowed fallback=move || view! {
            <div class="mx-auto max-w-md px-4 py-16 text-center">
                <div class="rounded-2xl border-2 border-teal-500/30 bg-neutral-800/80 p-8 shadow-2xl">
                    <div class="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full bg-teal-500/15 text-3xl" aria-hidden="true">"♪"</div>
                    <h1 class="text-2xl font-black uppercase tracking-widest text-white">{t(title)}</h1>
                    <p class="mt-3 text-sm text-neutral-300">{t("This feature is for members. Log in or create a free account to play.")}</p>
                    <div class="mt-6 flex flex-col gap-3">
                        <A href="/register" attr:class="btn-primary w-full">{t("Create a free account")}</A>
                        <A href="/login" attr:class="w-full rounded-xl border-2 border-neutral-600 px-4 py-3 font-bold text-neutral-200 transition hover:border-teal-500 hover:text-white">{t("Log in")}</A>
                    </div>
                    <Show when=move || auth.user.get().map(|u| u.is_guest).unwrap_or(false)>
                        <p class="mt-4 text-xs text-neutral-500">{t("Your guest profile will be converted — your stats and XP are kept.")}</p>
                    </Show>
                </div>
            </div>
        }>
            {children()}
        </Show>
    }
}
