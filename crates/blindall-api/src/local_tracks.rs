use crate::auth::CurrentUser;
use crate::state::AppState;
use axum::{
    Json,
    body::Body,
    extract::{FromRequest, Path, Query, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use blindall_game::store::GameStore;
use serde::Deserialize;
use serde_json::json;

/// Cheap MP3 signature check — ID3v2 tag or MPEG frame sync. `mp3_duration`
/// already requires decodable frames; this rejects polyglot junk earlier.
fn looks_like_mp3(b: &[u8]) -> bool {
    b.starts_with(b"ID3") || (b.len() >= 2 && b[0] == 0xFF && (b[1] & 0xE0) == 0xE0)
}

/// Parses `Range: bytes=start-end|start-|-suffix` into an inclusive pair.
fn parse_range(header: &str, total: u64) -> Option<(u64, u64)> {
    let spec = header.strip_prefix("bytes=")?.split(',').next()?.trim();
    let (a, b) = spec.split_once('-')?;
    let (start, end) = if a.is_empty() {
        let suffix: u64 = b.parse().ok()?;
        if suffix == 0 || total == 0 {
            return None;
        }
        (total.saturating_sub(suffix), total - 1)
    } else {
        let start: u64 = a.parse().ok()?;
        let end: u64 = if b.is_empty() {
            total - 1
        } else {
            b.parse().ok()?
        };
        (start, end.min(total.saturating_sub(1)))
    };
    if start > end || start >= total {
        return None;
    }
    Some((start, end))
}

fn err(field: &str, msg: &str) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"message": msg, "errors": { field: [msg] }})),
    )
        .into_response()
}

pub async fn store(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    request: axum::extract::Request,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    // PHP auth.moderator accepts admins OR public moderators.
    if !(user.is_administrator || state.auth.is_public_moderator(user.id).await) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let mut multipart = match axum::extract::Multipart::from_request(request, &state).await {
        Ok(m) => m,
        Err(e) => return e.into_response(),
    };
    let mut audio: Option<Vec<u8>> = None;
    let mut artwork: Option<Vec<u8>> = None;
    let mut artist_name = String::new();
    let mut track_name = String::new();

    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("audio") => match field.bytes().await {
                Ok(b) if b.len() <= 4 * 1024 * 1024 => audio = Some(b.to_vec()),
                _ => return err("audio", "Le fichier audio ne doit pas dépasser 4 Mo."),
            },
            Some("artwork") => match field.bytes().await {
                Ok(b) if b.len() <= 8 * 1024 * 1024 => artwork = Some(b.to_vec()),
                _ => return err("artwork", "La pochette ne doit pas dépasser 8 Mo."),
            },
            Some("artist_name") => {
                artist_name = field.text().await.unwrap_or_default();
            }
            Some("track_name") => {
                track_name = field.text().await.unwrap_or_default();
            }
            _ => {}
        }
    }

    let artist_name = artist_name.trim().to_string();
    let track_name = track_name.trim().to_string();
    if artist_name.is_empty() || artist_name.len() > 255 {
        return err("artist_name", "validation");
    }
    if track_name.is_empty() || track_name.len() > 255 {
        return err("track_name", "validation");
    }
    let (Some(audio), Some(artwork)) = (audio, artwork) else {
        return err("audio", "validation");
    };
    if !looks_like_mp3(&audio) {
        return err("audio", "Le fichier doit être un MP3.");
    }

    let duration = mp3_duration::from_read(&mut std::io::Cursor::new(&audio))
        .map(|d| d.as_secs_f64())
        .unwrap_or(-1.0);
    if (duration - 30.0).abs() > 1.0 {
        return err(
            "audio",
            "Le fichier audio doit durer précisément 30 secondes.",
        );
    }

    match state
        .local_tracks
        .name_taken(&artist_name, &track_name, None)
        .await
    {
        Ok(true) => {
            return err(
                "track_name",
                "Cette chanson existe déjà dans la base de Blind'All Music.",
            );
        }
        Ok(false) => {}
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }

    let webp = match crate::storage::image_to_webp_contain(&artwork, 300, 300) {
        Ok(w) => w,
        Err(()) => return err("artwork", "The artwork must be an image."),
    };
    let audio_path = format!("uploads/{}.mp3", uuid::Uuid::now_v7().simple());
    let artwork_path = format!(
        "artworks/{}_{}.webp",
        blindall_core::sanitize::str_slug(&artist_name, '-'),
        blindall_core::sanitize::str_slug(&track_name, '-')
    );
    if state.storage.put(&audio_path, &audio).await.is_err()
        || state.storage.put(&artwork_path, &webp).await.is_err()
    {
        let _ = state.storage.delete(&audio_path).await;
        let _ = state.storage.delete(&artwork_path).await;
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    match state
        .local_tracks
        .create(
            &artist_name,
            &track_name,
            &audio_path,
            &artwork_path,
            user.id,
        )
        .await
    {
        Ok(id) => (StatusCode::CREATED, Json(json!({"id": id}))).into_response(),
        Err(_) => {
            let _ = state.storage.delete(&audio_path).await;
            let _ = state.storage.delete(&artwork_path).await;
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

fn audio_headers(h: &mut axum::http::HeaderMap) {
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static("audio/mpeg"));
    h.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    h.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, HEAD"),
    );
    h.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("*"),
    );
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=60"),
    );
    h.insert(header::ACCEPT_RANGES, HeaderValue::from_static("bytes"));
}

pub async fn audio(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: axum::http::HeaderMap,
) -> Response {
    let paths = match state.local_tracks.paths(id).await {
        Ok(Some(p)) => p,
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let bytes = match state.storage.read(&paths.0).await {
        Ok(b) => b,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let total = bytes.len() as u64;
    // iOS/Safari <audio> refuses plain 200 responses — honour byte ranges.
    if let Some(range) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        let Some((start, end)) = parse_range(range, total) else {
            return StatusCode::RANGE_NOT_SATISFIABLE.into_response();
        };
        let slice = bytes[start as usize..=end as usize].to_vec();
        let mut res = (StatusCode::PARTIAL_CONTENT, Body::from(slice)).into_response();
        audio_headers(res.headers_mut());
        if let Ok(v) = HeaderValue::from_str(&format!("bytes {start}-{end}/{total}")) {
            res.headers_mut().insert(header::CONTENT_RANGE, v);
        }
        return res;
    }
    let mut res = Body::from(bytes).into_response();
    audio_headers(res.headers_mut());
    res
}

#[derive(Deserialize)]
pub struct SearchQuery {
    term: Option<String>,
}

pub async fn search(State(state): State<AppState>, Query(q): Query<SearchQuery>) -> Response {
    let Some(term) = q.term.filter(|t| !t.is_empty()) else {
        return Json(json!([])).into_response();
    };
    match state.local_tracks.search(&term).await {
        Ok(rows) => {
            let app_url = state.app_url.trim_end_matches('/');
            Json(json!(
                rows.iter()
                    .map(|(id, artist, track, artwork_path)| {
                        let url = format!("{app_url}/api/v1/local-tracks/{id}/audio");
                        json!({
                            "provider": "local",
                            "provider_id": id,
                            "provider_url": url,
                            "provider_popularity": 0,
                            "artist_name": artist,
                            "track_name": track,
                            "album_name": null,
                            "preview_url": url,
                            "artwork_url": state.storage.url(artwork_path),
                        })
                    })
                    .collect::<Vec<_>>()
            ))
            .into_response()
        }
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

// ---- moderation --------------------------------------------------------------

#[derive(Deserialize)]
pub struct PageQuery {
    search: Option<String>,
    sort_by: Option<String>,
    sort_direction: Option<String>,
    page: Option<i64>,
    per_page: Option<i64>,
}

pub async fn index(
    State(state): State<AppState>,
    Query(q): Query<PageQuery>,
    CurrentUser(user): CurrentUser,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }
    if !(user.is_administrator || state.auth.is_public_moderator(user.id).await) {
        return StatusCode::FORBIDDEN.into_response();
    }
    match state
        .local_tracks
        .paginate(
            q.search.as_deref(),
            q.sort_by.as_deref().unwrap_or("created_at"),
            q.sort_direction.as_deref().unwrap_or("desc"),
            q.page.unwrap_or(1),
            q.per_page.unwrap_or(20).clamp(1, 100),
        )
        .await
    {
        Ok((tracks, total)) => Json(json!({
            "data": tracks,
            "total": total,
            "current_page": q.page.unwrap_or(1).max(1),
        }))
        .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Deserialize)]
pub struct UpdateBody {
    artist_name: String,
    track_name: String,
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<UpdateBody>,
) -> Response {
    if let Some(r) = crate::rooms::require_not_guest(&user) {
        return r;
    }

    if !(user.is_administrator || state.auth.is_public_moderator(user.id).await) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let artist = body.artist_name.trim();
    let track = body.track_name.trim();
    if artist.is_empty() || artist.len() > 255 || track.is_empty() || track.len() > 255 {
        return err("track_name", "validation");
    }
    match state.local_tracks.name_taken(artist, track, Some(id)).await {
        Ok(true) => return err("track_name", "The track name has already been taken."),
        Ok(false) => {}
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
    match state.local_tracks.update(id, artist, track).await {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
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

    if !(user.is_administrator || state.auth.is_public_moderator(user.id).await) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let paths = match state.local_tracks.paths(id).await {
        Ok(Some(p)) => p,
        Ok(None) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    if !paths.0.is_empty() {
        state.storage.delete(&paths.0).await;
    }
    if !paths.1.is_empty() {
        state.storage.delete(&paths.1).await;
    }
    match state.local_tracks.linked_track_ids(id).await {
        Ok(ids) => {
            for track_id in ids {
                match state
                    .store
                    .delete_track(track_id, Some(user.name.clone()))
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
                        tracing::error!(error = %e, track_id, "local_tracks: linked track delete failed");
                    }
                }
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "local_tracks: linked track lookup failed");
        }
    }
    match state.local_tracks.delete(id).await {
        Ok(true) => Json(json!({"ok": true})).into_response(),
        Ok(false) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
