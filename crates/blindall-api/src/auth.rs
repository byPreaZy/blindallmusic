use crate::state::AppState;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use blindall_db::entities::User;

#[derive(Debug, Clone)]
pub struct CurrentUser(pub User);

impl CurrentUser {
    pub fn id(&self) -> i64 {
        self.0.id
    }
}

#[derive(Debug)]
pub enum AuthError {
    Missing,
    Invalid,
    Banned,
}

impl axum::response::IntoResponse for AuthError {
    fn into_response(self) -> axum::response::Response {
        let (status, code) = match self {
            Self::Missing => (axum::http::StatusCode::UNAUTHORIZED, "unauthenticated"),
            Self::Invalid => (axum::http::StatusCode::UNAUTHORIZED, "invalid_token"),
            Self::Banned => (axum::http::StatusCode::FORBIDDEN, "banned"),
        };
        axum::response::Response::builder()
            .status(status)
            .header("content-type", "application/json")
            .body(format!("{{\"error\":\"{code}\"}}").into())
            .unwrap()
    }
}

pub fn bearer_token(parts: &Parts) -> Option<String> {
    parts
        .headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")
        .map(str::to_string)
}

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer_token(parts).ok_or(AuthError::Missing)?;
        let user = state
            .auth
            .user_for_token(&token)
            .await
            .map_err(|_| AuthError::Invalid)?
            .ok_or(AuthError::Invalid)?;
        if state.moderation.is_banned(user.id).await.unwrap_or(false) {
            return Err(AuthError::Banned);
        }
        Ok(Self(user))
    }
}

#[derive(Debug, Clone)]
pub struct OptionalUser(pub Option<User>);

impl FromRequestParts<AppState> for OptionalUser {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = match bearer_token(parts) {
            Some(t) => state.auth.user_for_token(&t).await.ok().flatten(),
            None => None,
        };
        Ok(Self(user))
    }
}
