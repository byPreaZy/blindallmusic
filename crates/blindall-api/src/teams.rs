use crate::auth::{CurrentUser, OptionalUser};
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;

fn forbidden() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({"error": "unauthorized"})),
    )
        .into_response()
}

fn validation(field: &str, msg: &str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"message": msg, "errors": {field: [msg]}})),
    )
        .into_response()
}

fn alpha_dash(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == ' ')
}

#[derive(Deserialize)]
pub struct ListQuery {
    search: Option<String>,
    page: Option<i64>,
}

pub async fn index(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
    Query(q): Query<ListQuery>,
) -> Response {
    match state
        .teams
        .list(q.search.as_deref(), q.page.unwrap_or(1))
        .await
    {
        Ok((teams, total)) => {
            let teams: Vec<serde_json::Value> = teams
                .into_iter()
                .map(|mut tm| {
                    let path = tm["photo_path"].as_str().unwrap_or("").to_string();
                    let name = tm["name"].as_str().unwrap_or("").to_string();
                    tm["photo"] = json!(crate::profiles::photo_url(&state, &path, &name));
                    let pts = tm["team_points"].as_f64().unwrap_or(0.0);
                    tm["team_points_abbreviated"] = json!(abbreviate(pts));
                    tm["seats"] = json!(8);
                    tm
                })
                .collect();
            let viewer = match user {
                Some(u) => {
                    let (pending, declined) =
                        state.teams.team_request_ids(u.id).await.unwrap_or_default();
                    json!({
                        "team_id": u.team_id,
                        "is_guest": u.is_guest,
                        "pending_requests": pending,
                        "declined_requests": declined,
                    })
                }
                None => json!(null),
            };
            Json(json!({
                "teams": teams,
                "total": total,
                "page": q.page.unwrap_or(1).max(1),
                "per_page": 12,
                "viewer": viewer,
            }))
            .into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn abbreviate(n: f64) -> String {
    const UNITS: [(f64, &str); 3] = [(1e9, "B"), (1e6, "M"), (1e3, "K")];
    for (div, suffix) in UNITS {
        if n.abs() >= div {
            let v = n / div;
            let s = if v >= 100.0 {
                format!("{v:.0}")
            } else {
                let t = format!("{v:.1}");
                t.trim_end_matches(".0").to_string()
            };
            return format!("{s}{suffix}");
        }
    }
    format!("{}", n.round() as i64)
}

#[derive(Deserialize)]
pub struct TeamBody {
    name: String,
}

pub async fn store(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<TeamBody>,
) -> Response {
    if user.is_guest {
        return forbidden();
    }
    if user.team_id.is_some() {
        return validation("team", "You are already part of a team");
    }
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 30 || !alpha_dash(name) {
        return validation(
            "name",
            "The name field is required, must not exceed 30 characters and may only contain letters, numbers, spaces, dashes and underscores.",
        );
    }
    let mods = state
        .auth
        .reserved_moderator_names()
        .await
        .unwrap_or_default();
    if crate::auth_session::validate_name(name, &mods).is_err() {
        return validation("name", "validation.reserved");
    }
    if state.teams.name_taken(name, None).await.unwrap_or(false) {
        return validation("name", "The name has already been taken.");
    }
    match state.teams.create(user.id, name).await {
        Ok(id) => (StatusCode::CREATED, Json(json!({"team_id": id}))).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn show(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    OptionalUser(user): OptionalUser,
) -> Response {
    let Some(team) = state.teams.find(id).await.ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match state.teams.team_show(id).await {
        Ok(mut v) => {
            if let Some(members) = v["members"].as_array_mut() {
                for m in members.iter_mut() {
                    let path = m["photo"].as_str().unwrap_or("").to_string();
                    let name = m["name"].as_str().unwrap_or("").to_string();
                    m["photo"] = json!(crate::profiles::photo_url(&state, &path, &name));
                }
            }
            let owner_name = state.auth.find_name(team.user_id).await.unwrap_or_default();
            let path = team.photo_path.clone().unwrap_or_default();
            v["team"] = json!({
                "id": team.id,
                "name": team.name,
                "user_id": team.user_id,
                "photo_path": team.photo_path,
                "photo": crate::profiles::photo_url(&state, &path, &team.name),
                "seats": 8,
                "owner": { "id": team.user_id, "name": owner_name },
            });
            let viewer = match &user {
                Some(u) => {
                    let (pending, declined) =
                        state.teams.team_request_ids(u.id).await.unwrap_or_default();
                    json!({
                        "id": u.id,
                        "logged_in": true,
                        "is_owner": u.id == team.user_id,
                        "is_member": u.team_id == Some(id),
                        "has_team": u.team_id.is_some(),
                        "has_pending_request": pending.contains(&id),
                        "has_declined_request": declined.contains(&id),
                        "can_change_team_picture": state
                            .auth
                            .can_change_picture(u.id)
                            .await
                            .unwrap_or(false),
                    })
                }
                None => json!({
                    "logged_in": false,
                    "is_owner": false,
                    "is_member": false,
                    "has_team": false,
                    "has_pending_request": false,
                    "has_declined_request": false,
                    "can_change_team_picture": false,
                }),
            };
            v["viewer"] = viewer;
            Json(v).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<TeamBody>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let Some(team) = state.teams.find(id).await.ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if team.user_id != user.id {
        return validation("team", "This is not your team");
    }
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 30 || !alpha_dash(name) {
        return validation(
            "name",
            "The name field is required, must not exceed 30 characters and may only contain letters, numbers, spaces, dashes and underscores.",
        );
    }
    let mods = state
        .auth
        .reserved_moderator_names()
        .await
        .unwrap_or_default();
    if !name.eq_ignore_ascii_case(&team.name)
        && crate::auth_session::validate_name(name, &mods).is_err()
    {
        return validation("name", "validation.reserved");
    }
    if state
        .teams
        .name_taken(name, Some(id))
        .await
        .unwrap_or(false)
    {
        return validation("name", "The name has already been taken.");
    }
    match state.teams.rename(id, name).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn update_photo(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    mut multipart: axum::extract::Multipart,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let Some(team) = state.teams.find(id).await.ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if team.user_id != user.id {
        return forbidden();
    }
    if !state
        .auth
        .can_change_picture(user.id)
        .await
        .unwrap_or(false)
    {
        return forbidden();
    }
    let mut bytes = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("photo") {
            match field.bytes().await {
                Ok(b) if b.len() <= 10 * 1024 * 1024 => bytes = Some(b),
                _ => return validation("photo", "The photo may not be larger than 10 MB."),
            }
        }
    }
    let Some(bytes) = bytes else {
        return validation("photo", "The photo field is required.");
    };
    let webp = match crate::storage::image_to_webp_contain(&bytes, 350, 350) {
        Ok(w) => w,
        Err(()) => return validation("photo", "The photo must be an image."),
    };
    let path = format!("teams/{}.webp", uuid::Uuid::now_v7().simple());
    if state.storage.put(&path, &webp).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    match state.teams.set_photo(id, &path).await {
        Ok(prev) => {
            if let Some(p) = prev {
                state.storage.delete(&p).await;
            }
            Json(json!({"ok": true, "photo_path": path})).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn destroy(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let Some(team) = state.teams.find(id).await.ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let members = state.teams.member_ids(id).await.unwrap_or_default();
    if team.user_id != user.id || members.len() != 1 {
        return validation("team", "This is not your team");
    }
    match state.teams.delete(id).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn leave(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let Some(team) = state.teams.find(id).await.ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if team.user_id == user.id {
        return validation(
            "team",
            "Impossible to leave your own team. You must transfer ownership to another member first",
        );
    }
    if user.team_id != Some(id) {
        return validation("team", "You are not part of this team");
    }
    match state.teams.remove_member(user.id).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn switch_owner(
    State(state): State<AppState>,
    Path((id, uid)): Path<(i64, i64)>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let Some(team) = state.teams.find(id).await.ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if team.user_id != user.id {
        return forbidden();
    }
    match state.teams.switch_owner(id, uid).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn remove_member(
    State(state): State<AppState>,
    Path((id, uid)): Path<(i64, i64)>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let Some(team) = state.teams.find(id).await.ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if team.user_id != user.id {
        return forbidden();
    }
    match state.teams.remove_member(uid).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn request_join(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    if user.team_id.is_some() {
        return validation(
            "team",
            "It is not possible to join another team. You must leave the current team and make a new request",
        );
    }
    let Some(team) = state.teams.find(id).await.ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if team.deleted_at.is_some() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let pending = state
        .teams
        .pending_requests_count(user.id)
        .await
        .unwrap_or(0);
    if pending > 2 {
        return validation(
            "team",
            "You cannot do more than 3 requests at the same time",
        );
    }
    let members = state.teams.member_ids(id).await.unwrap_or_default();
    if blindall_db::TEAM_SEATS <= members.len() as i64 {
        return validation(
            "team",
            "It is not possible to join this team. The maximum number of members has been reached",
        );
    }
    match state.teams.create_request(id, user.id).await {
        Ok(request_id) => {
            let _ = state
                .notifications
                .notify(
                    team.user_id,
                    "App\\Notifications\\NewTeamRequest",
                    json!({
                        "teamRequest": {
                            "id": request_id,
                            "team_id": id,
                            "user_id": user.id,
                            "user": {"id": user.id, "name": user.name},
                            "team": {"id": id, "name": team.name},
                        },
                        "message": format!("@{} wishes to join your team", user.name),
                    }),
                )
                .await;
            Json(json!({"ok": true, "request_id": request_id})).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn cancel_request(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    match state.teams.cancel_request(id, user.id).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct RequestActionBody {
    notification_id: Option<String>,
}

async fn mark_notification_read(state: &AppState, user_id: i64, body: &RequestActionBody) {
    if let Some(id) = body.notification_id.as_deref() {
        let _ = state.notifications.mark_read(user_id, id).await;
    }
}

pub async fn accept_request(
    State(state): State<AppState>,
    Path(request_id): Path<i64>,
    CurrentUser(user): CurrentUser,
    body: Option<Json<RequestActionBody>>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let Some((team_id, requester_id)) = state.teams.find_request(request_id).await.ok().flatten()
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(team) = state.teams.find(team_id).await.ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if team.user_id != user.id {
        return forbidden();
    }
    if let Some(Json(b)) = &body {
        mark_notification_read(&state, user.id, b).await;
    }
    match state.teams.approve_request(request_id).await {
        Ok(true) => {
            let data = json!({
                "team": {"id": team.id, "name": team.name, "user_id": team.user_id},
                "message": format!("Your request has been accepted!<br> Welcome to the team {}", team.name),
            });
            // state.notifications is hub-aware: notify() broadcasts too.
            let _ = state
                .notifications
                .notify(
                    requester_id,
                    "App\\Notifications\\TeamRequestApproved",
                    data,
                )
                .await;
            state
                .levels
                .dispatch(requester_id, blindall_db::LevelTrigger::Score);
            Json(json!({"ok": true})).into_response()
        }
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn decline_request(
    State(state): State<AppState>,
    Path(request_id): Path<i64>,
    CurrentUser(user): CurrentUser,
    body: Option<Json<RequestActionBody>>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let Some((team_id, requester_id)) = state.teams.find_request(request_id).await.ok().flatten()
    else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(team) = state.teams.find(team_id).await.ok().flatten() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    if team.user_id != user.id {
        return forbidden();
    }
    if let Some(Json(b)) = &body {
        mark_notification_read(&state, user.id, b).await;
    }
    match state.teams.decline_request(request_id).await {
        Ok(()) => {
            let data = json!({
                "team": {"id": team.id, "name": team.name, "user_id": team.user_id},
                "message": "Your request has been declined",
            });
            // state.notifications is hub-aware: notify() broadcasts too.
            let _ = state
                .notifications
                .notify(
                    requester_id,
                    "App\\Notifications\\TeamRequestRejected",
                    data,
                )
                .await;
            Json(json!({"ok": true})).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
