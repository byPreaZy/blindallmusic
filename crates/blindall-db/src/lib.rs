//! blindall-db — PostgreSQL persistence via sqlx against the existing schema.

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
pub mod seo;
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
pub use seo::SeoRepo;
pub use teams::{TeamRepo, TeamRow, TEAM_SEATS};
pub use votes::VoteRepo;

pub async fn connect(database_url: &str) -> Result<sqlx::postgres::PgPool, sqlx::Error> {
    sqlx::postgres::PgPoolOptions::new()
        .max_connections(50)
        .acquire_timeout(std::time::Duration::from_secs(60))
        .connect(database_url)
        .await
}

/// SQL schema lives in `blindall-rs/migrations/` and is embedded at compile
/// time (the `sqlx::migrate!` macro needs proc-macro features we disable for
/// the Windows toolchain — same result via `Migration::new`).
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
    ];

    sqlx::migrate::Migrator {
        migrations: migrations.into(),
        ..sqlx::migrate::Migrator::DEFAULT
    }
}

/// Applies pending SQL migrations. sqlx records them in `_sqlx_migrations`
/// and takes a lock so concurrent server replicas stay safe.
///
/// Migration 3 (`demo_data`) used to be embedded conditionally on `SEED_DEMO`,
/// which crashed any boot where the flag went off after the migration had
/// been applied ("previously applied but is missing"). The demo seed is now a
/// runtime step ([`seed_demo`]) — the stray tracking row is dropped here so
/// previously-seeded databases keep booting.
pub async fn migrate(pool: &sqlx::postgres::PgPool) -> anyhow::Result<()> {
    sqlx::query(
        "DELETE FROM _sqlx_migrations WHERE version = 3 AND description = 'demo_data'",
    )
    .execute(pool)
    .await
    .ok(); // table may not exist yet on a fresh database
    migrations().run(pool).await?;
    Ok(())
}

/// `SEED_DEMO=1` (local dev only) — playable room + tracks so gameplay can be
/// exercised end-to-end without external providers. Idempotent (`NOT EXISTS`
/// guards) and never part of the sqlx chain. Never call in prod.
pub async fn seed_demo(pool: &sqlx::postgres::PgPool) -> anyhow::Result<()> {
    sqlx::raw_sql(include_str!("../../../migrations/0003_demo_data.sql"))
        .execute(pool)
        .await?;
    Ok(())
}
