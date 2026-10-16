use crate::i18n::t;
use leptos::prelude::*;
use send_wrapper::SendWrapper;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

/// Floating "Install app" chip. Browsers only allow an install prompt after a
/// `beforeinstallprompt` event; iOS Safari doesn't fire it at all, so on iOS we
/// show a hint to use "Add to Home Screen" instead.
#[component]
pub fn InstallPrompt() -> impl IntoView {
    let deferred = RwSignal::new_local(Option::<web_sys::Event>::None);
    let dismissed = RwSignal::new(false);
    let ios_hint = RwSignal::new(false);
    let installed = RwSignal::new(false);

    {
        let deferred = deferred.clone();
        let installed = installed.clone();
        let ios_hint = ios_hint.clone();
        on_cleanup({
            let deferred = deferred.clone();
            move || deferred.set(None)
        });
        let cb = Closure::wrap(Box::new(move |ev: web_sys::Event| {
            ev.prevent_default();
            deferred.set(Some(ev));
        }) as Box<dyn FnMut(_)>);
        if let Some(w) = web_sys::window() {
            let _ = w.add_event_listener_with_callback(
                "beforeinstallprompt",
                cb.as_ref().unchecked_ref(),
            );
            let ok_cb = Closure::wrap(Box::new(move |_: web_sys::Event| {
                installed.set(true);
            }) as Box<dyn FnMut(_)>);
            let _ = w.add_event_listener_with_callback(
                "appinstalled",
                ok_cb.as_ref().unchecked_ref(),
            );
            // iOS Safari: no beforeinstallprompt — show manual instructions
            // once, but only when not already running standalone.
            let ua = w.navigator().user_agent().unwrap_or_default();
            let is_ios = ua.contains("iPhone") || ua.contains("iPad") || ua.contains("iPod");
            let standalone = w
                .match_media("(display-mode: standalone)")
                .ok()
                .flatten()
                .map(|m| m.matches())
                .unwrap_or(false)
                || js_sys::Reflect::get(&w.navigator(), &"standalone".into())
                    .ok()
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
            if is_ios && !standalone {
                ios_hint.set(true);
            }
            let _cb = SendWrapper::new(cb);
            let _ok = SendWrapper::new(ok_cb);
            on_cleanup(move || {
                let _ = w.remove_event_listener_with_callback(
                    "beforeinstallprompt",
                    _cb.as_ref().unchecked_ref(),
                );
                let _ = w.remove_event_listener_with_callback(
                    "appinstalled",
                    _ok.as_ref().unchecked_ref(),
                );
            });
        }
    }

    let visible = move || {
        !dismissed.get() && !installed.get() && (deferred.get().is_some() || ios_hint.get())
    };

    let install = move |_| {
        if let Some(ev) = deferred.get_untracked() {
            let _ = js_sys::Reflect::get(&ev, &"prompt".into())
                .ok()
                .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
                .map(|f| f.call0(&ev));
            deferred.set(None);
        }
    };

    view! {
        <Show when=visible>
            <div class="fixed bottom-4 left-1/2 z-50 w-[92vw] max-w-sm -translate-x-1/2 rounded-xl border border-brand-accent/40 bg-brand-midnight/95 p-3 shadow-2xl backdrop-blur sm:left-auto sm:right-4 sm:translate-x-0">
                <div class="flex items-start gap-3">
                    <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="mt-0.5 h-6 w-6 shrink-0 text-brand-accent">
                        <path stroke-linecap="round" stroke-linejoin="round" d="M3 16.5v2.25A2.25 2.25 0 0 0 5.25 21h13.5A2.25 2.25 0 0 0 21 18.75V16.5M16.5 12 12 16.5m0 0L7.5 12m4.5 4.5V3" />
                    </svg>
                    <div class="min-w-0 flex-1">
                        <p class="text-sm font-semibold text-white">{t("Install the app")}</p>
                        <p class="mt-0.5 text-xs leading-snug text-white/60">
                            {move || if ios_hint.get() && deferred.get().is_none() {
                                t("Tap Share then \"Add to Home Screen\"")
                            } else {
                                t("Play in one tap, even in the subway")
                            }}
                        </p>
                    </div>
                    {move || deferred.get().map(|_| view! {
                        <button type="button" class="btn-primary btn-sm shrink-0" on:click=install>
                            {t("Install")}
                        </button>
                    })}
                    <button type="button" aria-label=t("Dismiss") class="shrink-0 text-white/40 hover:text-white/80"
                        on:click=move |_| dismissed.set(true)>
                        <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20" fill="currentColor" class="h-5 w-5">
                            <path d="M6.28 5.22a.75.75 0 0 0-1.06 1.06L8.94 10l-3.72 3.72a.75.75 0 1 0 1.06 1.06L10 11.06l3.72 3.72a.75.75 0 1 0 1.06-1.06L11.06 10l3.72-3.72a.75.75 0 0 0-1.06-1.06L10 8.94 6.28 5.22Z" />
                        </svg>
                    </button>
                </div>
            </div>
        </Show>
    }
}
