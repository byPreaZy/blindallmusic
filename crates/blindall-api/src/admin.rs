use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::{Value, json};

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
        .create_user(
            name,
            &body.email,
            body.password.as_deref(),
            body.is_administrator,
        )
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

#[derive(Deserialize)]
pub struct SetRoleBody {
    role: String,
}

pub async fn set_user_role(
    State(state): State<AppState>,
    Path(user_id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<SetRoleBody>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    if user_id == user.id {
        return err("role", "You cannot change your own role.");
    }
    if !matches!(body.role.as_str(), "user" | "moderator" | "admin") {
        return err("role", "validation");
    }
    match state.admin.set_role(user_id, &body.role).await {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

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

pub async fn clear_cache(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Response {
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
        deleted += state.redis.del::<u64, _>(chunk.to_vec()).await.unwrap_or(0);
    }
    Json(json!({"deleted_keys": deleted})).into_response()
}

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
            if let Err(e) = state
                .redis
                .set::<(), _, _>(&key, serialized, None, None, false)
                .await
            {
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
        .answer_type_create(
            body.name.trim(),
            body.pronoun.as_deref(),
            body.svg_icon.as_deref(),
        )
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
        .answer_type_update(
            id,
            body.name.trim(),
            body.pronoun.as_deref(),
            body.svg_icon.as_deref(),
        )
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
    match state.admin.page_title_taken(body.title.trim()).await {
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
        .create_page(
            body.title.trim(),
            &body.content,
            original_id,
            body.parent_id,
        )
        .await
    {
        Ok(rev_id) => Json(json!({"id": rev_id})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

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
        return err(
            "name",
            "The name field is required and must not exceed 50 characters.",
        );
    }
    match state
        .admin
        .admin_update_playlist(
            id,
            name,
            body.description.as_deref(),
            body.is_public.unwrap_or(false),
        )
        .await
    {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

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
        return err(
            "name",
            "The name field is required and must not exceed 50 characters.",
        );
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
        Ok(p) => Json(
            json!({"rooms": p.to_json(), "filters": {"search": q.search, "trashed": q.trashed}}),
        )
        .into_response(),
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
        return err(
            "name",
            "The name field is required and must not exceed 30 characters.",
        );
    }
    if !state
        .rooms
        .category_exists(body.category_id.unwrap_or(1))
        .await
        .unwrap_or(false)
    {
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
        return err(
            "name",
            "The name field is required and must not exceed 30 characters.",
        );
    }
    if state.rooms.name_taken(name, None).await.unwrap_or(false) {
        return err("name", "The name has already been taken.");
    }
    if let Some(url) = body.discord_webhook_url.as_deref() {
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return err(
                "discord_webhook_url",
                "The discord webhook url field must be a valid URL.",
            );
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
            state
                .levels
                .dispatch(user.id, blindall_db::LevelTrigger::RoomsCount);
            (StatusCode::CREATED, Json(json!({"id": id}))).into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

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
        Ok(p) => Json(
            json!({"teams": p.to_json(), "filters": {"search": q.search, "trashed": q.trashed}}),
        )
        .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct AdminTeamBody {
    name: String,
    user_id: i64,
}

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
        return err(
            "name",
            "The name field is required and must not exceed 50 characters.",
        );
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

pub async fn answer_types(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.admin.admin_answer_types().await {
        Ok(v) => Json(json!({"answer_types": v})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ---- data quality ------------------------------------------------------------

/// Suggest a work name for a track: parenthetical "(from X)"/"(de X)" first,
/// else the title minus boilerplate tokens.
fn suggest_work_name(title: &str) -> Option<String> {
    static PAREN: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static BOILER: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let paren = PAREN.get_or_init(|| regex::Regex::new(r"[\(\[]([^\)\]]{2,80})[\)\]]").unwrap());
    let boiler = BOILER.get_or_init(|| {
        regex::Regex::new(concat!(
            r"(?i)\b(main\s+theme|menu\s+theme|end\s+theme|title\s+theme|theme|ost|",
            r"original\s+soundtrack|soundtrack|score|trailer|medley|cover|version|",
            r"instrumental|piano|violin|orchestral|orchestra|music|song|opening|ending|",
            r"credits|suite|remix|reprise|intro|menu|title|from|the\s+motion\s+picture|",
            r"tv\s+size|episode|ep\.?|op\.?|ed\.?|movie|film|de|du|des|the|a|an)\b"
        ))
        .unwrap()
    });
    for cap in paren.captures_iter(title) {
        let inner = cap[1].trim();
        let inner = regex::Regex::new(r"(?i)^(from|de|du|de\s+la|le|la|the)\s+")
            .unwrap()
            .replace(inner, "")
            .trim()
            .trim_matches('"')
            .to_string();
        if inner.chars().count() >= 3
            && !regex::Regex::new(
                r"(?i)^(theme|version|cover|ost|soundtrack|edit|size|radio|extended|remix|live)",
            )
            .unwrap()
            .is_match(&inner)
        {
            return Some(inner);
        }
    }
    let core = paren.replace_all(title, " ");
    let core = boiler.replace_all(&core, " ");
    let core = core
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c: char| c == '-' || c == ':' || c == '–' || c == '|')
        .to_string();
    if core.chars().count() >= 3 {
        Some(core)
    } else {
        None
    }
}

/// Reviewers: admins and public moderators. Destructive ops stay admin-only.
async fn can_review(state: &AppState, user: &blindall_db::entities::User) -> bool {
    user.is_administrator || state.auth.is_public_moderator(user.id).await
}

pub async fn data_quality_rooms(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !can_review(&state, &user).await {
        return forbidden();
    }
    match state.admin.data_quality_rooms().await {
        Ok(v) => Json(json!({ "rooms": v })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct DataQualityQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    q: Option<String>,
}

pub async fn data_quality_missing(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(room_id): Path<i64>,
    Query(q): Query<DataQualityQuery>,
) -> Response {
    if !can_review(&state, &user).await {
        return forbidden();
    }
    let Some((type_id, type_name)) = (match state.admin.room_expected_answer_type(room_id).await {
        Ok(v) => v,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }) else {
        return Json(json!({ "data": [], "total": 0, "last_page": 1 })).into_response();
    };
    let page = q.page.unwrap_or(1).max(1);
    let per_page = q.per_page.unwrap_or(50).clamp(1, 100);
    let search = q.q.unwrap_or_default();
    match state
        .admin
        .tracks_missing_answer_type(room_id, type_id, &search, page, per_page)
        .await
    {
        Ok((rows, total)) => {
            let data: Vec<Value> = rows
                .into_iter()
                .map(|mut r| {
                    let title = r["title"].as_str().unwrap_or_default().to_string();
                    let best = suggest_work_name(&title)
                        .or_else(|| r["dict_work"].as_str().map(|s| s.to_string()))
                        .or_else(|| r["mb_work"].as_str().map(|s| s.to_string()));
                    r.as_object_mut().unwrap().insert(
                        "suggestion".into(),
                        match best {
                            Some(s) => json!(s),
                            None => Value::Null,
                        },
                    );
                    r
                })
                .collect();
            Json(json!({
                "data": data,
                "total": total,
                "last_page": (total + per_page - 1) / per_page,
                "answer_type_id": type_id,
                "type_name": type_name,
            }))
            .into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct DqTrackBody {
    room_id: i64,
    playlist_id: Option<i64>,
    value: Option<String>,
}

/// Apply a reviewed answer: insert + audit in one transaction. Idempotent.
pub async fn data_quality_apply_answer(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(track_id): Path<i64>,
    Json(b): Json<DqTrackBody>,
) -> Response {
    if !can_review(&state, &user).await {
        return forbidden();
    }
    let Some(value) = b.value.as_deref().map(str::trim).filter(|v| !v.is_empty()) else {
        return err("value", "required");
    };
    if value.chars().count() > 200 {
        return err("value", "too long");
    }
    let expected = match state.admin.room_expected_answer_type(b.room_id).await {
        Ok(v) => v,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let Some((type_id, _)) = expected else {
        return err("room", "no_expected_type");
    };
    match state
        .admin
        .apply_answer(
            track_id,
            b.playlist_id.unwrap_or(0),
            b.room_id,
            type_id,
            value,
            user.id,
        )
        .await
    {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Dismiss a candidate from a room's review queue (idempotent).
pub async fn data_quality_dismiss(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(track_id): Path<i64>,
    Json(b): Json<DqTrackBody>,
) -> Response {
    if !can_review(&state, &user).await {
        return forbidden();
    }
    let expected = match state.admin.room_expected_answer_type(b.room_id).await {
        Ok(v) => v,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let Some((type_id, _)) = expected else {
        return err("room", "no_expected_type");
    };
    match state
        .admin
        .dismiss_candidate(
            track_id,
            b.playlist_id.unwrap_or(0),
            b.room_id,
            type_id,
            user.id,
        )
        .await
    {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Remove a track from its playlist — snapshot kept in the audit payload.
pub async fn data_quality_remove_track(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(track_id): Path<i64>,
    Json(b): Json<DqTrackBody>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    let expected = match state.admin.room_expected_answer_type(b.room_id).await {
        Ok(v) => v,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let Some((type_id, _)) = expected else {
        return err("room", "no_expected_type");
    };
    let Some(playlist_id) = b.playlist_id else {
        return err("playlist_id", "required");
    };
    // The track must actually belong to that playlist.
    if !state
        .playlists
        .track_belongs_to(playlist_id, track_id)
        .await
    {
        return StatusCode::NOT_FOUND.into_response();
    }
    match state
        .admin
        .purge_track(track_id, playlist_id, b.room_id, type_id, user.id)
        .await
    {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn data_quality_actions(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !can_review(&state, &user).await {
        return forbidden();
    }
    match state.admin.data_quality_actions(200).await {
        Ok(v) => Json(json!({ "actions": v })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Batch-import external research evidence (album, MusicBrainz work, dict
/// match, confidence). Rows for missing tracks are skipped; capped at 500.
pub async fn data_quality_enrichment(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(b): Json<Value>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    let rows = b["rows"].as_array().cloned().unwrap_or_default();
    if rows.is_empty() || rows.len() > 500 {
        return err("rows", "1-500 rows required");
    }
    match state.admin.upsert_enrichment(&rows).await {
        Ok(n) => Json(json!({ "ok": true, "imported": n })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn data_quality_playlists(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.admin.playlist_options().await {
        Ok(v) => Json(json!({ "playlists": v })).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct DqMoveBody {
    room_id: i64,
    playlist_id: i64,
    to_playlist_id: i64,
}

/// Move a misplaced track to another playlist, audited.
pub async fn data_quality_move_track(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(track_id): Path<i64>,
    Json(b): Json<DqMoveBody>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    if b.playlist_id == b.to_playlist_id {
        return err("to_playlist_id", "same playlist");
    }
    let expected = match state.admin.room_expected_answer_type(b.room_id).await {
        Ok(v) => v,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let Some((type_id, _)) = expected else {
        return err("room", "no_expected_type");
    };
    match state
        .admin
        .move_track(
            track_id,
            b.playlist_id,
            b.to_playlist_id,
            b.room_id,
            type_id,
            user.id,
        )
        .await
    {
        Ok(true) => Json(json!({ "ok": true })).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Restore a track removed through the review queue (snapshot replay).
pub async fn data_quality_undo_remove(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(action_id): Path<i64>,
) -> Response {
    if !user.is_administrator {
        return forbidden();
    }
    match state.admin.undo_remove(action_id, user.id).await {
        Ok(true) => Json(json!({ "ok": true })).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// Undo a dismissal (restore the candidate into the queue).
pub async fn data_quality_restore(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(action_id): Path<i64>,
) -> Response {
    if !can_review(&state, &user).await {
        return forbidden();
    }
    match state.admin.restore_candidate(action_id).await {
        Ok(true) => Json(json!({ "ok": true })).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
