use blindall_api::{AppState, router};
use blindall_db::{AuthRepo, DbGameStore};
use blindall_game::GameManager;
use blindall_realtime::Hub;
use blindall_redis::{RoomPresenceStore, RoundScoreStore};
use fred::prelude::*;
use std::sync::Arc;

mod sweeper;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "blindall=info,tower_http=info".into()),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@127.0.0.1:5432/postgres".into());
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".into());
    let bind = std::env::var("BIND").unwrap_or_else(|_| "0.0.0.0:8080".into());

    let pool = blindall_db::connect(&database_url).await?;
    let app_key = std::env::var("APP_KEY").unwrap_or_default();
    if app_key.is_empty() {
        tracing::warn!("APP_KEY not set : reset tokens / signed URLs won't verify across stacks");
    }
    tracing::info!("postgres connected");

    if std::env::var("SKIP_MIGRATIONS").as_deref() != Ok("1") {
        blindall_db::migrate(&pool).await?;
        tracing::info!("migrations applied");
    }
    if std::env::var("SEED_DEMO")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
    {
        blindall_db::seed_demo(&pool).await?;
        tracing::info!("demo seed applied");
    }

    let redis_cfg = fred::types::config::Config::from_url_centralized(&redis_url)?;
    let redis = Client::new(redis_cfg, None, None, None);
    let _handle = redis.init().await?;
    tracing::info!("redis connected");

    let scores = Arc::new(RoundScoreStore::new(redis.clone()));
    let presence = Arc::new(RoomPresenceStore::new(redis.clone()));
    let hub = Hub::new();
    let cache_prefix = std::env::var("CACHE_PREFIX").unwrap_or_else(|_| "blindall_cache_".into());
    let audius_app_name = std::env::var("AUDIUS_APP_NAME").unwrap_or_else(|_| "blindall".into());
    let resolver = Arc::new(blindall_providers::ProviderResolver::new(
        redis.clone(),
        &cache_prefix,
        &audius_app_name,
    ));
    let providers = std::sync::Arc::new(blindall_providers::MusicProviders::new(
        blindall_providers::ProvidersConfig {
            spotify_client_id: std::env::var("SPOTIFY_CLIENT_ID").unwrap_or_default(),
            spotify_client_secret: std::env::var("SPOTIFY_CLIENT_SECRET").unwrap_or_default(),
            spotify_market: std::env::var("SPOTIFY_MARKET").unwrap_or_else(|_| "FR".into()),
            audius_app_name: audius_app_name.clone(),
            youtube_api_key: std::env::var("YOUTUBE_API_KEY").unwrap_or_default(),
            app_url: std::env::var("APP_URL").unwrap_or_default(),
            app_locale: std::env::var("APP_LOCALE").unwrap_or_else(|_| "fr".into()),
        },
        redis.clone(),
    ));
    let store = Arc::new(
        DbGameStore::with_resolver(pool.clone(), resolver.clone())
            .environment(std::env::var("APP_ENV").unwrap_or_else(|_| "local".into())),
    );
    let storage = blindall_api::storage::backend_from_env(
        std::env::var("STORAGE_ROOT")
            .unwrap_or_else(|_| "storage/app".into())
            .into(),
        std::env::var("STORAGE_PUBLIC_PREFIX").unwrap_or_else(|_| "/storage".into()),
    );
    let photo_prefix = storage.url("").trim_end_matches('/').to_string();
    let manager = GameManager::new(
        store.clone(),
        scores.clone(),
        presence.clone(),
        hub.clone(),
        photo_prefix,
    );

    if let Err(e) = manager.recover().await {
        tracing::error!(error = %e, "game recovery failed");
    }

    sweeper::spawn(
        pool.clone(),
        manager.clone(),
        std::env::var("STORAGE_ROOT")
            .unwrap_or_else(|_| "storage/app".into())
            .into(),
        std::sync::Arc::new(blindall_api::levels::LevelDispatcher::new(
            blindall_db::LevelRepo::new(pool.clone()),
            hub.clone(),
        )),
    );

    let state = AppState {
        manager,
        store,
        auth: AuthRepo::new(pool.clone()),
        playlists: blindall_db::PlaylistRepo::new(pool.clone()),
        rooms: blindall_db::RoomRepo::new(pool.clone()),
        chat: blindall_db::ChatRepo::new(pool.clone()),
        chat_moderation: std::sync::Arc::new(blindall_api::chat::ChatModeration::default()),
        teams: blindall_db::TeamRepo::new(pool.clone()),
        profiles: blindall_db::ProfileRepo::new(pool.clone()),
        votes: blindall_db::VoteRepo::new(pool.clone()),
        scores,
        presence,
        hub: hub.clone(),
        providers,
        http: reqwest::Client::new(),
        throttle: std::sync::Arc::new(blindall_api::auth_session::LoginThrottle::default()),
        guest_throttle: std::sync::Arc::new(blindall_api::auth_session::GuestThrottle::default()),
        chat_rate: std::sync::Arc::new(blindall_api::auth_session::RateBucket::default()),
        presence_rate: std::sync::Arc::new(blindall_api::auth_session::RateBucket::default()),
        check_rate: std::sync::Arc::new(blindall_api::auth_session::RateBucket::default()),
        contact_rate: std::sync::Arc::new(blindall_api::auth_session::RateBucket::default()),
        reset_rate: std::sync::Arc::new(blindall_api::auth_session::RateBucket::default()),
        suggestion_rate: std::sync::Arc::new(blindall_api::auth_session::RateBucket::default()),
        ws_tickets: std::sync::Arc::new(blindall_api::auth_session::WsTickets::default()),
        levels: std::sync::Arc::new(blindall_api::levels::LevelDispatcher::new(
            blindall_db::LevelRepo::new(pool.clone()),
            hub.clone(),
        )),
        password_resets: blindall_db::PasswordResetRepo::new(pool.clone(), app_key.clone()),
        mail: blindall_api::mail::MailConfig {
            host: std::env::var("MAIL_HOST").unwrap_or_default(),
            port: std::env::var("MAIL_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(587),
            username: std::env::var("MAIL_USERNAME").unwrap_or_default(),
            password: std::env::var("MAIL_PASSWORD").unwrap_or_default(),
            encryption: std::env::var("MAIL_ENCRYPTION").unwrap_or_else(|_| "tls".into()),
            from_email: std::env::var("MAIL_FROM_ADDRESS")
                .unwrap_or_else(|_| "no-reply@blindallmusic.com".into()),
            from_name: std::env::var("MAIL_FROM_NAME").unwrap_or_else(|_| "Blind'All Music".into()),
            app_url: std::env::var("APP_URL").unwrap_or_else(|_| "http://localhost:8080".into()),
        },
        app_key,
        minigames: blindall_db::MinigameRepo::new(pool.clone(), resolver),
        anagram_questions: std::sync::Arc::new(blindall_api::minigames::AnagramQuestions::default()),
        parties: std::sync::Arc::new(blindall_api::party::PartyStore::default()),
        moderation: blindall_db::ModerationRepo::new(pool.clone()),
        admin: blindall_db::AdminRepo::new(pool.clone()),
        faqs: blindall_db::FaqRepo::new(pool.clone()),
        rankings: blindall_db::RankingsRepo::new(pool.clone()),
        seasons: blindall_db::SeasonRepo::new(pool.clone()),
        notifications: blindall_db::NotificationRepo::new(pool.clone()),
        rankings_cache: std::sync::Arc::new(blindall_api::rankings::RankingsCache::default()),
        local_tracks: blindall_db::LocalTrackRepo::new(pool.clone()),
        redis: redis.clone(),
        cache_prefix: cache_prefix.clone(),
        oauth: {
            let mut providers = std::collections::HashMap::new();
            for (name, id_vars, secret_vars, redirect_var) in [
                (
                    "google",
                    &["GOOGLE_CLIENT_ID"][..],
                    &["GOOGLE_CLIENT_SECRET"][..],
                    "GOOGLE_REDIRECT_URI",
                ),
                (
                    "discord",
                    &["DISCORD_APP_ID"][..],
                    &["DISCORD_APP_SECRET"][..],
                    "DISCORD_REDIRECT_URI",
                ),
            ] {
                let env_first = |names: &[&str]| {
                    names
                        .iter()
                        .filter_map(|n| std::env::var(n).ok())
                        .find(|v| !v.is_empty())
                        .unwrap_or_default()
                };
                let client_id = env_first(id_vars);
                if !client_id.is_empty() {
                    providers.insert(
                        name.to_string(),
                        blindall_api::oauth::OAuthProviderEntry {
                            client_id,
                            client_secret: env_first(secret_vars),
                            redirect_uri: std::env::var(redirect_var).unwrap_or_default(),
                        },
                    );
                }
            }
            blindall_api::oauth::OAuthConfig { providers }
        },
        oauth_states: std::sync::Arc::new(blindall_api::oauth::OAuthStates::default()),
        storage,
        storage_root: std::env::var("STORAGE_ROOT")
            .unwrap_or_else(|_| "storage/app".into())
            .into(),
        storage_prefix: std::env::var("STORAGE_PUBLIC_PREFIX")
            .unwrap_or_else(|_| "/storage".into()),
        app_url: std::env::var("APP_URL").unwrap_or_else(|_| "http://localhost:8080".into()),
        seo: blindall_db::SeoRepo::new(pool.clone()),
        home: blindall_db::HomeRepo::new(pool.clone()),
        hidden_category_ids: std::env::var("HIDDEN_CATEGORY_IDS")
            .unwrap_or_default()
            .split(',')
            .filter_map(|s| s.trim().parse::<i64>().ok())
            .collect(),
        donations_repo: blindall_db::DonationRepo::new(pool),
        donations: {
            let mut cfg = blindall_api::donations::DonationsConfig::default();
            if let Ok(v) = std::env::var("DONATIONS_MONTHLY_GOAL_CENTS") {
                cfg.monthly_goal_cents = v.parse().unwrap_or(cfg.monthly_goal_cents);
            }
            if let Ok(v) = std::env::var("STRIPE_DONATION_URL") {
                cfg.stripe_payment_url = v;
            }
            cfg.stripe_webhook_secret = std::env::var("STRIPE_WEBHOOK_SECRET").unwrap_or_default();
            cfg
        },
        adsense_client: std::env::var("ADSENSE_CLIENT_ID").unwrap_or_default(),
        adsense_slot: std::env::var("ADSENSE_SLOT").unwrap_or_default(),
        spa_dir: std::env::var("SPA_DIR")
            .unwrap_or_else(|_| "web/dist".into())
            .into(),
        chat_reaction_emojis: [
            "👍", "😂", "❤️", "🔥", "😮", "😢", "👏", "😡", "🎉", "😎", "🤔", "🙌", "💯", "🎵",
            "🥁", "🎸", "🎤", "🎻", "🎺", "🎷", "🎶", "😊", "😃", "😞", "😉", "😛", "😲", "😘",
            "😕", "🤑",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
    };

    let slow_ms: u64 = std::env::var("SLOW_REQUEST_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(500);

    let app = router(state)
        // Never log query strings: /ws carries the bearer token
        .layer(
            tower_http::trace::TraceLayer::new_for_http()
                .make_span_with(|req: &axum::http::Request<axum::body::Body>| {
                    tracing::info_span!("http", method = %req.method(), path = %req.uri().path())
                })
                .on_response(
                    move |resp: &axum::http::Response<axum::body::Body>,
                          latency: std::time::Duration,
                          _span: &tracing::Span| {
                        if latency > std::time::Duration::from_millis(slow_ms) {
                            tracing::warn!(
                                status = %resp.status().as_u16(),
                                latency_ms = latency.as_millis(),
                                "slow request"
                            );
                        }
                    },
                ),
        )
        .layer(
            tower_http::set_header::SetResponseHeaderLayer::overriding(
                axum::http::header::X_CONTENT_TYPE_OPTIONS,
                axum::http::HeaderValue::from_static("nosniff"),
            ),
        )
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::header::X_FRAME_OPTIONS,
            axum::http::HeaderValue::from_static("DENY"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::header::REFERRER_POLICY,
            axum::http::HeaderValue::from_static("strict-origin-when-cross-origin"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::HeaderName::from_static("permissions-policy"),
            axum::http::HeaderValue::from_static("camera=(), microphone=(), geolocation=()"),
        ))
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::header::CONTENT_SECURITY_POLICY,
            axum::http::HeaderValue::from_static(
                "default-src 'self'; script-src 'self' 'wasm-unsafe-eval' 'unsafe-inline' https://pagead2.googlesyndication.com https://www.googletagservices.com https://adservice.google.com https://ep1.adtrafficquality.google https://ep2.adtrafficquality.google;                  style-src 'self' 'unsafe-inline' https://fonts.bunny.net;                  font-src 'self' https://fonts.bunny.net;                  img-src 'self' data: https:; media-src 'self' https:;                  connect-src 'self' wss: ws: https:;                  object-src 'none';                  frame-src 'self' https://*.googlesyndication.com https://*.doubleclick.net https://www.google.com https://ep1.adtrafficquality.google https://ep2.adtrafficquality.google;                  base-uri 'self'; frame-ancestors 'none'; form-action 'self'",
            ),
        ));

    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!(%bind, "blindall-server listening");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}
