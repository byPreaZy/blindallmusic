use blindall_game::store::PreviewResolver;
use serde::Serialize;
use sqlx::Row;
use sqlx::postgres::PgPool;
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
        let Some(track) = self
            .pick_random_track(answer_type_id, require_artwork)
            .await?
        else {
            return Ok(None);
        };
        let correct = self
            .answer_of(track.id, answer_type_id)
            .await?
            .unwrap_or_default();
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

    pub async fn track_with_title(&self) -> Result<Option<TitleQuestion>, sqlx::Error> {
        let Some(track) = self.pick_random_track(ANSWER_TYPE_TITLE, false).await? else {
            return Ok(None);
        };
        let raw = self
            .answer_of(track.id, ANSWER_TYPE_TITLE)
            .await?
            .unwrap_or_default();
        Ok(Some(TitleQuestion {
            track,
            correct_value: blindall_core::sanitize::sanitize_string(&raw),
        }))
    }

    pub fn random_anagram(&self) -> Option<(String, String)> {
        let list = anagram_list();
        if list.is_empty() {
            return None;
        }
        use rand::Rng;
        let (artist, anagram) = list[rand::rng().random_range(0..list.len())];
        Some((anagram.to_string(), artist.to_string()))
    }

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

    pub async fn history_for_user(
        &self,
        user_id: i64,
        page: i64,
        per_page: i64,
    ) -> Result<serde_json::Value, sqlx::Error> {
        let total: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM minigame_scores WHERE user_id = $1")
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
        let random_ids: Vec<i64> = {
            use rand::Rng;
            let mut rng = rand::rng();
            (0..MAX_RANDOM_RETRIES)
                .map(|_| rng.random_range(1..=max_id))
                .collect()
        };
        let mut found = None;
        for rid in random_ids {
            found = self.pick_one(answer_type_id, require_artwork, rid).await?;
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
        let audio = self
            .resolver
            .resolve(
                &provider,
                &provider_id.unwrap_or_default(),
                &preview_url.unwrap_or_default(),
            )
            .await;
        Ok(Some(MinigameTrack {
            id,
            preview_url: audio,
            artwork_url,
        }))
    }

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
            if require_artwork {
                "AND t.artwork_url IS NOT NULL"
            } else {
                ""
            }
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

fn shuffle<T>(v: &mut [T]) {
    use rand::seq::SliceRandom;
    v.shuffle(&mut rand::rng());
}

fn anagram_list() -> &'static [(&'static str, &'static str)] {
    static LIST: std::sync::LazyLock<Vec<(&'static str, &'static str)>> =
        std::sync::LazyLock::new(|| {
            serde_json::from_str::<Vec<serde_json::Value>>(ANAGRAM_JSON)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|v| {
                    let artist = v.get("artist")?.as_str()?;
                    let anagram = v.get("anagram")?.as_str()?;
                    // SAFETY-free: leak boxed copies, list built once.
                    let artist: &'static str = Box::leak(artist.to_string().into_boxed_str());
                    let anagram: &'static str = Box::leak(anagram.to_string().into_boxed_str());
                    Some((artist, anagram))
                })
                .collect()
        });
    &LIST
}

const ANAGRAM_JSON: &str = include_str!("../data/anagram_artists.json");

// ---- duels & daily challenge ---------------------------------------------------

impl MinigameRepo {
    /// Serializable question (includes correct_value, kept server-side in
    /// the duel/daily row and stripped before it reaches the client).
    pub async fn gen_question(&self, game_type: &str) -> Option<serde_json::Value> {
        match game_type {
            "quiz" | "who_sang" | "album_cover" => {
                let q = self.track_with_choices(game_type).await.ok()??;
                Some(serde_json::json!({
                    "track": q.track,
                    "choices": q.choices,
                    "correct_value": q.correct_value,
                }))
            }
            "first_letter" => {
                let q = self.track_with_title().await.ok()??;
                Some(serde_json::json!({
                    "track": q.track,
                    "correct_value": q.correct_value,
                }))
            }
            "anagram" => {
                let (scrambled, artist) = self.random_anagram()?;
                Some(serde_json::json!({
                    "scrambled_artist": scrambled,
                    "correct_value": artist,
                }))
            }
            _ => None,
        }
    }

    /// `n` questions with distinct track ids when the type is track-based.
    pub async fn gen_question_set(&self, game_type: &str, n: usize) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let track_based = game_type != "anagram";
        for _ in 0..n * 4 {
            if out.len() >= n {
                break;
            }
            let Some(q) = self.gen_question(game_type).await else {
                break;
            };
            let key = q["track"]["id"].as_i64().unwrap_or_else(|| rand::random());
            if !track_based || seen.insert(key) {
                out.push(q);
            }
        }
        out
    }

    pub async fn create_duel(
        &self,
        creator_id: i64,
        opponent_id: Option<i64>,
        game_type: &str,
        questions: &serde_json::Value,
    ) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO minigame_duels (game_type, creator_id, opponent_id, questions)
             VALUES ($1, $2, $3, $4::jsonb) RETURNING id",
        )
        .bind(game_type)
        .bind(creator_id)
        .bind(opponent_id)
        .bind(serde_json::to_string(questions).unwrap_or_else(|_| "[]".into()))
        .fetch_one(&self.pool)
        .await
    }

    pub async fn duel(&self, id: i64) -> Result<Option<serde_json::Value>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT d.*, c.name AS creator_name, o.name AS opponent_name
             FROM minigame_duels d
             JOIN users c ON c.id = d.creator_id
             LEFT JOIN users o ON o.id = d.opponent_id
             WHERE d.id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| {
            serde_json::json!({
                "id": r.get::<i64, _>("id"),
                "game_type": r.get::<String, _>("game_type"),
                "status": r.get::<String, _>("status"),
                "creator_id": r.get::<i64, _>("creator_id"),
                "opponent_id": r.get::<Option<i64>, _>("opponent_id"),
                "creator_name": r.get::<String, _>("creator_name"),
                "opponent_name": r.get::<Option<String>, _>("opponent_name"),
                "creator_score": r.get::<i32, _>("creator_score"),
                "opponent_score": r.get::<i32, _>("opponent_score"),
                "creator_done": r.get::<bool, _>("creator_done"),
                "opponent_done": r.get::<bool, _>("opponent_done"),
                "creator_answers": r.get::<serde_json::Value, _>("creator_answers"),
                "opponent_answers": r.get::<serde_json::Value, _>("opponent_answers"),
                "questions": r.get::<serde_json::Value, _>("questions"),
                "created_at": r.get::<chrono::DateTime<chrono::Utc>, _>("created_at")
                    .format("%Y-%m-%dT%H:%M:%SZ").to_string(),
            })
        }))
    }

    /// Duels visible to the user: own duels (recent first) + open challenges
    /// from other players.
    pub async fn duels_for(&self, user_id: i64) -> Result<Vec<serde_json::Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT d.id, d.game_type, d.status, d.creator_id, d.opponent_id,
                    d.creator_score, d.opponent_score, d.creator_done, d.opponent_done,
                    jsonb_array_length(d.questions)::bigint AS qcount,
                    c.name AS creator_name, o.name AS opponent_name, d.created_at
             FROM minigame_duels d
             JOIN users c ON c.id = d.creator_id
             LEFT JOIN users o ON o.id = d.opponent_id
             WHERE d.creator_id = $1 OR d.opponent_id = $1
                OR (d.status = 'open' AND d.creator_id <> $1)
             ORDER BY d.created_at DESC
             LIMIT 50",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(duel_listing).collect())
    }

    /// Accept an open duel (targeted or public). 0 rows = not acceptable.
    pub async fn accept_duel(&self, id: i64, user_id: i64) -> Result<u64, sqlx::Error> {
        sqlx::query(
            "UPDATE minigame_duels SET opponent_id = $2, status = 'active', updated_at = NOW()
             WHERE id = $1 AND status = 'open' AND creator_id <> $2
               AND (opponent_id IS NULL OR opponent_id = $2)",
        )
        .bind(id)
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map(|r| r.rows_affected())
    }

    /// Record one answer on a side ('creator'|'opponent'). Replay-safe per
    /// index; auto-completes the side and finishes the duel when both are
    /// done. Returns the refreshed row or None on replay.
    pub async fn duel_record(
        &self,
        id: i64,
        side: &str,
        user_id: i64,
        idx: i64,
        correct: bool,
        given: &str,
    ) -> Result<Option<serde_json::Value>, sqlx::Error> {
        let (c, o) = if side == "creator" {
            ("creator", "opponent")
        } else {
            ("opponent", "creator")
        };
        let answer = serde_json::json!([{"idx": idx, "correct": correct, "value": given}]);
        let sql = format!(
            "UPDATE minigame_duels SET
               {c}_answers = {c}_answers || $3::jsonb,
               {c}_score = {c}_score + $4,
               {c}_done = (jsonb_array_length({c}_answers || $3::jsonb)
                          >= jsonb_array_length(questions)),
               status = CASE
                 WHEN jsonb_array_length({c}_answers || $3::jsonb)
                       >= jsonb_array_length(questions)
                      AND {o}_done THEN 'finished' ELSE status END,
               updated_at = NOW()
             WHERE id = $1 AND {c}_id = $2
               AND NOT EXISTS (SELECT 1 FROM jsonb_array_elements({c}_answers) a
                               WHERE (a->>'idx')::bigint = $5)
             RETURNING id, creator_score, opponent_score, creator_done, opponent_done, status",
            c = c,
            o = o
        );
        let row = sqlx::query(&sql)
            .bind(id)
            .bind(user_id)
            .bind(serde_json::to_string(&answer).unwrap_or_else(|_| "[]".into()))
            .bind(if correct { 1i32 } else { 0i32 })
            .bind(idx)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| {
            serde_json::json!({
                "id": r.get::<i64, _>("id"),
                "creator_score": r.get::<i32, _>("creator_score"),
                "opponent_score": r.get::<i32, _>("opponent_score"),
                "creator_done": r.get::<bool, _>("creator_done"),
                "opponent_done": r.get::<bool, _>("opponent_done"),
                "status": r.get::<String, _>("status"),
            })
        }))
    }

    /// Wins leaderboard across finished duels (draws listed separately).
    pub async fn duel_leaderboard(
        &self,
        limit: i64,
    ) -> Result<Vec<serde_json::Value>, sqlx::Error> {
        let rows = sqlx::query(
            "WITH per_user AS (
               SELECT creator_id AS uid, opponent_score AS mine, creator_score AS theirs
               FROM minigame_duels WHERE status = 'finished'
               UNION ALL
               SELECT opponent_id, creator_score, opponent_score
               FROM minigame_duels WHERE status = 'finished'
             )
             SELECT u.id, u.name, u.elo,
                    COUNT(*) FILTER (WHERE mine > theirs) AS wins,
                    COUNT(*) FILTER (WHERE mine = theirs) AS draws,
                    COUNT(*) AS total
             FROM per_user p
             JOIN users u ON u.id = p.uid
             GROUP BY u.id, u.name, u.elo
             ORDER BY wins DESC, draws DESC, u.elo DESC
             LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.get::<i64, _>("id"),
                    "name": r.get::<String, _>("name"),
                    "elo": r.get::<i64, _>("elo"),
                    "wins": r.get::<i64, _>("wins"),
                    "draws": r.get::<i64, _>("draws"),
                    "total": r.get::<i64, _>("total"),
                })
            })
            .collect())
    }

    // ---- daily -----------------------------------------------------------------

    /// Get-or-generate today's question set for a game type (fixed at first
    /// request, shared by everyone).
    pub async fn daily_questions(
        &self,
        day: chrono::NaiveDate,
        game_type: &str,
        n: usize,
    ) -> Result<Vec<serde_json::Value>, sqlx::Error> {
        if let Some(v) = sqlx::query_scalar::<_, serde_json::Value>(
            "SELECT questions FROM minigame_daily WHERE day = $1 AND game_type = $2",
        )
        .bind(day)
        .bind(game_type)
        .fetch_optional(&self.pool)
        .await?
        {
            return Ok(v.as_array().cloned().unwrap_or_default());
        }
        let qs = self.gen_question_set(game_type, n).await;
        sqlx::query(
            "INSERT INTO minigame_daily (day, game_type, questions)
             VALUES ($1, $2, $3::jsonb) ON CONFLICT DO NOTHING",
        )
        .bind(day)
        .bind(game_type)
        .bind(serde_json::to_string(&qs).unwrap_or_else(|_| "[]".into()))
        .execute(&self.pool)
        .await?;
        Ok(qs)
    }

    /// Record a daily answer. Replay-safe per index via WHERE NOT EXISTS.
    /// Returns (score, answered_count) or None on replay.
    pub async fn daily_record(
        &self,
        user_id: i64,
        day: chrono::NaiveDate,
        game_type: &str,
        idx: i64,
        correct: bool,
        given: &str,
    ) -> Result<Option<(i64, i64)>, sqlx::Error> {
        let answer = serde_json::json!([{"idx": idx, "correct": correct, "value": given}]);
        let res = sqlx::query(
            "INSERT INTO minigame_daily_scores (user_id, day, game_type, score, answers)
             VALUES ($1, $2, $3, $4, $5::jsonb)
             ON CONFLICT (user_id, day, game_type) DO UPDATE SET
               answers = minigame_daily_scores.answers || EXCLUDED.answers,
               score = minigame_daily_scores.score + EXCLUDED.score,
               updated_at = NOW()
             WHERE NOT EXISTS (
               SELECT 1 FROM jsonb_array_elements(minigame_daily_scores.answers) a
               WHERE (a->>'idx')::bigint = $6
             )
             RETURNING score, jsonb_array_length(answers)::bigint",
        )
        .bind(user_id)
        .bind(day)
        .bind(game_type)
        .bind(if correct { 1i32 } else { 0i32 })
        .bind(serde_json::to_string(&answer).unwrap_or_else(|_| "[]".into()))
        .bind(idx)
        .fetch_optional(&self.pool)
        .await?;
        Ok(res.map(|r| {
            (
                r.get::<i32, _>("score") as i64,
                r.get::<i64, _>("jsonb_array_length"),
            )
        }))
    }

    pub async fn daily_progress(
        &self,
        user_id: i64,
        day: chrono::NaiveDate,
        game_type: &str,
    ) -> Result<Option<(i64, i64)>, sqlx::Error> {
        sqlx::query(
            "SELECT score, jsonb_array_length(answers)::bigint FROM minigame_daily_scores
             WHERE user_id = $1 AND day = $2 AND game_type = $3",
        )
        .bind(user_id)
        .bind(day)
        .bind(game_type)
        .fetch_optional(&self.pool)
        .await
        .map(|o| {
            o.map(|r| {
                (
                    r.get::<i32, _>("score") as i64,
                    r.get::<i64, _>("jsonb_array_length"),
                )
            })
        })
    }

    /// Today's leaderboard: higher score first, earlier finish wins ties.
    pub async fn daily_leaderboard(
        &self,
        day: chrono::NaiveDate,
        game_type: &str,
        limit: i64,
    ) -> Result<Vec<serde_json::Value>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT u.id, u.name, u.elo, s.score, s.answers
             FROM minigame_daily_scores s
             JOIN users u ON u.id = s.user_id
             WHERE s.day = $1 AND s.game_type = $2
             ORDER BY s.score DESC, s.updated_at ASC
             LIMIT $3",
        )
        .bind(day)
        .bind(game_type)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.get::<i64, _>("id"),
                    "name": r.get::<String, _>("name"),
                    "elo": r.get::<i64, _>("elo"),
                    "score": r.get::<i32, _>("score"),
                    "answered": r.get::<serde_json::Value, _>("answers")
                        .as_array().map(|a| a.len()).unwrap_or(0),
                })
            })
            .collect())
    }
}

fn duel_listing(r: &sqlx::postgres::PgRow) -> serde_json::Value {
    use sqlx::Row;
    serde_json::json!({
        "id": r.get::<i64, _>("id"),
        "game_type": r.get::<String, _>("game_type"),
        "status": r.get::<String, _>("status"),
        "creator_id": r.get::<i64, _>("creator_id"),
        "opponent_id": r.get::<Option<i64>, _>("opponent_id"),
        "creator_score": r.get::<i32, _>("creator_score"),
        "opponent_score": r.get::<i32, _>("opponent_score"),
        "creator_done": r.get::<bool, _>("creator_done"),
        "opponent_done": r.get::<bool, _>("opponent_done"),
        "question_count": r.get::<i64, _>("qcount"),
        "creator_name": r.get::<String, _>("creator_name"),
        "opponent_name": r.get::<Option<String>, _>("opponent_name"),
        "created_at": r.get::<chrono::DateTime<chrono::Utc>, _>("created_at")
            .format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    })
}
