use crate::auth::CurrentUser;
use crate::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::{Value, json};

fn err(field: &str, msg: &str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"message": msg, "errors": { field: [msg] }})),
    )
        .into_response()
}

fn forbidden() -> Response {
    StatusCode::FORBIDDEN.into_response()
}

fn self_or_admin(user: &blindall_db::entities::User, id: i64) -> bool {
    user.id == id || user.is_administrator
}

#[derive(Deserialize)]
pub struct IndexQuery {
    search: Option<String>,
    page: Option<i64>,
}

pub async fn index(
    State(state): State<AppState>,
    CurrentUser(_user): CurrentUser,
    axum::extract::Query(q): axum::extract::Query<IndexQuery>,
) -> Response {
    let search = q.search.unwrap_or_default().trim().to_string();
    let page = q.page.unwrap_or(1).max(1);
    match state.auth.search_users(&search, page).await {
        Ok((rows, total)) => {
            let data: Vec<Value> = rows
                .into_iter()
                .map(|(id, name)| json!({"id": id, "name": name}))
                .collect();
            Json(json!({
                "filters": {"search": search},
                "users": {
                    "data": data,
                    "current_page": page,
                    "last_page": ((total + 19) / 20).max(1),
                    "per_page": 20,
                    "total": total,
                }
            }))
            .into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct UpdateProfileBody {
    name: String,
    email: String,
    #[serde(default)]
    custom_tag: Option<String>,
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<UpdateProfileBody>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    if user.id != id {
        return forbidden();
    }
    let name = body.name.trim();
    let email = body.email.trim().to_lowercase();
    if name.len() < 2 || name.len() > 25 {
        return err("name", "validation");
    }
    if email.is_empty() || email.len() > 255 || !email.contains('@') {
        return err("email", "validation");
    }
    let moderator_names = state
        .auth
        .reserved_moderator_names()
        .await
        .unwrap_or_default();
    if let Err(msg) = crate::auth_session::validate_name(name, &moderator_names) {
        return err("name", msg);
    }
    match state.auth.name_taken_ignore(name, id).await {
        Ok(true) => return err("name", "The name has already been taken."),
        Ok(false) => {}
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
    match state.auth.email_taken_ignore(&email, id).await {
        Ok(true) => return err("email", "The email has already been taken."),
        Ok(false) => {}
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
    let tag: Option<String> = body
        .custom_tag
        .as_deref()
        .map(|t| {
            t.chars()
                .filter(|c| !c.is_control() && *c != '<' && *c != '>')
                .collect::<String>()
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(24)
                .collect::<String>()
        })
        .filter(|t| !t.is_empty());
    if let Err(_) = state.auth.update_custom_tag(id, tag.as_deref()).await {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    match state.auth.update_profile(id, name, &email).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct UpdatePasswordBody {
    password: String,
    password_confirmation: Option<String>,
    current_password: Option<String>,
}

pub async fn update_password(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<UpdatePasswordBody>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    if user.id != id {
        return forbidden();
    }
    if body.password.len() < 8 {
        return err("password", "validation");
    }
    if body.password_confirmation.as_deref() != Some(body.password.as_str()) {
        return err("password", "The password confirmation does not match.");
    }
    let existing = match state.auth.password_hash(id).await {
        Ok(v) => v,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    if let Some(hash) = existing {
        let ok = body
            .current_password
            .as_deref()
            .is_some_and(|p| bcrypt::verify(p, &hash).unwrap_or(false));
        if !ok {
            return err("current_password", "The password is incorrect.");
        }
    }
    let hash = match bcrypt::hash(&body.password, 12) {
        Ok(h) => h,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match state.auth.update_password(id, &hash).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct DonationPreferencesBody {
    show_donation_history_on_profile: bool,
}

pub async fn update_donation_preferences(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<DonationPreferencesBody>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    if user.id != id {
        return forbidden();
    }
    match state
        .auth
        .update_donation_preferences(id, body.show_donation_history_on_profile)
        .await
    {
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

    if user.id != id {
        return forbidden();
    }
    let mut bytes = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("photo") {
            match field.bytes().await {
                Ok(b) if b.len() <= 10 * 1024 * 1024 => bytes = Some(b),
                _ => return err("photo", "The photo may not be larger than 10 MB."),
            }
        }
    }
    let Some(bytes) = bytes else {
        return err("photo", "The photo field is required.");
    };
    let webp = match crate::storage::image_to_webp_contain(&bytes, 350, 350) {
        Ok(w) => w,
        Err(()) => return err("photo", "The photo must be an image."),
    };
    let path = format!("users/{}.webp", uuid::Uuid::now_v7().simple());
    if state.storage.put(&path, &webp).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let previous = state.auth.photo_path(id).await.unwrap_or(None);
    if state.auth.set_photo(id, &path).await.is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    if let Some(p) = previous {
        state.storage.delete(&p).await;
    }
    Json(json!({"ok": true, "photo_path": path})).into_response()
}

pub async fn destroy_photo(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    if user.id != id {
        return forbidden();
    }
    let previous = match state.auth.photo_path(id).await {
        Ok(v) => v,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    if let Some(path) = previous {
        state.storage.delete(&path).await;
        if let Err(_) = state.auth.clear_photo(id).await {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    }
    Json(json!({"ok": true})).into_response()
}

pub async fn me(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> Response {
    if user.is_guest {
        return StatusCode::FORBIDDEN.into_response();
    }
    let header = state.profiles.header(user.id).await.ok().flatten();
    let email = state.auth.email_of(user.id).await.ok().flatten();
    let has_password = state
        .auth
        .password_hash(user.id)
        .await
        .ok()
        .flatten()
        .is_some();
    let photo_path = state.auth.photo_path(user.id).await.ok().flatten();
    let photo = crate::profiles::photo_url(&state, photo_path.as_deref().unwrap_or(""), &user.name);
    let key = blindall_db::donations::current_month_key();
    let is_supporter = state
        .donations_repo
        .user_is_supporter(user.id, &key)
        .await
        .unwrap_or(false);
    let plan = state
        .subscriptions
        .active_plan(user.id)
        .await
        .ok()
        .flatten();
    let mut perks: Vec<String> = if is_supporter || plan.is_some() {
        state.donations.supporter_perks.clone()
    } else {
        vec![]
    };
    if plan.is_some() {
        for p in crate::donations::PLUS_PERKS {
            if !perks.iter().any(|x| x == p) {
                perks.push(p.to_string());
            }
        }
    }
    let donation_summary = state
        .donations_repo
        .user_summary(user.id)
        .await
        .unwrap_or(json!({}));
    let stats = header
        .as_ref()
        .map(|h| h["stats"].clone())
        .filter(|s| s.is_object())
        .unwrap_or(json!({"rooms": 0, "playlists": 0, "bookmarks": 0}));
    let created_at = header
        .as_ref()
        .map(|h| h["created_at"].clone())
        .unwrap_or(Value::Null);
    let created = created_at
        .as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&chrono::Utc));
    let created_at_fr = created.map(|d| d.format("%d/%m/%Y").to_string());
    let created_at_from_now = created.map(|d| diff_for_humans_fr(d));
    Json(json!({
        "account": {
            "id": user.id,
            "name": user.name,
            "email": email,
            "photo": photo,
            "has_custom_photo": photo_path.is_some(),
            "created_at": created_at_fr,
            "created_at_from_now": created_at_from_now,
            "has_password": has_password,
            "stats": stats,
            "donation_summary": donation_summary,
            "show_donation_history_on_profile": header
                .as_ref()
                .map(|h| h["show_donation_history_on_profile"].as_bool().unwrap_or(true))
                .unwrap_or(true),
            "donor_perks": perks,
            "is_supporter": is_supporter || plan.is_some(),
            "plan": plan,
            "is_administrator": user.is_administrator,
            "custom_tag": state
                .auth
                .custom_tag_of(user.id)
                .await
                .ok()
                .flatten()
                .unwrap_or_default(),
        }
    }))
    .into_response()
}

pub async fn notifications(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    let list = state
        .notifications
        .unread(user.id, 50)
        .await
        .unwrap_or_default();
    let count = state.notifications.unread_count(user.id).await.unwrap_or(0);
    Json(json!({"notifications": list, "unread_count": count})).into_response()
}

pub async fn notification_read(
    State(state): State<AppState>,
    Path(id): Path<String>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    match state.notifications.mark_read(user.id, &id).await {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn push_notification(
    state: &AppState,
    user_id: i64,
    type_: &str,
    notification_id: &str,
    data: Value,
) {
    state
        .hub
        .broadcast(
            &format!("user:{user_id}"),
            "Illuminate\\Notifications\\Events\\BroadcastNotificationCreated",
            json!({
                "id": notification_id,
                "type": type_,
                "notifiable_type": "App\\Models\\User",
                "notifiable_id": user_id,
                "data": data,
            }),
            None,
        )
        .await;
}

pub async fn notification_done(
    State(state): State<AppState>,
    Path(id): Path<String>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }
    match state.notifications.mark_done(user.id, &id).await {
        Ok(n) => Json(json!({"ok": true, "marked": n})).into_response(),
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

    if !self_or_admin(&user, id) {
        return forbidden();
    }
    if let Ok(Some(path)) = state.auth.photo_path(id).await {
        state.storage.delete(&path).await;
    }
    match state.auth.delete_account(id).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn diff_for_humans_fr(d: chrono::DateTime<chrono::Utc>) -> String {
    let secs = (chrono::Utc::now() - d).num_seconds().max(0);
    let (n, unit) = if secs < 60 {
        (secs.max(1), "seconde")
    } else if secs < 3_600 {
        (secs / 60, "minute")
    } else if secs < 86_400 {
        (secs / 3_600, "heure")
    } else if secs < 604_800 {
        (secs / 86_400, "jour")
    } else if secs < 2_592_000 {
        (secs / 604_800, "semaine")
    } else if secs < 31_536_000 {
        (secs / 2_592_000, "mois")
    } else {
        (secs / 31_536_000, "an")
    };
    let s = if n > 1 && unit != "mois" { "s" } else { "" };
    format!("{n} {unit}{s}")
}
