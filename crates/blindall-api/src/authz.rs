use blindall_game::GameStore;
use blindall_realtime::ws::{AuthzError, ChannelAuthorizer};
use serde_json::{Value, json};

pub struct RoomChannelAuthorizer {
    pub app: crate::state::AppState,
}

#[async_trait::async_trait]
impl ChannelAuthorizer for RoomChannelAuthorizer {
    async fn authorize(&self, user_id: i64, channel: &str) -> Result<Option<Value>, AuthzError> {
        let deny = |code: &'static str| Err(AuthzError { code });

        if let Some(rest) = channel.strip_prefix("room.public:") {
            return match rest.parse::<i64>() {
                Ok(room_id) if self.room_exists(room_id).await => Ok(None),
                _ => deny("room_not_found"),
            };
        }
        if let Some(rest) = channel.strip_prefix("room:") {
            return match rest.parse::<i64>() {
                Ok(room_id) if self.room_exists(room_id).await => {
                    let info = self
                        .app
                        .store
                        .players_info(&[user_id])
                        .await
                        .ok()
                        .and_then(|m| m.get(&user_id).cloned());
                    let Some(p) = info else {
                        return Ok(None);
                    };
                    let score_total = match self
                        .app
                        .manager
                        .snapshot(room_id)
                        .await
                        .ok()
                        .flatten()
                        .and_then(|s| s.round.filter(|r| r.finished_at.is_none()))
                    {
                        Some(r) => self
                            .app
                            .scores
                            .user_score(r.id, user_id)
                            .await
                            .unwrap_or(0.0),
                        None => 0.0,
                    };
                    let perks: Vec<String> = if !p.is_guest
                        && self
                            .app
                            .donations_repo
                            .user_is_supporter(
                                user_id,
                                &blindall_db::donations::current_month_key(),
                            )
                            .await
                            .unwrap_or(false)
                    {
                        self.app.donations.supporter_perks.clone()
                    } else {
                        vec![]
                    };
                    Ok(Some(crate::rooms::player_info_json(
                        &self.app,
                        &p,
                        score_total,
                        &perks,
                    )))
                }
                _ => deny("room_not_found"),
            };
        }
        // Live minigame match — only its players may subscribe.
        if let Some(rest) = channel.strip_prefix("match.") {
            return match rest.parse::<i64>() {
                Ok(match_id) if self.app.matchmaking.is_participant(match_id, user_id) => {
                    let info = self
                        .app
                        .store
                        .players_info(&[user_id])
                        .await
                        .ok()
                        .and_then(|m| m.get(&user_id).cloned());
                    Ok(Some(json!({
                        "id": user_id,
                        "name": info.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
                        "photo": info.as_ref().and_then(|p| p.photo.clone()),
                    })))
                }
                _ => deny("forbidden"),
            };
        }
        if let Some(rest) = channel.strip_prefix("chat-room.") {
            return match rest.parse::<i64>() {
                Ok(room_id) if self.room_exists(room_id).await => {
                    let info = self
                        .app
                        .store
                        .players_info(&[user_id])
                        .await
                        .ok()
                        .and_then(|m| m.get(&user_id).cloned());
                    Ok(Some(json!({
                        "id": user_id,
                        "name": info.as_ref().map(|p| p.name.clone()).unwrap_or_default(),
                        "team": info.as_ref().and_then(|p| {
                            p.team_id.map(|id| json!({
                                "id": id,
                                "name": p.team_name.clone().unwrap_or_default(),
                            }))
                        }),
                        "photo": info.as_ref().and_then(|p| p.photo.clone()),
                        "role": info.as_ref().map(|p| if p.is_administrator { "admin" } else if p.is_moderator { "moderator" } else { "" }).unwrap_or(""),
                        "custom_tag": info.as_ref().and_then(|p| p.custom_tag.as_deref()).unwrap_or(""),
                    })))
                }
                _ => deny("room_not_found"),
            };
        }
        if let Some(rest) = channel.strip_prefix("chat.message.") {
            return match rest.parse::<i64>() {
                Ok(_) => Ok(None),
                _ => deny("invalid_channel"),
            };
        }
        if let Some(rest) = channel.strip_prefix("user:") {
            return match rest.parse::<i64>() {
                Ok(id) if id == user_id => Ok(None),
                _ => deny("forbidden"),
            };
        }
        deny("unknown_channel")
    }
}

impl RoomChannelAuthorizer {
    async fn room_exists(&self, room_id: i64) -> bool {
        self.app.store.room_config(room_id).await.is_ok()
    }
}
