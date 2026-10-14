//! `GET /api/v1/home` — homepage catalog, port of `HomeController::index` +
//! `HomeCatalogService` (tabs, category filter, popularity sort, pagination)
//! + `WeeklyTopUsers` + featured rooms + tab player counts.

use crate::auth::OptionalUser;
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use blindall_db::CatalogRoom;
use serde::Deserialize;
use serde_json::{json, Value};

const PER_PAGE: usize = 16;

#[derive(Deserialize)]
pub struct HomeQuery {
    tab: Option<String>,
    search: Option<String>,
    #[serde(default, deserialize_with = "de_cat_ids")]
    category_ids: Vec<i64>,
    category_id: Option<i64>,
    page: Option<usize>,
}

fn de_cat_ids<'de, D>(d: D) -> Result<Vec<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(i64),
        Many(Vec<i64>),
        Str(String),
    }
    Ok(match Option::<OneOrMany>::deserialize(d)? {
        Some(OneOrMany::One(v)) => vec![v],
        Some(OneOrMany::Many(v)) => v,
        Some(OneOrMany::Str(s)) => s
            .split(',')
            .filter_map(|x| x.trim().parse::<i64>().ok())
            .collect(),
        None => vec![],
    })
}

fn room_json(
    r: &CatalogRoom,
    storage: &std::sync::Arc<dyn crate::storage::Storage>,
) -> Value {
    let photo = r.photo_path.as_ref().map(|p| {
        if p.starts_with("http") {
            p.clone()
        } else {
            storage.url(p)
        }
    });
    json!({
        "id": r.id,
        "name": r.name,
        "slug": r.slug,
        "description": r.description,
        "photo": photo,
        "photo_path": r.photo_path,
        "color": r.color,
        "is_public": r.is_public,
        "is_autostart": r.is_autostart,
        "is_playing": r.is_playing,
        "is_password_protected": r.has_password,
        "tracks_by_round": r.tracks_by_round,
        "current_track_index": r.current_track_index,
        "subscriptions": r.subscriptions,
        "user_count": r.subscriptions,
        "rounds_count": r.rounds_count,
        "tracks_count": r.tracks_count,
        "category": r.category_id.map(|id| json!({
            "id": id,
            "name": r.category_name,
        })),
        "owner": r.owner_name.as_ref().map(|n| json!({
            "id": r.user_id,
            "name": n,
            "photo": r.owner_photo_path.as_ref().map(|p| storage.url(p)),
        })),
    })
}

/// `roomPopularitySortKey` — [subscriptions, is_playing, rounds_count, id] desc.
fn popularity_key(r: &CatalogRoom) -> (i64, i64, i64, i64) {
    (
        r.subscriptions,
        r.is_playing as i64,
        r.rounds_count,
        r.id,
    )
}

pub async fn index(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<std::net::SocketAddr>,
    Query(q): Query<HomeQuery>,
    user: OptionalUser,
) -> Result<Json<Value>, StatusCode> {
    // banhammer `ip.banned` + `auth.banned` middleware — `GET /` in PHP.
    let ip = crate::auth_session::client_ip(&headers, &addr.ip().to_string());
    if state.auth.is_ip_banned(&ip).await {
        return Err(StatusCode::FORBIDDEN);
    }
    if let Some(u) = &user.0 {
        if state.moderation.is_banned(u.id).await.unwrap_or(false) {
            return Err(StatusCode::FORBIDDEN);
        }
    }
    let tab = q.tab.as_deref().unwrap_or("official");
    let logged_in = user.0.as_ref().is_some_and(|u| !u.is_guest);
    let tab = match tab {
        "community" | "official" | "minigames" => tab,
        "mine" | "favorites" if logged_in => tab,
        _ => "official",
    };

    let category_ids: Vec<i64> = if !q.category_ids.is_empty() {
        q.category_ids.clone()
    } else {
        q.category_id.into_iter().collect()
    };

    let storage = state.storage.clone();
    let now = chrono::Utc::now().timestamp();

    // `?search=` — navbar room search (`HomeController` early-returns
    // `search_result` instead of the catalog).
    if let Some(search) = q.search.as_deref().map(str::trim).filter(|s| !s.is_empty())
    {
        let mut rooms = state.home.search_rooms(search).await.unwrap_or_default();
        for r in &mut rooms {
            r.subscriptions = state.presence.member_count(r.id, now).await.unwrap_or(0);
        }
        let ids: Vec<i64> = rooms.iter().map(|r| r.id).collect();
        if let Ok(idx) = state.home.live_track_indices(&ids).await {
            for r in &mut rooms {
                r.current_track_index = idx.get(&r.id).copied().unwrap_or(0);
            }
        }
        let data: Vec<Value> = rooms.iter().map(|r| room_json(r, &storage)).collect();
        return Ok(Json(json!({
            "filters": { "search": search },
            "search_result": data,
        })));
    }

    // --- catalog rows ---------------------------------------------------
    let mut rooms: Vec<CatalogRoom> = match tab {
        "community" => state.home.catalog_rooms(false).await.unwrap_or_default(),
        "favorites" => state
            .home
            .bookmarked_rooms(user.0.as_ref().map(|u| u.id).unwrap_or(0))
            .await
            .unwrap_or_default(),
        "mine" => state
            .home
            .moderated_private_rooms(user.0.as_ref().map(|u| u.id).unwrap_or(0))
            .await
            .unwrap_or_default(),
        _ => state.home.catalog_rooms(true).await.unwrap_or_default(),
    };

    // Live overlays: presence + current track index (+ private tracks_count).
    for r in &mut rooms {
        r.subscriptions = state.presence.member_count(r.id, now).await.unwrap_or(0);
    }
    let ids: Vec<i64> = rooms.iter().map(|r| r.id).collect();
    if let Ok(idx) = state.home.live_track_indices(&ids).await {
        for r in &mut rooms {
            r.current_track_index = idx.get(&r.id).copied().unwrap_or(0);
        }
    }
    if matches!(tab, "community" | "mine") {
        if let Ok(tc) = state.home.tracks_counts(&ids).await {
            for r in &mut rooms {
                r.tracks_count = tc.get(&r.id).copied();
            }
        }
    }

    rooms.sort_by(|a, b| popularity_key(b).cmp(&popularity_key(a)));

    // Category filter — explicit selection wins over hidden defaults.
    if !category_ids.is_empty() {
        rooms.retain(|r| r.category_id.is_some_and(|c| category_ids.contains(&c)));
    } else if tab == "official" {
        rooms.retain(|r| {
            !r.category_id
                .is_some_and(|c| state.hidden_category_ids.contains(&c))
        });
    }

    // --- pagination ------------------------------------------------------
    let total = rooms.len();
    let page = q.page.unwrap_or(1).max(1);
    let last_page = total.div_ceil(PER_PAGE).max(1);
    let data: Vec<Value> = rooms
        .into_iter()
        .skip((page - 1) * PER_PAGE)
        .take(PER_PAGE)
        .map(|r| room_json(&r, &storage))
        .collect();

    // --- sidebar props ----------------------------------------------------
    let (public_categories, community_categories, weekly_top_users, featured) = tokio::join!(
        state.home.categories_with_room_counts(true),
        state.home.categories_with_room_counts(false),
        state.home.weekly_top_users(),
        state.home.featured_rooms(),
    );

    let featured_rooms: Vec<Value> = match featured {
        Ok(mut fr) => {
            for r in &mut fr {
                r.subscriptions =
                    state.presence.member_count(r.id, now).await.unwrap_or(0);
            }
            fr.iter().map(|r| room_json(r, &storage)).collect()
        }
        Err(_) => vec![],
    };

    // Tab player counts — presence totals across each whole catalog.
    let mut counts = json!({"official": 0, "community": 0});
    for (key, is_public) in [("official", true), ("community", false)] {
        if let Ok(all) = state.home.catalog_rooms(is_public).await {
            let mut n = 0i64;
            for r in &all {
                n += state.presence.member_count(r.id, now).await.unwrap_or(0);
            }
            counts[key] = json!(n);
        }
    }

    // Minigames tab — same list as /minigames (type, name, description, score).
    let mut minigame_items: Vec<Value> = vec![];
    if tab == "minigames" {
        minigame_items =
            crate::minigames::games_list(&state, user.0.as_ref().map(|u| u.id)).await;
    }
    let catalog_items = if tab == "minigames" {
        json!({"data": minigame_items, "total": minigame_items.len(), "current_page": 1, "last_page": 1})
    } else {
        json!({
            "data": data,
            "total": total,
            "current_page": page,
            "last_page": last_page,
            "per_page": PER_PAGE,
        })
    };

    Ok(Json(json!({
        "catalog": tab,
        "catalog_category_ids": category_ids,
        "catalog_items": catalog_items,
        "public_categories": public_categories.unwrap_or_default(),
        "community_categories": community_categories.unwrap_or_default(),
        "catalog_tab_player_counts": counts,
        "weekly_top_users": weekly_top_users.unwrap_or_default(),
        "featured_rooms": featured_rooms,
    })))
}
