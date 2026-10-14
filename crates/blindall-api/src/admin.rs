//! Admin API — `auth.administrator` routes: users CRUD, dashboard
//! maintenance (`rounds:force-clear`, `cache:clear`, `topusers:weekly`).

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use crate::auth::CurrentUser;
use crate::state::AppState;

fn forbidden() -> Response {
    StatusCode::FORBIDDEN.into_response()
}

#[derive(Deserialize)]
pub struct UsersQuery {
    search: Option<String>,
    role: Option<String>,
    trashed: Option<String>,
    page: Option<i64>,
    per_page: Option<i64>,
}

/// `GET /api/v1/admin/users` — filterable list (search/role/trashed).
pub async fn users(
    State(state): State<AppState>,
    Query(q): Query<UsersQuery>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state
        .admin
        .users(
            q.search.as_deref(),
            q.role.as_deref(),
            q.trashed.as_deref(),
            q.page.unwrap_or(1),
            q.per_page.unwrap_or(10).clamp(1, 100),
        )
        .await
    {
        Ok(p) => Json(json!({"users": p.to_json()})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct CreateUserBody {
    name: String,
    email: String,
    password: Option<String>,
    is_administrator: bool,
}

/// `POST /api/v1/admin/users`.
pub async fn create_user(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<CreateUserBody>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    let name = body.name.trim();
    if name.is_empty() || name.len() > 50 {
        return err("name", "validation");
    }
    if !validator::ValidateEmail::validate_email(&body.email) || body.email.len() > 50 {
        return err("email", "The email field must be a valid email address.");
    }
    if state.auth.email_taken(&body.email).await.unwrap_or(true) {
        return err("email", "The email has already been taken.");
    }
    match state
        .admin
        .create_user(name, &body.email, body.password.as_deref(), body.is_administrator)
        .await
    {
        Ok(id) => (StatusCode::CREATED, Json(json!({"id": id}))).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct UpdateUserBody {
    name: String,
    email: String,
    team_id: Option<i64>,
    is_administrator: bool,
    password: Option<String>,
}

/// `PUT /api/v1/admin/users/{user}`.
pub async fn update_user(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<UpdateUserBody>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    if body.name.trim().is_empty() || body.name.len() > 50 {
        return err("name", "validation");
    }
    if !validator::ValidateEmail::validate_email(&body.email) || body.email.len() > 50 {
        return err("email", "The email field must be a valid email address.");
    }
    match state
        .admin
        .update_user(
            user_id,
            body.name.trim(),
            &body.email,
            body.team_id,
            body.is_administrator,
            body.password.as_deref(),
        )
        .await
    {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `DELETE /api/v1/admin/users/{user}` — soft delete.
pub async fn delete_user(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.admin.delete_user(user_id).await {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `PUT /api/v1/admin/users/{user}/restore`.
pub async fn restore_user(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.admin.restore_user(user_id).await {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ---- maintenance -----------------------------------------------------------

/// `POST /api/v1/admin/force-clear-rounds` — stop actors + close rounds.
pub async fn force_clear_rounds(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    let stopped = state.manager.stop_all().await;
    match state.admin.force_clear_rounds().await {
        Ok((rooms, rounds)) => Json(json!({
            "rooms_updated": rooms,
            "rounds_finished": rounds,
            "actors_stopped": stopped,
        }))
        .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `POST /api/v1/admin/clear-cache` — `cache:clear`: delete all keys under
/// the Laravel cache prefix (scores/presence keys live outside it).
pub async fn clear_cache(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    use fred::interfaces::KeysInterface;
    use fred::types::scan::Scanner;
    use futures::StreamExt;
    let pattern = format!("{}*", state.cache_prefix);
    let mut keys: Vec<String> = Vec::new();
    let mut stream = state.redis.scan(pattern.as_str(), Some(1_000), None);
    while let Some(page) = stream.next().await {
        match page {
            Ok(mut page) => {
                if let Some(page_keys) = page.take_results() {
                    keys.extend(page_keys.iter().map(|k| k.as_str_lossy().to_string()));
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "clear-cache scan failed");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
        }
    }
    let mut deleted = 0u64;
    for chunk in keys.chunks(5_000) {
        deleted += state
            .redis
            .del::<u64, _>(chunk.to_vec())
            .await
            .unwrap_or(0);
    }
    Json(json!({"deleted_keys": deleted})).into_response()
}

/// `POST /api/v1/admin/regenerate-top-10` — recompute + write the
/// PHP-serialized `weekly-top-10-users` cache key.
pub async fn regenerate_top_10(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.admin.weekly_top_users().await {
        Ok(list) => {
            use fred::interfaces::KeysInterface;
            let key = format!("{}weekly-top-10-users", state.cache_prefix);
            let serialized = blindall_core::phpser::to_php(&json!(list));
            let n = list.len();
            if let Err(e) = state.redis.set::<(), _, _>(&key, serialized, None, None, false).await {
                tracing::warn!(error = %e, "weekly-top-10 write failed");
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
            Json(json!({"cached": n})).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn err(field: &str, msg: &str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"message": msg, "errors": { field: [msg] }})),
    )
        .into_response()
}

// ---- content CRUD ------------------------------------------------------------

/// `GET /api/v1/admin/categories`.
pub async fn categories(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.admin.categories().await {
        Ok(v) => Json(json!({"categories": v})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct NameBody {
    name: String,
}

/// `POST /api/v1/admin/categories` + `PUT …/{id}`.
pub async fn create_category(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<NameBody>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    let name = body.name.trim();
    if name.is_empty() || name.len() > 50 {
        return err("name", "validation");
    }
    if state.admin.category_name_taken(name, None).await {
        return err("name", "The name has already been taken.");
    }
    match state.admin.create_category(name).await {
        Ok(id) => (StatusCode::CREATED, Json(json!({"id": id}))).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn update_category(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<NameBody>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    let name = body.name.trim();
    if name.is_empty() || name.len() > 50 {
        return err("name", "validation");
    }
    if state.admin.category_name_taken(name, Some(id)).await {
        return err("name", "The name has already been taken.");
    }
    match state.admin.update_category(id, name).await {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn delete_category(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.admin.delete_category(id).await {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct AnswerTypeBody {
    name: String,
    pronoun: Option<String>,
    svg_icon: Option<String>,
}

/// `POST /api/v1/admin/answer-types` + `PUT …/{id}`.
pub async fn create_answer_type(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<AnswerTypeBody>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    if body.name.trim().is_empty() || body.name.len() > 50 {
        return err("name", "validation");
    }
    match state
        .admin
        .answer_type_create(body.name.trim(), body.pronoun.as_deref(), body.svg_icon.as_deref())
        .await
    {
        Ok(id) => (StatusCode::CREATED, Json(json!({"id": id}))).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn update_answer_type(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<AnswerTypeBody>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state
        .admin
        .answer_type_update(id, body.name.trim(), body.pronoun.as_deref(), body.svg_icon.as_deref())
        .await
    {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn delete_answer_type(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.admin.answer_type_delete(id).await {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `GET /api/v1/admin/pages`.
pub async fn pages(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.admin.pages().await {
        Ok(v) => Json(json!({"pages": v})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct PageBody {
    title: String,
    content: String,
    parent_id: Option<i64>,
}

/// `POST /api/v1/admin/pages` — new page; `PUT …/{id}` — new revision.
pub async fn create_page(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<PageBody>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    if body.title.trim().is_empty() || body.title.len() > 50 || body.content.is_empty() {
        return err("title", "validation");
    }
    // PHP validates `unique:pages` on store only.
    match state
        .admin
        .page_title_taken(body.title.trim())
        .await
    {
        Ok(true) => return err("title", "The title has already been taken."),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        _ => {}
    }
    match state
        .admin
        .create_page(body.title.trim(), &body.content, None, body.parent_id)
        .await
    {
        Ok(id) => (StatusCode::CREATED, Json(json!({"id": id}))).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// PHP `update` — creates a new revision with `original_id` pointing at
/// the original page (`$page->original_id ?? $page->id`).
pub async fn update_page(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<PageBody>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    if body.title.trim().is_empty() || body.title.len() > 50 || body.content.is_empty() {
        return err("title", "validation");
    }
    let original_id = match state.admin.page_original(id).await {
        Ok(Some(o)) => Some(o.unwrap_or(id)),
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    match state
        .admin
        .create_page(body.title.trim(), &body.content, original_id, body.parent_id)
        .await
    {
        Ok(rev_id) => Json(json!({"id": rev_id})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `DELETE /api/v1/admin/pages/{id}` — `Admin\PageController::destroy`
/// (hard delete — Page has no SoftDeletes).
pub async fn delete_page(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.admin.delete_page(id).await {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `faqs`.
pub async fn faqs(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.admin.faqs().await {
        Ok(v) => Json(json!({"faqs": v})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct FaqBody {
    locale: String,
    question: String,
    answer: String,
}

pub async fn create_faq(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<FaqBody>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    if body.locale.len() > 2 || body.question.is_empty() || body.question.len() > 255 {
        return err("question", "validation");
    }
    match state
        .admin
        .create_faq(&body.locale, &body.question, &body.answer)
        .await
    {
        Ok(id) => (StatusCode::CREATED, Json(json!({"id": id}))).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn update_faq(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<FaqBody>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state
        .admin
        .update_faq(id, &body.locale, &body.question, &body.answer)
        .await
    {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn delete_faq(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.admin.delete_faq(id).await {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ---- dashboard -------------------------------------------------------------

/// `GET /api/v1/admin/dashboard` — `Admin\DashboardController::index`
/// (diskSpace + cacheDriver).
pub async fn dashboard(State(_state): State<AppState>, CurrentUser(user): CurrentUser) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    let disk = std::process::Command::new("df")
        .arg("-h")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .unwrap_or_else(|| "Unable to retrieve disk space information".to_string());
    Json(json!({
        "diskSpace": disk,
        "cacheDriver": "redis",
    }))
    .into_response()
}

// ---- playlists ---------------------------------------------------------------

#[derive(Deserialize)]
pub struct AdminListQuery {
    search: Option<String>,
    trashed: Option<String>,
    page: Option<i64>,
    per_page: Option<i64>,
}

/// `GET /api/v1/admin/playlists` — `Admin\PlaylistController::index`.
pub async fn playlists(
    State(state): State<AppState>,
    Query(q): Query<AdminListQuery>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state
        .admin
        .admin_playlists(q.search.as_deref(), q.page.unwrap_or(1), q.per_page.unwrap_or(5).clamp(1, 100))
        .await
    {
        Ok(p) => Json(json!({"playlists": p.to_json(), "filters": {"search": q.search, "trashed": q.trashed}})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct AdminPlaylistBody {
    name: String,
    description: Option<String>,
    is_public: Option<bool>,
}

/// `PUT /api/v1/admin/playlists/{id}` — `Admin\PlaylistController::update`.
pub async fn update_playlist(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<AdminPlaylistBody>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 50 {
        return err("name", "The name field is required and must not exceed 50 characters.");
    }
    match state
        .admin
        .admin_update_playlist(id, name, body.description.as_deref(), body.is_public.unwrap_or(false))
        .await
    {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `POST /api/v1/admin/playlists` — `Admin\PlaylistController::store`
/// (`name` required max:50 unique, `is_public` bool; created under the
/// admin user + moderator pivot).
pub async fn create_playlist(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<AdminPlaylistBody>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 50 {
        return err("name", "The name field is required and must not exceed 50 characters.");
    }
    if state.admin.playlist_name_taken(name).await.unwrap_or(false) {
        return err("name", "The name has already been taken.");
    }
    match state
        .admin
        .admin_create_playlist(user.id, name, body.is_public.unwrap_or(false))
        .await
    {
        Ok(id) => (StatusCode::CREATED, Json(json!({"id": id}))).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `DELETE /api/v1/admin/playlists/{id}` — hard delete (playlists have no
/// SoftDeletes upstream).
pub async fn destroy_playlist(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.playlists.delete(id).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ---- rooms -------------------------------------------------------------------

/// `GET /api/v1/admin/rooms` — `Admin\RoomController::index`.
pub async fn rooms(
    State(state): State<AppState>,
    Query(q): Query<AdminListQuery>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state
        .admin
        .admin_rooms(
            q.search.as_deref(),
            q.trashed.as_deref(),
            q.page.unwrap_or(1),
            q.per_page.unwrap_or(5).clamp(1, 100),
        )
        .await
    {
        Ok(p) => Json(json!({"rooms": p.to_json(), "filters": {"search": q.search, "trashed": q.trashed}})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct AdminRoomBody {
    name: String,
    description: Option<String>,
    category_id: Option<i64>,
    tracks_by_round: Option<i64>,
    track_duration: Option<i64>,
    pause_between_tracks: Option<i64>,
    pause_between_rounds: Option<i64>,
    is_public: Option<bool>,
    is_pro: Option<bool>,
    is_random: Option<bool>,
    is_active: Option<bool>,
    is_chat_active: Option<bool>,
    discord_webhook_url: Option<String>,
    color: Option<String>,
    password: Option<String>,
}

/// `PUT /api/v1/admin/rooms/{id}` — `Admin\RoomController::update`.
pub async fn update_room(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<AdminRoomBody>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 30 {
        return err("name", "The name field is required and must not exceed 30 characters.");
    }
    if !state.rooms.category_exists(body.category_id.unwrap_or(1)).await.unwrap_or(false) {
        return err("category_id", "The selected category id is invalid.");
    }
    match state
        .admin
        .admin_update_room(
            id,
            name,
            body.description.as_deref(),
            body.category_id.unwrap_or(1),
            body.tracks_by_round.unwrap_or(15).clamp(1, 50),
            body.track_duration.unwrap_or(30).clamp(5, 30),
            body.pause_between_tracks.unwrap_or(5).clamp(0, 60),
            body.pause_between_rounds.unwrap_or(10).clamp(0, 60),
            body.is_public.unwrap_or(false),
            body.is_pro.unwrap_or(false),
            body.is_random.unwrap_or(false),
            body.is_active.unwrap_or(true),
            body.is_chat_active.unwrap_or(true),
            body.discord_webhook_url.as_deref(),
            body.color.as_deref(),
            body.password.as_deref(),
        )
        .await
    {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `POST /api/v1/admin/rooms` — `Admin\RoomController::store`. Validation
/// mirrors PHP; the fillable list decides what actually gets written
/// (PHP drops is_pro/is_active/is_chat_active/pause_between_tracks/
/// playlist_id/color/photo from its create() array). Then moderator
/// attach + `UpdateUserLevel` ('rooms_count').
pub async fn create_room(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<AdminRoomBody>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 30 {
        return err("name", "The name field is required and must not exceed 30 characters.");
    }
    if state.rooms.name_taken(name, None).await.unwrap_or(false) {
        return err("name", "The name has already been taken.");
    }
    if let Some(url) = body.discord_webhook_url.as_deref() {
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return err("discord_webhook_url", "The discord webhook url field must be a valid URL.");
        }
    }
    match state
        .admin
        .admin_create_room(
            user.id,
            name,
            body.description.as_deref(),
            body.password.as_deref(),
            body.tracks_by_round.unwrap_or(15).clamp(1, 50),
            body.track_duration.unwrap_or(30).clamp(5, 30),
            body.pause_between_rounds.unwrap_or(10).clamp(0, 60),
            body.is_public.unwrap_or(false),
            body.is_random.unwrap_or(false),
            body.discord_webhook_url.as_deref(),
        )
        .await
    {
        Ok(id) => {
            // PHP `UpdateUserLevel::dispatch($user, 'rooms_count')`.
            state
                .levels
                .dispatch(user.id, blindall_db::LevelTrigger::RoomsCount);
            (StatusCode::CREATED, Json(json!({"id": id}))).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `DELETE /api/v1/admin/rooms/{id}` — soft delete.
pub async fn destroy_room(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.rooms.soft_delete(id).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `PUT /api/v1/admin/rooms/{id}/restore`.
pub async fn restore_room(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.rooms.restore(id).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ---- teams -------------------------------------------------------------------

/// `GET /api/v1/admin/teams` — `Admin\TeamController::index`.
pub async fn teams(
    State(state): State<AppState>,
    Query(q): Query<AdminListQuery>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state
        .admin
        .admin_teams(
            q.search.as_deref(),
            q.trashed.as_deref(),
            q.page.unwrap_or(1),
            q.per_page.unwrap_or(5).clamp(1, 100),
        )
        .await
    {
        Ok(p) => Json(json!({"teams": p.to_json(), "filters": {"search": q.search, "trashed": q.trashed}})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct AdminTeamBody {
    name: String,
    user_id: i64,
}

/// `PUT /api/v1/admin/teams/{id}` — `Admin\TeamController::update`.
pub async fn update_team(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<AdminTeamBody>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 50 {
        return err("name", "The name field is required and must not exceed 50 characters.");
    }
    if !state.rooms.user_exists(body.user_id).await.unwrap_or(false) {
        return err("user_id", "The selected user id is invalid.");
    }
    match state.admin.admin_update_team(id, name, body.user_id).await {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `DELETE /api/v1/admin/teams/{id}` — soft delete.
pub async fn destroy_team(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.admin.team_set_deleted(id, true).await {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `PUT /api/v1/admin/teams/{id}/restore`.
pub async fn restore_team(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.admin.team_set_deleted(id, false).await {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ---- answer types ------------------------------------------------------------

/// `GET /api/v1/admin/answer-types` — `Admin\AnswerTypeController::index`.
pub async fn answer_types(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.admin.admin_answer_types().await {
        Ok(v) => Json(json!({"answer_types": v})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
