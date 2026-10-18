use blindall_game::GameManager;
use sqlx::PgPool;
use std::path::{Component, Path, PathBuf};

const SWEEP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3600);

pub fn spawn(pool: PgPool, manager: GameManager, storage_root: PathBuf) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(SWEEP_INTERVAL);
        loop {
            tick.tick().await;
            sweep_once(&pool, &manager, &storage_root).await;
        }
    });
}

async fn sweep_once(pool: &PgPool, manager: &GameManager, storage_root: &Path) {
    let live = manager.live_room_ids().await;
    match blindall_db::maintenance::reset_stuck_rounds(pool, &live).await {
        Ok(n) if n > 0 => tracing::info!(rooms = n, "sweeper: reset stuck rounds"),
        Err(e) => tracing::warn!(error = %e, "sweeper: reset_stuck_rounds failed"),
        _ => {}
    }
    match blindall_db::maintenance::purge_old_messages(pool).await {
        Ok(n) if n > 0 => tracing::info!(deleted = n, "sweeper: purged old messages"),
        Err(e) => tracing::warn!(error = %e, "sweeper: purge_old_messages failed"),
        _ => {}
    }
    match blindall_db::maintenance::delete_scoreless_rounds(pool).await {
        Ok(n) if n > 0 => tracing::info!(deleted = n, "sweeper: deleted scoreless rounds"),
        Err(e) => tracing::warn!(error = %e, "sweeper: delete_scoreless_rounds failed"),
        _ => {}
    }
    match blindall_db::maintenance::delete_empty_playlists(pool).await {
        Ok(n) if n > 0 => tracing::info!(deleted = n, "sweeper: deleted empty playlists"),
        Err(e) => tracing::warn!(error = %e, "sweeper: delete_empty_playlists failed"),
        _ => {}
    }
    match blindall_db::maintenance::dead_room_ids(pool).await {
        Ok(rooms) => {
            for (id, photo_path) in &rooms {
                if let Some(p) = photo_path {
                    delete_stored_file(storage_root, p).await;
                }
                if let Err(e) = blindall_db::maintenance::purge_dead_room(pool, *id).await {
                    tracing::warn!(error = %e, room = id, "sweeper: purge_dead_room failed");
                }
            }
            if !rooms.is_empty() {
                tracing::info!(deleted = rooms.len(), "sweeper: force-deleted rooms");
            }
        }
        Err(e) => tracing::warn!(error = %e, "sweeper: dead_room_ids failed"),
    }
    match blindall_db::maintenance::purge_expired_guests(pool).await {
        Ok(n) if n > 0 => tracing::info!(deleted = n, "sweeper: purged expired guests"),
        Err(e) => tracing::warn!(error = %e, "sweeper: purge_expired_guests failed"),
        _ => {}
    }
}

async fn delete_stored_file(storage_root: &Path, path: &str) {
    let rel = Path::new(path);
    if rel.components().any(|c| {
        matches!(
            c,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return;
    }
    let _ = tokio::fs::remove_file(storage_root.join(rel)).await;
}
