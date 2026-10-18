use sqlx::Row;
use sqlx::postgres::PgPool;

#[derive(Clone)]
pub struct SeoRepo {
    pool: PgPool,
}

pub struct SitemapRoom {
    pub slug: String,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub struct SitemapCategory {
    pub slug: String,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub struct SitemapPage {
    pub slug: String,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl SeoRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn public_rooms(&self) -> Result<Vec<SitemapRoom>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT r.slug, r.updated_at FROM rooms r \
             WHERE r.is_public = true AND r.is_active = true \
             AND r.password IS NULL AND r.deleted_at IS NULL \
             ORDER BY (SELECT COUNT(*) FROM rounds ro WHERE ro.room_id = r.id) DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| SitemapRoom {
                slug: r.get("slug"),
                updated_at: r.get("updated_at"),
            })
            .collect())
    }

    pub async fn indexable_categories(
        &self,
        hidden_ids: &[i64],
    ) -> Result<Vec<SitemapCategory>, sqlx::Error> {
        let sql = format!(
            "SELECT c.slug, c.updated_at FROM categories c \
             WHERE EXISTS(SELECT 1 FROM rooms r \
                          WHERE r.category_id = c.id AND r.is_public \
                          AND r.password IS NULL AND r.deleted_at IS NULL) \
             {} ORDER BY c.name",
            if hidden_ids.is_empty() {
                String::new()
            } else {
                "AND NOT (c.id = ANY($1))".to_string()
            }
        );
        let mut q = sqlx::query(&sql);
        if !hidden_ids.is_empty() {
            q = q.bind(hidden_ids.to_vec());
        }
        let rows = q.fetch_all(&self.pool).await?;
        Ok(rows
            .iter()
            .map(|r| SitemapCategory {
                slug: r.get("slug"),
                updated_at: r.get("updated_at"),
            })
            .collect())
    }

    pub async fn pages(&self) -> Result<Vec<SitemapPage>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT p.slug, COALESCE(p.revised_at, p.updated_at) AS ts \
             FROM pages p \
             JOIN (SELECT slug, MAX(COALESCE(revised_at, updated_at)) AS mts \
                   FROM pages GROUP BY slug) latest \
               ON latest.slug = p.slug \
              AND COALESCE(p.revised_at, p.updated_at) = latest.mts \
             GROUP BY p.slug, ts",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| SitemapPage {
                slug: r.get("slug"),
                updated_at: r.get("ts"),
            })
            .collect())
    }
}
