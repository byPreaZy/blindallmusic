use sqlx::{PgPool, Row};
use serde_json::{json, Value};

#[derive(Clone)]
pub struct NotificationRepo {
    pool: PgPool,
}

impl NotificationRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
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
        let rows = sqlx::query(
            "SELECT id, data FROM notifications \
             WHERE notifiable_id = $1 AND type = $2 AND read_at IS NULL",
        )
        .bind(user_id)
        .bind(&type_)
        .fetch_all(&self.pool)
        .await?;
        let mut done = 0u64;
        for r in rows {
            let d: Value = r
                .get::<Option<String>, _>("data")
                .and_then(|d| serde_json::from_str(&d).ok())
                .unwrap_or(json!({}));
            if d["message"].as_str().unwrap_or("") == message {
                let res = sqlx::query(
                    "UPDATE notifications SET read_at = NOW(), updated_at = NOW() WHERE id = $1",
                )
                .bind(r.get::<String, _>("id"))
                .execute(&self.pool)
                .await?;
                done += res.rows_affected();
            }
        }
        Ok(done)
    }
}
