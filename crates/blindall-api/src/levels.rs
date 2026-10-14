//! `UpdateUserLevel` dispatch equivalent — dedup per user for 120s
//! (Laravel `uniqueFor`), runs the recalc in a spawned task and
//! broadcasts `UserLevelUpdated` on the user's private channel.

use blindall_db::{LevelRepo, LevelTrigger};
use blindall_realtime::Hub;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const UNIQUE_FOR: Duration = Duration::from_secs(120);

pub struct LevelDispatcher {
    levels: LevelRepo,
    hub: Hub,
    last_dispatch: Mutex<HashMap<i64, Instant>>,
}

impl LevelDispatcher {
    pub fn new(levels: LevelRepo, hub: Hub) -> Self {
        Self {
            levels,
            hub,
            last_dispatch: Mutex::new(HashMap::new()),
        }
    }

    /// Read-side passthrough — `auth.user` level snapshot.
    pub async fn snapshot(
        &self,
        user_id: i64,
    ) -> Option<serde_json::Value> {
        self.levels.snapshot(user_id).await.ok().flatten()
    }

    /// Fire-and-forget equivalent of `UpdateUserLevel::dispatch`.
    /// Skipped silently if a recalc ran for this user < 120s ago.
    pub fn dispatch(&self, user_id: i64, trigger: LevelTrigger) {
        {
            let mut map = self.last_dispatch.lock().unwrap();
            let now = Instant::now();
            if map.len() > 10_000 {
                map.retain(|_, t| now.duration_since(*t) < UNIQUE_FOR);
            }
            if map.get(&user_id).is_some_and(|t| now.duration_since(*t) < UNIQUE_FOR) {
                return;
            }
            map.insert(user_id, now);
        }

        let levels = self.levels.clone();
        let hub = self.hub.clone();
        tokio::spawn(async move {
            match levels.update(user_id, trigger).await {
                Ok(Some(payload)) => {
                    hub.broadcast(
                        &format!("user:{user_id}"),
                        "UserLevelUpdated",
                        payload,
                        None,
                    )
                    .await;
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::warn!(user_id, error = %e, "user level update failed")
                }
            }
        });
    }

    #[cfg(test)]
    fn dispatched_count(&self) -> usize {
        self.last_dispatch.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dispatcher() -> LevelDispatcher {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://u:u@127.0.0.1/u")
            .unwrap();
        LevelDispatcher::new(LevelRepo::new(pool), Hub::new())
    }

    #[tokio::test(flavor = "current_thread")]
    async fn dedup_within_unique_for_window() {
        let d = dispatcher();
        d.dispatch(1, LevelTrigger::Score);
        d.dispatch(1, LevelTrigger::Score); // skipped — < 120s
        d.dispatch(2, LevelTrigger::Login); // different user → allowed
        assert_eq!(d.dispatched_count(), 2);
    }
}
