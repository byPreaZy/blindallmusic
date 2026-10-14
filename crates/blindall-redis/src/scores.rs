//! Port of `RoundScoreService` — every method, same keys, same TTLs.
//!
//! Multi-command sequences run inside Lua so each public method is atomic.
//! One semantic improvement over PHP is documented: `record_answer`
//! computes `order` inside the script, making first/second/third truly
//! sequential under concurrency (PHP did SCARD-then-SADD in two calls and
//! could hand the same order to two racers).

use crate::keys;
use crate::Result;
use fred::prelude::*;
use serde::{Deserialize, Serialize};

/// One element of `round:{id}:tracks:{userId}` — same shape as PHP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackRecord {
    pub track_id: i64,
    #[serde(default)]
    pub answer_id: Option<i64>,
    #[serde(default)]
    pub response_time: Option<f64>,
    #[serde(default)]
    pub position: Option<i64>,
    #[serde(default)]
    pub score: Option<f64>,
}

/// addScore: track player, bump score + podium, refresh TTLs — atomic.
const LUA_ADD_SCORE: &str = r#"
redis.call('SADD', KEYS[1], ARGV[1])
redis.call('EXPIRE', KEYS[1], ARGV[3])
redis.call('INCRBYFLOAT', KEYS[2], ARGV[2])
redis.call('EXPIRE', KEYS[2], ARGV[3])
redis.call('ZINCRBY', KEYS[3], ARGV[2], ARGV[1])
redis.call('EXPIRE', KEYS[3], ARGV[3])
return 1
"#;

/// record_answer (dup-check + global players set + history write).
/// KEYS: answers_key, global_key, history_key
/// ARGV: answer_id, user_id, ttl, history_json
/// Returns {was_new(0/1), global_count_before}.
const LUA_RECORD_ANSWER: &str = r#"
local was_added = redis.call('SADD', KEYS[1], ARGV[1])
redis.call('EXPIRE', KEYS[1], ARGV[3])
local before = redis.call('SCARD', KEYS[2])
if was_added == 0 then
    return {0, before}
end
redis.call('SADD', KEYS[2], ARGV[2])
redis.call('EXPIRE', KEYS[2], ARGV[3])
redis.call('SET', KEYS[3], ARGV[4])
redis.call('EXPIRE', KEYS[3], ARGV[3])
return {1, before}
"#;

/// record_listened (no answer): track player + history write.
const LUA_RECORD_LISTENED: &str = r#"
redis.call('SADD', KEYS[1], ARGV[1])
redis.call('EXPIRE', KEYS[1], ARGV[2])
redis.call('SET', KEYS[2], ARGV[3])
redis.call('EXPIRE', KEYS[2], ARGV[2])
return 1
"#;

#[derive(Clone)]
pub struct RoundScoreStore {
    client: Client,
}

impl RoundScoreStore {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    /// `getUserScore` — 0.0 when unset.
    pub async fn user_score(&self, round_id: i64, user_id: i64) -> Result<f64> {
        let v: Option<f64> = self
            .client
            .get(keys::round_user_score(round_id, user_id))
            .await?;
        Ok(v.unwrap_or(0.0))
    }

    /// `addScore` — atomic SADD/INCRBYFLOAT/ZINCRBY + TTLs.
    pub async fn add_score(&self, round_id: i64, user_id: i64, score: f64) -> Result<()> {
        self.client
            .eval::<(), _, _, _>(
                LUA_ADD_SCORE,
                vec![
                    keys::round_players(round_id),
                    keys::round_user_score(round_id, user_id),
                    keys::round_podium(round_id),
                ],
                vec![
                    user_id.to_string(),
                    score.to_string(),
                    keys::ROUND_TTL_SECONDS.to_string(),
                ],
            )
            .await?;
        Ok(())
    }

    /// `getPodium` — ZREVRANGE WITHSCORES → ordered (user_id, score) pairs.
    pub async fn podium(&self, round_id: i64, limit: usize) -> Result<Vec<(i64, f64)>> {
        let raw: Vec<(String, f64)> = self
            .client
            .zrevrange(
                keys::round_podium(round_id),
                0,
                (limit.max(1) - 1) as i64,
                true,
            )
            .await?;
        Ok(raw
            .into_iter()
            .filter_map(|(m, s)| m.parse::<i64>().ok().map(|id| (id, s)))
            .collect())
    }

    /// `getAllScores` — podium(10_000) else per-user fallback (legacy rounds).
    pub async fn all_scores(&self, round_id: i64) -> Result<Vec<(i64, f64)>> {
        let scores = self.podium(round_id, 10_000).await?;
        if !scores.is_empty() {
            return Ok(scores);
        }
        let mut out = Vec::new();
        for uid in self.player_ids(round_id).await? {
            if let Some(s) = self
                .client
                .get::<Option<f64>, _>(keys::round_user_score(round_id, uid))
                .await?
            {
                out.push((uid, s));
            }
        }
        Ok(out)
    }

    /// `recordTrackDetails` with answer_id → (was_new). History JSON merge
    /// happens in Rust (same non-atomic GET-modify-SET window as PHP), the
    /// SADD dup-check + global set + SET run atomically in Lua.
    pub async fn record_answer(
        &self,
        round_id: i64,
        user_id: i64,
        record: &TrackRecord,
    ) -> Result<bool> {
        let answer_id = record.answer_id.expect("record_answer needs answer_id");
        let history = self.merged_history(round_id, user_id, record).await?;
        let res: Vec<i64> = self
            .client
            .eval(
                LUA_RECORD_ANSWER,
                vec![
                    keys::round_user_answers(round_id, user_id, record.track_id),
                    keys::round_answer_players(round_id, answer_id),
                    keys::round_user_tracks(round_id, user_id),
                ],
                vec![
                    answer_id.to_string(),
                    user_id.to_string(),
                    keys::ROUND_TTL_SECONDS.to_string(),
                    history,
                ],
            )
            .await?;
        Ok(res.first().copied().unwrap_or(0) == 1)
    }

    /// `recordTrackDetails` without answer — track-listened path.
    pub async fn record_listened(
        &self,
        round_id: i64,
        user_id: i64,
        record: &TrackRecord,
    ) -> Result<()> {
        let history = self.merged_history(round_id, user_id, record).await?;
        self.client
            .eval::<(), _, _, _>(
                LUA_RECORD_LISTENED,
                vec![
                    keys::round_players(round_id),
                    keys::round_user_tracks(round_id, user_id),
                ],
                vec![
                    user_id.to_string(),
                    keys::ROUND_TTL_SECONDS.to_string(),
                    history,
                ],
            )
            .await?;
        Ok(())
    }

    /// GET history, merge `record` into it (PHP merge semantics), return JSON.
    async fn merged_history(
        &self,
        round_id: i64,
        user_id: i64,
        record: &TrackRecord,
    ) -> Result<String> {
        let mut tracks = self.tracks_history(round_id, user_id).await?;

        let existing = tracks.iter_mut().position(|t| match record.answer_id {
            Some(aid) => t.track_id == record.track_id && t.answer_id == Some(aid),
            None => t.track_id == record.track_id,
        });

        match existing {
            Some(i) => {
                let t = &mut tracks[i];
                // array_merge($existing, array_filter($new, non-null))
                if record.answer_id.is_some() {
                    t.answer_id = record.answer_id;
                }
                if let Some(rt) = record.response_time {
                    t.response_time = Some(rt);
                }
                if let Some(p) = record.position {
                    t.position = Some(p);
                }
                if let Some(s) = record.score {
                    t.score = Some(s);
                }
            }
            None => tracks.push(record.clone()),
        }

        Ok(serde_json::to_string(&tracks)?)
    }

    /// `getTracksHistory` — decoded JSON array, empty on missing/invalid.
    pub async fn tracks_history(
        &self,
        round_id: i64,
        user_id: i64,
    ) -> Result<Vec<TrackRecord>> {
        let raw: Option<String> = self
            .client
            .get(keys::round_user_tracks(round_id, user_id))
            .await?;
        let Some(raw) = raw else {
            return Ok(vec![]);
        };
        if raw.is_empty() {
            return Ok(vec![]);
        }
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    }

    /// `getFoundAnswerIds` — SMEMBERS; falls back to history scan.
    pub async fn found_answer_ids(
        &self,
        round_id: i64,
        user_id: i64,
        track_id: i64,
    ) -> Result<Vec<i64>> {
        let ids: Vec<i64> = self
            .client
            .smembers(keys::round_user_answers(round_id, user_id, track_id))
            .await?;
        if !ids.is_empty() {
            return Ok(ids);
        }
        let history = self.tracks_history(round_id, user_id).await?;
        let mut out: Vec<i64> = history
            .into_iter()
            .filter(|t| t.track_id == track_id)
            .filter_map(|t| t.answer_id)
            .collect();
        out.sort_unstable();
        out.dedup();
        Ok(out)
    }

    /// `countPlayersWhoFoundAnswer` — SCARD minus 1 if user inside;
    /// no DB fallback here (caller handles legacy rounds if needed).
    pub async fn players_who_found(
        &self,
        round_id: i64,
        answer_id: i64,
        exclude_user: Option<i64>,
    ) -> Result<i64> {
        let key = keys::round_answer_players(round_id, answer_id);
        let mut count: i64 = self.client.scard(&key).await?;
        if let Some(uid) = exclude_user {
            if count > 0 && self.client.sismember::<bool, _, _>(&key, uid).await? {
                count -= 1;
            }
        }
        Ok(count)
    }

    /// `countFoundAnswersForUser`.
    pub async fn count_found_for_user(
        &self,
        round_id: i64,
        user_id: i64,
        track_id: i64,
    ) -> Result<usize> {
        Ok(self.found_answer_ids(round_id, user_id, track_id).await?.len())
    }

    /// `getAllTracksHistory` — per-user history for every tracked player.
    pub async fn all_tracks_history(
        &self,
        round_id: i64,
    ) -> Result<Vec<(i64, Vec<TrackRecord>)>> {
        let mut out = Vec::new();
        for uid in self.player_ids(round_id).await? {
            let h = self.tracks_history(round_id, uid).await?;
            if !h.is_empty() {
                out.push((uid, h));
            }
        }
        Ok(out)
    }

    /// `cleanup` — DEL podium/players + per-user keys (same set as PHP;
    /// per-answer keys rely on TTL, identical to the PHP behaviour).
    pub async fn cleanup(&self, round_id: i64) -> Result<()> {
        let players = self.player_ids(round_id).await?;
        let mut keys_to_del = vec![keys::round_podium(round_id), keys::round_players(round_id)];
        for uid in players {
            keys_to_del.push(keys::round_user_score(round_id, uid));
            keys_to_del.push(keys::round_user_tracks(round_id, uid));
        }
        keys_to_del.sort();
        keys_to_del.dedup();
        self.client.del::<(), _>(keys_to_del).await?;
        Ok(())
    }

    async fn player_ids(&self, round_id: i64) -> Result<Vec<i64>> {
        let ids: Vec<i64> = self.client.smembers(keys::round_players(round_id)).await?;
        Ok(ids)
    }
}
