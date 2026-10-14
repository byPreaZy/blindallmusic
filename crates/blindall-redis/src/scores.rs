use crate::keys;
use crate::Result;
use fred::prelude::*;
use serde::{Deserialize, Serialize};

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

const LUA_ADD_SCORE: &str = r#"
redis.call('SADD', KEYS[1], ARGV[1])
redis.call('EXPIRE', KEYS[1], ARGV[3])
redis.call('INCRBYFLOAT', KEYS[2], ARGV[2])
redis.call('EXPIRE', KEYS[2], ARGV[3])
redis.call('ZINCRBY', KEYS[3], ARGV[2], ARGV[1])
redis.call('EXPIRE', KEYS[3], ARGV[3])
return 1
"#;

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

    pub async fn user_score(&self, round_id: i64, user_id: i64) -> Result<f64> {
        let v: Option<f64> = self
            .client
            .get(keys::round_user_score(round_id, user_id))
            .await?;
        Ok(v.unwrap_or(0.0))
    }

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

    pub async fn count_found_for_user(
        &self,
        round_id: i64,
        user_id: i64,
        track_id: i64,
    ) -> Result<usize> {
        Ok(self.found_answer_ids(round_id, user_id, track_id).await?.len())
    }

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
