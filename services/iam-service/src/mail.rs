//! Outbound SMTP sender for self-service mails (password reset / invitation).
//!
//! Empty `smtp.host` means mail is intentionally not configured. Callers must
//! fail closed instead of pretending the mail was delivered.

use crate::config::Config;
use lettre::message::{header::ContentType, Mailbox};
use lettre::transport::smtp::authentication::Credentials;
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor};

#[derive(Debug)]
pub enum MailError {
    NotConfigured,
    Delivery(String),
}

pub struct Mailer {
    transport: Option<AsyncSmtpTransport<Tokio1Executor>>,
    from: Mailbox,
}

impl Mailer {
    pub fn from_config(config: &Config) -> Self {
        let from = config
            .smtp
            .from
            .parse::<Mailbox>()
            .unwrap_or_else(|_| default_from());
        let host = config.smtp.host.trim();
        if host.is_empty() {
            return Self {
                transport: None,
                from,
            };
        }

        let mut builder =
            AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(host).port(config.smtp.port);

        if config.smtp.starttls {
            match TlsParameters::new(host.to_string()) {
                Ok(params) => builder = builder.tls(Tls::Required(params)),
                Err(e) => {
                    tracing::error!(error = %e, host, "invalid SMTP TLS parameters");
                    return Self {
                        transport: None,
                        from,
                    };
                }
            }
        }

        if !config.smtp.username.is_empty() {
            builder = builder.credentials(Credentials::new(
                config.smtp.username.clone(),
                config.smtp.password.clone(),
            ));
        }

        Self {
            transport: Some(builder.build()),
            from,
        }
    }

    pub fn is_configured(&self) -> bool {
        self.transport.is_some()
    }

    pub async fn send_html(&self, to: &str, subject: &str, html: String) -> Result<(), MailError> {
        let transport = self.transport.as_ref().ok_or(MailError::NotConfigured)?;
        let to = to
            .parse::<Mailbox>()
            .map_err(|e| MailError::Delivery(format!("invalid recipient {to}: {e}")))?;
        let email = Message::builder()
            .from(self.from.clone())
            .to(to)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(html)
            .map_err(|e| MailError::Delivery(e.to_string()))?;
        transport
            .send(email)
            .await
            .map_err(|e| MailError::Delivery(e.to_string()))?;
        Ok(())
    }
}

fn default_from() -> Mailbox {
    "no-reply@ai-pm.local"
        .parse()
        .expect("default mail from address must be valid")
}
