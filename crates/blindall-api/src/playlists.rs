use crate::auth::CurrentUser;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use blindall_db::playlists::PlaylistRepo;
use blindall_db::PlaylistRow;
use blindall_game::store::GameStore;
use serde::Deserialize;
use serde_json::{json, Value};

fn err(status: StatusCode, msg: &str) -> axum::response::Response {
    (status, Json(json!({ "error": msg }))).into_response()
}

async fn editable(
    repo: &PlaylistRepo,
    id: i64,
    user: &CurrentUser,
) -> Result<PlaylistRow, axum::response::Response> {
    let p = repo
        .find(id)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db_error"))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "playlist_not_found"))?;
    if !repo.can_edit(&p, user.id(), user.0.is_administrator).await {
        return Err(err(StatusCode::FORBIDDEN, "unauthorized"));
    }
    Ok(p)
}

async fn owned(
    repo: &PlaylistRepo,
    id: i64,
    user: &CurrentUser,
) -> Result<PlaylistRow, axum::response::Response> {
    let p = repo
        .find(id)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db_error"))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "playlist_not_found"))?;
    if p.user_id != user.id() && !user.0.is_administrator {
        return Err(err(StatusCode::FORBIDDEN, "unauthorized"));
    }
    Ok(p)
}

#[derive(Deserialize)]
pub struct IndexParams {
    scope: Option<String>,
}

pub async fn index(
    State(state): State<AppState>,
    user: CurrentUser,
    Query(q): Query<IndexParams>,
) -> impl IntoResponse {
    if q.scope.as_deref() == Some("public") {
        return match state.playlists.list_public().await {
            Ok(rows) => Json(json!({
                "playlists": rows.iter().map(|(id, name, tracks_count)| json!({
                    "id": id, "name": name, "tracks_count": tracks_count,
                })).collect::<Vec<_>>()
            }))
            .into_response(),
            Err(e) => {
                tracing::error!(error = %e, "playlists.index(public) failed");
                err(StatusCode::INTERNAL_SERVER_ERROR, "db_error")
            }
        };
    }
    match state.playlists.list_for_user(user.id()).await {
        Ok(playlists) => Json(json!({ "playlists": playlists })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "playlists.index failed");
            err(StatusCode::INTERNAL_SERVER_ERROR, "db_error")
        }
    }
}

pub async fn show(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let p = match state
        .playlists
        .find(id)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db_error"))
    {
        Ok(Some(p)) => p,
        Ok(None) => return err(StatusCode::NOT_FOUND, "playlist_not_found"),
        Err(r) => return r,
    };
    let can_edit = state
        .playlists
        .can_edit(&p, user.id(), user.0.is_administrator)
        .await;
    let owned = p.user_id == user.id() || user.0.is_administrator;
    let moderators = state.playlists.moderators(id).await.unwrap_or_default();
    let rooms = state
        .playlists
        .rooms_for_playlist(id)
        .await
        .unwrap_or_default();
    let difficulties = state
        .playlists
        .difficulty_counts(id)
        .await
        .unwrap_or_default();
    let total_tracks = state.playlists.track_count(id).await.unwrap_or(0);
    Json(json!({
        "playlist": {
            "id": p.id,
            "name": p.name,
            "description": p.description,
            "user_id": p.user_id,
            "is_public": p.is_public,
            "moderators": moderators.iter().map(|m| json!({
                "id": m.id, "name": m.name,
            })).collect::<Vec<_>>(),
            "rooms": rooms.iter().map(|(rid, slug, name)| json!({
                "id": rid, "slug": slug, "name": name,
            })).collect::<Vec<_>>(),
            "difficulties": difficulties.iter().map(|(d, c)| json!({
                "dificulty": d, "count": c,
            })).collect::<Vec<_>>(),
            "total_tracks": total_tracks,
        },
        "can_edit": can_edit,
        "owned": owned,
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct CreateBody {
    name: String,
}

pub async fn store(
    State(state): State<AppState>,
    user: CurrentUser,
    Json(body): Json<CreateBody>,
) -> impl IntoResponse {
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 50 {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "invalid_name");
    }
    match state.playlists.create(user.id(), name).await {
        Ok(id) => {
            state
                .levels
                .dispatch(user.id(), blindall_db::LevelTrigger::PlaylistsCount);
            (StatusCode::CREATED, Json(json!({ "id": id }))).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "playlists.store failed");
            err(StatusCode::INTERNAL_SERVER_ERROR, "db_error")
        }
    }
}

#[derive(Deserialize)]
pub struct UpdateBody {
    name: String,
    description: Option<String>,
    user_id: i64,
}

pub async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    Json(body): Json<UpdateBody>,
) -> impl IntoResponse {
    let p = match owned(&state.playlists, id, &user).await {
        Ok(p) => p,
        Err(r) => return r,
    };
    if body.name.trim().is_empty() || body.name.chars().count() > 50 {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "invalid_name");
    }
    match state
        .playlists
        .update(p.id, body.name.trim(), body.description.as_deref(), body.user_id)
        .await
    {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "playlists.update failed");
            err(StatusCode::INTERNAL_SERVER_ERROR, "db_error")
        }
    }
}

pub async fn destroy(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let p = match owned(&state.playlists, id, &user).await {
        Ok(p) => p,
        Err(r) => return r,
    };
    if p.is_public && state.playlists.has_rooms(p.id).await {
        return err(
            StatusCode::CONFLICT,
            "Impossible to delete a playlist that is public and associated to a room",
        );
    }
    match state.playlists.delete(p.id).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "playlists.destroy failed");
            err(StatusCode::INTERNAL_SERVER_ERROR, "db_error")
        }
    }
}

#[derive(Deserialize)]
pub struct PageParams {
    page: Option<i64>,
    per_page: Option<i64>,
}

pub async fn tracks(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    Query(q): Query<PageParams>,
) -> impl IntoResponse {
    if let Err(r) = editable(&state.playlists, id, &user).await {
        return r;
    }
    let page = q.page.unwrap_or(1).max(1);
    let per = q.per_page.unwrap_or(10).clamp(1, 100);
    match state.playlists.tracks_page(id, page, per).await {
        Ok((tracks, total)) => Json(json!({
            "data": tracks,
            "total": total,
            "page": page,
            "per_page": per,
        }))
        .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "playlists.tracks failed");
            err(StatusCode::INTERNAL_SERVER_ERROR, "db_error")
        }
    }
}

#[derive(Deserialize)]
pub struct StoreTrackBody {
    provider: String,
    provider_id: String,
    provider_url: String,
    artist_name: String,
    track_name: String,
    preview_url: String,
    artwork_url: String,
}

pub async fn store_track(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    Json(b): Json<StoreTrackBody>,
) -> impl IntoResponse {
    if let Err(r) = editable(&state.playlists, id, &user).await {
        return r;
    }
    let ok = !b.provider.is_empty() && b.provider.len() <= 50
        && !b.provider_id.is_empty() && b.provider_id.len() <= 255
        && !b.provider_url.is_empty() && b.provider_url.len() <= 255
        && !b.artist_name.is_empty() && b.artist_name.len() <= 255
        && !b.track_name.is_empty() && b.track_name.len() <= 255
        && !b.preview_url.is_empty() && b.preview_url.len() <= 2048
        && !b.artwork_url.is_empty() && b.artwork_url.len() <= 255;
    if !ok {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "validation_failed");
    }
    match state
        .playlists
        .upsert_track(
            id,
            user.id(),
            &b.provider,
            &b.provider_id,
            &b.provider_url,
            &b.preview_url,
            &b.artwork_url,
            &b.artist_name,
            &b.track_name,
        )
        .await
    {
        Ok(track_id) => {
            let message = format!(
                "Le titre {} de {} a été ajouté par {}.",
                b.track_name, b.artist_name, user.0.name
            );
            state
                .store
                .post_discord_to_rooms(id, &message, 3066993)
                .await;
            (StatusCode::CREATED, Json(json!({ "id": track_id }))).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "playlists.store_track failed");
            err(StatusCode::INTERNAL_SERVER_ERROR, "db_error")
        }
    }
}

#[derive(Deserialize)]
pub struct UpdateTrackBody {
    dificulty: Option<i64>,
    hint: Option<String>,
}

pub async fn update_track(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((id, track_id)): Path<(i64, i64)>,
    Json(b): Json<UpdateTrackBody>,
) -> impl IntoResponse {
    if let Err(r) = editable(&state.playlists, id, &user).await {
        return r;
    }
    if !state.playlists.track_belongs_to(id, track_id).await {
        return err(StatusCode::NOT_FOUND, "track_not_found");
    }
    if let Some(d) = b.dificulty {
        if !(0..=5).contains(&d) {
            return err(StatusCode::UNPROCESSABLE_ENTITY, "invalid_dificulty");
        }
    }
    if b.hint.as_ref().map(|h| h.len() > 255).unwrap_or(false) {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "invalid_hint");
    }
    match state
        .playlists
        .update_track_meta(track_id, b.dificulty, b.hint.as_deref())
        .await
    {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "playlists.update_track failed");
            err(StatusCode::INTERNAL_SERVER_ERROR, "db_error")
        }
    }
}

pub async fn delete_track(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((id, track_id)): Path<(i64, i64)>,
) -> impl IntoResponse {
    if let Err(r) = editable(&state.playlists, id, &user).await {
        return r;
    }
    if !state.playlists.track_belongs_to(id, track_id).await {
        return err(StatusCode::NOT_FOUND, "track_not_found");
    }
    match state
        .store
        .delete_track(track_id, Some(user.0.name.clone()))
        .await
    {
        Ok(notified) => {
            for n in notified {
                crate::users::push_notification(
                    &state,
                    n.user_id,
                    "App\\Notifications\\TrackDeleted",
                    &n.notification_id,
                    n.data,
                )
                .await;
            }
            Json(json!({ "ok": true })).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "playlists.delete_track failed");
            err(StatusCode::INTERNAL_SERVER_ERROR, "db_error")
        }
    }
}

pub async fn clear_tracks(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if let Err(r) = editable(&state.playlists, id, &user).await {
        return r;
    }
    let ids = match state.playlists.track_ids(id).await {
        Ok(ids) => ids,
        Err(e) => {
            tracing::error!(error = %e, "playlists.track_ids failed");
            return err(StatusCode::INTERNAL_SERVER_ERROR, "db_error");
        }
    };
    for track_id in ids {
        match state
            .store
            .delete_track(track_id, Some(user.0.name.clone()))
            .await
        {
            Ok(notified) => {
                for n in notified {
                    crate::users::push_notification(
                        &state,
                        n.user_id,
                        "App\\Notifications\\TrackDeleted",
                        &n.notification_id,
                        n.data,
                    )
                    .await;
                }
            }
            Err(e) => {
                tracing::error!(error = %e, track_id, "playlists.clear_tracks failed");
            }
        }
    }
    Json(json!({ "ok": true })).into_response()
}

#[derive(Deserialize)]
pub struct SearchParams {
    term: Option<String>,
    providers: Option<String>,
}

pub async fn search_tracks(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    Query(q): Query<SearchParams>,
) -> impl IntoResponse {
    if let Err(r) = editable(&state.playlists, id, &user).await {
        return r;
    }
    let term = q.term.unwrap_or_default();
    let providers: Vec<&str> = q
        .providers
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    if term.trim().is_empty() || providers.is_empty() {
        return Json(json!({ "tracks": [], "errors": [] })).into_response();
    }

    let is_mod = state.auth.is_public_moderator(user.id()).await;
    let (results, errors) = state
        .providers
        .search(&term, &providers, is_mod)
        .await;

    let mut tracks = Vec::with_capacity(results.len());
    for t in &results {
        let added = state
            .playlists
            .provider_track_id(id, &t.provider_id)
            .await
            .ok()
            .flatten();
        let mut v = serde_json::to_value(t).unwrap_or_default();
        v["added"] = added.map(Value::from).unwrap_or(Value::Null);
        tracks.push(v);
    }
    Json(json!({ "tracks": tracks, "errors": errors })).into_response()
}

pub async fn answer_types(State(state): State<AppState>) -> impl IntoResponse {
    match state.playlists.answer_types().await {
        Ok(types) => Json(json!({
            "answer_types": types.iter().map(|(id, name)| json!({"id": id, "name": name}))
                .collect::<Vec<_>>()
        }))
        .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "answer_types failed");
            err(StatusCode::INTERNAL_SERVER_ERROR, "db_error")
        }
    }
}

async fn editable_track(
    state: &AppState,
    track_id: i64,
    user: &CurrentUser,
) -> Result<i64, axum::response::Response> {
    let playlist_id = state
        .playlists
        .track_playlist_id(track_id)
        .await
        .map_err(|_| err(StatusCode::INTERNAL_SERVER_ERROR, "db_error"))?
        .ok_or_else(|| err(StatusCode::NOT_FOUND, "track_not_found"))?;
    editable(&state.playlists, playlist_id, user).await.map(|_| playlist_id)
}

#[derive(Deserialize)]
pub struct AnswerBody {
    answer_type_id: i64,
    value: String,
    score: Option<f64>,
    aliases: Option<Vec<String>>,
}

fn validate_answer(b: &AnswerBody) -> Result<Option<Vec<String>>, axum::response::Response> {
    let value_len = b.value.trim().len();
    if value_len == 0 || value_len > 255 {
        return Err(err(StatusCode::UNPROCESSABLE_ENTITY, "invalid_value"));
    }
    let score = b.score.unwrap_or(0.5);
    if !(0.0..=99.0).contains(&score) {
        return Err(err(StatusCode::UNPROCESSABLE_ENTITY, "invalid_score"));
    }
    let mut seen = std::collections::HashSet::new();
    let aliases: Vec<String> = b
        .aliases
        .clone()
        .unwrap_or_default()
        .into_iter()
        .map(|a| a.trim().to_string())
        .filter(|a| !a.is_empty() && seen.insert(a.clone()))
        .take(20)
        .collect();
    Ok(if aliases.is_empty() { None } else { Some(aliases) })
}

pub async fn store_answer(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(track_id): Path<i64>,
    Json(b): Json<AnswerBody>,
) -> impl IntoResponse {
    if let Err(r) = editable_track(&state, track_id, &user).await {
        return r;
    }
    let known: Vec<i64> = state
        .playlists
        .answer_types()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    if !known.contains(&b.answer_type_id) {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "invalid_answer_type");
    }
    let aliases = match validate_answer(&b) {
        Ok(a) => a,
        Err(r) => return r,
    };
    match state
        .playlists
        .create_answer(
            track_id,
            b.answer_type_id,
            b.value.trim(),
            b.score.unwrap_or(0.5),
            aliases,
        )
        .await
    {
        Ok(id) => (StatusCode::CREATED, Json(json!({ "id": id }))).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "answers.store failed");
            err(StatusCode::INTERNAL_SERVER_ERROR, "db_error")
        }
    }
}

pub async fn update_answer(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((track_id, answer_id)): Path<(i64, i64)>,
    Json(b): Json<AnswerBody>,
) -> impl IntoResponse {
    if let Err(r) = editable_track(&state, track_id, &user).await {
        return r;
    }
    if !state.playlists.answer_belongs_to(track_id, answer_id).await {
        return err(StatusCode::NOT_FOUND, "answer_not_found");
    }
    let known: Vec<i64> = state
        .playlists
        .answer_types()
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    if !known.contains(&b.answer_type_id) {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "invalid_answer_type");
    }
    let aliases = match validate_answer(&b) {
        Ok(a) => a,
        Err(r) => return r,
    };
    match state
        .playlists
        .update_answer(
            answer_id,
            b.answer_type_id,
            b.value.trim(),
            b.score.unwrap_or(0.5),
            aliases,
        )
        .await
    {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "answers.update failed");
            err(StatusCode::INTERNAL_SERVER_ERROR, "db_error")
        }
    }
}

pub async fn delete_answer(
    State(state): State<AppState>,
    user: CurrentUser,
    Path((track_id, answer_id)): Path<(i64, i64)>,
) -> impl IntoResponse {
    if let Err(r) = editable_track(&state, track_id, &user).await {
        return r;
    }
    if !state.playlists.answer_belongs_to(track_id, answer_id).await {
        return err(StatusCode::NOT_FOUND, "answer_not_found");
    }
    match state.playlists.delete_answer(answer_id).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "answers.destroy failed");
            err(StatusCode::INTERNAL_SERVER_ERROR, "db_error")
        }
    }
}

#[derive(Deserialize)]
pub struct ProviderPlaylistBody {
    provider: String,
    playlist_id: String,
}

pub async fn find_provider_playlist(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    Json(b): Json<ProviderPlaylistBody>,
) -> impl IntoResponse {
    if let Err(r) = editable(&state.playlists, id, &user).await {
        return r;
    }
    if b.playlist_id.trim().is_empty() && b.provider != "blindall" {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "invalid_playlist_id");
    }
    if b.provider == "blindall" {
        let count = state
            .playlists
            .upvoted_track_ids(user.id())
            .await
            .unwrap_or_default()
            .len();
        return Json(json!({
            "playlist": {
                "id": Value::Null,
                "name": format!("{} likes", user.0.name),
                "description": "Tous les titres que tu as liké sur Blind'All Music",
                "tracks_count": count,
                "image": user.0.photo_path,
            }
        }))
        .into_response();
    }
    match state
        .providers
        .find_playlist(&b.provider, &b.playlist_id)
        .await
    {
        Ok(playlist) => Json(json!({ "playlist": playlist })).into_response(),
        Err(e) => (
            StatusCode::from_u16(e.status_code).unwrap_or(StatusCode::BAD_GATEWAY),
            Json(json!({ "error": e.message })),
        )
            .into_response(),
    }
}

pub async fn import_provider_playlist(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<i64>,
    Json(b): Json<ProviderPlaylistBody>,
) -> impl IntoResponse {
    if let Err(r) = editable(&state.playlists, id, &user).await {
        return r;
    }
    let mut imported = 0u64;
    if b.provider == "blindall" {
        let ids = state
            .playlists
            .upvoted_track_ids(user.id())
            .await
            .unwrap_or_default();
        for tid in ids {
            if let Ok(Some(_)) = state
                .playlists
                .duplicate_track(id, user.id(), tid)
                .await
            {
                imported += 1;
            }
        }
    } else {
        let tracks = match state
            .providers
            .playlist_tracks(&b.provider, &b.playlist_id)
            .await
        {
            Ok(t) => t,
            Err(e) => {
                return (
                    StatusCode::from_u16(e.status_code)
                        .unwrap_or(StatusCode::BAD_GATEWAY),
                    Json(json!({ "error": e.message })),
                )
                    .into_response()
            }
        };
        for t in tracks {
            let (Some(preview), Some(artwork)) = (t.preview_url, t.artwork_url) else {
                continue;
            };
            let provider_url = t.provider_url.unwrap_or_default();
            let artist = t.artist_name.unwrap_or_default();
            if artist.is_empty() || t.track_name.is_empty() || provider_url.is_empty() {
                continue;
            }
            if state
                .playlists
                .upsert_track(
                    id,
                    user.id(),
                    &t.provider,
                    &t.provider_id,
                    &provider_url,
                    &preview,
                    &artwork,
                    &artist,
                    &t.track_name,
                )
                .await
                .is_ok()
            {
                imported += 1;
            }
        }
    }
    Json(json!({ "imported": imported })).into_response()
}

// ---- moderators (owner/admin only — `PlaylistModeratorController`) -----------

#[derive(Deserialize)]
pub struct ModeratorBody {
    user_id: i64,
}

async fn load_owned_playlist(
    state: &AppState,
    id: i64,
    user: &CurrentUser,
) -> Result<PlaylistRow, axum::response::Response> {
    let Some(p) = state.playlists.find(id).await.ok().flatten() else {
        return Err(err(StatusCode::NOT_FOUND, "not_found"));
    };
    if p.user_id != user.0.id && !user.0.is_administrator {
        return Err(err(StatusCode::FORBIDDEN, "unauthorized"));
    }
    Ok(p)
}

pub async fn attach_playlist_moderator(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    user: CurrentUser,
    Json(body): Json<ModeratorBody>,
) -> axum::response::Response {
    if let Err(e) = load_owned_playlist(&state, id, &user).await {
        return e;
    }
    if !state.rooms.user_exists(body.user_id).await.unwrap_or(false) {
        return err(StatusCode::UNPROCESSABLE_ENTITY, "The selected user id is invalid.");
    }
    match state.playlists.attach_moderator(id, body.user_id).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn detach_playlist_moderator(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    user: CurrentUser,
    Json(body): Json<ModeratorBody>,
) -> axum::response::Response {
    if let Err(e) = load_owned_playlist(&state, id, &user).await {
        return e;
    }
    match state.playlists.detach_moderator(id, body.user_id).await {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ---- CSV export (`PlaylistCsvExportService::download`) -------------------------

fn csv_escape(s: &str) -> String {
    if s.contains(';') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub async fn export(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    user: CurrentUser,
) -> axum::response::Response {
    let Some(p) = state.playlists.find(id).await.ok().flatten() else {
        return err(StatusCode::NOT_FOUND, "not_found");
    };
    if !state
        .playlists
        .can_edit(&p, user.id(), user.0.is_administrator)
        .await
    {
        return err(StatusCode::FORBIDDEN, "unauthorized");
    }

    let types = state.playlists.answer_types().await.unwrap_or_default();
    let tracks = state.playlists.export_tracks(id).await.unwrap_or_default();

    let mut out = String::from("\u{feff}");
    let mut headings: Vec<String> = types.iter().map(|(_, n)| n.clone()).collect();
    headings.extend(
        ["Origine", "Lien", "Difficulté", "Vote +", "Vote -", "Ajouté le"]
            .iter()
            .map(|s| s.to_string()),
    );
    out.push_str(&headings.iter().map(|h| csv_escape(h)).collect::<Vec<_>>().join(";"));
    out.push('\n');

    for t in &tracks {
        let mut cols: Vec<String> = Vec::new();
        for (type_id, _) in &types {
            let vals: Vec<&str> = t
                .answers
                .iter()
                .filter(|a| a.answer_type_id == type_id.to_string())
                .map(|a| a.value.as_str())
                .collect();
            cols.push(csv_escape(&vals.join(", ")));
        }
        cols.push(csv_escape(&t.provider));
        cols.push(csv_escape(t.provider_url.as_deref().unwrap_or("")));
        cols.push(t.dificulty.map(|d| d.to_string()).unwrap_or_default());
        cols.push(t.up_votes.to_string());
        cols.push(t.down_votes.to_string());
        cols.push(
            t.created_at
                .map(|c| c.format("%d/%m/%y %H:%M").to_string())
                .unwrap_or_default(),
        );
        out.push_str(&cols.join(";"));
        out.push('\n');
    }

    let filename = format!("{}.csv", blindall_core::sanitize::str_slug(&p.name, '-'));
    (
        [
            (axum::http::header::CONTENT_TYPE, "text/csv; charset=utf-8".to_string()),
            (
                axum::http::header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        out,
    )
        .into_response()
}
