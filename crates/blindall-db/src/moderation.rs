//! Moderation — `TrashedMessageController`, `BannedUserController`,
//! `UserManagementController`, `ModeratorController` backing queries.
//! Bans use the mchev/banhammer `bans` table.

use serde_json::{json, Value};
use sqlx::postgres::PgPool;
use sqlx::Row;

#[derive(Clone)]
pub struct ModerationRepo {
    pool: PgPool,
}

pub struct Page {
    pub items: Vec<Value>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

impl Page {
    pub fn to_json(&self) -> Value {
        json!({
            "data": self.items,
            "total": self.total,
            "current_page": self.page,
            "per_page": self.per_page,
            "last_page": (self.total + self.per_page - 1) / self.per_page.max(1),
        })
    }
}

fn offset(page: i64, per_page: i64) -> i64 {
    (page.max(1) - 1) * per_page.max(1)
}

impl ModerationRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ---- trashed messages ---------------------------------------------------

    /// `TrashedMessageController::index` — soft-deleted messages, search
    /// on body or user name, latest deleted first.
    pub async fn trashed_messages(
        &self,
        search: Option<&str>,
        page: i64,
        per_page: i64,
    ) -> Result<Page, sqlx::Error> {
        let like = search.map(|s| format!("%{s}%"));
        let rows = sqlx::query(
            "SELECT m.id, m.body, m.room_id, m.deleted_at, m.created_at, \
             u.id AS user_id, u.name AS user_name, u.photo_path, \
             r.name AS room_name \
             FROM messages m \
             JOIN users u ON u.id = m.user_id \
             LEFT JOIN rooms r ON r.id = m.room_id \
             WHERE m.deleted_at IS NOT NULL \
             AND ($1 IS NULL OR m.body LIKE $2 OR u.name LIKE $3) \
             ORDER BY m.deleted_at DESC LIMIT $4 OFFSET $5",
        )
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .bind(per_page)
        .bind(offset(page, per_page))
        .fetch_all(&self.pool)
        .await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM messages m JOIN users u ON u.id = m.user_id \
             WHERE m.deleted_at IS NOT NULL \
             AND ($1 IS NULL OR m.body LIKE $2 OR u.name LIKE $3)",
        )
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .fetch_one(&self.pool)
        .await?;
        let items = rows
            .iter()
            .map(|r| {
                json!({
                    "id": r.get::<i64, _>("id"),
                    "body": r.get::<String, _>("body"),
                    "deleted_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("deleted_at"),
                    "created_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at"),
                    "user": {
                        "id": r.get::<i64, _>("user_id"),
                        "name": r.get::<String, _>("user_name"),
                        "photo": r.get::<Option<String>, _>("photo_path"),
                    },
                    "room": {
                        "id": r.get::<Option<i64>, _>("room_id"),
                        "name": r.get::<Option<String>, _>("room_name"),
                    },
                })
            })
            .collect();
        Ok(Page {
            items,
            total,
            page,
            per_page,
        })
    }

    /// `restore` / `forceDelete` — message must be trashed.
    pub async fn restore_message(&self, message_id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query(
            "UPDATE messages SET deleted_at = NULL WHERE id = $1 AND deleted_at IS NOT NULL",
        )
        .bind(message_id)
        .execute(&self.pool)
        .await
        .map(|r| r.rows_affected() > 0)
    }

    pub async fn force_delete_message(&self, message_id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query("DELETE FROM messages WHERE id = $1 AND deleted_at IS NOT NULL")
            .bind(message_id)
            .execute(&self.pool)
            .await
            .map(|r| r.rows_affected() > 0)
    }

    // ---- banned users -------------------------------------------------------

    /// `BannedUserController::index` — users having at least one ban row.
    pub async fn banned_users(
        &self,
        search: Option<&str>,
        page: i64,
        per_page: i64,
        sensitive: bool,
    ) -> Result<Page, sqlx::Error> {
        let like = search.map(|s| format!("%{s}%"));
        let rows = sqlx::query(
            "SELECT u.id, u.name, u.photo_path, u.email, u.ip, \
             (SELECT COUNT(*) FROM bans b WHERE b.bannable_id = u.id \
              AND b.bannable_type = 'App\\Models\\User') AS bans_count \
             FROM users u \
             WHERE EXISTS(SELECT 1 FROM bans b \
                          WHERE b.bannable_id = u.id \
                          AND b.bannable_type = 'App\\Models\\User' \
                          AND b.deleted_at IS NULL \
                          AND (b.expired_at IS NULL OR b.expired_at > NOW())) \
             AND ($1 IS NULL OR u.name LIKE $2 OR u.email LIKE $3) \
             ORDER BY u.id DESC LIMIT $4 OFFSET $5",
        )
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .bind(per_page)
        .bind(offset(page, per_page))
        .fetch_all(&self.pool)
        .await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM users u \
             WHERE EXISTS(SELECT 1 FROM bans b \
                          WHERE b.bannable_id = u.id \
                          AND b.bannable_type = 'App\\Models\\User' \
                          AND b.deleted_at IS NULL \
                          AND (b.expired_at IS NULL OR b.expired_at > NOW())) \
             AND ($1 IS NULL OR u.name LIKE $2 OR u.email LIKE $3)",
        )
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .fetch_one(&self.pool)
        .await?;
        let mut items = Vec::new();
        for r in &rows {
            let user_id: i64 = r.get("id");
            let bans = self.ban_history(user_id, sensitive).await?;
            let active = bans.iter().find(|b| {
                b["expires_at"].is_null()
                    || b["is_permanent"] == json!(true)
            }).cloned();
            let mut v = json!({
                "id": user_id,
                "name": r.get::<String, _>("name"),
                "photo": r.get::<Option<String>, _>("photo_path"),
                "bans_count": r.get::<i64, _>("bans_count"),
                "active_ban": active,
                "ban_history": bans,
            });
            if sensitive {
                v["email"] = r.get::<Option<String>, _>("email").into();
                v["ip"] = r.get::<Option<String>, _>("ip").into();
            }
            items.push(v);
        }
        Ok(Page {
            items,
            total,
            page,
            per_page,
        })
    }

    /// `formatBan` for all (non-deleted) bans of a user.
    async fn ban_history(&self, user_id: i64, sensitive: bool) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT b.id, b.comment, b.created_at, b.expired_at, b.ip, \
             cu.name AS banned_by \
             FROM bans b \
             LEFT JOIN users cu ON cu.id = b.created_by_id \
             WHERE b.bannable_type = 'App\\Models\\User' \
             AND b.bannable_id = $1 AND b.deleted_at IS NULL \
             ORDER BY b.created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                let expired: Option<chrono::DateTime<chrono::Utc>> = r.get("expired_at");
                let mut v = json!({
                    "id": r.get::<i64, _>("id"),
                    "comment": r.get::<Option<String>, _>("comment"),
                    "created_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at"),
                    "expires_at": expired,
                    "is_permanent": expired.is_none(),
                    "banned_by": r.get::<Option<String>, _>("banned_by"),
                });
                if sensitive {
                    v["ip"] = r.get::<Option<String>, _>("ip").into();
                }
                v
            })
            .collect())
    }

    /// `$user->isBanned()` — active (non-deleted, unexpired) ban exists.
    /// `PageController::bannedUser` — latest ban row for the banned-page
    /// payload (`comment`, `expired_at`).
    pub async fn latest_ban(&self, user_id: i64) -> Result<Option<Value>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT comment, expired_at FROM bans \
             WHERE bannable_type = 'App\\Models\\User' AND bannable_id = $1 \
             AND deleted_at IS NULL \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| {
            json!({
                "comment": r.get::<Option<String>, _>("comment"),
                "expired_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("expired_at"),
            })
        }))
    }

    pub async fn is_banned(&self, user_id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM bans \
             WHERE bannable_type = 'App\\Models\\User' AND bannable_id = $1 \
             AND deleted_at IS NULL \
             AND (expired_at IS NULL OR expired_at > NOW()))::int::bigint",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .map(|n| n > 0)
    }

    /// `$user->unban()` — soft-delete active bans.
    pub async fn unban(&self, user_id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query(
            "UPDATE bans SET deleted_at = $1 \
             WHERE bannable_type = 'App\\Models\\User' AND bannable_id = $2 \
             AND deleted_at IS NULL \
             AND (expired_at IS NULL OR expired_at > $3)",
        )
        .bind(chrono::Utc::now())
        .bind(user_id)
        .bind(chrono::Utc::now())
        .execute(&self.pool)
        .await
        .map(|r| r.rows_affected() > 0)
    }

    // ---- user management ----------------------------------------------------

    /// `paginateUsers` — non-guest users + counts, `formatListUser` shape.
    pub async fn users(
        &self,
        search: Option<&str>,
        page: i64,
        per_page: i64,
        sensitive: bool,
    ) -> Result<Page, sqlx::Error> {
        let like = search.map(|s| format!("%{s}%"));
        let rows = sqlx::query(
            "SELECT u.id, u.name, u.photo_path, u.created_at, u.provider, \
             u.is_guest, u.is_administrator, u.email, u.ip, \
             EXISTS(SELECT 1 FROM moderables m JOIN rooms r ON r.id = m.moderable_id \
                    AND r.is_public = true \
                    WHERE m.user_id = u.id AND m.moderable_type = 'App\\Models\\Room')::int::bigint \
                AS is_moderator, \
             EXISTS(SELECT 1 FROM bans b WHERE b.bannable_id = u.id \
                    AND b.bannable_type = 'App\\Models\\User' AND b.deleted_at IS NULL \
                    AND (b.expired_at IS NULL OR b.expired_at > NOW()))::int::bigint AS is_banned, \
             (SELECT COUNT(*) FROM messages m WHERE m.user_id = u.id) AS messages_count, \
             (SELECT COUNT(*) FROM messages m WHERE m.user_id = u.id \
              AND EXISTS(SELECT 1 FROM votes v WHERE v.votable_id = m.id \
                         AND v.votable_type = 'App\\Models\\Message' AND v.votes = -1)) \
                AS reported_messages_count, \
             (SELECT COUNT(*) FROM bans b WHERE b.bannable_id = u.id \
              AND b.bannable_type = 'App\\Models\\User') AS bans_count, \
             (SELECT COUNT(*) FROM rooms r WHERE r.user_id = u.id) AS rooms_count, \
             (SELECT COUNT(*) FROM playlists p WHERE p.user_id = u.id) AS playlists_count, \
             (SELECT MAX(m.created_at) FROM messages m WHERE m.user_id = u.id) AS last_message_at \
             FROM users u \
             WHERE u.is_guest = false AND u.deleted_at IS NULL \
             AND ($1 IS NULL OR u.name LIKE $2 OR u.email LIKE $3) \
             ORDER BY u.created_at DESC LIMIT $4 OFFSET $5",
        )
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .bind(per_page)
        .bind(offset(page, per_page))
        .fetch_all(&self.pool)
        .await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM users u \
             WHERE u.is_guest = false AND u.deleted_at IS NULL \
             AND ($1 IS NULL OR u.name LIKE $2 OR u.email LIKE $3)",
        )
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .fetch_one(&self.pool)
        .await?;
        let items = rows
            .iter()
            .map(|r| {
                let is_admin = r.get::<bool, _>("is_administrator");
                let is_mod = r.get::<i64, _>("is_moderator") > 0;
                let mut v = json!({
                    "id": r.get::<i64, _>("id"),
                    "name": r.get::<String, _>("name"),
                    "photo": r.get::<Option<String>, _>("photo_path"),
                    "created_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at"),
                    "provider": r.get::<Option<String>, _>("provider"),
                    "is_guest": r.get::<bool, _>("is_guest"),
                    "is_admin": is_admin,
                    "is_moderator": is_mod,
                    "is_banned": r.get::<i64, _>("is_banned") > 0,
                    "messages_count": r.get::<i64, _>("messages_count"),
                    "reported_messages_count": r.get::<i64, _>("reported_messages_count"),
                    "bans_count": r.get::<i64, _>("bans_count"),
                    "rooms_count": r.get::<i64, _>("rooms_count"),
                    "playlists_count": r.get::<i64, _>("playlists_count"),
                    "last_message_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("last_message_at"),
                    "can_ban": !is_admin && !is_mod,
                });
                if sensitive {
                    v["email"] = r.get::<Option<String>, _>("email").into();
                    v["ip"] = r.get::<Option<String>, _>("ip").into();
                }
                v
            })
            .collect();
        Ok(Page {
            items,
            total,
            page,
            per_page,
        })
    }

    /// `formatUserDetail` — same as list row for now (the rich PHP version
    /// adds conversations/IP forensics, ported later if needed).
    pub async fn user_detail(&self, user_id: i64, sensitive: bool) -> Result<Option<Value>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT id, name, photo_path, created_at, provider, is_guest, \
             is_administrator, email, ip, elo, team_id FROM users \
             WHERE id = $1 AND is_guest = false LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(r) = row else { return Ok(None) };
        let mut v = json!({
            "id": r.get::<i64, _>("id"),
            "name": r.get::<String, _>("name"),
            "photo": r.get::<Option<String>, _>("photo_path"),
            "created_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at"),
            "provider": r.get::<Option<String>, _>("provider"),
            "is_guest": r.get::<bool, _>("is_guest"),
            "is_admin": r.get::<bool, _>("is_administrator"),
            "elo": r.get::<i64, _>("elo"),
            "team_id": r.get::<Option<i64>, _>("team_id"),
            "is_banned": self.is_banned(user_id).await?,
            "ban_history": self.ban_history(user_id, sensitive).await?,
        });
        if sensitive {
            v["email"] = r.get::<Option<String>, _>("email").into();
            v["ip"] = r.get::<Option<String>, _>("ip").into();
        }
        Ok(Some(v))
    }

    /// `$user->ban([...])` — insert a ban row (created_by = moderator id).
    pub async fn ban(
        &self,
        user_id: i64,
        banned_by: i64,
        comment: &str,
        duration_minutes: Option<i64>,
        ip: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now();
        let expires = duration_minutes.map(|m| now + chrono::Duration::minutes(m));
        sqlx::query(
            "INSERT INTO bans (bannable_type, bannable_id, created_by_type, created_by_id, \
             comment, ip, expired_at, metas, created_at, updated_at) \
             VALUES ('App\\Models\\User', $1, 'App\\Models\\User', $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(user_id)
        .bind(banned_by)
        .bind(comment)
        .bind(ip)
        .bind(expires)
        .bind(sqlx::types::Json(json!({"banned_by": banned_by})))
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Staff flag check used by `ban` (cannot ban admins/moderators).
    pub async fn is_staff(&self, user_id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT (is_administrator OR EXISTS(\
             SELECT 1 FROM moderables m JOIN rooms r ON r.id = m.moderable_id \
             AND r.is_public = true \
             WHERE m.user_id = $1 AND m.moderable_type = 'App\\Models\\Room'))::int::bigint \
             FROM users WHERE id = $2",
        )
        .bind(user_id)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .map(|n| n > 0)
    }

    // ---- dashboard ----------------------------------------------------------

    /// `ModerationDashboard` stats.
    pub async fn stats(&self) -> Result<Value, sqlx::Error> {
        let one = |sql: &'static str| {
            let pool = self.pool.clone();
            async move {
                sqlx::query_scalar::<_, i64>(sql)
                    .fetch_one(&pool)
                    .await
                    .unwrap_or(0)
            }
        };
        Ok(json!({
            "total_users": one("SELECT COUNT(*) FROM users").await,
            "total_rooms": one("SELECT COUNT(*) FROM rooms").await,
            "public_rooms": one("SELECT COUNT(*) FROM rooms WHERE is_public = true").await,
            "private_rooms": one("SELECT COUNT(*) FROM rooms WHERE is_public = false").await,
            "total_playlists": one("SELECT COUNT(*) FROM playlists").await,
            "public_playlists": one("SELECT COUNT(*) FROM playlists WHERE is_public = true").await,
            "private_playlists": one("SELECT COUNT(*) FROM playlists WHERE is_public = false").await,
            "todays_messages": one("SELECT COUNT(*) FROM messages WHERE DATE(created_at) = CURDATE()").await,
            "banned_users": one(
                "SELECT COUNT(DISTINCT bannable_id) FROM bans \
                 WHERE bannable_type = 'App\\Models\\User' AND deleted_at IS NULL \
                 AND (expired_at IS NULL OR expired_at > NOW())"
            ).await,
        }))
    }

    /// Public moderators list — users attached to any public room.
    pub async fn moderators(&self, page: i64, per_page: i64) -> Result<Page, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT DISTINCT u.id, u.name, u.photo_path, u.created_at FROM users u \
             JOIN moderables m ON m.user_id = u.id \
             JOIN rooms r ON r.id = m.moderable_id AND m.moderable_type = 'App\\Models\\Room' \
             WHERE r.is_public = true ORDER BY u.name LIMIT $1 OFFSET $2",
        )
        .bind(per_page)
        .bind(offset(page, per_page))
        .fetch_all(&self.pool)
        .await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(DISTINCT u.id) FROM users u \
             JOIN moderables m ON m.user_id = u.id \
             JOIN rooms r ON r.id = m.moderable_id AND m.moderable_type = 'App\\Models\\Room' \
             WHERE r.is_public = true",
        )
        .fetch_one(&self.pool)
        .await?;
        let items = rows
            .iter()
            .map(|r| {
                json!({
                    "id": r.get::<i64, _>("id"),
                    "name": r.get::<String, _>("name"),
                    "photo": r.get::<Option<String>, _>("photo_path"),
                    "created_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at"),
                })
            })
            .collect();
        Ok(Page {
            items,
            total,
            page,
            per_page,
        })
    }
}
