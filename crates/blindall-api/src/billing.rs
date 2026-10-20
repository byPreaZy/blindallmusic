use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use crate::auth::{CurrentUser, OptionalUser};
use crate::state::AppState;
use blindall_db::subscriptions::{PLAN_PLUS, PLAN_PRO};

#[derive(Clone)]
pub struct BillingConfig {
    /// Stripe Payment Links created once in the dashboard (subscription mode).
    /// Empty = the plan is not for sale in this environment.
    pub plus_payment_url: String,
    pub pro_payment_url: String,
}

impl Default for BillingConfig {
    fn default() -> Self {
        Self {
            plus_payment_url: String::new(),
            pro_payment_url: String::new(),
        }
    }
}

pub async fn entitlements(state: &AppState, user_id: i64) -> Vec<String> {
    state
        .subscriptions
        .entitlements(user_id)
        .await
        .unwrap_or_default()
}

pub async fn is_plus(state: &AppState, user_id: i64) -> bool {
    state
        .subscriptions
        .has_plan(user_id, PLAN_PLUS)
        .await
        .unwrap_or(false)
}

pub async fn is_pro(state: &AppState, user_id: i64) -> bool {
    state
        .subscriptions
        .has_plan(user_id, PLAN_PRO)
        .await
        .unwrap_or(false)
}

/// client_reference_id carries "<user_id>:<plan>" — the Payment Link cannot
/// forward arbitrary metadata, so the plan travels in the reference.
fn subscription_url(base: &str, user_id: i64, plan: &str) -> String {
    let sep = if base.contains('?') { "&" } else { "?" };
    format!("{base}{sep}client_reference_id={user_id}:{plan}")
}

pub async fn billing(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> Response {
    let plans = entitlements(&state, user.id).await;
    let subs = state
        .subscriptions
        .list_for_user(user.id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|s| {
            json!({
                "plan": s.plan,
                "status": s.status,
                "current_period_end": s.current_period_end.map(|d| d.to_rfc3339()),
                "canceled": s.canceled_at.is_some(),
            })
        })
        .collect::<Vec<_>>();
    Json(json!({
        "plans": plans,
        "is_plus": plans.iter().any(|p| p == PLAN_PLUS),
        "is_pro": plans.iter().any(|p| p == PLAN_PRO),
        "subscriptions": subs,
        "plus_payment_url": if user.is_guest || state.billing.plus_payment_url.is_empty() {
            Value::Null
        } else {
            json!(subscription_url(&state.billing.plus_payment_url, user.id, PLAN_PLUS))
        },
        "pro_payment_url": if user.is_guest || state.billing.pro_payment_url.is_empty() {
            Value::Null
        } else {
            json!(subscription_url(&state.billing.pro_payment_url, user.id, PLAN_PRO))
        },
    }))
    .into_response()
}

// ---- webhook (called from donations::stripe_webhook dispatch) -------------

/// `checkout.session.completed` in subscription mode. Idempotent via the
/// UNIQUE index on stripe_checkout_session_id.
pub async fn record_session(state: &AppState, session: &Value) -> Result<(), Response> {
    let Some(reference) = session["client_reference_id"].as_str() else {
        return Ok(());
    };
    let Some((uid, plan)) = reference.split_once(':') else {
        return Ok(());
    };
    if plan != PLAN_PLUS && plan != PLAN_PRO {
        return Ok(());
    }
    let Ok(user_id) = uid.parse::<i64>() else {
        return Ok(());
    };
    let session_id = session["id"].as_str().unwrap_or("");
    state
        .subscriptions
        .record_checkout(
            user_id,
            plan,
            session_id,
            session["subscription"].as_str(),
            session["customer"].as_str(),
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "subscription checkout record failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        })?;
    tracing::info!(
        user = user_id,
        plan = plan,
        "subscription checkout recorded"
    );
    Ok(())
}

/// `customer.subscription.created|updated|deleted` — sync status + period end.
pub async fn sync_subscription(state: &AppState, sub: &Value) -> Result<(), Response> {
    let Some(sub_id) = sub["id"].as_str() else {
        return Ok(());
    };
    let status = sub["status"].as_str().unwrap_or("canceled");
    let period_end = sub["current_period_end"]
        .as_i64()
        .and_then(|t| chrono::DateTime::from_timestamp(t, 0));
    let canceled_at = sub["canceled_at"]
        .as_i64()
        .and_then(|t| chrono::DateTime::from_timestamp(t, 0));
    state
        .subscriptions
        .sync_status(
            sub_id,
            sub["customer"].as_str(),
            status,
            period_end,
            canceled_at,
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "subscription sync failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        })?;
    Ok(())
}

// ---- Plus perk: daily-challenge revive ------------------------------------

pub async fn daily_revive(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Json(body): Json<Value>,
) -> Response {
    let game_type = body["game_type"]
        .as_str()
        .unwrap_or(crate::minigames::TYPE_QUIZ)
        .replace('-', "_");
    if !crate::minigames::VALID_TYPES.contains(&game_type.as_str()) {
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({"error": "Invalid game type."})),
        )
            .into_response();
    }
    if !is_plus(&state, user.id).await {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "plus_required", "message": "Blind'All Plus requis"})),
        )
            .into_response();
    }
    let today = chrono::Utc::now().date_naive();
    match state
        .subscriptions
        .consume_daily_revive(user.id, today, &game_type)
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({"error": "revive_used", "message": "Revive déjà utilisé aujourd'hui"})),
            )
                .into_response();
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
    let _ = state
        .minigames
        .reset_daily_progress(user.id, today, &game_type)
        .await;
    Json(json!({"ok": true})).into_response()
}

// ---- Plus perk: advanced stats --------------------------------------------

pub async fn stats_advanced(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Path(id): Path<i64>,
) -> Response {
    // Personal data: a user can only read their own advanced stats.
    if user.id != id {
        return StatusCode::FORBIDDEN.into_response();
    }
    if !is_plus(&state, user.id).await {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "plus_required", "message": "Blind'All Plus requis"})),
        )
            .into_response();
    }
    match state.minigames.advanced_stats(user.id).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => {
            tracing::warn!(error = %e, "advanced stats failed");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Public plans page data (no auth): whether each plan is on sale + pricing.
pub async fn plans_public(
    State(state): State<AppState>,
    OptionalUser(user): OptionalUser,
) -> Response {
    let mut out = json!({
        "plus_available": !state.billing.plus_payment_url.is_empty(),
        "pro_available": !state.billing.pro_payment_url.is_empty(),
    });
    if let Some(u) = &user {
        if !u.is_guest {
            let plans = entitlements(&state, u.id).await;
            out["plans"] = json!(plans);
            out["is_plus"] = json!(plans.iter().any(|p| p == PLAN_PLUS));
            out["is_pro"] = json!(plans.iter().any(|p| p == PLAN_PRO));
            if !state.billing.plus_payment_url.is_empty() {
                out["plus_payment_url"] = json!(subscription_url(
                    &state.billing.plus_payment_url,
                    u.id,
                    PLAN_PLUS
                ));
            }
            if !state.billing.pro_payment_url.is_empty() {
                out["pro_payment_url"] = json!(subscription_url(
                    &state.billing.pro_payment_url,
                    u.id,
                    PLAN_PRO
                ));
            }
        }
    }
    Json(out).into_response()
}
