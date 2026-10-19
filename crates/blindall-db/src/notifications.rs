use serde_json::{Value, json};
use sqlx::{PgPool, Row};

#[derive(Clone)]
pub struct NotificationRepo {
    pool: PgPool,
    hub: Option<blindall_realtime::Hub>,
}

impl NotificationRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool, hub: None }
    }

    /// PHP's ->notify() broadcasts BroadcastNotificationCreated in realtime;
    /// wiring the hub here makes every notification live, whichever caller
    /// persists it.
    pub fn with_hub(mut self, hub: blindall_realtime::Hub) -> Self {
        self.hub = Some(hub);
        self
    }

    pub async fn notify(
        &self,
        notifiable_id: i64,
        type_: &str,
        data: Value,
    ) -> Result<String, sqlx::Error> {
        let id = uuid::Uuid::now_v7().to_string();
        let now = chrono::Utc::now();
        sqlx::query(
            "INSERT INTO notifications (id, type, notifiable_type, notifiable_id, data, created_at, updated_at) \
             VALUES ($1, $2, 'App\\Models\\User', $3, $4, $5, $6)",
        )
        .bind(&id)
        .bind(type_)
        .bind(notifiable_id)
        .bind(data.to_string())
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        if let Some(hub) = &self.hub {
            hub.broadcast(
                &format!("user:{notifiable_id}"),
                "Illuminate\\Notifications\\Events\\BroadcastNotificationCreated",
                json!({
                    "id": id,
                    "type": type_,
                    "notifiable_type": "App\\Models\\User",
                    "notifiable_id": notifiable_id,
                    "data": data,
                }),
                None,
            )
            .await;
        }
        Ok(id)
    }

    pub async fn unread(&self, user_id: i64, limit: i64) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, type, data, created_at FROM notifications \
             WHERE notifiable_type = 'App\\Models\\User' AND notifiable_id = $1 \
             AND read_at IS NULL ORDER BY created_at DESC LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                let data: Value = r
                    .get::<Option<String>, _>("data")
                    .and_then(|d| serde_json::from_str(&d).ok())
                    .unwrap_or(json!({}));
                json!({
                    "id": r.get::<String, _>("id"),
                    "type": r.get::<String, _>("type"),
                    "data": data,
                    "created_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at")
                        .map(|t| t.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
                })
            })
            .collect())
    }

    pub async fn suggestions(&self, user_id: i64, limit: i64) -> Result<Vec<Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, type, data, created_at, read_at FROM notifications \
             WHERE notifiable_type = 'App\\Models\\User' AND notifiable_id = $1 \
             AND type = 'App\\Notifications\\NewSuggestion' \
             ORDER BY read_at IS NOT NULL, created_at DESC LIMIT $2",
        )
        .bind(user_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                let data: Value = r
                    .get::<Option<String>, _>("data")
                    .and_then(|d| serde_json::from_str(&d).ok())
                    .unwrap_or(json!({}));
                json!({
                    "id": r.get::<String, _>("id"),
                    "type": r.get::<String, _>("type"),
                    "data": data,
                    "read": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("read_at").is_some(),
                    "created_at": r.get::<Option<chrono::DateTime<chrono::Utc>>, _>("created_at")
                        .map(|t| t.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
                })
            })
            .collect())
    }

    pub async fn unread_count(&self, user_id: i64) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM notifications \
             WHERE notifiable_type = 'App\\Models\\User' AND notifiable_id = $1 AND read_at IS NULL",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
    }

    pub async fn mark_read(&self, user_id: i64, id: &str) -> Result<bool, sqlx::Error> {
        let res = sqlx::query(
            "UPDATE notifications SET read_at = NOW(), updated_at = NOW() \
             WHERE id = $1 AND notifiable_id = $2 AND read_at IS NULL",
        )
        .bind(id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected() > 0)
    }

    pub async fn mark_done(&self, user_id: i64, id: &str) -> Result<u64, sqlx::Error> {
        let row = sqlx::query(
            "SELECT type, data FROM notifications WHERE id = $1 AND notifiable_id = $2",
        )
        .bind(id)
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else {
            return Ok(0);
        };
        let type_: String = row.get("type");
        let data: Value = row
            .get::<Option<String>, _>("data")
            .and_then(|d| serde_json::from_str(&d).ok())
            .unwrap_or(json!({}));
        let message = data["message"].as_str().unwrap_or("").to_string();
        // Single UPDATE matching the JSON message — replaces the
        // fetch-all + per-row UPDATE loop.
        let res = sqlx::query(
            "UPDATE notifications SET read_at = NOW(), updated_at = NOW() \
             WHERE notifiable_id = $1 AND type = $2 AND read_at IS NULL \
             AND data::jsonb ->> 'message' = $3",
        )
        .bind(user_id)
        .bind(&type_)
        .bind(&message)
        .execute(&self.pool)
        .await?;
        Ok(res.rows_affected())
    }
}
