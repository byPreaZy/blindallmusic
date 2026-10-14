use blindall_db::{AuthRepo, ChatRepo, DbGameStore, PlaylistRepo, ProfileRepo, RoomRepo, TeamRepo, VoteRepo};
use blindall_game::GameManager;
use blindall_providers::SharedProviders;
use blindall_redis::{RoomPresenceStore, RoundScoreStore};
use blindall_realtime::Hub;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub manager: GameManager,
    pub store: Arc<DbGameStore>,
    pub auth: AuthRepo,
    pub playlists: PlaylistRepo,
    pub rooms: RoomRepo,
    pub chat: ChatRepo,
    /// `ChatModerationService` in-memory caches.
    pub chat_moderation: std::sync::Arc<crate::chat::ChatModeration>,
    pub teams: TeamRepo,
    pub profiles: ProfileRepo,
    pub votes: VoteRepo,
    pub scores: Arc<RoundScoreStore>,
    pub presence: Arc<RoomPresenceStore>,
    pub hub: Hub,
    /// Multi-provider search aggregator.
    pub providers: SharedProviders,
    /// Fallback HTTP client for the authz/room checks.
    pub http: reqwest::Client,
    /// `RateLimiter` equivalent for login: 5 attempts / 60s on email|ip.
    pub throttle: std::sync::Arc<crate::auth_session::LoginThrottle>,
    /// `config/guests.php` limits: 3/min, 20/h, 50/day per IP.
    pub guest_throttle: std::sync::Arc<crate::auth_session::GuestThrottle>,
    /// `UpdateUserLevel` dispatch (dedup 120s per user, async recalc).
    pub levels: std::sync::Arc<crate::levels::LevelDispatcher>,
    /// `PasswordBroker` — `password_reset_tokens` + HMAC token gen.
    pub password_resets: blindall_db::PasswordResetRepo,
    /// Brevo transactional mail config.
    pub mail: crate::mail::MailConfig,
    /// `APP_KEY` — signs email-verification URLs.
    pub app_key: String,
    /// Minigames: `TrackPickerService` + `MinigameScoreService`.
    pub minigames: blindall_db::MinigameRepo,
    /// `Cache` for anagram questions (one-shot, 300s TTL).
    pub anagram_questions: std::sync::Arc<crate::minigames::AnagramQuestions>,
    /// Moderation (bans, trashed messages, user management).
    pub moderation: blindall_db::ModerationRepo,
    /// Admin ops (user CRUD, maintenance).
    pub admin: blindall_db::AdminRepo,
    /// Public FAQ reads + votes.
    pub faqs: blindall_db::FaqRepo,
    /// Leaderboards (`GlobalLeaderboardService` + friends).
    pub rankings: blindall_db::RankingsRepo,
    /// Laravel `notifications` table (database channel).
    pub notifications: blindall_db::NotificationRepo,
    /// `Cache::remember` equivalent for leaderboard payloads (600s).
    pub rankings_cache: std::sync::Arc<crate::rankings::RankingsCache>,
    /// Local (uploaded) tracks.
    pub local_tracks: blindall_db::LocalTrackRepo,
    /// `Storage::disk()` — local driver (S3 swap-in point).
    pub storage: Arc<dyn crate::storage::Storage>,
    /// `STORAGE_ROOT` — filesystem root served at `storage_prefix`.
    pub storage_root: std::path::PathBuf,
    /// `STORAGE_PUBLIC_PREFIX` — URL prefix for stored files (`/storage`).
    pub storage_prefix: String,
    /// `APP_URL` — absolute base for generated URLs.
    pub app_url: String,
    /// Sitemap queries.
    pub seo: blindall_db::SeoRepo,
    pub home: blindall_db::HomeRepo,
    /// `blindall.homepage_hidden_category_ids` config.
    pub hidden_category_ids: Vec<i64>,
    /// `donations` table access.
    pub donations_repo: blindall_db::DonationRepo,
    /// `config('donations.*')`.
    pub donations: crate::donations::DonationsConfig,
    /// `ADSENSE_CLIENT_ID` (`ca-pub-…`) — empty disables ad slots entirely.
    pub adsense_client: String,
    /// `ADSENSE_SLOT` — default `data-ad-slot` for the generic unit.
    pub adsense_slot: String,
    /// `chat.reaction_emojis` (standard set for all authenticated users).
    pub chat_reaction_emojis: Vec<String>,
    /// SPA static root (`web/dist`) — ServeDir + index.html fallback.
    pub spa_dir: std::path::PathBuf,
    /// Raw Redis client (cache ops: clear-cache, weekly-top-10 write).
    pub redis: fred::clients::Client,
    /// `CACHE_PREFIX` (Laravel `cache.prefix`).
    pub cache_prefix: String,
    /// OAuth provider credentials (services.php config).
    pub oauth: crate::oauth::OAuthConfig,
    /// One-shot OAuth `state` values (session equivalent).
    pub oauth_states: std::sync::Arc<crate::oauth::OAuthStates>,
}
