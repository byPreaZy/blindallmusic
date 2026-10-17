use crate::i18n::t;
use leptos::prelude::*;

fn chip(label: String, classes: &'static str, title: &'static str) -> impl IntoView {
    view! {
        <span class=format!(
            "inline-flex shrink-0 items-center rounded border px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wide sm:text-[10px] {classes}"
        ) title=title>
            {label}
        </span>
    }
}

/// Role badge (admin / moderator) + user-defined tag, shown next to names.
#[component]
pub fn UserTags(
    #[prop(default = String::new())] role: String,
    #[prop(default = String::new())] custom_tag: String,
) -> impl IntoView {
    view! {
        {match role.as_str() {
            "admin" => Some(chip(
                t("Admin").to_string(),
                "border-red-400/50 bg-red-500/15 text-red-300",
                t("Administrator"),
            )),
            "moderator" => Some(chip(
                t("Modo").to_string(),
                "border-sky-400/50 bg-sky-500/15 text-sky-300",
                t("Moderator"),
            )),
            _ => None,
        }}
        {(!custom_tag.is_empty()).then(|| chip(
            custom_tag.clone(),
            "border-purple-400/50 bg-purple-500/15 text-purple-300",
            t("Custom tag"),
        ))}
    }
}
