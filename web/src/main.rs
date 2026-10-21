mod ads;
mod api;
mod app;
mod auth;
mod components;
mod i18n;
mod pages;
mod seo;
mod types;
mod ws;

fn main() {
    console_error_panic_hook::set_once();
    i18n::init_locale();
    leptos::mount::mount_to_body(app::App);
    // Server-rendered SEO landing block: needed by crawlers and no-JS agents,
    // removed once the client app takes over.
    if let Some(el) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.get_element_by_id("seo-landing"))
    {
        el.remove();
    }
}
