use crate::auth::use_auth;
use leptos::prelude::*;
use leptos_router::hooks::use_location;
use wasm_bindgen::JsCast;

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

pub fn should_serve_ads(path: &str, ads_disabled: bool) -> bool {
    !ads_disabled && !excluded(path)
}

fn ads_state_sig() -> ArcRwSignal<Option<(bool, String, String, String)>> {
    thread_local! {
        static SIG: ArcRwSignal<Option<(bool, String, String, String)>> = ArcRwSignal::new(None);
    }
    SIG.with(|s| s.clone())
}

pub fn euros(cents: i64) -> String {
    if cents % 100 == 0 {
        format!("{}", cents / 100)
    } else {
        format!("{:.2}", cents as f64 / 100.0)
    }
}

// :goal replacement for donation i18n — defaults to the historical 150 until loaded
pub fn goal_or(goal: &str) -> String {
    if goal.is_empty() { "150".into() } else { goal.to_string() }
}

fn ads_state() -> (bool, String, String, String) {
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
                v["goal_cents"].as_i64().map(euros).unwrap_or_default(),
            )));
        });
        return (false, String::new(), String::new(), String::new());
    }
    s.get().unwrap_or_default()
}

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

#[component]
pub fn Adsense(
    #[prop(optional)]
    ad_slot: Option<&'static str>,
    #[prop(optional, default = false)]
    compact: bool,
    #[prop(optional, default = "")]
    wrapper_class: &'static str,
    #[prop(optional, default = false)]
    force_serve: bool,
) -> impl IntoView {
    let auth = use_auth();
    let location = use_location();
    let pushed = RwSignal::new(false);

    let visible = move || {
        let (disabled, client, _, _) = ads_state();
        if client.is_empty() {
            return false;
        }
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
            let (_, client, _, _) = ads_state();
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
