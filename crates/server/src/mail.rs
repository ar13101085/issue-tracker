//! Outgoing email: a queue drained by one background task, so a slow or broken
//! mail server never holds up a request.

use crate::config::Config;
use lettre::message::{Mailbox, MultiPart};
use lettre::transport::smtp::authentication::Credentials;
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

pub struct Mail {
    pub to: String,
    pub subject: String,
    pub html: String,
    pub text: String,
}

/// Starts the sender task when SMTP is configured.
pub fn start(cfg: &Config) -> Result<Option<UnboundedSender<Mail>>, String> {
    let Some(smtp) = cfg.smtp.clone() else {
        return Ok(None);
    };
    let from: Mailbox = smtp.from.parse().map_err(|e| format!("SMTP_FROM is not a valid address: {e}"))?;
    let builder = match smtp.security.as_str() {
        "tls" => AsyncSmtpTransport::<Tokio1Executor>::relay(&smtp.host),
        "none" => Ok(AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&smtp.host)),
        _ => AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&smtp.host),
    }
    .map_err(|e| format!("SMTP_HOST: {e}"))?;
    let mut builder = builder.port(smtp.port);
    if let (Some(user), Some(pass)) = (smtp.username, smtp.password) {
        builder = builder.credentials(Credentials::new(user, pass));
    }
    let transport = builder.build();

    let (tx, mut rx) = unbounded_channel::<Mail>();
    tokio::spawn(async move {
        while let Some(mail) = rx.recv().await {
            let to: Mailbox = match mail.to.parse() {
                Ok(to) => to,
                Err(e) => {
                    tracing::warn!("skipping mail to an invalid address: {e}");
                    continue;
                }
            };
            let message = Message::builder()
                .from(from.clone())
                .to(to)
                .subject(mail.subject)
                .multipart(MultiPart::alternative_plain_html(mail.text, mail.html));
            match message {
                Ok(m) => {
                    if let Err(e) = transport.send(m).await {
                        tracing::warn!("mail was not sent: {e}");
                    }
                }
                Err(e) => tracing::warn!("mail could not be built: {e}"),
            }
        }
    });
    Ok(Some(tx))
}
