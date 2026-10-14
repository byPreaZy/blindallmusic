//! Minigames — `TrackPickerService` + `MinigameScoreService` port.
//! Games: quiz, who_sang, album_cover, first_letter, anagram.

use blindall_game::store::PreviewResolver;
use serde::Serialize;
use sqlx::postgres::PgPool;
use sqlx::Row;
use std::sync::Arc;

pub const ANSWER_TYPE_ARTIST: i64 = 1;
pub const ANSWER_TYPE_TITLE: i64 = 2;
const MAX_RANDOM_RETRIES: usize = 5;

#[derive(Clone)]
pub struct MinigameRepo {
    pool: PgPool,
    resolver: Arc<dyn PreviewResolver>,
}

#[derive(Debug, Serialize)]
pub struct MinigameTrack {
    pub id: i64,
    pub preview_url: Option<String>,
    pub artwork_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ChoicesQuestion {
    pub track: MinigameTrack,
    pub choices: Vec<String>,
    /// Not serialized to the client — used server-side only.
    #[serde(skip_serializing)]
    pub correct_value: String,
}

#[derive(Debug, Serialize)]
pub struct TitleQuestion {
    pub track: MinigameTrack,
    #[serde(skip_serializing)]
    pub correct_value: String,
}

impl MinigameRepo {
    pub fn new(pool: PgPool, resolver: Arc<dyn PreviewResolver>) -> Self {
        Self { pool, resolver }
    }

    /// `getRandomTrackWithChoices` — 1 correct + up to 3 wrong, shuffled.
    pub async fn track_with_choices(
        &self,
        game_type: &str,
    ) -> Result<Option<ChoicesQuestion>, sqlx::Error> {
        let require_artwork = game_type == "album_cover";
        let answer_type_id = if game_type == "who_sang" || game_type == "album_cover" {
            ANSWER_TYPE_ARTIST
        } else {
            ANSWER_TYPE_TITLE
        };
        let Some(track) = self.pick_random_track(answer_type_id, require_artwork).await? else {
            return Ok(None);
        };
        let correct = self.answer_of(track.id, answer_type_id).await?.unwrap_or_default();
        let mut choices = self
            .wrong_choices(track.id, answer_type_id, &correct, 3)
            .await?;
        choices.insert(0, correct.clone());
        shuffle(&mut choices);
        Ok(Some(ChoicesQuestion {
            track,
            choices,
            correct_value: correct,
        }))
    }

    /// `getRandomTrackWithTitle` — first_letter: title answer, sanitized.
    pub async fn track_with_title(&self) -> Result<Option<TitleQuestion>, sqlx::Error> {
        let Some(track) = self.pick_random_track(ANSWER_TYPE_TITLE, false).await? else {
            return Ok(None);
        };
        let raw = self.answer_of(track.id, ANSWER_TYPE_TITLE).await?.unwrap_or_default();
        Ok(Some(TitleQuestion {
            track,
            correct_value: blindall_core::sanitize::sanitize_string(&raw),
        }))
    }

    /// `getRandomArtistAnagram` — bundled `database/data/anagram_artists.json`.
    pub fn random_anagram(&self) -> Option<(String, String)> {
        let list = anagram_list();
        if list.is_empty() {
            return None;
        }
        use rand::Rng;
        let (artist, anagram) = list[rand::rng().random_range(0..list.len())];
        Some((anagram.to_string(), artist.to_string()))
    }

    /// `answers->firstWhere('answer_type_id', …)->value` for a track.
    pub async fn answer_of(
        &self,
        track_id: i64,
        answer_type_id: i64,
    ) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT value FROM track_answers WHERE track_id = $1 AND answer_type_id = $2 LIMIT 1",
        )
        .bind(track_id)
        .bind(answer_type_id)
        .fetch_optional(&self.pool)
        .await
    }

    /// `MinigameScoreService::record` — insert + return row id.
    /// (`UpdateUserLevel::dispatch` stays in the API layer.)
    pub async fn record_score(
        &self,
        user_id: i64,
        game_type: &str,
        score: i64,
        metadata: serde_json::Value,
    ) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO minigame_scores (user_id, game_type, score, metadata, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $6) RETURNING id",
        )
        .bind(user_id)
        .bind(game_type)
        .bind(score)
        .bind(sqlx::types::Json(&metadata))
        .bind(chrono::Utc::now())
        .bind(chrono::Utc::now())
        .fetch_one(&self.pool)
        .await
        
    }

    /// `getTotalsByTypeForUser` — per-game-type sums.
    pub async fn totals_for_user(&self, user_id: i64) -> Result<Vec<(String, i64)>, sqlx::Error> {
        sqlx::query(
            "SELECT game_type, CAST(COALESCE(SUM(score),0) AS bigint) AS total FROM minigame_scores \
             WHERE user_id = $1 GROUP BY game_type",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.iter()
                .map(|r| (r.get::<String, _>("game_type"), r.get::<i64, _>("total")))
                .collect()
        })
    }

    /// Profile minigames tab — paginated session history
    /// (`ProfilePageService::minigames` history, 15/page).
    pub async fn history_for_user(
        &self,
        user_id: i64,
        page: i64,
        per_page: i64,
    ) -> Result<serde_json::Value, sqlx::Error> {
        let total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM minigame_scores WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        let rows = sqlx::query(
            "SELECT id, game_type, score, created_at FROM minigame_scores \
             WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(user_id)
        .bind(per_page)
        .bind((page - 1).max(0) * per_page)
        .fetch_all(&self.pool)
        .await?;
        let last_page = ((total as f64) / (per_page as f64)).ceil().max(1.0) as i64;
        Ok(serde_json::json!({
            "data": rows.iter().map(|r| {
                let created: chrono::DateTime<chrono::Utc> = r.get("created_at");
                serde_json::json!({
                    "id": r.get::<i64, _>("id"),
                    "game_type": r.get::<String, _>("game_type"),
                    "score": r.get::<i64, _>("score"),
                    "played_at": created.format("%d/%m/%Y %H:%M").to_string(),
                })
            }).collect::<Vec<_>>(),
            "current_page": page,
            "last_page": last_page,
            "total": total,
        }))
    }

    /// `getLeaderboard` — top N totals for a game type.
    pub async fn leaderboard(
        &self,
        game_type: &str,
        limit: i64,
    ) -> Result<Vec<serde_json::Value>, sqlx::Error> {
        sqlx::query(
            "SELECT m.user_id, u.name, CAST(SUM(m.score) AS bigint) AS total FROM minigame_scores m \
             JOIN users u ON u.id = m.user_id WHERE m.game_type = $1 \
             GROUP BY m.user_id, u.name ORDER BY total DESC LIMIT $2",
        )
        .bind(game_type)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map(|rows| {
            rows.iter()
                .map(|r| {
                    serde_json::json!({
                        "user_id": r.get::<i64, _>("user_id"),
                        "name": r.get::<Option<String>, _>("name"),
                        "total": r.get::<i64, _>("total"),
                    })
                })
                .collect()
        })
    }

    /// `pickRandomTrackWithAnswer` — public-playlist tracks, `id >= rand`
    /// probing to avoid ORDER BY random(); live-resolves the preview URL.
    async fn pick_random_track(
        &self,
        answer_type_id: i64,
        require_artwork: bool,
    ) -> Result<Option<MinigameTrack>, sqlx::Error> {
        let max_id: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(id),0) FROM tracks")
            .fetch_one(&self.pool)
            .await?;
        if max_id < 1 {
            return Ok(None);
        }
        // rand::rng() (ThreadRng) is !Send — draw ids first, drop the rng
        // before any .await.
        let random_ids: Vec<i64> = {
            use rand::Rng;
            let mut rng = rand::rng();
            (0..MAX_RANDOM_RETRIES)
                .map(|_| rng.random_range(1..=max_id))
                .collect()
        };
        let mut found = None;
        for rid in random_ids {
            found = self
                .pick_one(answer_type_id, require_artwork, rid)
                .await?;
            if found.is_some() {
                break;
            }
        }
        if found.is_none() {
            found = self.pick_one(answer_type_id, require_artwork, 1).await?;
        }
        let Some((id, provider, provider_id, preview_url, artwork_url)) = found else {
            return Ok(None);
        };
        // `$track->audio` — live resolution for signed/expiring URLs.
        let audio = self
            .resolver
            .resolve(&provider, &provider_id.unwrap_or_default(), &preview_url.unwrap_or_default())
            .await;
        Ok(Some(MinigameTrack {
            id,
            preview_url: audio,
            artwork_url,
        }))
    }

    /// First track row with `tracks.id >= from_id` matching the filters
    /// (public playlist + typed answer + audio condition + artwork).
    async fn pick_one(
        &self,
        answer_type_id: i64,
        require_artwork: bool,
        from_id: i64,
    ) -> Result<Option<(i64, String, Option<String>, Option<String>, Option<String>)>, sqlx::Error>
    {
        let sql = format!(
            "SELECT t.id, t.provider, t.provider_id, t.preview_url, t.artwork_url \
             FROM tracks t \
             JOIN playlists p ON p.id = t.playlist_id AND p.is_public = true \
             JOIN track_answers ta ON ta.track_id = t.id AND ta.answer_type_id = $1 \
             WHERE t.provider != 'youtube' \
             AND (t.preview_url IS NOT NULL \
                  OR t.provider IN ('spotify','itunes','deezer','audius')) \
             {} AND t.id >= $2 ORDER BY t.id LIMIT 1",
            if require_artwork { "AND t.artwork_url IS NOT NULL" } else { "" }
        );
        sqlx::query(&sql)
            .bind(answer_type_id)
            .bind(from_id)
            .fetch_optional(&self.pool)
            .await
            .map(|o| {
                o.map(|r| {
                    (
                        r.get("id"),
                        r.get("provider"),
                        r.get("provider_id"),
                        r.get("preview_url"),
                        r.get("artwork_url"),
                    )
                })
            })
    }

    /// `getWrongChoices` — up to `count` distinct values from other
    /// public-playlist tracks, random `answer.id >= rand` probing ×3.
    async fn wrong_choices(
        &self,
        exclude_track: i64,
        answer_type_id: i64,
        exclude_value: &str,
        count: usize,
    ) -> Result<Vec<String>, sqlx::Error> {
        let max_id: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(ta.id),0) FROM track_answers ta \
             JOIN tracks t ON t.id = ta.track_id \
             JOIN playlists p ON p.id = t.playlist_id AND p.is_public = true \
             WHERE ta.answer_type_id = $1 AND ta.track_id != $2 AND ta.value IS NOT NULL",
        )
        .bind(answer_type_id)
        .bind(exclude_track)
        .fetch_one(&self.pool)
        .await?;
        if max_id < 1 {
            return Ok(vec![]);
        }
        let random_ids: Vec<i64> = {
            use rand::Rng;
            let mut rng = rand::rng();
            (0..3).map(|_| rng.random_range(1..=max_id)).collect()
        };
        let mut seen = std::collections::HashSet::new();
        seen.insert(exclude_value.to_string());
        let mut out = Vec::new();
        for rid in random_ids {
            if out.len() >= count {
                break;
            }
            let batch: Vec<String> = sqlx::query_scalar(
                "SELECT ta.value FROM track_answers ta \
                 JOIN tracks t ON t.id = ta.track_id \
                 JOIN playlists p ON p.id = t.playlist_id AND p.is_public = true \
                 WHERE ta.answer_type_id = $1 AND ta.track_id != $2 \
                 AND ta.value IS NOT NULL AND ta.value != $3 AND ta.id >= $4 \
                 ORDER BY ta.id LIMIT 5",
            )
            .bind(answer_type_id)
            .bind(exclude_track)
            .bind(exclude_value)
            .bind(rid)
            .fetch_all(&self.pool)
            .await?;
            for v in batch {
                if out.len() < count && seen.insert(v.clone()) {
                    out.push(v);
                }
            }
        }
        Ok(out)
    }

}

/// `collect([correct])->merge(wrong)->shuffle()` — PHP's shuffle.
fn shuffle<T>(v: &mut [T]) {
    use rand::seq::SliceRandom;
    v.shuffle(&mut rand::rng());
}

/// Bundled `database/data/anagram_artists.json` — parsed once.
fn anagram_list() -> &'static [(&'static str, &'static str)] {
    static LIST: std::sync::LazyLock<Vec<(&'static str, &'static str)>> =
        std::sync::LazyLock::new(|| {
            serde_json::from_str::<Vec<serde_json::Value>>(ANAGRAM_JSON)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|v| {
                    let artist = v.get("artist")?.as_str()?;
                    let anagram = v.get("anagram")?.as_str()?;
                    // SAFETY-free: leak boxed copies — list built once.
                    let artist: &'static str = Box::leak(artist.to_string().into_boxed_str());
                    let anagram: &'static str = Box::leak(anagram.to_string().into_boxed_str());
                    Some((artist, anagram))
                })
                .collect()
        });
    &LIST
}

const ANAGRAM_JSON: &str = include_str!("../data/anagram_artists.json");
