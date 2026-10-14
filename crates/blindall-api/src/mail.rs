//! Transactional mail via Brevo HTTP API — replaces the `brevo+api`
//! Symfony mailer transport (`MailServiceProvider`).

use serde_json::json;

#[derive(Clone)]
pub struct MailConfig {
    pub api_key: String,
    pub from_email: String,
    pub from_name: String,
    /// `APP_URL` — base for reset/verification links in emails.
    pub app_url: String,
}

impl MailConfig {
    pub fn configured(&self) -> bool {
        !self.api_key.is_empty()
    }
}

/// `POST https://api.brevo.com/v3/smtp/email` — best-effort; logs and
/// drops failures like a queue worker swallow-and-retry would.
pub async fn send(
    http: &reqwest::Client,
    cfg: &MailConfig,
    to_email: &str,
    to_name: &str,
    subject: &str,
    html: &str,
) -> anyhow::Result<()> {
    send_opts(http, cfg, to_email, to_name, subject, html, None).await
}

/// `send` + a `replyTo` address (`ContactMessage` sets it to the visitor's
/// email so the mailbox reply goes straight back to them).
pub async fn send_opts(
    http: &reqwest::Client,
    cfg: &MailConfig,
    to_email: &str,
    to_name: &str,
    subject: &str,
    html: &str,
    reply_to: Option<&str>,
) -> anyhow::Result<()> {
    if !cfg.configured() {
        tracing::warn!(to = to_email, subject, "mail not configured — email skipped");
        return Ok(());
    }
    let mut body = json!({
        "sender": { "email": cfg.from_email, "name": cfg.from_name },
        "to": [{ "email": to_email, "name": to_name }],
        "subject": subject,
        "htmlContent": html,
    });
    if let Some(rt) = reply_to {
        body["replyTo"] = json!({ "email": rt });
    }
    let resp = http
        .post("https://api.brevo.com/v3/smtp/email")
        .header("api-key", &cfg.api_key)
        .json(&body)
        .send()
        .await?;
    if !resp.status().is_success() {
        anyhow::bail!("brevo send failed: {}", resp.status());
    }
    Ok(())
}

/// `ProcessUserCreated` → `Welcome` notification mail. FR copy verbatim from
/// `lang/fr/messages.php` (PHP renders in the request locale; `fr` is the app
/// default and the only mail copy that exists).
pub async fn send_welcome(
    http: &reqwest::Client,
    cfg: &MailConfig,
    to_email: &str,
    to_name: &str,
) -> anyhow::Result<()> {
    let html = format!(
        "<p>Bienvenue {name}</p>\
         <p>Si tu reçois cet e-mail, cela signifie que tu viens de créer un compte sur blindallmusic.com.</p>\
         <p>Merci d'avoir rejoins la plus grande communauté de blind testeur et testeuse en ligne. \
         Tu peux maintenant créer tes propres playlists, des parties privées et rejoindre ou créer une team.</p>\
         <p><a href=\"{url}\">Jouer sur Blind'All Music</a></p>\
         <p>Martin de Blind'All Music</p>",
        name = to_name,
        url = cfg.app_url.trim_end_matches('/'),
    );
    send(http, cfg, to_email, to_name, "Bienvenue sur Blind'All Music", &html).await
}

/// `BrevoService::contacts()->create` — `POST /v3/contacts` with `listIds: [2]`.
/// Best-effort like the PHP `try/catch` around the SDK call.
pub async fn create_contact(
    http: &reqwest::Client,
    cfg: &MailConfig,
    email: &str,
    name: &str,
    provider: Option<&str>,
) -> anyhow::Result<()> {
    if !cfg.configured() {
        return Ok(());
    }
    let resp = http
        .post("https://api.brevo.com/v3/contacts")
        .header("api-key", &cfg.api_key)
        .header("accept", "application/json")
        .json(&json!({
            "email": email,
            "updateEnabled": true,
            "attributes": {
                "NOM": name,
                "INSCRIPTION": chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%:z").to_string(),
                "PROVIDER": provider,
            },
            "listIds": [2],
        }))
        .send()
        .await?;
    // 400 "Contact already exist" is fine — `updateEnabled` should cover it
    // but Brevo still rejects some dupes; swallow like PHP's try/catch.
    if !resp.status().is_success() {
        anyhow::bail!("brevo contact create failed: {}", resp.status());
    }
    Ok(())
}

/// `BrevoService::contacts()->delete` — `DELETE /v3/contacts/{email}`.
pub async fn delete_contact(
    http: &reqwest::Client,
    cfg: &MailConfig,
    email: &str,
) -> anyhow::Result<()> {
    if !cfg.configured() {
        return Ok(());
    }
    let _ = http
        .delete(format!(
            "https://api.brevo.com/v3/contacts/{}",
            urlencoding(email)
        ))
        .header("api-key", &cfg.api_key)
        .send()
        .await?;
    Ok(())
}

fn urlencoding(s: &str) -> String {
    s.replace('@', "%40").replace('+', "%2B")
}
