//! Admin — `Admin\UserController` CRUD + dashboard maintenance ops
//! (`rounds:force-clear`, `topusers:weekly`).

use serde_json::{json, Value};
use sqlx::postgres::PgPool;
use sqlx::Row;

#[derive(Clone)]
pub struct AdminRepo {
    pool: PgPool,
}

impl AdminRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// `index` — filterable user list (search/role/trashed), `format('d/m/Y H:i')`
    /// done client-side; we return ISO.
    pub async fn users(
        &self,
        search: Option<&str>,
        role: Option<&str>,
        trashed: Option<&str>,
        page: i64,
        per_page: i64,
    ) -> Result<crate::moderation::Page, sqlx::Error> {
        let like = search.map(|s| format!("%{s}%"));
        // `role` filter: 'admin' | 'user' | 'guest' (PHP filter scope).
        let (admin_only, guests_only) = match role {
            Some("admin") => (true, false),
            Some("guest") => (false, true),
            _ => (false, false),
        };
        // `trashed`: 'with' | 'only' | null (exclude).
        let trashed_clause = match trashed {
            Some("with") => "",
            Some("only") => "AND u.deleted_at IS NOT NULL",
            _ => "AND u.deleted_at IS NULL",
        };
        let sql = format!(
            "SELECT u.id, u.name, u.provider, u.is_administrator, u.email, \
             u.photo_path, u.created_at, u.deleted_at, u.team_id, \
             t.name AS team_name FROM users u \
             LEFT JOIN teams t ON t.id = u.team_id \
             WHERE 1=1 {trashed} \
             AND ($1 IS NULL OR u.name LIKE $2 OR u.email LIKE $3) \
             AND ($4 = 0 OR u.is_administrator = true) \
             AND ($5 = 0 OR u.is_guest = true) \
             ORDER BY u.created_at DESC LIMIT $6 OFFSET $7",
            trashed = trashed_clause
        );
        let count_sql = format!(
            "SELECT COUNT(*) FROM users u WHERE 1=1 {trashed} \
             AND ($1 IS NULL OR u.name LIKE $2 OR u.email LIKE $3) \
             AND ($4 = 0 OR u.is_administrator = true) \
             AND ($5 = 0 OR u.is_guest = true)",
            trashed = trashed_clause
        );
        let offset = (page.max(1) - 1) * per_page.max(1);
        let rows = sqlx::query(&sql)
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .bind(admin_only as i64)
            .bind(guests_only as i64)
            .bind(per_page)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let total: i64 = sqlx::query_scalar(&count_sql)
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .bind(admin_only as i64)
            .bind(guests_only as i64)
            .fetch_one(&self.pool)
            .await?;
        let items = rows
            .iter()
            .map(|r| {
                json!({
                    "id": r.get::<i64, _>("id"),
                    "name": r.get::<String, _>("name"),
                    "provider": r.get::<Option<String>, _>("provider"),
                    "is_administrator": r.get::<bool, _>("is_administrator"),
                    "email": r.get::<Option<String>, _>("email"),
                    "team": r.get::<Option<String>, _>("team_name")
                        .map(|n| json!({"id": r.get::<Option<i64>, _>("team_id"), "name": n})),
                    "photo": r.get::<Option<String>, _>("photo_path"),
                    "created_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at"),
                    "deleted_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("deleted_at"),
                })
            })
            .collect();
        Ok(crate::moderation::Page {
            items,
            total,
            page,
            per_page,
        })
    }

    /// `store` — admin creates a user (bcrypt password, is_administrator flag).
    pub async fn create_user(
        &self,
        name: &str,
        email: &str,
        password: Option<&str>,
        is_administrator: bool,
    ) -> Result<i64, sqlx::Error> {
        let hash = password
            .map(|p| bcrypt::hash(p, bcrypt::DEFAULT_COST).ok())
            .flatten();
        let now = chrono::Utc::now();
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO users (name, email, password, is_administrator, is_guest, \
             created_at, updated_at) VALUES ($1, $2, $3, $4, false, $5, $6) RETURNING id",
        )
        .bind(name)
        .bind(email)
        .bind(hash)
        .bind(is_administrator)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        
    }

    /// `update` — name/email/team_id/is_administrator (+optional password).
    pub async fn update_user(
        &self,
        user_id: i64,
        name: &str,
        email: &str,
        team_id: Option<i64>,
        is_administrator: bool,
        password: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        let res = sqlx::query(
            "UPDATE users SET name = $1, email = $2, team_id = $3, is_administrator = $4, \
             updated_at = $5 WHERE id = $6",
        )
        .bind(name)
        .bind(email)
        .bind(team_id)
        .bind(is_administrator)
        .bind(chrono::Utc::now())
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        if let Some(p) = password {
            if !p.is_empty() {
                let hash = bcrypt::hash(p, bcrypt::DEFAULT_COST)
                    .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
                sqlx::query("UPDATE users SET password = $1 WHERE id = $2")
                    .bind(hash)
                    .bind(user_id)
                    .execute(&self.pool)
                    .await?;
            }
        }
        Ok(res.rows_affected() > 0)
    }

    /// `destroy` / `restore` — soft delete + restore.
    pub async fn delete_user(&self, user_id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query("UPDATE users SET deleted_at = $1 WHERE id = $2")
            .bind(chrono::Utc::now())
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected() > 0)
    }

    pub async fn restore_user(&self, user_id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query("UPDATE users SET deleted_at = NULL WHERE id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected() > 0)
    }

    /// `rounds:force-clear` — stop playing rooms + close unfinished rounds.
    /// (In-memory actors should be stopped too — the API layer sends Stop.)
    pub async fn force_clear_rounds(&self) -> Result<(u64, u64), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let rooms = sqlx::query("UPDATE rooms SET is_playing = false WHERE is_playing = true")
            .execute(&mut *tx)
            .await?
            .rows_affected();
        let rounds = sqlx::query(
            "UPDATE rounds SET finished_at = $1, is_playing = false WHERE finished_at IS NULL",
        )
        .bind(chrono::Utc::now())
        .execute(&mut *tx)
        .await?
        .rows_affected();
        tx.commit().await?;
        Ok((rooms, rounds))
    }

    /// `topusers:weekly` — top 10 public-room standings over 7 days.
    /// Caller writes the result to `{prefix}weekly-top-10-users` (php-ser).
    pub async fn weekly_top_users(&self) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT rs.user_id, ROUND(SUM(rs.total_score)::numeric, 1)::float8 AS total_score, \
             u.id AS uid, u.name, u.photo_path, u.elo \
             FROM round_standings rs \
             JOIN rounds r ON rs.round_id = r.id \
             JOIN rooms rm ON rs.room_id = rm.id AND rm.is_public = true \
             JOIN users u ON u.id = rs.user_id \
             WHERE rs.created_at >= now() - interval '7 days' \
             GROUP BY rs.user_id, u.id, u.name, u.photo_path, u.elo \
             ORDER BY total_score DESC LIMIT 10",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                json!({
                    "user_id": r.get::<i64, _>("user_id"),
                    "total_score": r.get::<f64, _>("total_score"),
                    "user": {
                        "id": r.get::<i64, _>("uid"),
                        "name": r.get::<String, _>("name"),
                        "photo": r.get::<Option<String>, _>("photo_path"),
                        "elo": r.get::<i64, _>("elo"),
                    },
                })
            })
            .collect())
    }

    // ---- content CRUD (categories / answer_types / pages / faqs) -------------

    /// `Admin\CategoryController::index` — categories + room counts.
    pub async fn categories(&self) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT c.id, c.name, c.slug, c.created_at, \
             (SELECT COUNT(*) FROM rooms r WHERE r.category_id = c.id AND r.is_public = true) AS public_rooms_count, \
             (SELECT COUNT(*) FROM rooms r WHERE r.category_id = c.id AND r.is_public = false) AS private_rooms_count \
             FROM categories c ORDER BY c.name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                json!({
                    "id": r.get::<i64, _>("id"),
                    "name": r.get::<String, _>("name"),
                    "slug": r.get::<Option<String>, _>("slug"),
                    "public_rooms_count": r.get::<i64, _>("public_rooms_count"),
                    "private_rooms_count": r.get::<i64, _>("private_rooms_count"),
                })
            })
            .collect())
    }

    pub async fn category_name_taken(&self, name: &str, ignore_id: Option<i64>) -> bool {
        let row = sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM categories WHERE name = $1 AND ($2 IS NULL OR id != $3))::int::bigint",
        )
        .bind(name)
        .bind(ignore_id)
        .bind(ignore_id)
        .fetch_one(&self.pool)
        .await
        .unwrap_or(1);
        row > 0
    }

    /// `store` — name + `Str::slug`.
    pub async fn create_category(&self, name: &str) -> Result<i64, sqlx::Error> {
        let now = chrono::Utc::now();
        sqlx::query_scalar::<_, i64>("INSERT INTO categories (name, slug, created_at, updated_at) VALUES ($1, $2, $3, $4) RETURNING id")
            .bind(name)
            .bind(blindall_core::sanitize::str_slug(name, '-'))
            .bind(now)
            .bind(now)
            .fetch_one(&self.pool)
            .await
            
    }

    pub async fn update_category(&self, id: i64, name: &str) -> Result<bool, sqlx::Error> {
        sqlx::query("UPDATE categories SET name = $1, slug = $2, updated_at = $3 WHERE id = $4")
            .bind(name)
            .bind(blindall_core::sanitize::str_slug(name, '-'))
            .bind(chrono::Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected() > 0)
    }

    pub async fn delete_category(&self, id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query("DELETE FROM categories WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected() > 0)
    }

    /// `answer_types` CRUD.
    pub async fn answer_type_create(
        &self,
        name: &str,
        pronoun: Option<&str>,
        svg_icon: Option<&str>,
    ) -> Result<i64, sqlx::Error> {
        let now = chrono::Utc::now();
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO answer_types (name, pronoun, svg_icon, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
        )
        .bind(name)
        .bind(pronoun)
        .bind(svg_icon)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        
    }

    pub async fn answer_type_update(
        &self,
        id: i64,
        name: &str,
        pronoun: Option<&str>,
        svg_icon: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query(
            "UPDATE answer_types SET name = $1, pronoun = $2, svg_icon = $3, updated_at = $4 \
             WHERE id = $5",
        )
        .bind(name)
        .bind(pronoun)
        .bind(svg_icon)
        .bind(chrono::Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map(|r| r.rows_affected() > 0)
    }

    pub async fn answer_type_delete(&self, id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query("DELETE FROM answer_types WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected() > 0)
    }

    /// `pages` — versioned: update creates a new revision (`original_id`).
    pub async fn pages(&self) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, title, slug, original_id, parent_id, revised_at, created_at \
             FROM pages ORDER BY title",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                json!({
                    "id": r.get::<i64, _>("id"),
                    "title": r.get::<String, _>("title"),
                    "slug": r.get::<String, _>("slug"),
                    "original_id": r.get::<Option<i64>, _>("original_id"),
                    "parent_id": r.get::<Option<i64>, _>("parent_id"),
                    "revised_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("revised_at"),
                })
            })
            .collect())
    }

    /// `store`/`update` share the same INSERT — updates become revisions.
    pub async fn create_page(
        &self,
        title: &str,
        content: &str,
        original_id: Option<i64>,
        parent_id: Option<i64>,
    ) -> Result<i64, sqlx::Error> {
        let now = chrono::Utc::now();
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO pages (title, slug, content, original_id, parent_id, \
             revised_at, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
        )
        .bind(title)
        .bind(blindall_core::sanitize::str_slug(title, '-'))
        .bind(content)
        .bind(original_id)
        .bind(parent_id)
        .bind(now)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        
    }

    pub async fn page_original(&self, id: i64) -> Result<Option<Option<i64>>, sqlx::Error> {
        sqlx::query_scalar("SELECT original_id FROM pages WHERE id = $1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    /// `Admin\PageController::store` validation — `unique:pages` on title.
    pub async fn page_title_taken(&self, title: &str) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM pages WHERE title = $1)::int::bigint",
        )
        .bind(title)
        .fetch_one(&self.pool)
        .await
        .map(|v| v == 1)
    }

    /// `PageController::show` — latest revision by slug (public).
    pub async fn page_by_slug(&self, slug: &str) -> Result<Option<Value>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id, title, slug, content, parent_id, revised_at, updated_at \
             FROM pages WHERE slug = $1 ORDER BY revised_at DESC LIMIT 1",
        )
        .bind(slug)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| {
            json!({
                "id": r.get::<i64, _>("id"),
                "title": r.get::<String, _>("title"),
                "slug": r.get::<String, _>("slug"),
                "content": r.get::<String, _>("content"),
                "parent_id": r.get::<Option<i64>, _>("parent_id"),
                "revised_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("revised_at"),
                "updated_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("updated_at"),
            })
        }))
    }

    /// `faqs` CRUD.
    pub async fn faqs(&self) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, locale, question, answer, created_at FROM faqs ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                json!({
                    "id": r.get::<i64, _>("id"),
                    "locale": r.get::<String, _>("locale"),
                    "question": r.get::<String, _>("question"),
                    "answer": r.get::<String, _>("answer"),
                })
            })
            .collect())
    }

    pub async fn create_faq(
        &self,
        locale: &str,
        question: &str,
        answer: &str,
    ) -> Result<i64, sqlx::Error> {
        let now = chrono::Utc::now();
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO faqs (locale, question, answer, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
        )
        .bind(locale)
        .bind(question)
        .bind(answer)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await
        
    }

    pub async fn update_faq(
        &self,
        id: i64,
        locale: &str,
        question: &str,
        answer: &str,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query(
            "UPDATE faqs SET locale = $1, question = $2, answer = $3, updated_at = $4 WHERE id = $5",
        )
        .bind(locale)
        .bind(question)
        .bind(answer)
        .bind(chrono::Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map(|r| r.rows_affected() > 0)
    }

    pub async fn delete_faq(&self, id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query("DELETE FROM faqs WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected() > 0)
    }

    // ---- Admin/Playlists ---------------------------------------------------

    /// `Admin\PlaylistController::index` — all playlists (no SoftDeletes on
    /// playlists upstream; `trashed` filter is a no-op kept for parity),
    /// owner + moderators + tracks_count, ordered by updated_at, paginate 5.
    pub async fn admin_playlists(
        &self,
        search: Option<&str>,
        page: i64,
        per_page: i64,
    ) -> Result<crate::moderation::Page, sqlx::Error> {
        let like = search.map(|s| format!("%{s}%"));
        let offset = (page.max(1) - 1) * per_page.max(1);
        let rows = sqlx::query(
            "SELECT p.id, p.name, p.description, p.is_public, p.user_id, \
             u.name AS owner_name, \
             (SELECT COUNT(*) FROM tracks t WHERE t.playlist_id = p.id) AS tracks_count \
             FROM playlists p JOIN users u ON u.id = p.user_id \
             WHERE ($1 IS NULL OR p.name LIKE $2 OR u.name LIKE $3) \
             ORDER BY p.updated_at LIMIT $4 OFFSET $5",
        )
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .bind(per_page)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM playlists p JOIN users u ON u.id = p.user_id \
             WHERE ($1 IS NULL OR p.name LIKE $2 OR u.name LIKE $3)",
        )
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .fetch_one(&self.pool)
        .await?;
        let mut items = Vec::with_capacity(rows.len());
        for r in &rows {
            let pid: i64 = r.get("id");
            let mods = sqlx::query(
                "SELECT u.id, u.name FROM moderables m JOIN users u ON u.id = m.user_id \
                 WHERE m.moderable_id = $1 AND m.moderable_type = 'App\\Models\\Playlist'",
            )
            .bind(pid)
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|m| json!({"id": m.get::<i64, _>("id"), "name": m.get::<String, _>("name")}))
            .collect::<Vec<_>>();
            items.push(json!({
                "id": pid,
                "name": r.get::<String, _>("name"),
                "description": r.get::<Option<String>, _>("description"),
                "owner": {"id": r.get::<i64, _>("user_id"), "name": r.get::<String, _>("owner_name")},
                "moderators": mods,
                "tracks_count": r.get::<i64, _>("tracks_count"),
                "is_public": r.get::<bool, _>("is_public"),
                "deleted_at": Value::Null,
            }));
        }
        Ok(crate::moderation::Page { items, total, page, per_page })
    }

    /// `Admin\PlaylistController::update` — name + description + is_public.
    pub async fn admin_update_playlist(
        &self,
        id: i64,
        name: &str,
        description: Option<&str>,
        is_public: bool,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query(
            "UPDATE playlists SET name = $1, description = $2, is_public = $3, \
             updated_at = $4 WHERE id = $5",
        )
        .bind(name)
        .bind(description)
        .bind(is_public)
        .bind(chrono::Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await
        .map(|r| r.rows_affected() > 0)
    }

    // ---- Admin/Rooms -------------------------------------------------------

    /// `Admin\RoomController::index` — search/trashed filters, ordered
    /// is_public DESC then name, paginate 5.
    pub async fn admin_rooms(
        &self,
        search: Option<&str>,
        trashed: Option<&str>,
        page: i64,
        per_page: i64,
    ) -> Result<crate::moderation::Page, sqlx::Error> {
        let like = search.map(|s| format!("%{s}%"));
        let trashed_clause = match trashed {
            Some("with") => "",
            Some("only") => "AND r.deleted_at IS NOT NULL",
            _ => "AND r.deleted_at IS NULL",
        };
        let offset = (page.max(1) - 1) * per_page.max(1);
        let sql = format!(
            "SELECT r.id, r.name, r.slug, r.description, r.is_public, r.photo_path, \
             r.user_id, r.deleted_at, u.name AS owner_name \
             FROM rooms r JOIN users u ON u.id = r.user_id \
             WHERE 1=1 {trashed_clause} \
             AND ($1 IS NULL OR r.name LIKE $2 OR u.name LIKE $3) \
             ORDER BY r.is_public DESC, r.name LIMIT $4 OFFSET $5"
        );
        let count_sql = format!(
            "SELECT COUNT(*) FROM rooms r JOIN users u ON u.id = r.user_id \
             WHERE 1=1 {trashed_clause} \
             AND ($1 IS NULL OR r.name LIKE $2 OR u.name LIKE $3)"
        );
        let rows = sqlx::query(&sql)
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .bind(per_page)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let total: i64 = sqlx::query_scalar(&count_sql)
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .fetch_one(&self.pool)
            .await?;
        let mut items = Vec::with_capacity(rows.len());
        for r in &rows {
            let rid: i64 = r.get("id");
            let mods = sqlx::query(
                "SELECT u.id, u.name FROM moderables m JOIN users u ON u.id = m.user_id \
                 WHERE m.moderable_id = $1 AND m.moderable_type = 'App\\Models\\Room'",
            )
            .bind(rid)
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|m| json!({"id": m.get::<i64, _>("id"), "name": m.get::<String, _>("name")}))
            .collect::<Vec<_>>();
            let playlists = sqlx::query(
                "SELECT p.id, p.name FROM playlist_room pr \
                 JOIN playlists p ON p.id = pr.playlist_id WHERE pr.room_id = $1",
            )
            .bind(rid)
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|m| json!({"id": m.get::<i64, _>("id"), "name": m.get::<String, _>("name")}))
            .collect::<Vec<_>>();
            items.push(json!({
                "id": rid,
                "name": r.get::<String, _>("name"),
                "slug": r.get::<Option<String>, _>("slug"),
                "description": r.get::<Option<String>, _>("description"),
                "photo": r.get::<Option<String>, _>("photo_path"),
                "owner": {"id": r.get::<i64, _>("user_id"), "name": r.get::<String, _>("owner_name")},
                "moderators": mods,
                "playlists": playlists,
                "is_public": r.get::<bool, _>("is_public"),
                "deleted_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("deleted_at"),
            }));
        }
        Ok(crate::moderation::Page { items, total, page, per_page })
    }

    /// `Admin\RoomController::update` — full room fields incl. pro/active/
    /// webhook/color (wider than the public `updateOptions`).
    pub async fn admin_update_room(
        &self,
        id: i64,
        name: &str,
        description: Option<&str>,
        category_id: i64,
        tracks_by_round: i64,
        track_duration: i64,
        pause_between_tracks: i64,
        pause_between_rounds: i64,
        is_public: bool,
        is_pro: bool,
        is_random: bool,
        is_active: bool,
        is_chat_active: bool,
        discord_webhook_url: Option<&str>,
        color: Option<&str>,
        password: Option<&str>,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query(
            "UPDATE rooms SET name = $1, description = $2, category_id = $3, \
             tracks_by_round = $4, track_duration = $5, pause_between_tracks = $6, \
             pause_between_rounds = $7, is_public = $8, is_pro = $9, is_random = $10, \
             is_active = $11, is_chat_active = $12, discord_webhook_url = $13, \
             color = $14, updated_at = $15 WHERE id = $16",
        )
        .bind(name)
        .bind(description)
        .bind(category_id)
        .bind(tracks_by_round)
        .bind(track_duration)
        .bind(pause_between_tracks)
        .bind(pause_between_rounds)
        .bind(is_public)
        .bind(is_pro)
        .bind(is_random)
        .bind(is_active)
        .bind(is_chat_active)
        .bind(discord_webhook_url)
        .bind(color)
        .bind(chrono::Utc::now())
        .bind(id)
        .execute(&self.pool)
        .await?;
        if let Some(pwd) = password {
            sqlx::query("UPDATE rooms SET password = $1 WHERE id = $2")
                .bind(pwd)
                .bind(id)
                .execute(&self.pool)
                .await?;
        }
        Ok(true)
    }

    // ---- Admin/Teams ---------------------------------------------------------

    /// `Admin\TeamController::index` — search/trashed filters, `members_count`,
    /// ordered by updated_at, paginate 5.
    pub async fn admin_teams(
        &self,
        search: Option<&str>,
        trashed: Option<&str>,
        page: i64,
        per_page: i64,
    ) -> Result<crate::moderation::Page, sqlx::Error> {
        let like = search.map(|s| format!("%{s}%"));
        let trashed_clause = match trashed {
            Some("with") => "",
            Some("only") => "AND t.deleted_at IS NOT NULL",
            _ => "AND t.deleted_at IS NULL",
        };
        let offset = (page.max(1) - 1) * per_page.max(1);
        let sql = format!(
            "SELECT t.id, t.name, t.user_id, t.photo_path, t.deleted_at, \
             u.name AS owner_name, \
             (SELECT COUNT(*) FROM users m WHERE m.team_id = t.id AND m.deleted_at IS NULL) \
              AS members_count \
             FROM teams t JOIN users u ON u.id = t.user_id \
             WHERE 1=1 {trashed_clause} AND ($1 IS NULL OR t.name LIKE $2) \
             ORDER BY t.updated_at LIMIT $3 OFFSET $4"
        );
        let count_sql = format!(
            "SELECT COUNT(*) FROM teams t WHERE 1=1 {trashed_clause} \
             AND ($1 IS NULL OR t.name LIKE $2)"
        );
        let rows = sqlx::query(&sql)
            .bind(&like)
            .bind(&like)
            .bind(per_page)
            .bind(offset)
            .fetch_all(&self.pool)
            .await?;
        let total: i64 = sqlx::query_scalar(&count_sql)
            .bind(&like)
            .bind(&like)
            .fetch_one(&self.pool)
            .await?;
        let items = rows
            .iter()
            .map(|r| {
                json!({
                    "id": r.get::<i64, _>("id"),
                    "name": r.get::<String, _>("name"),
                    "user_id": r.get::<i64, _>("user_id"),
                    "owner": {"id": r.get::<i64, _>("user_id"), "name": r.get::<String, _>("owner_name")},
                    "photo": r.get::<Option<String>, _>("photo_path"),
                    "members_count": r.get::<i64, _>("members_count"),
                    "deleted_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("deleted_at"),
                })
            })
            .collect();
        Ok(crate::moderation::Page { items, total, page, per_page })
    }

    /// `Admin\TeamController::update` — name + owner (`user_id`).
    pub async fn admin_update_team(
        &self,
        id: i64,
        name: &str,
        user_id: i64,
    ) -> Result<bool, sqlx::Error> {
        sqlx::query("UPDATE teams SET name = $1, user_id = $2, updated_at = $3 WHERE id = $4")
            .bind(name)
            .bind(user_id)
            .bind(chrono::Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected() > 0)
    }

    /// Soft-delete/restore parity (`Team` uses SoftDeletes).
    pub async fn team_set_deleted(&self, id: i64, deleted: bool) -> Result<bool, sqlx::Error> {
        if deleted {
            sqlx::query("UPDATE teams SET deleted_at = $1 WHERE id = $2")
                .bind(chrono::Utc::now())
                .bind(id)
                .execute(&self.pool)
                .await
                .map(|r| r.rows_affected() > 0)
        } else {
            sqlx::query("UPDATE teams SET deleted_at = NULL WHERE id = $1")
                .bind(id)
                .execute(&self.pool)
                .await
                .map(|r| r.rows_affected() > 0)
        }
    }

    // ---- Admin/AnswerTypes -------------------------------------------------

    /// `Admin\AnswerTypeController::index` — full list for the CRUD page.
    pub async fn admin_answer_types(&self) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, name, pronoun, svg_icon, created_at FROM answer_types ORDER BY id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                json!({
                    "id": r.get::<i64, _>("id"),
                    "name": r.get::<String, _>("name"),
                    "pronoun": r.get::<Option<String>, _>("pronoun"),
                    "svg_icon": r.get::<Option<String>, _>("svg_icon"),
                    "created_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at"),
                })
            })
            .collect())
    }

    /// `Admin\PlaylistController::store` — create under the admin user +
    /// moderator pivot (same shape as `PlaylistRepo::create` but honours
    /// the validated `is_public` flag).
    pub async fn admin_create_playlist(
        &self,
        user_id: i64,
        name: &str,
        is_public: bool,
    ) -> Result<i64, sqlx::Error> {
        let now = chrono::Utc::now();
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO playlists (name, user_id, is_public, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
        )
        .bind(name)
        .bind(user_id)
        .bind(is_public)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO moderables (user_id, moderable_type, moderable_id, \
             created_at, updated_at) VALUES ($1, 'App\\Models\\Playlist', $2, $3, $4)",
        )
        .bind(user_id)
        .bind(id)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(id)
    }

    /// `Admin\RoomController::store` — PHP's `create()` array silently drops
    /// non-fillable keys (is_pro, is_active, is_chat_active, playlist_id,
    /// pause_between_tracks, color, photo_path), so only the fillable set is
    /// written here. Then `moderators()->attach` + slug from the name.
    pub async fn admin_create_room(
        &self,
        user_id: i64,
        name: &str,
        description: Option<&str>,
        password: Option<&str>,
        tracks_by_round: i64,
        track_duration: i64,
        pause_between_rounds: i64,
        is_public: bool,
        is_random: bool,
        discord_webhook_url: Option<&str>,
    ) -> Result<i64, sqlx::Error> {
        // `RoomRepo::unique_slug` — kebab name, `-N` suffix on collision.
        let base = blindall_core::sanitize::str_slug(name, '-');
        let mut slug = base.clone();
        let mut count = 0;
        while sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM rooms WHERE slug = $1)::int::bigint",
        )
        .bind(&slug)
        .fetch_one(&self.pool)
        .await?
            == 1
        {
            slug = format!("{base}-{count}");
            count += 1;
        }
        let now = chrono::Utc::now();
        let mut tx = self.pool.begin().await?;
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO rooms (user_id, name, slug, description, password, \
             tracks_by_round, track_duration, pause_between_rounds, is_public, \
             is_random, discord_webhook_url, created_at, updated_at) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13) RETURNING id",
        )
        .bind(user_id)
        .bind(name)
        .bind(&slug)
        .bind(description)
        .bind(password)
        .bind(tracks_by_round)
        .bind(track_duration)
        .bind(pause_between_rounds)
        .bind(is_public)
        .bind(is_random)
        .bind(discord_webhook_url)
        .bind(now)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO moderables (user_id, moderable_type, moderable_id, \
             created_at, updated_at) VALUES ($1, 'App\\Models\\Room', $2, $3, $4)",
        )
        .bind(user_id)
        .bind(id)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(id)
    }

    /// `Admin\PageController::destroy` — Page has no SoftDeletes → hard delete.
    pub async fn delete_page(&self, id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query("DELETE FROM pages WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected() > 0)
    }

    /// `Admin\PlaylistController::store` validation helper — `unique:playlists`.
    pub async fn playlist_name_taken(&self, name: &str) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM playlists WHERE name = $1)::int::bigint",
        )
        .bind(name)
        .fetch_one(&self.pool)
        .await
        .map(|v| v == 1)
    }

}
