use crate::auth::Auth;
use crate::components::footer::Footer;
use crate::components::navbar::Navbar;
use crate::pages::admin::{
    AdminAnswerTypes, AdminCategories, AdminDashboard, AdminDataQuality, AdminFaqs, AdminPages,
    AdminPlaylists, AdminRooms, AdminTeams, AdminUsers,
};
use crate::pages::docs::{
    BannedPage, CategoryLandingPage, ContactPage, DocsCreateContentPage, DocsEloPage,
    DocsGlossaryPage, DocsHowtoPage, DocsIndexPage, DocsLevelPage, DocsSupportPage,
    FaqRedirect, LanguageSwitch, LocaleRedirect, RankingSortRedirect,
};
use crate::pages::donations::DonationsPage;
use crate::pages::faq::FaqPage;
use crate::pages::games::MinigamesDisabled;
use crate::pages::home::HomePage;
use crate::pages::login::{
    ForgotPasswordPage, LoginPage, OAuthCallbackPage, RegisterPage, ResetPasswordPage,
    VerifyEmailConfirmPage, VerifyEmailPage,
};
use crate::pages::moderation::{
    ModBannedUsers, ModDashboard, ModModerators, ModSuggestions, ModTracks, ModTrashedMessages,
    ModUserShow, ModUsers,
};
use crate::pages::pages::StaticPage;
use crate::pages::playlist_edit::PlaylistEditPage;
use crate::pages::playlists::{PlaylistCreatePage, PlaylistsPage};
use crate::pages::profile::ProfilePage;
use crate::pages::rankings::RankingsPage;
use crate::pages::room_forms::{RoomCreatePage, RoomEditPage};
use crate::pages::room_show::RoomPage;
use crate::pages::rooms::RoomsIndexPage;
use crate::pages::settings::SettingsPage;
use crate::pages::teams::{TeamShowPage, TeamsPage};
use leptos::prelude::*;
use leptos_router::components::{Route, Router, Routes};
use leptos_router::hooks::{use_location, use_params_map};
use leptos_router::path;

/// Remounts its children whenever the route params change. Navigating between
/// two param'd URLs of the same route (e.g. `/rooms/a` -> `/rooms/b`) does not
/// recreate the view otherwise, so param-driven pages would stay stale.
#[component]
fn ParamRemount(children: ChildrenFn) -> impl IntoView {
    let params = use_params_map();
    move || {
        params.get();
        children()
    }
}

#[component]
fn AppFooter() -> impl IntoView {
    let location = use_location();
    Effect::new(move |_| {
        let _ = location.pathname.get();
        crate::seo::reset();
    });
    let is_room_show = move || {
        location
            .pathname
            .get()
            .strip_prefix("/rooms/")
            .map(|rest| !rest.is_empty() && rest != "create" && !rest.contains('/'))
            .unwrap_or(false)
    };
    view! {
        <Show when=move || !is_room_show()>
            <crate::ads::Adsense wrapper_class="mt-8 mb-4"/>
            <Footer/>
        </Show>
    }
}

pub type SharedRealtime = RwSignal<Option<crate::ws::Realtime>, LocalStorage>;

pub type NotificationFeed = RwSignal<Vec<serde_json::Value>>;
pub type NotificationPopup = RwSignal<Option<serde_json::Value>>;

#[component]
pub fn App() -> impl IntoView {
    let auth = Auth::provide();
    let notif_feed = NotificationFeed::new(Vec::new());
    provide_context(notif_feed);
    let notif_popup = NotificationPopup::new(None);
    provide_context(notif_popup);
    let shared_rt = SharedRealtime::new_local(None);
    provide_context(shared_rt);

    let user_subscribed = RwSignal::new(Option::<i64>::None);
    Effect::new(move |_| {
        let _ = shared_rt.get(); // re-run if the room page created the socket
        let Some(u) = auth.user.get() else { return };
        if user_subscribed.get_untracked() == Some(u.id) {
            return;
        }
        let rt = match shared_rt.get_untracked() {
            Some(rt) => rt,
            None => {
                let Some(token) = crate::api::stored_token() else { return };
                let rt = crate::ws::Realtime::connect(&token);
                rt.start_heartbeat().forget();
                shared_rt.set(Some(rt.clone()));
                rt
            }
        };
        let ch = format!("user:{}", u.id);
        rt.join(&ch);
        rt.listen(
            &ch,
            "Illuminate\\Notifications\\Events\\BroadcastNotificationCreated",
            std::rc::Rc::new(move |data| {
                notif_feed.update(|l| l.insert(0, data.clone()));
                let ty = data["type"].as_str().unwrap_or("");
                if ty.ends_with("NewRoomAlert") {
                    notif_popup.set(Some(data.clone()));
                    set_timeout(
                        move || notif_popup.set(None),
                        std::time::Duration::from_secs(3),
                    );
                }
            }),
        );
        let user_sig = auth.user;
        rt.listen(&ch, "UserLevelUpdated", std::rc::Rc::new(move |data| {
            user_sig.update(|u| {
                if let Some(u) = u.as_mut() {
                    if let Some(v) = data["level"].as_i64() { u.level = v; }
                    if let Some(v) = data["current_xp"].as_i64() { u.current_xp = v; }
                    if let Some(v) = data["xp_for_next_level"].as_i64() { u.xp_for_next_level = v; }
                    if let Some(v) = data["total_xp"].as_i64() { u.total_xp = v; }
                    if data["level_metrics"].is_object() {
                        u.level_metrics = Some(data["level_metrics"].clone());
                    }
                }
            });
        }));
        user_subscribed.set(Some(u.id));
    });

    view! {
        <Router>
            <div class="flex min-h-screen flex-col">
                <Navbar/>
                <main class="flex-1 pb-20 md:pb-4">
                    <Routes fallback=|| {
                        view! {
                            <div class="mx-auto max-w-md px-4 py-24 text-center text-white/60">
                                "404 : Page introuvable"
                            </div>
                        }
                    }>
                        <Route path=path!("/") view=HomePage/>
                        <Route path=path!("/login") view=LoginPage/>
                        <Route path=path!("/register") view=RegisterPage/>
                        <Route path=path!("/forgot-password") view=ForgotPasswordPage/>
                        <Route path=path!("/reset-password/:token") view=ResetPasswordPage/>
                        <Route path=path!("/verify-email") view=VerifyEmailPage/>
                        <Route path=path!("/verify-email/:id/:hash") view=VerifyEmailConfirmPage/>
                        <Route path=path!("/oauth/:provider/callback") view=OAuthCallbackPage/>
                        <Route path=path!("/playlists") view=PlaylistsPage/>
                        <Route path=path!("/playlists/create") view=PlaylistCreatePage/>
                        <Route path=path!("/playlists/:id/edit") view=|| view! { <ParamRemount><PlaylistEditPage/></ParamRemount> }/>
                        <Route path=path!("/rooms/create") view=RoomCreatePage/>
                        <Route path=path!("/rooms") view=RoomsIndexPage/>
                        <Route path=path!("/rooms/:id") view=|| view! { <ParamRemount><RoomPage/></ParamRemount> }/>
                        <Route path=path!("/rooms/:id/edit") view=|| view! { <ParamRemount><RoomEditPage/></ParamRemount> }/>
                        <Route path=path!("/rankings") view=RankingsPage/>
                        <Route path=path!("/rankings/season") view=RankingsPage/>
                        <Route path=path!("/rankings/teams") view=RankingsPage/>
                        <Route path=path!("/rankings/minigames") view=RankingsPage/>
                        <Route path=path!("/rankings/:sort") view=RankingSortRedirect/>
                        <Route path=path!("/docs") view=DocsIndexPage/>
                        <Route path=path!("/docs/level") view=DocsLevelPage/>
                        <Route path=path!("/docs/faq") view=FaqPage/>
                        <Route path=path!("/docs/elo") view=DocsEloPage/>
                        <Route path=path!("/docs/glossary") view=DocsGlossaryPage/>
                        <Route path=path!("/docs/howto") view=DocsHowtoPage/>
                        <Route path=path!("/docs/create-content") view=DocsCreateContentPage/>
                        <Route path=path!("/docs/support") view=DocsSupportPage/>
                        <Route path=path!("/contact") view=ContactPage/>
                        <Route path=path!("/banned") view=BannedPage/>
                        <Route path=path!("/profile/:id") view=|| view! { <ParamRemount><ProfilePage/></ParamRemount> }/>
                        <Route path=path!("/me") view=SettingsPage/>
                        <Route path=path!("/teams") view=TeamsPage/>
                        <Route path=path!("/teams/:id") view=|| view! { <ParamRemount><TeamShowPage/></ParamRemount> }/>
                        <Route path=path!("/minigames") view=MinigamesDisabled/>
                        <Route path=path!("/minigames/:type") view=MinigamesDisabled/>
                        <Route path=path!("/games") view=MinigamesDisabled/>
                        <Route path=path!("/games/:type") view=MinigamesDisabled/>
                        <Route path=path!("/donations") view=DonationsPage/>
                        <Route path=path!("/faq") view=FaqRedirect/>
                        <Route path=path!("/user/banned") view=BannedPage/>
                        <Route path=path!("/settings") view=SettingsPage/>
                        <Route path=path!("/pages/:slug") view=|| view! { <ParamRemount><StaticPage/></ParamRemount> }/>
                        <Route path=path!("/admin") view=AdminDashboard/>
                        <Route path=path!("/admin/users") view=AdminUsers/>
                        <Route path=path!("/admin/teams") view=AdminTeams/>
                        <Route path=path!("/admin/rooms") view=AdminRooms/>
                        <Route path=path!("/admin/playlists") view=AdminPlaylists/>
                        <Route path=path!("/admin/categories") view=AdminCategories/>
                        <Route path=path!("/admin/answer-types") view=AdminAnswerTypes/>
                        <Route path=path!("/admin/pages") view=AdminPages/>
                        <Route path=path!("/admin/faqs") view=AdminFaqs/>
                        <Route path=path!("/admin/data-quality") view=AdminDataQuality/>
                        <Route path=path!("/moderation") view=ModDashboard/>
                        <Route path=path!("/moderation/users") view=ModUsers/>
                        <Route path=path!("/moderation/users/:id") view=|| view! { <ParamRemount><ModUserShow/></ParamRemount> }/>
                        <Route path=path!("/moderation/banned-users") view=ModBannedUsers/>
                        <Route path=path!("/moderation/trashed-messages") view=ModTrashedMessages/>
                        <Route path=path!("/moderation/moderators") view=ModModerators/>
                        <Route path=path!("/moderation/suggestions") view=ModSuggestions/>
                        <Route path=path!("/moderation/tracks") view=ModTracks/>
                        <Route path=path!("/language/:lang") view=LanguageSwitch/>
                        <Route path=path!("/en") view=LocaleRedirect/>
                        <Route path=path!("/es") view=LocaleRedirect/>
                        <Route path=path!("/en/*path") view=LocaleRedirect/>
                        <Route path=path!("/es/*path") view=LocaleRedirect/>
                        <Route path=path!("/:cat") view=CategoryLandingPage/>
                    </Routes>
                </main>
                <AppFooter/>
                <crate::components::install_prompt::InstallPrompt/>
            </div>
        </Router>
    }
}
