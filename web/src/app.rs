use crate::auth::Auth;
use crate::components::footer::Footer;
use crate::components::navbar::Navbar;
use crate::pages::admin::{
    AdminAnswerTypes, AdminCategories, AdminDashboard, AdminFaqs, AdminPages, AdminPlaylists,
    AdminRooms, AdminTeams, AdminUsers,
};
use crate::pages::docs::{
    BannedPage, CategoryLandingPage, ContactPage, DocsCreateContentPage, DocsEloPage,
    DocsGlossaryPage, DocsHowtoPage, DocsIndexPage, DocsLevelPage, DocsSupportPage,
    FaqRedirect, LanguageSwitch, LocaleRedirect, RankingSortRedirect,
};
use crate::pages::donations::DonationsPage;
use crate::pages::faq::FaqPage;
use crate::pages::games::{GamePlayPage, GamesIndexPage};
use crate::pages::home::HomePage;
use crate::pages::login::{
    ForgotPasswordPage, LoginPage, OAuthCallbackPage, RegisterPage, ResetPasswordPage,
    VerifyEmailConfirmPage, VerifyEmailPage,
};
use crate::pages::moderation::{
    ModBannedUsers, ModDashboard, ModModerators, ModTracks, ModTrashedMessages, ModUserShow,
    ModUsers,
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
use leptos_router::hooks::use_location;
use leptos_router::path;

/// Renders the footer except on the room gameplay page, which uses a
/// full-height layout (`calc(100dvh - nav)`) like Laravel's RoomLayout.
/// Lives inside `<Router>` — `use_location` needs the router context.
#[component]
fn AppFooter() -> impl IntoView {
    let location = use_location();
    // `Laravel\Head` parity — every page rebuilds head server-side; the SPA
    // restores the index.html defaults on route change, then each page
    // re-applies its own tags (`seo::apply_page` / room head).
    Effect::new(move |_| {
        let _ = location.pathname.get();
        crate::seo::reset();
    });
    // `Rooms/Show` uses RoomLayout (no footer/ads); every other page —
    // including Rooms/Create — renders AppLayout with them.
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
            // `AppLayout` mounts an AdSense slot just above the footer on
            // every non-excluded page.
            <crate::ads::Adsense wrapper_class="mt-8 mb-4"/>
            <Footer/>
        </Show>
    }
}

/// Realtime socket shared app-wide (PHP opens one Echo connection for
/// everything). `App` owns it for the `user:{id}` notification channel;
/// `RoomPage` reuses it for room channels instead of opening a second socket.
pub type SharedRealtime = RwSignal<Option<crate::ws::Realtime>, LocalStorage>;

/// Notification feed shared between `App`'s realtime listener and the
/// `Notifications` bell component.
pub type NotificationFeed = RwSignal<Vec<serde_json::Value>>;
/// `Notifications.vue` `popup` — top-of-screen flash on broadcast (PHP shows
/// the `NewRoomAlert` card for 3s).
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

    // `private-App.Models.User.{id}` — broadcast `BroadcastNotificationCreated`
    // events land in the bell live (TrackDeleted, team requests, alerts…).
    // Tracks BOTH auth.user and shared_rt: when the room page created the
    // socket before login, we still attach the user subscription to it.
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
                // `popup` flash — PHP sets it for every broadcast but only
                // renders the NewRoomAlert card inside.
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
        // `.user.level.updated` — live XP/level on the navbar badge.
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
                                "404 — Page introuvable"
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
                        <Route path=path!("/playlists/:id/edit") view=PlaylistEditPage/>
                        <Route path=path!("/rooms/create") view=RoomCreatePage/>
                        <Route path=path!("/rooms") view=RoomsIndexPage/>
                        <Route path=path!("/rooms/:id") view=RoomPage/>
                        <Route path=path!("/rooms/:id/edit") view=RoomEditPage/>
                        <Route path=path!("/rankings") view=RankingsPage/>
                        // PHP `rankings.teams` / `rankings.minigames` — real
                        // URLs, must beat the `/:sort` redirect below.
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
                        <Route path=path!("/profile/:id") view=ProfilePage/>
                        // `me` → `Me/Show.vue` — the account page (SettingsPage),
                        // not the public profile.
                        <Route path=path!("/me") view=SettingsPage/>
                        <Route path=path!("/teams") view=TeamsPage/>
                        <Route path=path!("/teams/:id") view=TeamShowPage/>
                        // PHP mounts these under `/minigames` — keep `/games`
                        // as an alias for convenience.
                        <Route path=path!("/minigames") view=GamesIndexPage/>
                        <Route path=path!("/minigames/:type") view=GamePlayPage/>
                        <Route path=path!("/games") view=GamesIndexPage/>
                        <Route path=path!("/games/:type") view=GamePlayPage/>
                        // PHP `routes/minigames.php` URL aliases (indexed/external links).
                        <Route path=path!("/minigames") view=GamesIndexPage/>
                        <Route path=path!("/minigames/:type") view=GamePlayPage/>
                        <Route path=path!("/donations") view=DonationsPage/>
                        <Route path=path!("/faq") view=FaqRedirect/>
                        <Route path=path!("/user/banned") view=BannedPage/>
                        <Route path=path!("/settings") view=SettingsPage/>
                        <Route path=path!("/pages/:slug") view=StaticPage/>
                        <Route path=path!("/admin") view=AdminDashboard/>
                        <Route path=path!("/admin/users") view=AdminUsers/>
                        <Route path=path!("/admin/teams") view=AdminTeams/>
                        <Route path=path!("/admin/rooms") view=AdminRooms/>
                        <Route path=path!("/admin/playlists") view=AdminPlaylists/>
                        <Route path=path!("/admin/categories") view=AdminCategories/>
                        <Route path=path!("/admin/answer-types") view=AdminAnswerTypes/>
                        <Route path=path!("/admin/pages") view=AdminPages/>
                        <Route path=path!("/admin/faqs") view=AdminFaqs/>
                        <Route path=path!("/moderation") view=ModDashboard/>
                        <Route path=path!("/moderation/users") view=ModUsers/>
                        <Route path=path!("/moderation/users/:id") view=ModUserShow/>
                        <Route path=path!("/moderation/banned-users") view=ModBannedUsers/>
                        <Route path=path!("/moderation/trashed-messages") view=ModTrashedMessages/>
                        <Route path=path!("/moderation/moderators") view=ModModerators/>
                        <Route path=path!("/moderation/tracks") view=ModTracks/>
                        // Locale prefixes + blind-test landings: matched LAST —
                        // leptos_router picks the first matching route in
                        // declaration order, so params/wildcards must trail
                        // every static route.
                        <Route path=path!("/language/:lang") view=LanguageSwitch/>
                        <Route path=path!("/en") view=LocaleRedirect/>
                        <Route path=path!("/es") view=LocaleRedirect/>
                        <Route path=path!("/en/*path") view=LocaleRedirect/>
                        <Route path=path!("/es/*path") view=LocaleRedirect/>
                        // Laravel `blind-test-{category:slug}` — a single URL
                        // segment; the router can't mix static+param in one
                        // segment so we match `/:cat` and filter inside.
                        <Route path=path!("/:cat") view=CategoryLandingPage/>
                    </Routes>
                </main>
                <AppFooter/>
            </div>
        </Router>
    }
}
