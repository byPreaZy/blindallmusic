//! Free-tier content limits (Blind'All Plus/Pro lift them entirely).
//!
//! Enforced at creation time only: existing content is grandfathered,
//! a free user over the cap simply cannot add more until they subscribe
//! or delete something.

use crate::state::AppState;

pub const FREE_MAX_ROOMS: i64 = 2;
pub const FREE_MAX_PLAYLISTS: i64 = 3;
pub const FREE_MAX_TRACKS_PER_PLAYLIST: i64 = 50;

/// True when the user has no entitled subscription. Admins are never
/// limited (they curate official content).
pub async fn plan_is_free(state: &AppState, user_id: i64, is_admin: bool) -> bool {
    if is_admin {
        return false;
    }
    state
        .subscriptions
        .active_plan(user_id)
        .await
        .ok()
        .flatten()
        .is_none()
}

/// Same check for content owned by someone else (e.g. a moderator adding
/// tracks to a playlist): the OWNER's plan governs, looked up cheaply.
pub async fn owner_is_limited(state: &AppState, owner_id: i64) -> bool {
    let is_admin: bool = sqlx::query_scalar("SELECT is_administrator FROM users WHERE id = $1")
        .bind(owner_id)
        .fetch_one(&state.pool)
        .await
        .unwrap_or(false);
    plan_is_free(state, owner_id, is_admin).await
}

/// The plan cap applies when there is no entitled plan and the user is
/// not an administrator. `plan` is the already-fetched active plan.
pub fn limited_for(plan: Option<&str>, is_admin: bool) -> bool {
    !is_admin && plan.is_none()
}

/// Usage counters + caps for the billing/status payload and the Pro page.
/// `max` is null when the user is unlimited (paid plan, admin).
pub async fn usage(state: &AppState, user_id: i64, limited: bool) -> serde_json::Value {
    let rooms_used = state.rooms.owned_active_count(user_id).await.unwrap_or(0);
    let playlists_used = state
        .playlists
        .count_owned_by(user_id)
        .await
        .unwrap_or(0);
    serde_json::json!({
        "rooms_used": rooms_used,
        "rooms_max": limited.then_some(FREE_MAX_ROOMS),
        "playlists_used": playlists_used,
        "playlists_max": limited.then_some(FREE_MAX_PLAYLISTS),
        "tracks_max": limited.then_some(FREE_MAX_TRACKS_PER_PLAYLIST),
    })
}
