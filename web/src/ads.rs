//! Google AdSense port — `resources/js/ads/index.js` + `Components/Adsense.vue`.
//!
//! Slot visibility mirrors `shouldServeAds(path)` + `useAdsDisabled()`:
//! excluded paths, donation-goal reached (`ads_disabled` — ads off for the
//! whole community), or the `ad_free` donor perk.

use crate::auth::use_auth;
use leptos::prelude::*;
use leptos_router::hooks::use_location;
use wasm_bindgen::JsCast;

/// `ads/index.js` — pages where no slot is ever served.
fn excluded(path: &str) -> bool {
    const PREFIXES: &[&str] = &["/auth/", "/callback/", "/guest/", "/admin", "/moderation"];
    const EXACT: &[&str] = &[
        "/login",
        "/register",
        "/user/banned",
        "/create/room",
        "/minigames/quiz",
        "/minigames/who-sang",
        "/minigames/anagram",
        "/minigames/first-letter",
        "/minigames/album-cover",
    ];
    if EXACT.contains(&path) || PREFIXES.iter().any(|p| path.starts_with(p)) {
        return true;
    }
    // `/rooms/{slug}` (+ /edit) and `/playlists/{id}/edit`.
    if let Some(rest) = path.strip_prefix("/rooms/") {
        let _ = rest;
        return true;
    }
    if let Some(rest) = path.strip_prefix("/playlists/") {
        if rest.ends_with("/edit") {
            return true;
        }
    }
    false
}

/// `shouldServeAds(path, { adsDisabled })`.
pub fn should_serve_ads(path: &str, ads_disabled: bool) -> bool {
    !ads_disabled && !excluded(path)
}

// Ads config — `(ads_disabled, adsense_client, adsense_slot)` fetched once
// per session from the public donations progress payload. An empty client
// means no AdSense account is configured ⇒ every slot stays hidden.
// `ArcRwSignal` — no owner, so it can never be disposed (unlike `RwSignal`,
// which dies with its component owner; even `Owner::new()` registers under
// the current owner, so it was still torn down on navigation).
fn ads_state_sig() -> ArcRwSignal<Option<(bool, String, String)>> {
    thread_local! {
        static SIG: ArcRwSignal<Option<(bool, String, String)>> = ArcRwSignal::new(None);
    }
    SIG.with(|s| s.clone())
}

/// `(ads_disabled, client, slot)` — `(false, "", "")` until the fetch lands.
fn ads_state() -> (bool, String, String) {
    let s = ads_state_sig();
    if s.get_untracked().is_none() {
        leptos::task::spawn_local(async move {
            let v = crate::api::get::<serde_json::Value>("/api/v1/donations/progress")
                .await
                .unwrap_or_default();
            s.set(Some((
                v["ads_disabled"].as_bool().unwrap_or(false),
                v["adsense_client"].as_str().unwrap_or("").to_string(),
                v["adsense_slot"].as_str().unwrap_or("").to_string(),
            )));
        });
        return (false, String::new(), String::new());
    }
    s.get().unwrap_or_default()
}

/// Injects the AdSense loader once — `index.html` no longer ships a
/// hardcoded `ca-pub`, the client id comes from server config.
fn ensure_adsense_script(client: &str) {
    let Some(w) = web_sys::window() else { return };
    if js_sys::Reflect::get(&w, &wasm_bindgen::JsValue::from_str("__adsense_loaded"))
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return;
    }
    let Some(doc) = w.document() else { return };
    let Ok(el) = doc.create_element("script") else { return };
    el.set_attribute("async", "").ok();
    el.set_attribute(
        "src",
        &format!("https://pagead2.googlesyndication.com/pagead/js/adsbygoogle.js?client={client}"),
    )
    .ok();
    el.set_attribute("crossorigin", "anonymous").ok();
    if let Some(head) = doc.head() {
        head.append_child(&el).ok();
        let _ = js_sys::Reflect::set(
            &w,
            &wasm_bindgen::JsValue::from_str("__adsense_loaded"),
            &wasm_bindgen::JsValue::TRUE,
        );
    }
}

/// `pushAdsenseSlot()` — `(window.adsbygoogle ||= []).push({})`, swallowing
/// errors exactly like the JS (blockers / script absent).
fn push_adsense_slot() {
    if let Some(w) = web_sys::window() {
        let arr = js_sys::Reflect::get(&w, &wasm_bindgen::JsValue::from_str("adsbygoogle"))
            .unwrap_or_else(|_| js_sys::Array::new().into());
        let arr = if arr.is_array() {
            arr
        } else {
            let a = js_sys::Array::new();
            let _ = js_sys::Reflect::set(
                &w,
                &wasm_bindgen::JsValue::from_str("adsbygoogle"),
                a.as_ref(),
            );
            a.into()
        };
        if let Ok(arr) = arr.dyn_into::<js_sys::Array>() {
            arr.push(&js_sys::Object::new());
        }
    }
}

/// `Adsense.vue` — `<ins class="adsbygoogle">` slot in the styled wrapper;
/// pushes one slot on mount when visible. Renders nothing unless the
/// server reports a configured `adsense_client`.
#[component]
pub fn Adsense(
    /// `data-ad-slot` override — defaults to the server-configured
    /// `ADSENSE_SLOT`.
    #[prop(optional)]
    ad_slot: Option<&'static str>,
    #[prop(optional, default = false)]
    compact: bool,
    #[prop(optional, default = "")]
    wrapper_class: &'static str,
    /// Room lobby: `force-serve` allows the slot on `/rooms/{slug}`.
    #[prop(optional, default = false)]
    force_serve: bool,
) -> impl IntoView {
    let auth = use_auth();
    let location = use_location();
    let pushed = RwSignal::new(false);

    let visible = move || {
        let (disabled, client, _) = ads_state();
        if client.is_empty() {
            return false;
        }
        // `userHasDonorPerk(user, 'ad_free')` — `donor_perks.includes`.
        let donor_ad_free = auth
            .user
            .get()
            .map(|u| u.donor_perks.iter().any(|p| p == "ad_free"))
            .unwrap_or(false);
        if disabled || donor_ad_free {
            return false;
        }
        if force_serve {
            return true;
        }
        should_serve_ads(&location.pathname.get(), false)
    };

    Effect::new(move |_| {
        let v = visible();
        if !v {
            pushed.set(false);
            return;
        }
        if !pushed.get_untracked() {
            pushed.set(true);
            let (_, client, _) = ads_state();
            ensure_adsense_script(&client);
            push_adsense_slot();
        }
    });

    view! {
        <Show when=visible>
            <div class=format!(
                "adsense-slot overflow-hidden rounded-xl border border-slate-700/40 bg-slate-800/20 {} {}",
                if compact { "min-h-[60px]" } else { "min-h-[90px]" },
                wrapper_class,
            )>
                <ins class="adsbygoogle" style="display: block"
                    attr:data-ad-client=ads_state().1
                    attr:data-ad-slot=ad_slot.map(str::to_string).or_else(|| {
                        let slot = ads_state().2;
                        (!slot.is_empty()).then_some(slot)
                    })
                    attr:data-ad-format="auto"
                    attr:data-full-width-responsive="true"></ins>
            </div>
        </Show>
    }
}
