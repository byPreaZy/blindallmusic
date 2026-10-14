use sqlx::postgres::PgPool;

#[derive(Clone)]
pub struct VoteRepo {
    pool: PgPool,
}

impl VoteRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    async fn user_vote(&self, user_id: i64, track_id: i64) -> Result<Option<i64>, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "SELECT votes FROM votes WHERE user_id = $1 \
             AND votable_type = 'App\\Models\\Track' AND votable_id = $2",
        )
        .bind(user_id)
        .bind(track_id)
        .fetch_optional(&self.pool)
        .await
    }

    async fn cancel(&self, user_id: i64, track_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "DELETE FROM votes WHERE user_id = $1 \
             AND votable_type = 'App\\Models\\Track' AND votable_id = $2",
        )
        .bind(user_id)
        .bind(track_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn set_vote(&self, user_id: i64, track_id: i64, votes: i64, reason: Option<&str>) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now();
        let updated = sqlx::query(
            "UPDATE votes SET votes = $1, downvote_reason = $2, updated_at = $3 \
             WHERE user_id = $4 AND votable_type = 'App\\Models\\Track' \
             AND votable_id = $5",
        )
        .bind(votes)
        .bind(reason)
        .bind(now)
        .bind(user_id)
        .bind(track_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if updated == 0 {
            sqlx::query(
                "INSERT INTO votes (user_id, votes, downvote_reason, votable_type, \
                 votable_id, created_at, updated_at) \
                 VALUES ($1, $2, $3, 'App\\Models\\Track', $4, $5, $6)",
            )
            .bind(user_id)
            .bind(votes)
            .bind(reason)
            .bind(track_id)
            .bind(now)
            .bind(now)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn apply_upvote(&self, user_id: i64, track_id: i64) -> Result<(), sqlx::Error> {
        match self.user_vote(user_id, track_id).await? {
            Some(1) => self.cancel(user_id, track_id).await,
            _ => self.set_vote(user_id, track_id, 1, None).await,
        }
    }

    pub async fn apply_downvote(
        &self,
        user_id: i64,
        track_id: i64,
        reason: Option<&str>,
    ) -> Result<Result<(), ()>, sqlx::Error> {
        match self.user_vote(user_id, track_id).await? {
            Some(-1) => {
                if reason.is_none() {
                    self.cancel(user_id, track_id).await?;
                } else {
                    self.set_vote(user_id, track_id, -1, reason).await?;
                }
                Ok(Ok(()))
            }
            _ => {
                if reason.is_none() {
                    return Ok(Err(()));
                }
                self.set_vote(user_id, track_id, -1, reason).await?;
                Ok(Ok(()))
            }
        }
    }

    pub async fn payload(&self, track_id: i64, user_id: i64) -> Result<serde_json::Value, sqlx::Error> {
        let up: i64 = sqlx::query_scalar(
            "SELECT CAST(COALESCE(SUM(votes), 0) AS bigint) FROM votes \
             WHERE votable_type = 'App\\Models\\Track' AND votable_id = $1 AND votes > 0",
        )
        .bind(track_id)
        .fetch_one(&self.pool)
        .await?;
        let down: i64 = sqlx::query_scalar(
            "SELECT CAST(COALESCE(ABS(SUM(votes)), 0) AS bigint) FROM votes \
             WHERE votable_type = 'App\\Models\\Track' AND votable_id = $1 AND votes < 0",
        )
        .bind(track_id)
        .fetch_one(&self.pool)
        .await?;
        let user_vote = self.user_vote(user_id, track_id).await?.unwrap_or(0);
        Ok(serde_json::json!({
            "upvotes": format_votes(up),
            "downvotes": format_votes(down),
            "user_voted_up": user_vote == 1,
            "user_voted_down": user_vote == -1,
        }))
    }
}

fn format_votes(n: i64) -> serde_json::Value {
    if n >= 1000 {
        let k = ((n as f64 / 1000.0) * 10.0).round() / 10.0;
        serde_json::Value::String(format!("{k}k"))
    } else {
        serde_json::Value::from(n)
    }
}
