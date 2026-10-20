pub mod admin;
pub mod auth;
pub mod auth_reset;
pub mod auth_session;
pub mod authz;
pub mod avatar;
pub mod billing;
pub mod category_content;
pub mod chat;
pub mod content;
pub mod donations;
pub mod faqs;
pub mod home;
pub mod levels;
pub mod local_tracks;
pub mod mail;
pub mod minigames;
pub mod moderation;
pub mod oauth;
pub mod official_seo;
pub mod party;
pub mod playlists;
pub mod profiles;
pub mod providers;
pub mod rankings;
pub mod rooms;
pub mod round;
pub mod seasons;
pub mod seo;
pub mod state;
pub mod storage;
pub mod teams;
pub mod users;
pub mod ws;

pub use state::AppState;

use axum::Json;
use axum::Router;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use tower_http::services::{ServeDir, ServeFile};

const PHOTO_UPLOAD_LIMIT: usize = 12 * 1024 * 1024;
const TRACK_UPLOAD_LIMIT: usize = 16 * 1024 * 1024;

async fn api_not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({"message": "Not Found"})),
    )
}

async fn html_no_cache(
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut resp = next.run(req).await;
    let is_html = resp
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.starts_with("text/html"))
        .unwrap_or(false);
    if is_html {
        resp.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-cache"),
        );
    }
    resp
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/auth/register", post(auth_session::register))
        .route("/api/v1/auth/login", post(auth_session::login))
        .route("/api/v1/auth/guest", post(auth_session::guest))
        .route("/api/v1/auth/logout", post(auth_session::logout))
        .route("/api/v1/auth/me", get(auth_session::me))
        .route(
            "/api/v1/auth/forgot-password",
            post(auth_reset::forgot_password),
        )
        .route(
            "/api/v1/auth/reset-password",
            post(auth_reset::reset_password),
        )
        .route(
            "/api/v1/auth/email/verification-notification",
            post(auth_reset::verification_notification),
        )
        .route(
            "/api/v1/auth/email/verify/{id}/{hash}",
            get(auth_reset::verify_email),
        )
        .route(
            "/api/v1/auth/oauth/{provider}/redirect",
            get(oauth::redirect),
        )
        .route(
            "/api/v1/auth/oauth/{provider}/callback",
            get(oauth::callback),
        )
        .route("/auth/redirect/{provider}", get(oauth::redirect_compat))
        .route("/callback/{provider}", get(oauth::callback_compat))
        .route("/api/v1/home", get(home::index))
        .route("/api/v1/rooms", get(rooms::list).post(rooms::store))
        .route("/api/v1/rooms/mine", get(rooms::mine))
        .route("/api/v1/categories", get(rooms::categories))
        .route(
            "/api/v1/rooms/{id}",
            get(rooms::show).put(rooms::update).delete(rooms::destroy),
        )
        .route("/api/v1/rooms/{id}/edit", get(rooms::edit))
        .route(
            "/api/v1/rooms/{id}/restore",
            axum::routing::put(rooms::restore),
        )
        .route(
            "/api/v1/rooms/{id}/options",
            axum::routing::put(rooms::update_options),
        )
        .route(
            "/api/v1/rooms/{id}/playlists/attach",
            post(rooms::attach_playlist),
        )
        .route(
            "/api/v1/rooms/{id}/playlists/detach",
            axum::routing::delete(rooms::detach_playlist),
        )
        .route(
            "/api/v1/rooms/{id}/moderators/attach",
            post(rooms::attach_moderator),
        )
        .route(
            "/api/v1/rooms/{id}/moderators/detach",
            axum::routing::delete(rooms::detach_moderator),
        )
        .route(
            "/api/v1/rooms/{id}/bookmark",
            post(rooms::bookmark).delete(rooms::unbookmark),
        )
        .route(
            "/api/v1/rooms/{id}/photo",
            post(rooms::update_photo).layer(DefaultBodyLimit::max(PHOTO_UPLOAD_LIMIT)),
        )
        .route(
            "/api/v1/rooms/{id}/picture",
            axum::routing::delete(rooms::delete_picture),
        )
        .route(
            "/api/v1/rooms/{id}/tracks/search",
            get(rooms::search_room_tracks),
        )
        .route("/api/v1/rooms/{id}/scores", get(rooms::scores))
        .route(
            "/api/v1/rooms/{room}/tracks/{track}/upvote",
            post(rooms::upvote_track),
        )
        .route(
            "/api/v1/rooms/{room}/tracks/{track}/downvote",
            post(rooms::downvote_track),
        )
        .route(
            "/api/v1/rooms/{room}/tracks/{track}",
            axum::routing::delete(rooms::mod_track_destroy),
        )
        .route(
            "/api/v1/rooms/{room}/tracks/{track}/answers",
            get(rooms::mod_track_answers).post(rooms::mod_track_store_answer),
        )
        .route(
            "/api/v1/rooms/{room}/tracks/{track}/answers/{aid}",
            axum::routing::put(rooms::mod_track_update_answer)
                .delete(rooms::mod_track_delete_answer),
        )
        .route(
            "/api/v1/rooms/{id}/user-count",
            post(rooms::update_user_count),
        )
        .route("/api/v1/rooms/{id}/alert", post(rooms::alert))
        .route("/api/v1/rooms/{id}/suggestion", post(rooms::suggestion))
        .route("/api/v1/ws-ticket", post(crate::auth_session::ws_ticket))
        .route("/api/v1/rooms/{id}/joined", get(rooms::joined))
        .route(
            "/api/v1/rooms/{id}/presence-joined",
            post(rooms::presence_joined),
        )
        .route(
            "/api/v1/rooms/{id}/presence-left",
            post(rooms::presence_left),
        )
        .route("/api/v1/rooms/{id}/public-state", get(rooms::public_state))
        .route("/api/v1/rooms/{id}/live-state", get(rooms::live_state))
        .route("/api/v1/rooms/{id}/qr.svg", get(rooms::qr_code))
        .route(
            "/api/v1/rooms/{id}/messages",
            get(chat::history).post(chat::store),
        )
        .route(
            "/api/v1/messages/{id}",
            axum::routing::delete(chat::destroy),
        )
        .route("/api/v1/chat/config", get(chat::config))
        .route("/api/v1/messages/{id}/report", post(chat::report))
        .route(
            "/api/v1/messages/{id}/restore",
            axum::routing::put(chat::restore),
        )
        .route("/api/v1/minigames", get(minigames::index))
        .route(
            "/api/v1/minigames/{game}/leaderboard",
            get(minigames::leaderboard),
        )
        .route("/api/v1/minigames/{game}/next", post(minigames::next))
        .route("/api/v1/minigames/{game}/check", post(minigames::check))
        .route(
            "/api/v1/duels",
            get(minigames::duels_index).post(minigames::duel_create),
        )
        .route(
            "/api/v1/duels/leaderboard",
            get(minigames::duels_leaderboard),
        )
        .route("/api/v1/duels/{id}", get(minigames::duel_show))
        .route("/api/v1/duels/{id}/accept", post(minigames::duel_accept))
        .route("/api/v1/duels/{id}/answer", post(minigames::duel_answer))
        .route("/api/v1/daily-challenge", get(minigames::daily_show))
        .route("/api/v1/rooms/{id}/party", get(party::show))
        .route("/api/v1/rooms/{id}/party/start", post(party::start))
        .route("/api/v1/rooms/{id}/party/stop", post(party::stop))
        .route("/api/v1/rooms/{id}/party/answer", post(party::answer))
        .route(
            "/api/v1/daily-challenge/answer",
            post(minigames::daily_answer),
        )
        .route("/api/v1/moderation", get(moderation::dashboard))
        .route(
            "/api/v1/moderation/suggestions",
            get(moderation::suggestions),
        )
        .route(
            "/api/v1/moderation/tracks/{id}/rematch",
            get(moderation::track_rematch_candidates).post(moderation::track_rematch_apply),
        )
        .route(
            "/api/v1/moderation/trashed-messages",
            get(moderation::trashed_messages),
        )
        .route(
            "/api/v1/moderation/trashed-messages/{id}/restore",
            post(moderation::restore_message),
        )
        .route(
            "/api/v1/moderation/trashed-messages/{id}",
            axum::routing::delete(moderation::destroy_message),
        )
        .route(
            "/api/v1/moderation/banned-users",
            get(moderation::banned_users),
        )
        .route(
            "/api/v1/moderation/banned-users/{user}/unban",
            post(moderation::unban),
        )
        .route("/api/v1/moderation/users", get(moderation::users))
        .route(
            "/api/v1/moderation/users/{user}",
            get(moderation::user_show),
        )
        .route("/api/v1/moderation/users/{user}/ban", post(moderation::ban))
        .route("/api/v1/moderation/moderators", get(moderation::moderators))
        .route(
            "/api/v1/moderation/moderators/rooms/{room}/users/{user}",
            axum::routing::delete(moderation::detach_room_moderator),
        )
        .route(
            "/api/v1/moderation/moderators/playlists/{playlist}/users/{user}",
            axum::routing::delete(moderation::detach_playlist_moderator),
        )
        .route(
            "/api/v1/admin/users",
            get(admin::users).post(admin::create_user),
        )
        .route(
            "/api/v1/admin/users/{user}",
            axum::routing::put(admin::update_user).delete(admin::delete_user),
        )
        .route(
            "/api/v1/admin/users/{user}/role",
            axum::routing::put(admin::set_user_role),
        )
        .route(
            "/api/v1/admin/users/{user}/restore",
            axum::routing::put(admin::restore_user),
        )
        .route(
            "/api/v1/admin/force-clear-rounds",
            post(admin::force_clear_rounds),
        )
        .route("/api/v1/admin/clear-cache", post(admin::clear_cache))
        .route(
            "/api/v1/admin/regenerate-top-10",
            post(admin::regenerate_top_10),
        )
        .route("/api/v1/admin/dashboard", get(admin::dashboard))
        .route(
            "/api/v1/admin/playlists",
            get(admin::playlists).post(admin::create_playlist),
        )
        .route(
            "/api/v1/admin/playlists/{id}",
            axum::routing::put(admin::update_playlist).delete(admin::destroy_playlist),
        )
        .route(
            "/api/v1/admin/rooms",
            get(admin::rooms).post(admin::create_room),
        )
        .route(
            "/api/v1/admin/rooms/{id}",
            axum::routing::put(admin::update_room).delete(admin::destroy_room),
        )
        .route(
            "/api/v1/admin/rooms/{id}/restore",
            axum::routing::put(admin::restore_room),
        )
        .route("/api/v1/admin/teams", get(admin::teams))
        .route(
            "/api/v1/admin/teams/{id}",
            axum::routing::put(admin::update_team).delete(admin::destroy_team),
        )
        .route(
            "/api/v1/admin/teams/{id}/restore",
            axum::routing::put(admin::restore_team),
        )
        .route(
            "/api/v1/admin/categories",
            get(admin::categories).post(admin::create_category),
        )
        .route(
            "/api/v1/admin/categories/{id}",
            axum::routing::put(admin::update_category).delete(admin::delete_category),
        )
        .route(
            "/api/v1/admin/answer-types",
            get(admin::answer_types).post(admin::create_answer_type),
        )
        .route(
            "/api/v1/admin/answer-types/{id}",
            axum::routing::put(admin::update_answer_type).delete(admin::delete_answer_type),
        )
        .route(
            "/api/v1/admin/pages",
            get(admin::pages).post(admin::create_page),
        )
        .route(
            "/api/v1/admin/pages/{id}",
            axum::routing::put(admin::update_page).delete(admin::delete_page),
        )
        .route(
            "/api/v1/local-tracks",
            post(local_tracks::store).layer(DefaultBodyLimit::max(TRACK_UPLOAD_LIMIT)),
        )
        .route("/api/v1/local-tracks/{id}/audio", get(local_tracks::audio))
        .route(
            "/api/v1/providers/local/search/track",
            get(local_tracks::search),
        )
        .route("/api/v1/moderation/tracks", get(local_tracks::index))
        .route(
            "/api/v1/moderation/tracks/{id}",
            axum::routing::put(local_tracks::update).delete(local_tracks::destroy),
        )
        .route(
            "/api/v1/messages/{id}/reactions",
            get(chat::reactions).post(chat::toggle_reaction),
        )
        .route("/api/v1/avatar", get(avatar::avatar))
        .route("/api/v1/pages/{slug}", get(content::page))
        .route("/api/v1/user/banned", get(content::banned_user))
        .route("/api/v1/contact", post(content::contact))
        .route("/api/v1/donations/progress", get(donations::progress))
        .route("/api/v1/donations/history", get(donations::history))
        .route("/api/v1/donations/supporters", get(donations::supporters))
        .route("/api/v1/donations/recent", get(donations::recent))
        .route(
            "/api/v1/users/{id}/donations",
            get(donations::user_donations),
        )
        .route("/api/v1/billing", get(billing::billing))
        .route("/api/v1/billing/plans", get(billing::plans_public))
        .route(
            "/api/v1/daily-challenge/revive",
            post(billing::daily_revive),
        )
        .route(
            "/api/v1/users/{id}/stats/advanced",
            get(billing::stats_advanced),
        )
        .route("/api/v1/stripe/webhook", post(donations::stripe_webhook))
        .route("/robots.txt", get(seo::robots))
        .route("/llms.txt", get(seo::llms))
        .route("/sitemap.xml", get(seo::sitemap))
        .route(
            "/contact",
            post(content::contact).get_service(ServeFile::new(state.spa_dir.join("index.html"))),
        )
        .route("/stripe/webhook", post(donations::stripe_webhook))
        .route("/api/v1/faqs", get(faqs::index))
        .route("/api/v1/faqs/{id}/vote/{direction}", post(faqs::vote))
        .route(
            "/api/v1/admin/faqs",
            get(admin::faqs).post(admin::create_faq),
        )
        .route(
            "/api/v1/admin/faqs/{id}",
            axum::routing::put(admin::update_faq).delete(admin::delete_faq),
        )
        .route(
            "/api/v1/admin/data-quality/rooms",
            get(admin::data_quality_rooms),
        )
        .route(
            "/api/v1/admin/data-quality/rooms/{room}/missing",
            get(admin::data_quality_missing),
        )
        .route(
            "/api/v1/admin/data-quality/tracks/{track}/answer",
            post(admin::data_quality_apply_answer),
        )
        .route(
            "/api/v1/admin/data-quality/tracks/{track}/dismiss",
            post(admin::data_quality_dismiss),
        )
        .route(
            "/api/v1/admin/data-quality/tracks/{track}/remove",
            post(admin::data_quality_remove_track),
        )
        .route(
            "/api/v1/admin/data-quality/tracks/{track}/move",
            post(admin::data_quality_move_track),
        )
        .route(
            "/api/v1/admin/data-quality/enrichment",
            post(admin::data_quality_enrichment),
        )
        .route(
            "/api/v1/admin/data-quality/playlists",
            get(admin::data_quality_playlists),
        )
        .route(
            "/api/v1/admin/data-quality/actions",
            get(admin::data_quality_actions),
        )
        .route(
            "/api/v1/admin/data-quality/actions/{action}/undo",
            post(admin::data_quality_undo_remove),
        )
        .route(
            "/api/v1/admin/data-quality/actions/{action}/restore",
            post(admin::data_quality_restore),
        )
        .route(
            "/api/v1/admin/data-quality/queue/artists",
            get(admin::data_quality_artist_queue),
        )
        .route(
            "/api/v1/admin/data-quality/queue/featuring",
            get(admin::data_quality_featuring_queue),
        )
        .route(
            "/api/v1/admin/data-quality/duplicates",
            get(admin::data_quality_duplicates),
        )
        .route(
            "/api/v1/admin/data-quality/tracks/{track}/re-artist",
            post(admin::data_quality_re_artist),
        )
        .route(
            "/api/v1/admin/data-quality/tracks/{track}/answer-typed",
            post(admin::data_quality_apply_typed),
        )
        .route(
            "/api/v1/admin/data-quality/tracks/{track}/dismiss-typed",
            post(admin::data_quality_dismiss_typed),
        )
        .route("/api/v1/rankings", get(rankings::players))
        .route("/api/v1/rankings/teams", get(rankings::teams))
        .route("/api/v1/rankings/minigames", get(rankings::minigames))
        .route("/api/v1/seasons", get(seasons::index))
        .route(
            "/api/v1/rankings/users/{user}/level-metrics",
            get(rankings::user_level_metrics),
        )
        .route("/api/v1/teams", get(teams::index).post(teams::store))
        .route(
            "/api/v1/teams/{id}",
            get(teams::show).put(teams::update).delete(teams::destroy),
        )
        .route("/api/v1/teams/{id}/leave", post(teams::leave))
        .route(
            "/api/v1/teams/{id}/photo",
            post(teams::update_photo).layer(DefaultBodyLimit::max(PHOTO_UPLOAD_LIMIT)),
        )
        .route("/api/v1/teams/{id}/request", post(teams::request_join))
        .route(
            "/api/v1/teams/{id}/request/cancel",
            post(teams::cancel_request),
        )
        .route("/api/v1/teams/{id}/owner/{uid}", post(teams::switch_owner))
        .route(
            "/api/v1/teams/{id}/members/{uid}/remove",
            post(teams::remove_member),
        )
        .route(
            "/api/v1/teams/requests/{req}/accept",
            post(teams::accept_request),
        )
        .route(
            "/api/v1/teams/requests/{req}/decline",
            post(teams::decline_request),
        )
        .route("/api/v1/profile/{user}", get(profiles::show))
        .route("/api/v1/profile/{user}/scores", get(profiles::scores))
        .route("/api/v1/profile/{user}/likes", get(profiles::likes))
        .route("/api/v1/profile/{user}/bookmarks", get(profiles::bookmarks))
        .route(
            "/api/v1/profile/{user}/minigames",
            get(profiles::profile_minigames),
        )
        .route(
            "/api/v1/profile/{user}/score-evolution",
            get(profiles::score_evolution),
        )
        .route(
            "/api/v1/profile/{user}/highlights",
            get(profiles::highlights),
        )
        .route(
            "/api/v1/profile/likes/{track}",
            axum::routing::delete(profiles::unlike_track),
        )
        .route("/api/v1/users", get(users::index))
        .route("/api/v1/users/me", get(users::me))
        .route("/api/v1/users/notifications", get(users::notifications))
        .route(
            "/api/v1/users/notifications/{id}/read",
            post(users::notification_read),
        )
        .route(
            "/api/v1/users/notifications/{id}/done",
            post(users::notification_done),
        )
        .route(
            "/api/v1/users/{id}",
            post(users::update).delete(users::destroy),
        )
        .route(
            "/api/v1/users/{id}/password",
            axum::routing::put(users::update_password),
        )
        .route(
            "/api/v1/users/{id}/donation-preferences",
            axum::routing::patch(users::update_donation_preferences),
        )
        .route(
            "/api/v1/users/{id}/photo",
            post(users::update_photo)
                .layer(DefaultBodyLimit::max(PHOTO_UPLOAD_LIMIT))
                .delete(users::destroy_photo),
        )
        .route("/api/v1/rooms/{id}/start", post(round::start))
        .route("/api/v1/rooms/{id}/pause", post(round::pause))
        .route("/api/v1/rooms/{id}/resume", post(round::resume))
        .route("/api/v1/rooms/{id}/next", post(round::next))
        .route("/api/v1/rooms/{id}/stop", post(round::stop))
        .route(
            "/api/v1/rounds/{round}/tracks/{track}/check",
            post(round::check),
        )
        .route(
            "/api/v1/rounds/{round}/tracks/{track}/listened",
            post(round::track_listened),
        )
        .route("/api/v1/rounds/{round}/scores", get(round::scores))
        .route("/api/v1/providers/search", get(providers::search))
        .route(
            "/api/v1/playlists",
            get(playlists::index).post(playlists::store),
        )
        .route(
            "/api/v1/playlists/{id}",
            get(playlists::show)
                .put(playlists::update)
                .delete(playlists::destroy),
        )
        .route(
            "/api/v1/playlists/{id}/providers/find",
            post(playlists::find_provider_playlist),
        )
        .route(
            "/api/v1/playlists/{id}/providers/import",
            post(playlists::import_provider_playlist),
        )
        .route(
            "/api/v1/playlists/{id}/tracks",
            get(playlists::tracks).post(playlists::store_track),
        )
        .route(
            "/api/v1/playlists/{id}/tracks/clear",
            axum::routing::delete(playlists::clear_tracks),
        )
        .route(
            "/api/v1/playlists/{id}/tracks/{track}",
            axum::routing::put(playlists::update_track).delete(playlists::delete_track),
        )
        .route(
            "/api/v1/playlists/{id}/tracks/search",
            get(playlists::search_tracks),
        )
        .route("/api/v1/playlists/{id}/export", get(playlists::export))
        .route(
            "/api/v1/playlists/{id}/moderators",
            post(playlists::attach_playlist_moderator).delete(playlists::detach_playlist_moderator),
        )
        .route(
            "/api/v1/categories/{slug}/landing",
            get(rooms::category_landing),
        )
        .route("/api/v1/answer-types", get(playlists::answer_types))
        .route("/api/v1/tracks/{id}/answers", post(playlists::store_answer))
        .route(
            "/api/v1/tracks/{id}/answers/{aid}",
            axum::routing::put(playlists::update_answer).delete(playlists::delete_answer),
        )
        .route("/ws", get(ws::handler))
        .route("/healthz", get(|| async { "ok" }))
        .route(
            "/readyz",
            get(|State(state): State<AppState>| async move {
                let db = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();
                let redis = fred::interfaces::ClientLike::ping::<()>(&state.redis, None)
                    .await
                    .is_ok();
                if db && redis {
                    (StatusCode::OK, "ready").into_response()
                } else {
                    (
                        StatusCode::SERVICE_UNAVAILABLE,
                        Json(serde_json::json!({"db": db, "redis": redis})),
                    )
                        .into_response()
                }
            }),
        )
        .route(
            "/api/v1/version",
            get(|| async {
                let build = std::env::var("APP_VERSION").unwrap_or_else(|_| "dev".into());
                // A release tag (vX.Y.Z) is the real product version; a plain
                // git sha or "dev" falls back to the cargo version.
                let tag = build.trim_start_matches('v');
                let is_release = tag.split('.').count() == 3
                    && tag
                        .split('.')
                        .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
                Json(serde_json::json!({
                    "version": if is_release { tag } else { env!("CARGO_PKG_VERSION") },
                    "build": build,
                }))
            }),
        )
        .route("/api/{*rest}", axum::routing::any(api_not_found))
        .route(
            &format!("{}/{{*path}}", state.storage_prefix),
            get(storage::serve_stored),
        )
        .fallback_service(
            ServeDir::new(&state.spa_dir)
                .fallback(ServeFile::new(state.spa_dir.join("index.html"))),
        )
        .layer(axum::middleware::from_fn(html_no_cache))
        .with_state(state)
}
