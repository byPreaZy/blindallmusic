use serde_json::{Value, json};
use sqlx::Row;
use sqlx::postgres::PgPool;

#[derive(Clone)]
pub struct FaqRepo {
    pool: PgPool,
}

impl FaqRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn list(
        &self,
        search: Option<&str>,
        page: i64,
        per_page: i64,
    ) -> Result<(Vec<Value>, i64), sqlx::Error> {
        let like = search.map(|s| format!("%{s}%"));
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM faqs f \
             WHERE ($1 IS NULL OR f.question LIKE $2 OR f.answer LIKE $3)",
        )
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .fetch_one(&self.pool)
        .await?;

        let rows = sqlx::query(
            "SELECT f.id, f.locale, f.question, f.answer, \
             (SELECT CAST(COALESCE(SUM(v.votes), 0) AS bigint) FROM votes v \
              WHERE v.votable_type = 'App\\Models\\FAQ' AND v.votable_id = f.id AND v.votes > 0) AS total_upvotes, \
             (SELECT CAST(COALESCE(ABS(SUM(v.votes)), 0) AS bigint) FROM votes v \
              WHERE v.votable_type = 'App\\Models\\FAQ' AND v.votable_id = f.id AND v.votes < 0) AS total_downvotes \
             FROM faqs f \
             WHERE ($1 IS NULL OR f.question LIKE $2 OR f.answer LIKE $3) \
             ORDER BY total_upvotes DESC, f.question \
             LIMIT $4 OFFSET $5",
        )
        .bind(&like)
        .bind(&like)
        .bind(&like)
        .bind(per_page)
        .bind((page.max(1) - 1) * per_page)
        .fetch_all(&self.pool)
        .await?;

        let faqs = rows
            .iter()
            .map(|r| {
                json!({
                    "id": r.get::<i64, _>("id"),
                    "locale": r.get::<String, _>("locale"),
                    "question": r.get::<String, _>("question"),
                    "answer": r.get::<String, _>("answer"),
                    "upvotes": r.get::<i64, _>("total_upvotes"),
                    "downvotes": r.get::<i64, _>("total_downvotes"),
                })
            })
            .collect();
        Ok((faqs, total))
    }

    async fn user_vote(&self, user_id: i64, faq_id: i64) -> Result<Option<i64>, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT votes FROM votes WHERE user_id = $1 \
             AND votable_type = 'App\\Models\\FAQ' AND votable_id = $2",
        )
        .bind(user_id)
        .bind(faq_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn vote(&self, user_id: i64, faq_id: i64, value: i64) -> Result<(), sqlx::Error> {
        if self.user_vote(user_id, faq_id).await? == Some(value) {
            sqlx::query(
                "DELETE FROM votes WHERE user_id = $1 \
                 AND votable_type = 'App\\Models\\FAQ' AND votable_id = $2",
            )
            .bind(user_id)
            .bind(faq_id)
            .execute(&self.pool)
            .await?;
            return Ok(());
        }
        let now = chrono::Utc::now();
        let updated = sqlx::query(
            "UPDATE votes SET votes = $1, updated_at = $2 WHERE user_id = $3 \
             AND votable_type = 'App\\Models\\FAQ' AND votable_id = $4",
        )
        .bind(value)
        .bind(now)
        .bind(user_id)
        .bind(faq_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if updated == 0 {
            sqlx::query(
                "INSERT INTO votes (user_id, votes, votable_type, votable_id, \
                 created_at, updated_at) \
                 VALUES ($1, $2, 'App\\Models\\FAQ', $3, $4, $5)",
            )
            .bind(user_id)
            .bind(value)
            .bind(faq_id)
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }
}
