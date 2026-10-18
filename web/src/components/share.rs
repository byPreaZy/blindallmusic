use crate::i18n::t;
use leptos::prelude::*;

#[component]
pub fn ShareModal(
    open: RwSignal<bool>,
    title: impl Fn() -> String + Send + Sync + 'static + Copy,
    url: impl Fn() -> String + Send + Sync + 'static + Copy,
) -> impl IntoView {
    let copied = RwSignal::new(false);
    let share_url = move || {
        format!(
            "{}{}",
            web_sys::window()
                .and_then(|w| w.location().origin().ok())
                .unwrap_or_default(),
            url()
        )
    };
    let copy = Callback::new(move |_: web_sys::MouseEvent| {
        if let Some(w) = web_sys::window() {
            let _ = w.navigator().clipboard().write_text(&share_url());
        }
        copied.set(true);
        gloo_timers::callback::Timeout::new(1_500, move || {
            let _ = copied.try_set(false);
        })
        .forget();
    });
    view! {
        <Show when=move || open.get()>
            <div class="fixed inset-0 z-50 flex items-center justify-center px-4">
                <div class="retro-modal-overlay" on:click=move |_| open.set(false)></div>
                <div class="retro-modal-panel relative max-w-md">
                    <div class="flex items-center justify-between border-b border-white/10 px-6 py-4">
                        <h3 class="retro-title retro-title--accent text-lg">{title()}</h3>
                        <button type="button" class="retro-icon-btn !h-8 !w-8" on:click=move |_| open.set(false)>
                            <svg xmlns="http://www.w3.org/2000/svg" fill="none" viewBox="0 0 24 24" stroke-width="1.5" stroke="currentColor" class="h-5 w-5">
                                <path stroke-linecap="round" stroke-linejoin="round" d="M6 18L18 6M6 6l12 12" />
                            </svg>
                        </button>
                    </div>
                    <div class="p-6">
                        <div class="flex gap-2">
                            <input type="text" readonly class="form-input flex-1" prop:value=share_url()/>
                            <button type="button" class="btn-primary btn-sm" on:click=move |e| copy.run(e)>
                                {move || if copied.get() { t("Copied!") } else { t("Copy") }}
                            </button>
                        </div>
                    </div>
                </div>
            </div>
        </Show>
    }
}
