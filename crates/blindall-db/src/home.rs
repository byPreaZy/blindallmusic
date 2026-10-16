use serde_json::{json, Value};
use sqlx::postgres::PgPool;
use sqlx::Row;
use std::collections::HashMap;

#[derive(Clone)]
pub struct HomeRepo {
    pool: PgPool,
}

#[derive(Debug)]
pub struct CatalogRoom {
    pub id: i64,
    pub user_id: i64,
    pub name: String,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub photo_path: Option<String>,
    pub tracks_by_round: i64,
    pub is_public: bool,
    pub is_autostart: bool,
    pub is_playing: bool,
    pub has_password: bool,
    pub color: Option<String>,
    pub category_id: Option<i64>,
    pub category_name: Option<String>,
    pub owner_name: Option<String>,
    pub owner_photo_path: Option<String>,
    pub rounds_count: i64,
    pub current_track_index: i64,
    pub subscriptions: i64,
    pub tracks_count: Option<i64>,
}

impl HomeRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    const COLS: &'static str = "r.id, r.user_id, r.name, r.slug, r.description, \
        r.photo_path, r.tracks_by_round, r.is_public, r.is_autostart, \
        r.is_playing, r.password, r.color, r.category_id, \
        c.name AS category_name, u.name AS owner_name, u.photo_path AS owner_photo_path, \
        (SELECT COUNT(*) FROM rounds rd WHERE rd.room_id = r.id) AS rounds_count";

    const JOINS: &'static str = "LEFT JOIN categories c ON c.id = r.category_id \
        LEFT JOIN users u ON u.id = r.user_id";

    fn map(r: sqlx::postgres::PgRow) -> CatalogRoom {
        CatalogRoom {
            id: r.get("id"),
            user_id: r.get("user_id"),
            name: r.get("name"),
            slug: r.get("slug"),
            description: r.get("description"),
            photo_path: r.get("photo_path"),
            tracks_by_round: r.get("tracks_by_round"),
            is_public: r.get("is_public"),
            is_autostart: r.get("is_autostart"),
            is_playing: r.get("is_playing"),
            has_password: r.get::<Option<String>, _>("password").is_some(),
            color: r.get("color"),
            category_id: r.get("category_id"),
            category_name: r.get("category_name"),
            owner_name: r.get("owner_name"),
            owner_photo_path: r.get("owner_photo_path"),
            rounds_count: r.get::<i64, _>("rounds_count"),
            current_track_index: 0,
            subscriptions: 0,
            tracks_count: None,
        }
    }

    pub async fn catalog_rooms(
        &self,
        is_public: bool,
    ) -> Result<Vec<CatalogRoom>, sqlx::Error> {
        sqlx::query(&format!(
            "SELECT {} FROM rooms r {} WHERE r.is_public = $1 \
             AND r.password IS NULL AND r.deleted_at IS NULL \
             ORDER BY r.is_playing DESC, r.id",
            Self::COLS,
            Self::JOINS
        ))
        .bind(is_public)
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.into_iter().map(Self::map).collect())
    }

    pub async fn bookmarked_rooms(
        &self,
        user_id: i64,
    ) -> Result<Vec<CatalogRoom>, sqlx::Error> {
        sqlx::query(&format!(
            "SELECT {} FROM rooms r {} \
             JOIN bookmarks b ON b.bookmarkable_id = r.id \
             AND b.bookmarkable_type = 'App\\Models\\Room' \
             WHERE b.user_id = $1 AND r.password IS NULL AND r.deleted_at IS NULL \
             ORDER BY b.created_at DESC",
            Self::COLS,
            Self::JOINS
        ))
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.into_iter().map(Self::map).collect())
    }

    pub async fn moderated_private_rooms(
        &self,
        user_id: i64,
    ) -> Result<Vec<CatalogRoom>, sqlx::Error> {
        sqlx::query(&format!(
            "SELECT {} FROM rooms r {} \
             JOIN moderables m ON m.moderable_id = r.id \
             AND m.moderable_type = 'App\\Models\\Room' \
             WHERE m.user_id = $1 AND r.is_public = false AND r.deleted_at IS NULL \
             ORDER BY r.id",
            Self::COLS,
            Self::JOINS
        ))
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.into_iter().map(Self::map).collect())
    }

    pub async fn search_rooms(&self, search: &str) -> Result<Vec<CatalogRoom>, sqlx::Error> {
        let like = format!("%{search}%");
        sqlx::query(&format!(
            "SELECT {} FROM rooms r {} \
             WHERE r.password IS NULL AND r.deleted_at IS NULL \
             AND EXISTS (SELECT 1 FROM playlist_room pr WHERE pr.room_id = r.id) \
             AND (r.name LIKE $1 OR u.name LIKE $2) \
             ORDER BY r.is_playing DESC, r.is_public DESC, rounds_count DESC \
             LIMIT 20",
            Self::COLS,
            Self::JOINS
        ))
        .bind(&like)
        .bind(&like)
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.into_iter().map(Self::map).collect())
    }

    pub async fn featured_rooms(&self) -> Result<Vec<CatalogRoom>, sqlx::Error> {
        sqlx::query(&format!(
            "SELECT {} FROM rooms r {} \
             WHERE r.is_featured = true AND r.password IS NULL AND r.deleted_at IS NULL \
             ORDER BY r.id",
            Self::COLS,
            Self::JOINS
        ))
        .fetch_all(&self.pool)
        .await
        .map(|rows| rows.into_iter().map(Self::map).collect())
    }

    pub async fn categories_with_room_counts(
        &self,
        is_public: bool,
    ) -> Result<Vec<Value>, sqlx::Error> {
        sqlx::query(
            "SELECT c.id, c.name, COUNT(r.id) AS rooms_count \
             FROM categories c JOIN rooms r ON r.category_id = c.id \
             WHERE r.is_public = $1 AND r.password IS NULL AND r.deleted_at IS NULL \
             GROUP BY c.id, c.name ORDER BY c.id",
        )
        .bind(is_public)
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|r| {
                    json!({
                        "id": r.get::<i64, _>("id"),
                        "name": r.get::<String, _>("name"),
                        "rooms_count": r.get::<i64, _>("rooms_count"),
                    })
                })
                .collect()
        })
    }

    pub async fn live_track_indices(
        &self,
        room_ids: &[i64],
    ) -> Result<HashMap<i64, i64>, sqlx::Error> {
        if room_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut out = HashMap::new();
        for r in sqlx::query(
            "SELECT rd.room_id, rd.current FROM rounds rd \
             JOIN rooms rm ON rm.id = rd.room_id \
             WHERE rd.room_id = ANY($1) AND rm.is_playing \
             AND rd.is_playing AND rd.finished_at IS NULL \
             ORDER BY rd.id DESC",
        )
        .bind(room_ids.to_vec())
        .fetch_all(&self.pool)
        .await?
        {
            let room_id: i64 = r.get("room_id");
            out.entry(room_id).or_insert_with(|| r.get::<i64, _>("current"));
        }
        Ok(out)
    }

    pub async fn tracks_counts(
        &self,
        room_ids: &[i64],
    ) -> Result<HashMap<i64, i64>, sqlx::Error> {
        if room_ids.is_empty() {
            return Ok(HashMap::new());
        }
        let mut out = HashMap::new();
        for r in sqlx::query(
            "SELECT pr.room_id, COUNT(t.id) AS n FROM playlist_room pr \
             JOIN tracks t ON t.playlist_id = pr.playlist_id \
             WHERE pr.room_id = ANY($1) GROUP BY pr.room_id",
        )
        .bind(room_ids.to_vec())
        .fetch_all(&self.pool)
        .await?
        {
            out.insert(r.get::<i64, _>("room_id"), r.get::<i64, _>("n"));
        }
        Ok(out)
    }

    pub async fn weekly_top_users(&self) -> Result<Vec<Value>, sqlx::Error> {
        sqlx::query(
            "SELECT rs.user_id, ROUND(SUM(rs.total_score)::numeric, 1)::float8 AS total_score, \
             u.name, u.photo_path, u.elo \
             FROM round_standings rs \
             JOIN rounds rd ON rs.round_id = rd.id \
             JOIN rooms rm ON rs.room_id = rm.id \
             JOIN users u ON u.id = rs.user_id \
             WHERE rm.is_public = true AND u.is_guest = false AND rs.created_at >= now() - interval '7 days' \
             GROUP BY rs.user_id, u.name, u.photo_path, u.elo \
             ORDER BY total_score DESC LIMIT 10",
        )
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.into_iter()
                .map(|r| {
                    let photo_path: Option<String> = r.get("photo_path");
                    json!({
                        "user_id": r.get::<i64, _>("user_id"),
                        "total_score": r.get::<f64, _>("total_score"),
                        "user": {
                            "id": r.get::<i64, _>("user_id"),
                            "name": r.get::<String, _>("name"),
                            "photo": photo_path.map(|p| format!("/storage/{p}")),
                            "elo": r.get::<i64, _>("elo"),
                        },
                    })
                })
                .collect()
        })
    }
}
