use lettre::message::{Mailbox, SinglePart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

#[derive(Clone)]
pub struct MailConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    /// "tls" (implicit, port 465) | "starttls" (port 587) | "none" (plain, dev relays)
    pub encryption: String,
    pub from_email: String,
    pub from_name: String,
    pub app_url: String,
}

impl MailConfig {
    pub fn configured(&self) -> bool {
        !self.host.is_empty()
    }

    fn transport(&self) -> anyhow::Result<AsyncSmtpTransport<Tokio1Executor>> {
        let enc = self.encryption.to_ascii_lowercase();
        let mut builder = match enc.as_str() {
            "tls" | "ssl" => AsyncSmtpTransport::<Tokio1Executor>::relay(&self.host)?,
            "none" | "plain" => AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&self.host),
            _ => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&self.host)?,
        }
        .port(self.port);
        if !self.username.is_empty() {
            builder = builder.credentials(Credentials::new(
                self.username.clone(),
                self.password.clone(),
            ));
        }
        Ok(builder.build())
    }
}

pub async fn send(
    cfg: &MailConfig,
    to_email: &str,
    to_name: &str,
    subject: &str,
    html: &str,
) -> anyhow::Result<()> {
    send_opts(cfg, to_email, to_name, subject, html, None).await
}

pub async fn send_opts(
    cfg: &MailConfig,
    to_email: &str,
    to_name: &str,
    subject: &str,
    html: &str,
    reply_to: Option<&str>,
) -> anyhow::Result<()> {
    if !cfg.configured() {
        tracing::warn!(
            to = to_email,
            subject,
            "mail not configured : email skipped"
        );
        return Ok(());
    }
    let from = Mailbox::new(
        (!cfg.from_name.is_empty()).then(|| cfg.from_name.clone()),
        cfg.from_email.parse()?,
    );
    let to = Mailbox::new(
        (!to_name.is_empty()).then(|| to_name.to_string()),
        to_email.parse()?,
    );
    let mut builder = Message::builder().from(from).to(to).subject(subject);
    if let Some(rt) = reply_to.and_then(|r| r.parse().ok()) {
        builder = builder.reply_to(Mailbox::new(None, rt));
    }
    let msg = builder.singlepart(SinglePart::html(html.to_string()))?;
    cfg.transport()?.send(msg).await?;
    Ok(())
}

pub async fn send_welcome(cfg: &MailConfig, to_email: &str, to_name: &str) -> anyhow::Result<()> {
    let html = format!(
        "<p>Bienvenue {name}</p>\
         <p>Si tu reçois cet e-mail, cela signifie que tu viens de créer un compte sur Blind'All Music.</p>\
         <p>Merci d'avoir rejoins la plus grande communauté de blind testeur et testeuse en ligne. \
         Tu peux maintenant créer tes propres playlists, des parties privées et rejoindre ou créer une team.</p>\
         <p><a href=\"{url}\">Jouer sur Blind'All Music</a></p>\
         <p>Martin de Blind'All Music</p>",
        name = to_name,
        url = cfg.app_url.trim_end_matches('/'),
    );
    send(
        cfg,
        to_email,
        to_name,
        "Bienvenue sur Blind'All Music",
        &html,
    )
    .await
}
