pub mod admin;
pub mod auth;
pub mod chat;
pub mod donations;
pub mod entities;
pub mod faqs;
pub mod game_store;
pub mod home;
pub mod levels;
pub mod local_tracks;
pub mod maintenance;
pub mod minigames;
pub mod moderation;
pub mod notifications;
pub mod password_reset;
pub mod playlists;
pub mod profiles;
pub mod rankings;
pub mod rooms;
pub mod seasons;
pub mod seo;
pub mod subscriptions;
pub mod teams;
pub mod votes;

pub use admin::AdminRepo;
pub use auth::AuthRepo;
pub use chat::ChatRepo;
pub use donations::DonationRepo;
pub use faqs::FaqRepo;
pub use game_store::DbGameStore;
pub use home::{CatalogRoom, HomeRepo};
pub use levels::{LevelRepo, LevelTrigger};
pub use local_tracks::LocalTrackRepo;
pub use minigames::MinigameRepo;
pub use moderation::ModerationRepo;
pub use notifications::NotificationRepo;
pub use password_reset::PasswordResetRepo;
pub use playlists::{PlaylistRepo, PlaylistRow};
pub use profiles::ProfileRepo;
pub use rankings::RankingsRepo;
pub use rooms::{RoomOptions, RoomRepo, RoomRow};
pub use seasons::SeasonRepo;
pub use subscriptions::SubscriptionRepo;
pub use seo::SeoRepo;
pub use teams::{TEAM_SEATS, TeamRepo, TeamRow};
pub use votes::VoteRepo;

/// Single source of truth for the paid-perk `custom_tag` gate: the tag is
/// only served when the user is an administrator, a current-month donor
/// (`dn.month_key = $2`), or holds an active/trialing/past_due
/// subscription. Stored tags survive subscription loss — reads mask them.
/// Every SELECT that exposes `u.custom_tag` must embed this fragment or
/// the gates drift.
pub const CUSTOM_TAG_CASE: &str = "CASE WHEN u.is_administrator \
    OR EXISTS(SELECT 1 FROM donations dn WHERE dn.user_id = u.id AND dn.month_key = $2) \
    OR EXISTS(SELECT 1 FROM user_subscriptions s WHERE s.user_id = u.id \
      AND s.status IN ('active','trialing','past_due') \
      AND (s.current_period_end IS NULL OR s.current_period_end > now())) \
THEN u.custom_tag ELSE '' END AS custom_tag";

pub async fn connect(database_url: &str) -> Result<sqlx::postgres::PgPool, sqlx::Error> {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(50)
        .acquire_timeout(std::time::Duration::from_secs(60))
        .connect(database_url)
        .await
}

fn migrations() -> sqlx::migrate::Migrator {
    let migrations = vec![
        sqlx::migrate::Migration::new(
            1,
            "initial_schema".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0001_initial_schema.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            2,
            "seed_data".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0002_seed_data.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            4,
            "rls".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0004_rls.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            5,
            "reset_sequences".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0005_reset_sequences.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            6,
            "derive_track_answers".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0006_derive_track_answers.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            7,
            "track_answers_track_id_index".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0007_track_answers_track_id_index.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            8,
            "drop_facebook_data_deletion".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0008_drop_facebook_data_deletion.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            9,
            "fix_media_answers".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0009_fix_media_answers.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            10,
            "media_extraction_pass2".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0010_media_extraction_pass2.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            11,
            "media_aliases".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0011_media_aliases.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            12,
            "cleanup_featuring".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0012_cleanup_featuring.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            13,
            "media_extraction_pass3".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0013_media_extraction_pass3.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            14,
            "media_extraction_pass4".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0014_media_extraction_pass4.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            15,
            "media_fill_null".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0015_media_fill_null.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            16,
            "seasons".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0016_seasons.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            17,
            "seasons_unique".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0017_seasons_unique.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            18,
            "user_custom_tag".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0018_user_custom_tag.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            19,
            "rooms_active_default".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0019_rooms_active_default.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            20,
            "data_quality_audit".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0020_data_quality_audit.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            21,
            "track_enrichment".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0021_track_enrichment.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            22,
            "minigame_duels_daily".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0022_minigame_duels_daily.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            23,
            "level_metrics_backfill".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0023_level_metrics_backfill.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            24,
            "remove_free_volunteer_copy".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0024_remove_free_volunteer_copy.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            25,
            "room_venue_name".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0025_room_venue_name.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            26,
            "performance_indexes".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0026_performance_indexes.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            27,
            "tracks_provider_unique".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0027_tracks_provider_unique.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            28,
            "resync_serial_sequences".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0028_resync_serial_sequences.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            29,
            "user_subscriptions".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0029_user_subscriptions.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            30,
            "remove_em_dashes".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0030_remove_em_dashes.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            31,
            "legal_commercial".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0031_legal_commercial.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            32,
            "privacy_smtp_correction".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0032_privacy_smtp_correction.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            33,
            "legal_identity".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0033_legal_identity.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            34,
            "legal_address_no_mediator".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0034_legal_address_no_mediator.sql").into(),
            false,
        ),
        sqlx::migrate::Migration::new(
            35,
            "update_cgu".into(),
            sqlx::migrate::MigrationType::Simple,
            include_str!("../../../migrations/0035_update_cgu.sql").into(),
            false,
        ),
    ];

    sqlx::migrate::Migrator {
        migrations: migrations.into(),
        ..sqlx::migrate::Migrator::DEFAULT
    }
}

pub async fn migrate(pool: &sqlx::postgres::PgPool) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 3 AND description = 'demo_data'")
        .execute(pool)
        .await
        .ok(); // table may not exist yet on a fresh database
    migrations().run(pool).await?;
    Ok(())
}

pub async fn seed_demo(pool: &sqlx::postgres::PgPool) -> anyhow::Result<()> {
    sqlx::raw_sql(include_str!("../../../migrations/0003_demo_data.sql"))
        .execute(pool)
        .await?;
    Ok(())
}
