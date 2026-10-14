//! Sanctum-compatible token lookup. Personal access tokens are stored as
//! `personal_access_tokens.token = sha256(plaintext)` where the client
//! sends `{id}|{plaintext}` — the same lookup Laravel does, so existing
//! tokens keep working.

use crate::entities::User;
use sqlx::postgres::PgPool;
use sqlx::Row;

#[derive(Clone)]
pub struct AuthRepo {
    pool: PgPool,
}

impl AuthRepo {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// `Authorization: Bearer {id}|{token}` → user. Mirrors
    /// `Sanctum::personalAccessTokenModel()::findToken` (expires_at +
    /// last_used_at update included).
    pub async fn user_for_token(&self, bearer: &str) -> Result<Option<User>, sqlx::Error> {
        let Some((id, plaintext)) = bearer.split_once('|') else {
            return Ok(None);
        };
        let Ok(_token_id) = id.parse::<i64>() else {
            return Ok(None);
        };
        let hash = format!("{:x}", sha2_digest(plaintext.as_bytes()));

        let row = sqlx::query(
            // NB: `pat.expires_at` is NOT in the schema (Sanctum's optional
            // expires_at migration is absent from this project — tokens live
            // forever until revoked, same as Laravel here).
            "SELECT u.id, u.name, u.photo_path, u.elo, u.is_guest, \
             u.is_administrator, u.team_id, pat.id AS pat_id \
             FROM personal_access_tokens pat \
             JOIN users u ON u.id = pat.tokenable_id \
             WHERE pat.id = $1 AND pat.token = $2 \
             AND pat.tokenable_type = 'App\\Models\\User' \
             LIMIT 1",
        )
        .bind(id.parse::<i64>().unwrap_or_default())
        .bind(&hash)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else { return Ok(None) };

        let pat_id: i64 = row.get("pat_id");
        let _ = sqlx::query("UPDATE personal_access_tokens SET last_used_at = $1 WHERE id = $2")
            .bind(chrono::Utc::now())
            .bind(pat_id)
            .execute(&self.pool)
            .await;

        Ok(Some(User {
            id: row.get("id"),
            name: row.get("name"),
            photo_path: row.get("photo_path"),
            elo: row.get("elo"),
            is_guest: row.get("is_guest"),
            is_administrator: row.get("is_administrator"),
            team_id: row.get("team_id"),
        }))
    }

    /// `User::isPublicModerator` — user moderates at least one public
    /// room (morph pivot `moderables` → rooms.is_public).
    pub async fn is_public_moderator(&self, user_id: i64) -> bool {
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM moderables m \
             JOIN rooms r ON r.id = m.moderable_id \
             WHERE m.user_id = $1 AND m.moderable_type = 'App\\Models\\Room' \
             AND r.is_public = true)::int::bigint",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
        .map(|v| v == 1)
        .unwrap_or(false)
    }

    /// Gate `sendSuggestion` inputs: `created_at` + total score across rooms.
    pub async fn created_at_and_total_score(
        &self,
        user_id: i64,
    ) -> Result<Option<(Option<chrono::DateTime<chrono::Utc>>, f64)>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT u.created_at, COALESCE(SUM(ts.score), 0) AS total \
             FROM users u LEFT JOIN total_scores ts \
             ON ts.totalscorable_type = 'App\\Models\\User' AND ts.totalscorable_id = u.id \
             WHERE u.id = $1 GROUP BY u.id",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| (r.get("created_at"), r.get("total"))))
    }

    /// `User::publicModerators()` — users moderating ≥1 public room.
    pub async fn public_moderator_ids(&self) -> Result<Vec<i64>, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT DISTINCT m.user_id FROM moderables m \
             JOIN rooms r ON r.id = m.moderable_id \
             WHERE m.moderable_type = 'App\\Models\\Room' AND r.is_public = true",
        )
        .fetch_all(&self.pool)
        .await
    }

    // ---- login / register / token issue -----------------------------------

    /// `LoginRequest::authenticate` — fetch by email for bcrypt check.
    /// Returns user + raw password hash + banned flag (PHP's
    /// `logout.banned` middleware blocks banned users afterwards).
    pub async fn find_for_login(
        &self,
        email: &str,
    ) -> Result<Option<(User, Option<String>, bool)>, sqlx::Error> {
        // `isBanned()` (mchev/banhammer): non-deleted ban row with
        // expired_at NULL or in the future. `users.banned_at` was dropped
        // by the 2023 migration — do not select it.
        let row = sqlx::query(
            "SELECT u.id, u.name, u.photo_path, u.elo, u.is_guest, u.is_administrator, \
             u.team_id, u.password, \
             EXISTS(SELECT 1 FROM bans b \
                    WHERE b.bannable_type = 'App\\Models\\User' \
                    AND b.bannable_id = u.id AND b.deleted_at IS NULL \
                    AND (b.expired_at IS NULL OR b.expired_at > NOW()))::int::bigint AS banned \
             FROM users u \
             WHERE u.email = $1 AND u.deleted_at IS NULL LIMIT 1",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|r| {
            (
                User {
                    id: r.get("id"),
                    name: r.get("name"),
                    photo_path: r.get("photo_path"),
                    elo: r.get("elo"),
                    is_guest: r.get("is_guest"),
                    is_administrator: r.get("is_administrator"),
                    team_id: r.get("team_id"),
                },
                r.get::<Option<String>, _>("password"),
                r.get::<i64, _>("banned") > 0,
            )
        }))
    }

    /// banhammer `ip.banned` middleware — an active (non-deleted, unexpired)
    /// ban row carrying this `ip` blocks the request. Applied in PHP to
    /// register, the home page and room show/public-state.
    pub async fn is_ip_banned(&self, ip: &str) -> bool {
        sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM bans WHERE ip = $1 AND deleted_at IS NULL \
             AND (expired_at IS NULL OR expired_at > NOW()))::int::bigint",
        )
        .bind(ip)
        .fetch_one(&self.pool)
        .await
        .map(|v| v == 1)
        .unwrap_or(false)
    }

    pub async fn name_taken(&self, name: &str) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query_scalar::<_, i64>("SELECT EXISTS(SELECT 1 FROM users WHERE name = $1)::int::bigint")
            .bind(name)
            .fetch_one(&self.pool)
            .await?
            == 1)
    }

    pub async fn email_taken(&self, email: &str) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query_scalar::<_, i64>("SELECT EXISTS(SELECT 1 FROM users WHERE email = $1)::int::bigint")
            .bind(email)
            .fetch_one(&self.pool)
            .await?
            == 1)
    }

    /// `User::filter(['search' => …])` — `Api\UserController::index`: name or
    /// email LIKE %s%, exact/prefix matches first, `paginate(20)` — used by
    /// the moderators pickers (`{id, name}` only, like `select('id','name')`).
    pub async fn search_users(
        &self,
        search: &str,
        page: i64,
    ) -> Result<(Vec<(i64, String)>, i64), sqlx::Error> {
        let like = format!("%{}%", search.replace('%', "\\%").replace('_', "\\_"));
        let prefix = format!("{}%", search.replace('%', "\\%").replace('_', "\\_"));
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM users WHERE deleted_at IS NULL AND (name LIKE $1 ESCAPE '\\' OR email LIKE $1 ESCAPE '\\')",
        )
        .bind(&like)
        .fetch_one(&self.pool)
        .await?;
        let rows = sqlx::query_as::<_, (i64, String)>(
            "SELECT id, name FROM users
             WHERE deleted_at IS NULL AND (name LIKE $1 ESCAPE '\\' OR email LIKE $1 ESCAPE '\\')
             ORDER BY CASE WHEN name = $2 THEN 1 WHEN name LIKE $3 ESCAPE '\\' THEN 2 ELSE 3 END, id
             LIMIT 20 OFFSET $4",
        )
        .bind(&like)
        .bind(search)
        .bind(&prefix)
        .bind((page - 1).max(0) * 20)
        .fetch_all(&self.pool)
        .await?;
        Ok((rows, total))
    }

    /// `Rule::unique('users')->ignore($user->id)` variants for profile updates.
    pub async fn name_taken_ignore(&self, name: &str, user_id: i64) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM users WHERE name = $1 AND id != $2)::int::bigint",
        )
        .bind(name)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?
            == 1)
    }

    pub async fn email_taken_ignore(&self, email: &str, user_id: i64) -> Result<bool, sqlx::Error> {
        Ok(sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM users WHERE email = $1 AND id != $2)::int::bigint",
        )
        .bind(email)
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?
            == 1)
    }

    /// `UserController::update` — `$user->update(['name' =>, 'email' =>])`.
    /// Changing email clears `email_verified_at` (Laravel does NOT — it keeps
    /// the column as-is; the request only fills name+email).
    pub async fn update_profile(
        &self,
        user_id: i64,
        name: &str,
        email: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET name = $1, email = $2, updated_at = $3 WHERE id = $4")
            .bind(name)
            .bind(email)
            .bind(chrono::Utc::now())
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Raw `password` hash — `hasPassword()` is `filled(password)`.
    pub async fn password_hash(&self, user_id: i64) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar("SELECT password FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map(|o| o.flatten())
    }

    /// `updatePassword` — `Hash::make` is done by the caller (bcrypt).
    pub async fn update_password(&self, user_id: i64, hash: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET password = $1, updated_at = $2 WHERE id = $3")
            .bind(hash)
            .bind(chrono::Utc::now())
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `updateDonationPreferences`.
    pub async fn update_donation_preferences(
        &self,
        user_id: i64,
        show: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE users SET show_donation_history_on_profile = $1, updated_at = $2 \
             WHERE id = $3",
        )
        .bind(show)
        .bind(chrono::Utc::now())
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// `users.name` lookup by id (team owner link, notification payloads…).
    pub async fn find_name(&self, user_id: i64) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar("SELECT name FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
    }

    /// `changeTeamPicture`/`changeRoomPicture` Gate — administrator, or
    /// account older than 3 months with a total score over 2000.
    pub async fn can_change_picture(&self, user_id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, bool>(
            "SELECT u.is_administrator OR (u.created_at < NOW() - INTERVAL '3 months' \
             AND COALESCE((SELECT SUM(ts.score) FROM total_scores ts \
              WHERE ts.totalscorable_type = 'App\\Models\\User' \
              AND ts.totalscorable_id = u.id), 0) > 2000) \
             FROM users u WHERE u.id = $1",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await
    }

    /// `photo_path` (for the storage layer to delete the file).
    pub async fn photo_path(&self, user_id: i64) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar("SELECT photo_path FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map(|o| o.flatten())
    }

    /// `deletePhoto` — clears `photo_path`; the file deletion is the caller's.
    pub async fn clear_photo(&self, user_id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET photo_path = NULL, updated_at = $1 WHERE id = $2")
            .bind(chrono::Utc::now())
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `updatePhoto` — `users/{uniqid}.webp`.
    pub async fn set_photo(&self, user_id: i64, photo_path: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET photo_path = $1, updated_at = $2 WHERE id = $3")
            .bind(photo_path)
            .bind(chrono::Utc::now())
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `User::hasPassword` — `filled($user->getRawOriginal('password'))`.
    pub async fn has_password(&self, user_id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, Option<String>>("SELECT password FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
            .map(|p| p.flatten().is_some_and(|p| !p.is_empty()))
    }

    /// `UserAccountDeletionService::delete` — photo file removal is the
    /// caller's concern; here: rooms (soft), playlists, scores,
    /// total_scores, then `forceDelete` (hard delete).
    pub async fn delete_account(&self, user_id: i64) -> Result<(), sqlx::Error> {
        let now = chrono::Utc::now();
        sqlx::query("UPDATE rooms SET deleted_at = $1, updated_at = $2 WHERE user_id = $3")
            .bind(now)
            .bind(now)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        for sql in [
            "DELETE FROM playlists WHERE user_id = $1",
            "DELETE FROM scores WHERE user_id = $1",
            // `totalScores()` morphMany — no user_id column on the table.
            "DELETE FROM total_scores WHERE totalscorable_type = 'App\\Models\\User' AND totalscorable_id = $1",
        ] {
            sqlx::query(sql).bind(user_id).execute(&self.pool).await?;
        }
        sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `Reserved` rule part 3 — moderator names of public rooms + mchev.
    pub async fn reserved_moderator_names(&self) -> Result<Vec<String>, sqlx::Error> {
        let rows = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT u.name FROM moderables m \
             JOIN rooms r ON r.id = m.moderable_id AND r.is_public = true \
             JOIN users u ON u.id = m.user_id \
             WHERE m.moderable_type = 'App\\Models\\Room'",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut names: Vec<String> = rows.into_iter().map(|n| n.to_lowercase()).collect();
        names.push("mchev".into());
        Ok(names)
    }

    // ---- oauth / social -----------------------------------------------------

    /// `User::where('provider_id', $id)` — find by social provider id.
    pub async fn find_by_provider_id(
        &self,
        provider: &str,
        provider_id: &str,
    ) -> Result<Option<User>, sqlx::Error> {
        sqlx::query(
            "SELECT id, name, photo_path, elo, is_guest, is_administrator, team_id \
             FROM users WHERE provider = $1 AND provider_id = $2 \
             AND deleted_at IS NULL LIMIT 1",
        )
        .bind(provider)
        .bind(provider_id)
        .fetch_optional(&self.pool)
        .await
        .map(|o| o.map(|r| User {
            id: r.get("id"),
            name: r.get("name"),
            photo_path: r.get("photo_path"),
            elo: r.get("elo"),
            is_guest: r.get("is_guest"),
            is_administrator: r.get("is_administrator"),
            team_id: r.get("team_id"),
        }))
    }

    /// Link an existing account (matched by email) to a provider —
    /// `$email->provider = ...; $email->provider_id = ...`.
    pub async fn link_provider(
        &self,
        user_id: i64,
        provider: &str,
        provider_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET provider = $1, provider_id = $2 WHERE id = $3")
            .bind(provider)
            .bind(provider_id)
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// `User::create` for social auth — no password.
    pub async fn create_social_user(
        &self,
        name: &str,
        email: &str,
        provider: &str,
        provider_id: &str,
    ) -> Result<i64, sqlx::Error> {
        let now = chrono::Utc::now();
        let res = sqlx::query_scalar::<_, i64>(
            "INSERT INTO users (name, email, provider, provider_id, \
             is_administrator, is_guest, elo, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, false, false, 1500, $5, $6) RETURNING id",
        )
        .bind(name)
        .bind(email)
        .bind(provider)
        .bind(provider_id)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        Ok(res)
    }

    /// `RegisteredUserController::store` — User::create with bcrypt hash.
    pub async fn create_user(
        &self,
        name: &str,
        email: &str,
        password_hash: &str,
        ip: Option<&str>,
    ) -> Result<i64, sqlx::Error> {
        let now = chrono::Utc::now();
        let res = sqlx::query_scalar::<_, i64>(
            "INSERT INTO users (name, email, password, ip, is_administrator, \
             is_guest, elo, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, false, false, 1500, $5, $6) RETURNING id",
        )
        .bind(name)
        .bind(email)
        .bind(password_hash)
        .bind(ip)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        Ok(res)
    }

    /// `GuestAuthService::ensureGuestSession` — guest account:
    /// `Guest-NNNNN`, `guest-xxxxxxxx@bam.test`, no password, `guest_token` uuid.
    pub async fn create_guest(
        &self,
        name: &str,
        email: &str,
        guest_token: &str,
        ip: Option<&str>,
    ) -> Result<i64, sqlx::Error> {
        let now = chrono::Utc::now();
        let res = sqlx::query_scalar::<_, i64>(
            "INSERT INTO users (name, email, password, ip, is_administrator, \
             is_guest, guest_token, elo, created_at, updated_at) \
             VALUES ($1, $2, NULL, $3, false, true, $4, 1500, $5, $6) RETURNING id",
        )
        .bind(name)
        .bind(email)
        .bind(ip)
        .bind(guest_token)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        Ok(res)
    }

    /// `AuthenticatedSessionController::store` — `$user->update(['ip'=>…])`.
    /// `User::where('email', …)->first()` — for password reset links.
    pub async fn find_by_email(&self, email: &str) -> Result<Option<(i64, String)>, sqlx::Error> {
        sqlx::query("SELECT id, name FROM users WHERE email = $1 LIMIT 1")
            .bind(email)
            .fetch_optional(&self.pool)
            .await
            .map(|o| o.map(|r| (r.get("id"), r.get("name"))))
    }

    /// `EmailVerificationRequest::fulfill` — set `email_verified_at` once.
    pub async fn mark_email_verified(&self, user_id: i64) -> Result<bool, sqlx::Error> {
        sqlx::query(
            "UPDATE users SET email_verified_at = $1 WHERE id = $2 AND email_verified_at IS NULL",
        )
        .bind(chrono::Utc::now())
        .bind(user_id)
        .execute(&self.pool)
        .await
        .map(|r| r.rows_affected() > 0)
    }

    /// User row for the verification hash (`sha1(email)`).
    pub async fn email_of(&self, user_id: i64) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar("SELECT email FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn update_ip(&self, user_id: i64, ip: &str) {
        let _ = sqlx::query("UPDATE users SET ip = $1 WHERE id = $2")
            .bind(ip)
            .bind(user_id)
            .execute(&self.pool)
            .await;
    }

    /// `createToken` — `{id}|{plain}` where token = sha256(plain).
    /// Sanctum plaintext is 40 random [a-zA-Z0-9] chars.
    pub async fn issue_token(&self, user_id: i64, name: &str) -> Result<String, sqlx::Error> {
        let plain: String = {
            use rand::Rng;
            const CH: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
            let mut rng = rand::rng();
            (0..40).map(|_| CH[rng.random_range(0..CH.len())] as char).collect()
        };
        let hash = format!("{:x}", sha2_digest(plain.as_bytes()));
        let now = chrono::Utc::now();
        let res = sqlx::query_scalar::<_, i64>(
            "INSERT INTO personal_access_tokens (tokenable_type, tokenable_id, name, \
             token, abilities, created_at, updated_at) \
             VALUES ('App\\Models\\User', $1, $2, $3, '[\"*\"]', $4, $5) RETURNING id",
        )
        .bind(user_id)
        .bind(name)
        .bind(&hash)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        Ok(format!("{}|{plain}", res))
    }

    /// `logout` — delete the token row used for this call.
    pub async fn revoke_token(&self, bearer: &str) -> Result<(), sqlx::Error> {
        if let Some((id, plain)) = bearer.split_once('|') {
            let hash = format!("{:x}", sha2_digest(plain.as_bytes()));
            let _ = sqlx::query("DELETE FROM personal_access_tokens WHERE id = $1 AND token = $2")
                .bind(id.parse::<i64>().unwrap_or_default())
                .bind(hash)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }
}

/// SHA-256 digest (Sanctum stores `hash('sha256', $token)` — the generic
/// array implements `LowerHex` for the `{:x}` format).
fn sha2_digest(data: &[u8]) -> impl core::fmt::LowerHex {
    use sha2::Digest;
    sha2::Sha256::digest(data)
}
