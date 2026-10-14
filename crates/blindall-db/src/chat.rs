//! Chat persistence — `RoomMessageController` + `ChatModerationService` DB
//! parts. Messages are polymorphic (`messagable_type = 'App\\Models\\Room'`),
//! reports are downvotes in `votes`, auto-bans go to `bans`.

use sqlx::postgres::PgPool;
use sqlx::Row;

#[derive(Clone)]
pub struct ChatRepo {
    pool: PgPool,
}

pub struct MessageRow {
    pub id: i64,
    pub room_id: i64,
    pub user_id: i64,
    pub user_ip: String,
    pub body: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl ChatRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// `$room->messages()->create(...)` — morph type Room.
    pub async fn create_message(
        &self,
        room_id: i64,
        user_id: i64,
        ip: &str,
        body: &str,
    ) -> Result<i64, sqlx::Error> {
        let now = chrono::Utc::now();
        let res = sqlx::query_scalar::<_, i64>(
            "INSERT INTO messages (messagable_type, messagable_id, user_id, \
             user_ip, body, created_at, updated_at) \
             VALUES ('App\\Models\\Room', $1, $2, $3, $4, $5, $6) RETURNING id",
        )
        .bind(room_id)
        .bind(user_id)
        .bind(ip)
        .bind(body)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        Ok(res)
    }

    pub async fn find_message(&self, id: i64) -> Result<Option<MessageRow>, sqlx::Error> {
        sqlx::query(
            "SELECT id, messagable_id, user_id, user_ip, body, created_at, deleted_at \
             FROM messages WHERE id = $1 AND messagable_type = 'App\\Models\\Room'",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map(|o| {
            o.map(|r| MessageRow {
                id: r.get("id"),
                room_id: r.get("messagable_id"),
                user_id: r.get("user_id"),
                user_ip: r.get("user_ip"),
                body: r.get("body"),
                created_at: r.get("created_at"),
                deleted_at: r.get("deleted_at"),
            })
        })
    }

    /// Chat history for room hydration — latest N, oldest first, with the
    /// `user` relation (`users.id/name/team_id/photo_path`).
    pub async fn room_messages(
        &self,
        room_id: i64,
        limit: i64,
    ) -> Result<Vec<serde_json::Value>, sqlx::Error> {
        // PHP `latest_messages` — `whereDate(created_at, '>=', now-2h)`
        // (i.e. today's messages, the date cast makes the 2h a day boundary)
        // + `user.team` eager-loaded for the team badge.
        let rows = sqlx::query(
            "SELECT m.id, m.user_id, m.body, m.created_at, \
             u.name AS user_name, u.team_id, u.photo_path, t.name AS team_name, \
             (SELECT CAST(COALESCE(SUM(v.votes), 0) AS bigint) FROM votes v \
              WHERE v.votable_type = 'App\\Models\\Message' \
              AND v.votable_id = m.id AND v.votes < 0) AS reports \
             FROM messages m \
             JOIN users u ON u.id = m.user_id \
             LEFT JOIN teams t ON t.id = u.team_id \
             WHERE m.messagable_type = 'App\\Models\\Room' \
             AND m.messagable_id = $1 AND m.deleted_at IS NULL \
             AND m.created_at::date >= (now() - interval '2 hours')::date \
             ORDER BY m.id DESC LIMIT $2",
        )
        .bind(room_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .rev()
            .map(|r| {
                let created: chrono::DateTime<chrono::Utc> = r.get("created_at");
                let team_id = r.get::<Option<i64>, _>("team_id");
                let team = match (team_id, r.get::<Option<String>, _>("team_name")) {
                    (Some(id), Some(name)) => serde_json::json!({"id": id, "name": name}),
                    _ => serde_json::Value::Null,
                };
                serde_json::json!({
                    "id": r.get::<i64, _>("id"),
                    "user_id": r.get::<i64, _>("user_id"),
                    "body": r.get::<String, _>("body"),
                    "created_at": created,
                    "time": created.format("%H:%M").to_string(),
                    "reports": r.get::<i64, _>("reports"),
                    "user": {
                        "id": r.get::<i64, _>("user_id"),
                        "name": r.get::<String, _>("user_name"),
                        "team_id": team_id,
                        "team": team,
                        "photo_path": r.get::<Option<String>, _>("photo_path"),
                        // `photo` accessor — the SPA prefixes /storage/ itself.
                        "photo": r.get::<Option<String>, _>("photo_path"),
                    },
                })
            })
            .collect())
    }

    /// `Auth::user()->downVote($message)` — one vote row per user, votes=-1.
    /// No unique index on (user, votable) → update in place or insert.
    /// Returns the new `totalDownvotes` (negative sum).
    pub async fn down_vote(&self, message_id: i64, user_id: i64) -> Result<i64, sqlx::Error> {
        let now = chrono::Utc::now();
        let updated = sqlx::query(
            "UPDATE votes SET votes = -1, updated_at = $1 \
             WHERE user_id = $2 AND votable_type = 'App\\Models\\Message' \
             AND votable_id = $3",
        )
        .bind(now)
        .bind(user_id)
        .bind(message_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if updated == 0 {
            sqlx::query(
                "INSERT INTO votes (user_id, votes, votable_type, votable_id, \
                 created_at, updated_at) \
                 VALUES ($1, -1, 'App\\Models\\Message', $2, $3, $4)",
            )
            .bind(user_id)
            .bind(message_id)
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;
        }
        self.total_downvotes(message_id).await
    }

    /// `totalDownvotes()` — sum of negative votes (0 when none).
    pub async fn total_downvotes(&self, message_id: i64) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT CAST(COALESCE(SUM(votes), 0) AS bigint) FROM votes \
             WHERE votable_type = 'App\\Models\\Message' \
             AND votable_id = $1 AND votes < 0",
        )
        .bind(message_id)
        .fetch_one(&self.pool)
        .await
    }

    // ---- reactions (`message_reactions`) --------------------------------------

    /// `groupedReactions` — [{emoji, count, users:[{id,name}]}].
    pub async fn grouped_reactions(
        &self,
        message_id: i64,
    ) -> Result<Vec<serde_json::Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT r.emoji, r.user_id, u.name \
             FROM message_reactions r JOIN users u ON u.id = r.user_id \
             WHERE r.message_id = $1 ORDER BY r.id",
        )
        .bind(message_id)
        .fetch_all(&self.pool)
        .await?;
        let mut groups: Vec<(String, Vec<(i64, String)>)> = vec![];
        for r in &rows {
            let emoji: String = r.get("emoji");
            let user = (r.get("user_id"), r.get("name"));
            match groups.iter_mut().find(|(e, _)| *e == emoji) {
                Some((_, users)) => users.push(user),
                None => groups.push((emoji, vec![user])),
            }
        }
        Ok(groups
            .into_iter()
            .map(|(emoji, users)| {
                serde_json::json!({
                    "emoji": emoji,
                    "count": users.len(),
                    "users": users.iter().map(|(id, name)| {
                        serde_json::json!({"id": id, "name": name})
                    }).collect::<Vec<_>>(),
                })
            })
            .collect())
    }

    /// `userReaction` — the viewer's first reaction emoji on the message.
    pub async fn user_reaction(
        &self,
        message_id: i64,
        user_id: i64,
    ) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT emoji FROM message_reactions \
             WHERE message_id = $1 AND user_id = $2 ORDER BY id LIMIT 1",
        )
        .bind(message_id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
    }

    /// Toggle off — returns rows deleted (0 or 1).
    pub async fn delete_reaction(
        &self,
        message_id: i64,
        user_id: i64,
        emoji: &str,
    ) -> Result<u64, sqlx::Error> {
        sqlx::query(
            "DELETE FROM message_reactions \
             WHERE message_id = $1 AND user_id = $2 AND emoji = $3",
        )
        .bind(message_id)
        .bind(user_id)
        .bind(emoji)
        .execute(&self.pool)
        .await
        .map(|r| r.rows_affected())
    }

    pub async fn create_reaction(
        &self,
        message_id: i64,
        user_id: i64,
        emoji: &str,
    ) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now();
        sqlx::query(
            "INSERT INTO message_reactions (message_id, user_id, emoji, \
             created_at, updated_at) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(message_id)
        .bind(user_id)
        .bind(emoji)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn soft_delete_message(&self, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE messages SET deleted_at = $1 WHERE id = $2")
            .bind(chrono::Utc::now())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn restore_message(&self, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE messages SET deleted_at = NULL WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `BannedUserMessageCleanupService` — on `Ban::created`, every non-deleted
    /// message of the user is soft-deleted (and broadcast). Returns
    /// `(message_id, room_id)` of what was removed for the broadcast loop.
    pub async fn soft_delete_user_messages(
        &self,
        user_id: i64,
    ) -> Result<Vec<(i64, i64)>, sqlx::Error> {
        sqlx::query_as::<_, (i64, i64)>(
            "UPDATE messages SET deleted_at = $1 WHERE user_id = $2 AND deleted_at IS NULL \
             AND messagable_type = 'App\\Models\\Room' RETURNING id, messagable_id",
        )
        .bind(chrono::Utc::now())
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
    }

    /// `$user->ban([...])` — cybercog/laravel-ban inserts a `bans` row
    /// (`users.banned_at` was dropped in 2023 — the bans table is authoritative).
    /// PHP always stores `'ip' => $user->ip` (the victim's last-known IP).
    pub async fn ban_user(
        &self,
        user_id: i64,
        days: i64,
        comment: &str,
    ) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now();
        let ip = sqlx::query_scalar::<_, Option<String>>("SELECT ip FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?
            .flatten();
        sqlx::query(
            "INSERT INTO bans (bannable_type, bannable_id, created_by_type, \
             created_by_id, comment, ip, expired_at, created_at, updated_at) \
             VALUES ('App\\Models\\User', $1, NULL, NULL, $2, $3, $4, $5, $6)",
        )
        .bind(user_id)
        .bind(comment)
        .bind(ip)
        .bind(now + chrono::Duration::days(days))
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// `isBanned()` — an unexpired/non-deleted ban row exists.
    pub async fn is_banned(&self, user_id: i64) -> bool {
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM bans b WHERE b.bannable_type = 'App\\Models\\User' \
             AND b.bannable_id = $1 AND b.deleted_at IS NULL \
             AND (b.expired_at IS NULL OR b.expired_at > $2))::int::bigint",
        )
        .bind(user_id)
        .bind(chrono::Utc::now())
        .fetch_one(&self.pool)
        .await
        .map(|v| v == 1)
        .unwrap_or(false)
    }
}
