use sqlx::PgPool;

pub async fn reset_stuck_rounds(pool: &PgPool, live_room_ids: &[i64]) -> Result<u64, sqlx::Error> {
    let room_ids: Vec<i64> = sqlx::query_scalar(
        "UPDATE rounds SET finished_at = now(), is_playing = false \
         WHERE is_playing = true \
           AND created_at <= now() - interval '1 hour' \
           AND NOT (room_id = ANY($1)) \
         RETURNING room_id",
    )
    .bind(live_room_ids)
    .fetch_all(pool)
    .await?;
    if room_ids.is_empty() {
        return Ok(0);
    }
    sqlx::query("UPDATE rooms SET is_playing = false WHERE id = ANY($1)")
        .bind(&room_ids)
        .execute(pool)
        .await?;
    Ok(room_ids.len() as u64)
}

pub async fn purge_old_messages(pool: &PgPool) -> Result<u64, sqlx::Error> {
    sqlx::query("DELETE FROM messages WHERE created_at <= now() - interval '15 days'")
        .execute(pool)
        .await
        .map(|r| r.rows_affected())
}

pub async fn delete_scoreless_rounds(pool: &PgPool) -> Result<u64, sqlx::Error> {
    sqlx::query(
        "DELETE FROM rounds \
         WHERE created_at <= now() - interval '10 days' \
           AND NOT EXISTS (SELECT 1 FROM scores s WHERE s.round_id = rounds.id)",
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
}

pub async fn delete_empty_playlists(pool: &PgPool) -> Result<u64, sqlx::Error> {
    sqlx::query(
        "DELETE FROM playlists \
         WHERE created_at <= now() - interval '30 days' \
           AND NOT EXISTS (SELECT 1 FROM tracks t WHERE t.playlist_id = playlists.id)",
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
}

pub async fn dead_room_ids(pool: &PgPool) -> Result<Vec<(i64, Option<String>)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, photo_path::text FROM rooms \
         WHERE deleted_at IS NOT NULL \
            OR (created_at <= now() - interval '30 days' \
                AND NOT EXISTS (SELECT 1 FROM playlist_room pr WHERE pr.room_id = rooms.id))",
    )
    .fetch_all(pool)
    .await
}

pub async fn purge_dead_room(pool: &PgPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM rounds WHERE room_id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    sqlx::query(
        "DELETE FROM moderables \
         WHERE moderable_type = 'App\\Models\\Room' AND moderable_id = $1",
    )
    .bind(id)
    .execute(pool)
    .await?;
    sqlx::query("DELETE FROM rooms WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn purge_expired_guests(pool: &PgPool) -> Result<u64, sqlx::Error> {
    sqlx::query("DELETE FROM users WHERE is_guest = true AND created_at < now() - interval '1 day'")
        .execute(pool)
        .await
        .map(|r| r.rows_affected())
}
