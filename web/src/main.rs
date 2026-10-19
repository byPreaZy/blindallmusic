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
}
