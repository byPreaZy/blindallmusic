use serde_json::{Value, json};
use sqlx::Row;
use sqlx::postgres::PgPool;

#[derive(Clone)]
pub struct LocalTrackRepo {
    pool: PgPool,
}

impl LocalTrackRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn name_taken(
        &self,
        artist_name: &str,
        track_name: &str,
        ignore_id: Option<i64>,
    ) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM local_tracks \
             WHERE track_name = $1 AND artist_name = $2 AND ($3 IS NULL OR id != $4))::int::bigint",
        )
        .bind(track_name)
        .bind(artist_name)
        .bind(ignore_id)
        .bind(ignore_id)
        .fetch_one(&self.pool)
        .await?
            == 1)
    }

    pub async fn create(
        &self,
        artist_name: &str,
        track_name: &str,
        audio_path: &str,
        artwork_path: &str,
        user_id: i64,
    ) -> Result<i64, sqlx::Error> {
        let now = chrono::Utc::now();
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO local_tracks (artist_name, track_name, audio_path, \
             artwork_path, user_id, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
        )
        .bind(artist_name)
        .bind(track_name)
        .bind(audio_path)
        .bind(artwork_path)
        .bind(user_id)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn search(
        &self,
        term: &str,
    ) -> Result<Vec<(i64, String, String, String)>, sqlx::Error> {
        let like = format!("%{term}%");
        let rows = sqlx::query(
            "SELECT id, artist_name, track_name, artwork_path FROM local_tracks \
             WHERE artist_name LIKE $1 OR track_name LIKE $2 \
             ORDER BY created_at DESC LIMIT 10",
        )
        .bind(&like)
        .bind(&like)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                (
                    r.get("id"),
                    r.get("artist_name"),
                    r.get("track_name"),
                    r.get("artwork_path"),
                )
            })
            .collect())
    }

    pub async fn paths(&self, id: i64) -> Result<Option<(String, String)>, sqlx::Error> {
        sqlx::query("SELECT audio_path, artwork_path FROM local_tracks WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map(|o| o.map(|r| (r.get("audio_path"), r.get("artwork_path"))))
    }

    pub async fn paginate(
        &self,
        search: Option<&str>,
        sort_by: &str,
        sort_dir: &str,
        page: i64,
        per_page: i64,
    ) -> Result<(Vec<Value>, i64), sqlx::Error> {
        let sort = match sort_by {
            "track_name" | "artist_name" | "playlists_usage_count" => sort_by,
            _ => "created_at",
        };
        let dir = if sort_dir == "asc" { "ASC" } else { "DESC" };
        let like = search.map(|s| format!("%{s}%"));
        let numeric_term: Option<i64> = search
            .map(str::trim)
            .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|s| s.parse().ok());

        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM local_tracks lt \
             WHERE ($1 IS NULL OR lt.artist_name LIKE $2 OR lt.track_name LIKE $3 \
             OR ($4::bigint IS NOT NULL AND (lt.id = $4 OR lt.user_id = $4)))",
        )
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .bind(numeric_term)
        .fetch_one(&self.pool)
        .await?;

        let sql = format!(
            "SELECT lt.id, lt.artist_name, lt.track_name, lt.audio_path, \
             lt.artwork_path, lt.created_at, u.id AS user_id, u.name AS user_name, \
             (SELECT COUNT(*) FROM tracks t \
              WHERE t.provider = 'local' AND t.provider_id = lt.id::text) AS playlists_usage_count \
             FROM local_tracks lt \
             LEFT JOIN users u ON u.id = lt.user_id \
             WHERE ($1 IS NULL OR lt.artist_name LIKE $2 OR lt.track_name LIKE $3 \
             OR ($6::bigint IS NOT NULL AND (lt.id = $6 OR lt.user_id = $6))) \
             ORDER BY {sort} {dir} LIMIT $4 OFFSET $5"
        );
        let rows = sqlx::query(&sql)
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .bind(per_page)
            .bind((page.max(1) - 1) * per_page)
            .bind(numeric_term)
            .fetch_all(&self.pool)
            .await?;
        Ok((
            rows.iter()
                .map(|r| {
                    json!({
                        "id": r.get::<i64, _>("id"),
                        "artist_name": r.get::<String, _>("artist_name"),
                        "track_name": r.get::<String, _>("track_name"),
                        "audio_path": r.get::<String, _>("audio_path"),
                        "artwork_path": r.get::<String, _>("artwork_path"),
                        "playlists_usage_count": r.get::<i64, _>("playlists_usage_count"),
                        "user": r.get::<Option<i64>, _>("user_id").map(|uid| json!({
                            "id": uid,
                            "name": r.get::<Option<String>, _>("user_name"),
                        })),
                        "created_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at"),
                    })
                })
                .collect(),
            total,
        ))
    }

    pub async fn linked_track_ids(&self, id: i64) -> Result<Vec<i64>, sqlx::Error> {
        sqlx::query_scalar("SELECT id FROM tracks WHERE provider = 'local' AND provider_id = $1")
            .bind(id.to_string())
            .fetch_all(&self.pool)
            .await
    }

    pub async fn update(
        &self,
        id: i64,
        artist_name: &str,
        track_name: &str,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query(
            "UPDATE local_tracks SET artist_name = $1, track_name = $2, updated_at = $3 \
             WHERE id = $4",
        )
        .bind(artist_name)
        .bind(track_name)
        .bind(chrono::Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map(|r| r.rows_affected() > 0)
    }

    pub async fn delete(&self, id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query("DELETE FROM local_tracks WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected() > 0)
    }
}
