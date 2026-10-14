//! `PasswordBroker` / `DatabaseTokenRepository` port.
//!
//! `password_reset_tokens(email, token, created_at)` — token stored as
//! `hash_hmac('sha256', $token, config('app.key'))`, expiry 60 min
//! (Laravel `auth.passwords.users.expire` default).

use hmac::{Hmac, Mac};
use sha2::Sha256;
use sqlx::postgres::PgPool;
use sqlx::Row;

const EXPIRE_MINUTES: i64 = 60;

#[derive(Clone)]
pub struct PasswordResetRepo {
    pool: PgPool,
    /// `APP_KEY` — signs reset tokens (HMAC) like Laravel's hashKey.
    app_key: String,
}

impl PasswordResetRepo {
    pub fn new(pool: PgPool, app_key: String) -> Self {
        Self { pool, app_key }
    }

    /// `createToken` — deletes prior tokens for the email, stores the
    /// HMAC'd token, returns the plaintext token for the reset URL.
    pub async fn create_token(&self, email: &str) -> Result<String, sqlx::Error> {
        let token = random_token(40);
        let hashed = hmac_sha256_hex(&self.app_key, &token);
        sqlx::query("DELETE FROM password_reset_tokens WHERE email = $1")
            .bind(email)
            .execute(&self.pool)
            .await?;
        sqlx::query(
            "INSERT INTO password_reset_tokens (email, token, created_at) VALUES ($1, $2, $3)",
        )
        .bind(email)
        .bind(hashed)
        .bind(chrono::Utc::now())
        .execute(&self.pool)
        .await?;
        Ok(token)
    }

    /// `exists` — token HMAC matches and created_at within 60 min.
    pub async fn token_valid(&self, email: &str, token: &str) -> Result<bool, sqlx::Error> {
        let row = sqlx::query(
            "SELECT token, created_at FROM password_reset_tokens WHERE email = $1",
        )
        .bind(email)
        .fetch_optional(&self.pool)
        .await?;
        let Some(row) = row else { return Ok(false) };
        let stored: String = row.get("token");
        if stored != hmac_sha256_hex(&self.app_key, token) {
            return Ok(false);
        }
        let created: chrono::DateTime<chrono::Utc> = row.get("created_at");
        Ok(chrono::Utc::now() - created < chrono::Duration::minutes(EXPIRE_MINUTES))
    }

    /// `Password::reset` — new password + fresh remember_token, then
    /// `deleteToken` for the email.
    pub async fn reset(&self, email: &str, password_hash: &str) -> Result<bool, sqlx::Error> {
        let res = sqlx::query(
            "UPDATE users SET password = $1, remember_token = $2 WHERE email = $3",
        )
        .bind(password_hash)
        .bind(random_token(60))
        .bind(email)
        .execute(&self.pool)
        .await?;
        if res.rows_affected() == 0 {
            return Ok(false);
        }
        sqlx::query("DELETE FROM password_reset_tokens WHERE email = $1")
            .bind(email)
            .execute(&self.pool)
            .await?;
        Ok(true)
    }
}

/// `Str::random` equivalent — cryptographically secure url-safe chars.
pub fn random_token(len: usize) -> String {
    use rand::Rng;
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::rng();
    (0..len)
        .map(|_| ALPHABET[rng.random_range(0..ALPHABET.len())] as char)
        .collect()
}

pub fn hmac_sha256_hex(key: &str, msg: &str) -> String {
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(key.as_bytes())
        .expect("hmac accepts any key length");
    mac.update(msg.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}
