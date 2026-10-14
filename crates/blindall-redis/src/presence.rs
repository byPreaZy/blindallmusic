use crate::keys;
use crate::Result;
use fred::prelude::*;

#[derive(Clone)]
pub struct RoomPresenceStore {
    client: Client,
}

impl RoomPresenceStore {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    pub async fn add_member(&self, room_id: i64, user_id: i64, now_unix: i64) -> Result<()> {
        let key = keys::room_members(room_id);
        self.client
            .zadd::<(), _, _>(
                &key,
                None,
                None,
                false,
                false,
                vec![(now_unix as f64, user_id)],
            )
            .await?;
        self.client
            .expire::<(), _>(&key, keys::PRESENCE_TTL_SECONDS, None)
            .await?;
        Ok(())
    }

    pub async fn remove_member(&self, room_id: i64, user_id: i64) -> Result<()> {
        self.client
            .zrem::<(), _, _>(keys::room_members(room_id), user_id)
            .await?;
        Ok(())
    }

    pub async fn member_count(&self, room_id: i64, now_unix: i64) -> Result<i64> {
        let key = keys::room_members(room_id);
        let cutoff = now_unix - keys::PRESENCE_PRUNE_SECONDS;
        let _: () = self
            .client
            .zremrangebyscore(&key, f64::NEG_INFINITY, cutoff as f64)
            .await?;
        self.client.zcard(&key).await.map_err(Into::into)
    }

    pub async fn member_ids(&self, room_id: i64, now_unix: i64) -> Result<Vec<i64>> {
        let key = keys::room_members(room_id);
        let cutoff = now_unix - keys::PRESENCE_PRUNE_SECONDS;
        let _: () = self
            .client
            .zremrangebyscore(&key, f64::NEG_INFINITY, cutoff as f64)
            .await?;
        self.client
            .zrange(&key, 0, -1, None, false, None, false)
            .await
            .map_err(Into::into)
    }

    pub async fn grant_participation(&self, room_id: i64, user_id: i64) -> Result<()> {
        let key = keys::room_players(room_id);
        self.client.sadd::<(), _, _>(&key, user_id).await?;
        self.client
            .expire::<(), _>(&key, keys::PRESENCE_TTL_SECONDS, None)
            .await?;
        Ok(())
    }

    pub async fn has_participation(&self, room_id: i64, user_id: i64) -> Result<bool> {
        self.client
            .sismember(keys::room_players(room_id), user_id)
            .await
            .map_err(Into::into)
    }

    pub async fn member_counts(
        &self,
        room_ids: &[i64],
        now_unix: i64,
    ) -> Result<Vec<(i64, i64)>> {
        let cutoff = now_unix - keys::PRESENCE_PRUNE_SECONDS;
        let pipe = self.client.pipeline();
        for id in room_ids {
            let key = keys::room_members(*id);
            let _ = pipe
                .zremrangebyscore::<(), _, _, _>(&key, f64::NEG_INFINITY, cutoff as f64)
                .await?;
            let _ = pipe.zcard::<i64, _>(&key).await?;
        }
        let results: Vec<i64> = pipe.all().await?;
        Ok(room_ids
            .iter()
            .copied()
            .zip(results.into_iter().skip(1).step_by(2))
            .collect())
    }
}
