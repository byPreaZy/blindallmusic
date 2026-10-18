use serde_json::{Value, json};
use sqlx::Row;
use sqlx::postgres::PgPool;

#[derive(Clone)]
pub struct AdminRepo {
    pool: PgPool,
}

impl AdminRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn users(
        &self,
        search: Option<&str>,
        role: Option<&str>,
        trashed: Option<&str>,
        page: i64,
        per_page: i64,
    ) -> Result<crate::moderation::Page, sqlx::Error> {
        let like = search.map(|s| format!("%{s}%"));
        // Same moderator rule as everywhere else: moderables ∩ public rooms.
        let mod_exists = "EXISTS(SELECT 1 FROM moderables m JOIN rooms r \
             ON r.id = m.moderable_id AND r.is_public = true \
             WHERE m.user_id = u.id AND m.moderable_type = 'App\\Models\\Room')";
        let role_clause = match role {
            Some("admin") => "AND u.is_administrator = true".to_string(),
            Some("guest") => "AND u.is_guest = true".to_string(),
            Some("moderator") => format!("AND {mod_exists}"),
            Some("user") => format!(
                "AND u.is_administrator = false AND u.is_guest = false AND NOT {mod_exists}"
            ),
            _ => String::new(),
        };
        let trashed_clause = match trashed {
            Some("with") => "",
            Some("only") => "AND u.deleted_at IS NOT NULL",
            _ => "AND u.deleted_at IS NULL",
        };
        let sql = format!(
            "SELECT u.id, u.name, u.provider, u.is_administrator, u.email, \
             u.photo_path, u.created_at, u.deleted_at, u.team_id, \
             {mod_exists} AS is_moderator, \
             t.name AS team_name FROM users u \
             LEFT JOIN teams t ON t.id = u.team_id \
             WHERE 1=1 {trashed} \
             AND ($1 IS NULL OR u.name LIKE $2 OR u.email LIKE $3) \
             {role} \
             ORDER BY u.created_at DESC LIMIT $4 OFFSET $5",
            trashed = trashed_clause,
            mod_exists = mod_exists,
            role = role_clause
        );
        let count_sql = format!(
            "SELECT COUNT(*) FROM users u WHERE 1=1 {trashed} \
             AND ($1 IS NULL OR u.name LIKE $2 OR u.email LIKE $3) \
             {role}",
            trashed = trashed_clause,
            role = role_clause
        );
        let offset = (page.max(1) - 1) * per_page.max(1);
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
        let items = rows
            .iter()
            .map(|r| {
                let is_admin = r.get::<bool, _>("is_administrator");
                let is_mod = r.get::<bool, _>("is_moderator");
                json!({
                    "id": r.get::<i64, _>("id"),
                    "name": r.get::<String, _>("name"),
                    "provider": r.get::<Option<String>, _>("provider"),
                    "is_administrator": is_admin,
                    "is_moderator": is_mod,
                    "role": if is_admin { "admin" } else if is_mod { "moderator" } else { "" },
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

    /// Global role change from the admin UI.
    /// `admin` -> is_administrator flag; `moderator` -> moderator of every public room;
    /// `user` -> strips admin flag and all room moderation rights.
    pub async fn set_role(&self, user_id: i64, role: &str) -> Result<bool, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let now = chrono::Utc::now();
        let res = sqlx::query(
            "UPDATE users SET is_administrator = $1, updated_at = $2 \
             WHERE id = $3 AND deleted_at IS NULL",
        )
        .bind(role == "admin")
        .bind(now)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        if res.rows_affected() == 0 {
            tx.rollback().await?;
            return Ok(false);
        }
        match role {
            "moderator" => {
                sqlx::query(
                    "INSERT INTO moderables (user_id, moderable_type, moderable_id, \
                     created_at, updated_at) \
                     SELECT $1, 'App\\Models\\Room', r.id, $2, $2 FROM rooms r \
                     WHERE r.is_public = true AND r.deleted_at IS NULL \
                     AND NOT EXISTS (SELECT 1 FROM moderables m WHERE m.user_id = $1 \
                         AND m.moderable_type = 'App\\Models\\Room' \
                         AND m.moderable_id = r.id)",
                )
                .bind(user_id)
                .bind(now)
                .execute(&mut *tx)
                .await?;
            }
            "user" => {
                sqlx::query(
                    "DELETE FROM moderables WHERE user_id = $1 \
                     AND moderable_type = 'App\\Models\\Room'",
                )
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
            }
            _ => {}
        }
        tx.commit().await?;
        Ok(true)
    }

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

    pub async fn weekly_top_users(&self) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT rs.user_id, ROUND(SUM(rs.total_score)::numeric, 1)::float8 AS total_score, \
             u.id AS uid, u.name, u.photo_path, u.elo \
             FROM round_standings rs \
             JOIN rounds r ON rs.round_id = r.id \
             JOIN rooms rm ON rs.room_id = rm.id AND rm.is_public = true \
             JOIN users u ON u.id = rs.user_id \
             WHERE rs.created_at >= now() - interval '7 days' AND u.is_guest = false \
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

    pub async fn page_title_taken(&self, title: &str) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM pages WHERE title = $1)::int::bigint",
        )
        .bind(title)
        .fetch_one(&self.pool)
        .await
        .map(|v| v == 1)
    }

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

    pub async fn faqs(&self) -> Result<Vec<Value>, sqlx::Error> {
        let rows =
            sqlx::query("SELECT id, locale, question, answer, created_at FROM faqs ORDER BY id")
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
        Ok(crate::moderation::Page {
            items,
            total,
            page,
            per_page,
        })
    }

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
        Ok(crate::moderation::Page {
            items,
            total,
            page,
            per_page,
        })
    }

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
        Ok(crate::moderation::Page {
            items,
            total,
            page,
            per_page,
        })
    }

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

    pub async fn delete_page(&self, id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query("DELETE FROM pages WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected() > 0)
    }

    pub async fn playlist_name_taken(&self, name: &str) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM playlists WHERE name = $1)::int::bigint",
        )
        .bind(name)
        .fetch_one(&self.pool)
        .await
        .map(|v| v == 1)
    }

    // ---- data quality --------------------------------------------------------

    /// Coverage of "screen" answer types (Movie/Show/Anime/Cartoon/Video
    /// game/Musical) per room. A type is reported as the room's expected type
    /// when at least half of its tracks carry it — no manual mapping needed.
    pub async fn data_quality_rooms(&self) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            "WITH per_type AS (
               SELECT r.id AS room_id, r.name AS room_name, ta.answer_type_id,
                      COUNT(DISTINCT t.id) AS typed
               FROM rooms r
               JOIN playlist_room pr ON pr.room_id = r.id
               JOIN tracks t ON t.playlist_id = pr.playlist_id
               JOIN track_answers ta ON ta.track_id = t.id
               WHERE r.deleted_at IS NULL AND ta.answer_type_id IN (6,7,8,9,12,13)
               GROUP BY r.id, r.name, ta.answer_type_id
             ), totals AS (
               SELECT r.id, COUNT(DISTINCT t.id) AS total
               FROM rooms r
               JOIN playlist_room pr ON pr.room_id = r.id
               JOIN tracks t ON t.playlist_id = pr.playlist_id
               WHERE r.deleted_at IS NULL
               GROUP BY r.id
             )
             SELECT pt.room_id, pt.room_name, pt.answer_type_id,
                    at.name AS type_name, pt.typed, tt.total
             FROM per_type pt
             JOIN totals tt ON tt.id = pt.room_id
             JOIN answer_types at ON at.id = pt.answer_type_id
             WHERE pt.typed >= tt.total / 2.0
             ORDER BY pt.room_id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                let total: i64 = r.get("total");
                let typed: i64 = r.get("typed");
                json!({
                    "room_id": r.get::<i64, _>("room_id"),
                    "room_name": r.get::<String, _>("room_name"),
                    "answer_type_id": r.get::<i64, _>("answer_type_id"),
                    "type_name": r.get::<String, _>("type_name"),
                    "total": total,
                    "typed": typed,
                    "missing": total - typed,
                })
            })
            .collect())
    }

    /// Dominant screen type for a room, when a single type covers >=50% of
    /// its tracks.
    pub async fn room_expected_answer_type(
        &self,
        room_id: i64,
    ) -> Result<Option<(i64, String)>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT ta.answer_type_id, at.name, COUNT(DISTINCT t.id) AS typed,
                    (SELECT COUNT(DISTINCT x.id) FROM playlist_room pr2
                      JOIN tracks x ON x.playlist_id = pr2.playlist_id
                      WHERE pr2.room_id = $1) AS total
             FROM playlist_room pr
             JOIN tracks t ON t.playlist_id = pr.playlist_id
             JOIN track_answers ta ON ta.track_id = t.id
             JOIN answer_types at ON at.id = ta.answer_type_id
             WHERE pr.room_id = $1 AND ta.answer_type_id IN (6,7,8,9,12,13)
             GROUP BY ta.answer_type_id, at.name
             HAVING COUNT(DISTINCT t.id) >=
                    (SELECT COUNT(DISTINCT x.id) FROM playlist_room pr2
                      JOIN tracks x ON x.playlist_id = pr2.playlist_id
                      WHERE pr2.room_id = $1) / 2.0
             ORDER BY typed DESC
             LIMIT 1",
        )
        .bind(room_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| {
            (
                r.get::<i64, _>("answer_type_id"),
                r.get::<String, _>("name"),
            )
        }))
    }

    /// Tracks of a room missing a given answer type, paginated.
    /// Returns (rows, total).
    pub async fn tracks_missing_answer_type(
        &self,
        room_id: i64,
        type_id: i64,
        q: &str,
        page: i64,
        per_page: i64,
    ) -> Result<(Vec<Value>, i64), sqlx::Error> {
        let like = format!("%{}%", q);
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT t.id)
             FROM playlist_room pr
             JOIN tracks t ON t.playlist_id = pr.playlist_id
             WHERE pr.room_id = $1
               AND NOT EXISTS (SELECT 1 FROM track_answers ta
                               WHERE ta.track_id = t.id AND ta.answer_type_id = $2)
               AND NOT EXISTS (SELECT 1 FROM data_quality_actions dqa
                               WHERE dqa.track_id = t.id AND dqa.room_id = $1
                                 AND dqa.answer_type_id = $2 AND dqa.action = 'dismiss')
               AND ($5 = '' OR EXISTS (
                     SELECT 1 FROM track_answers a
                     WHERE a.track_id = t.id AND a.answer_type_id IN (1,2)
                       AND a.value ILIKE $5))",
        )
        .bind(room_id)
        .bind(type_id)
        .bind(page)
        .bind(per_page)
        .bind(if q.is_empty() { "" } else { &like })
        .fetch_one(&self.pool)
        .await?;
        let rows = sqlx::query(
            "SELECT DISTINCT t.id, t.playlist_id, t.preview_url, t.provider_url,
                    a1.value AS artist, a2.value AS title
             FROM playlist_room pr
             JOIN tracks t ON t.playlist_id = pr.playlist_id
             LEFT JOIN track_answers a1 ON a1.track_id = t.id AND a1.answer_type_id = 1
             LEFT JOIN track_answers a2 ON a2.track_id = t.id AND a2.answer_type_id = 2
             WHERE pr.room_id = $1
               AND NOT EXISTS (SELECT 1 FROM track_answers ta
                               WHERE ta.track_id = t.id AND ta.answer_type_id = $2)
               AND NOT EXISTS (SELECT 1 FROM data_quality_actions dqa
                               WHERE dqa.track_id = t.id AND dqa.room_id = $1
                                 AND dqa.answer_type_id = $2 AND dqa.action = 'dismiss')
               AND ($5 = '' OR a1.value ILIKE $5 OR a2.value ILIKE $5)
             ORDER BY t.id
             LIMIT $4 OFFSET $3",
        )
        .bind(room_id)
        .bind(type_id)
        .bind((page - 1).max(0) * per_page)
        .bind(per_page)
        .bind(if q.is_empty() { "" } else { &like })
        .fetch_all(&self.pool)
        .await?;
        Ok((
            rows.iter()
                .map(|r| {
                    json!({
                        "id": r.get::<i64, _>("id"),
                        "playlist_id": r.get::<i64, _>("playlist_id"),
                        "preview_url": r.get::<String, _>("preview_url"),
                        "provider_url": r.get::<Option<String>, _>("provider_url"),
                        "artist": r.get::<Option<String>, _>("artist"),
                        "title": r.get::<Option<String>, _>("title"),
                    })
                })
                .collect(),
            total,
        ))
    }

    /// Dismiss a (track, room, type) candidate. Idempotent via the partial
    /// unique index on data_quality_actions.
    pub async fn dismiss_candidate(
        &self,
        track_id: i64,
        playlist_id: i64,
        room_id: i64,
        answer_type_id: i64,
        reviewer_id: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO data_quality_actions
               (track_id, playlist_id, room_id, answer_type_id, action, reviewer_id)
             VALUES ($1, $2, $3, $4, 'dismiss', $5)
             ON CONFLICT (track_id, room_id, answer_type_id)
             WHERE action = 'dismiss' DO NOTHING",
        )
        .bind(track_id)
        .bind(playlist_id)
        .bind(room_id)
        .bind(answer_type_id)
        .bind(reviewer_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Undo a dismissal — the candidate re-enters the queue.
    pub async fn restore_candidate(&self, action_id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query("DELETE FROM data_quality_actions WHERE id = $1 AND action = 'dismiss'")
            .bind(action_id)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected() > 0)
    }

    /// Insert a track answer and audit it atomically. Idempotent: skips the
    /// insert when an identical (type, value) answer already exists.
    pub async fn apply_answer(
        &self,
        track_id: i64,
        playlist_id: i64,
        room_id: i64,
        answer_type_id: i64,
        value: &str,
        reviewer_id: i64,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let now = chrono::Utc::now();
        sqlx::query(
            "INSERT INTO track_answers (track_id, answer_type_id, value, score, created_at, updated_at)
             SELECT $1, $2, $3, 0.5, $4, $4
             WHERE NOT EXISTS (
               SELECT 1 FROM track_answers
               WHERE track_id = $1 AND answer_type_id = $2 AND value = $3)",
        )
        .bind(track_id)
        .bind(answer_type_id)
        .bind(value)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO data_quality_actions
               (track_id, playlist_id, room_id, answer_type_id, action, value, reviewer_id)
             VALUES ($1, $2, $3, $4, 'add_answer', $5, $6)",
        )
        .bind(track_id)
        .bind(playlist_id)
        .bind(room_id)
        .bind(answer_type_id)
        .bind(value)
        .bind(reviewer_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await
    }

    /// Delete a track from its playlist, snapshotting row + answers into the
    /// audit payload first (reversible record).
    pub async fn purge_track(
        &self,
        track_id: i64,
        playlist_id: i64,
        room_id: i64,
        answer_type_id: i64,
        reviewer_id: i64,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let payload: Value = sqlx::query_scalar(
            "SELECT jsonb_build_object(
               'track', to_jsonb(t),
               'answers', COALESCE((SELECT jsonb_agg(to_jsonb(a))
                                    FROM track_answers a WHERE a.track_id = t.id), '[]'::jsonb)
             ) FROM tracks t WHERE t.id = $1",
        )
        .bind(track_id)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(Value::Null);
        sqlx::query("DELETE FROM track_answers WHERE track_id = $1")
            .bind(track_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM tracks WHERE id = $1")
            .bind(track_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            "INSERT INTO data_quality_actions
               (track_id, playlist_id, room_id, answer_type_id, action, payload, reviewer_id)
             VALUES ($1, $2, $3, $4, 'remove_track', $5, $6)",
        )
        .bind(track_id)
        .bind(playlist_id)
        .bind(room_id)
        .bind(answer_type_id)
        .bind(payload)
        .bind(reviewer_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await
    }

    /// Recent audit entries for the data-quality review log.
    pub async fn data_quality_actions(&self, limit: i64) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT a.id, a.action, a.value, a.track_id, a.room_id, a.created_at,
                    u.name AS reviewer, r.name AS room_name, t.name AS type_name
             FROM data_quality_actions a
             JOIN users u ON u.id = a.reviewer_id
             LEFT JOIN rooms r ON r.id = a.room_id
             LEFT JOIN answer_types t ON t.id = a.answer_type_id
             ORDER BY a.id DESC
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                json!({
                    "id": r.get::<i64, _>("id"),
                    "action": r.get::<String, _>("action"),
                    "value": r.get::<Option<String>, _>("value"),
                    "track_id": r.get::<Option<i64>, _>("track_id"),
                    "room_name": r.get::<Option<String>, _>("room_name"),
                    "type_name": r.get::<Option<String>, _>("type_name"),
                    "reviewer": r.get::<String, _>("reviewer"),
                    "created_at": r.get::<chrono::DateTime<chrono::Utc>, _>("created_at"),
                })
            })
            .collect())
    }
}
