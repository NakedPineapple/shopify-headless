//! Email delivery service via SMTP.
//!
//! Provides a generic SMTP email sender. Application-specific templates
//! (verification codes, welcome emails, etc.) remain in the consuming crate.

use std::time::Instant;

use lettre::{
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
    message::{MultiPart, SinglePart, header::ContentType},
    transport::smtp::{Error as SmtpError, authentication::Credentials},
};
use secrecy::ExposeSecret;
use thiserror::Error;
use tracing::{debug, error, info, instrument, warn};

use crate::config::EmailConfig;

/// Errors that can occur when sending email.
#[derive(Debug, Error)]
pub enum EmailError {
    /// SMTP transport error.
    #[error("SMTP error: {0}")]
    Smtp(#[from] SmtpError),

    /// Failed to build email message.
    #[error("Failed to build message: {0}")]
    MessageBuild(#[from] lettre::error::Error),

    /// Invalid email address.
    #[error("Invalid email address: {0}")]
    InvalidAddress(String),
}

/// Email delivery service via SMTP.
#[derive(Clone)]
pub struct EmailService {
    mailer: AsyncSmtpTransport<Tokio1Executor>,
    from_address: String,
}

impl EmailService {
    /// Create a new email service from configuration.
    ///
    /// # Errors
    ///
    /// Returns error if SMTP connection fails.
    #[instrument(skip(config), fields(smtp_host = %config.smtp_host, smtp_port = config.smtp_port))]
    pub fn new(config: &EmailConfig) -> Result<Self, SmtpError> {
        debug!("Initializing email service");

        let credentials = Credentials::new(
            config.smtp_username.clone(),
            config.smtp_password.expose_secret().to_string(),
        );

        let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_host)?
            .port(config.smtp_port)
            .credentials(credentials)
            .build();

        info!(
            from_address = %config.from_address,
            "Email service initialized"
        );

        Ok(Self {
            mailer,
            from_address: config.from_address.clone(),
        })
    }

    /// Get the from address.
    #[must_use]
    pub fn from_address(&self) -> &str {
        &self.from_address
    }

    /// Send a multipart email with both plain text and HTML versions.
    ///
    /// # Errors
    ///
    /// Returns error if message building or sending fails.
    #[instrument(skip(self, text_body, html_body), fields(recipient = %to, subject = %subject))]
    pub async fn send_multipart_email(
        &self,
        to: &str,
        subject: &str,
        text_body: &str,
        html_body: &str,
    ) -> Result<(), EmailError> {
        debug!("Building email message");
        let start = Instant::now();

        let email = lettre::Message::builder()
            .from(self.from_address.parse().map_err(|_| {
                warn!(address = %self.from_address, "Invalid from address");
                EmailError::InvalidAddress(self.from_address.clone())
            })?)
            .to(to.parse().map_err(|_| {
                warn!(address = %to, "Invalid recipient address");
                EmailError::InvalidAddress(to.to_string())
            })?)
            .subject(subject)
            .multipart(
                MultiPart::alternative()
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_PLAIN)
                            .body(text_body.to_string()),
                    )
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::TEXT_HTML)
                            .body(html_body.to_string()),
                    ),
            )?;

        debug!("Sending email via SMTP");
        let send_start = Instant::now();

        match self.mailer.send(email).await {
            Ok(_) => {
                info!(
                    duration_ms = %start.elapsed().as_millis(),
                    smtp_duration_ms = %send_start.elapsed().as_millis(),
                    "Email sent successfully"
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    error = %e,
                    duration_ms = %start.elapsed().as_millis(),
                    "Failed to send email"
                );
                Err(e.into())
            }
        }
    }

    /// Send a plain text only email.
    ///
    /// # Errors
    ///
    /// Returns error if message building or sending fails.
    #[instrument(skip(self, body), fields(recipient = %to, subject = %subject))]
    pub async fn send_text_email(
        &self,
        to: &str,
        subject: &str,
        body: &str,
    ) -> Result<(), EmailError> {
        debug!("Building plain text email");
        let start = Instant::now();

        let email = lettre::Message::builder()
            .from(
                self.from_address
                    .parse()
                    .map_err(|_| EmailError::InvalidAddress(self.from_address.clone()))?,
            )
            .to(to
                .parse()
                .map_err(|_| EmailError::InvalidAddress(to.to_string()))?)
            .subject(subject)
            .body(body.to_string())?;

        match self.mailer.send(email).await {
            Ok(_) => {
                info!(
                    duration_ms = %start.elapsed().as_millis(),
                    "Plain text email sent successfully"
                );
                Ok(())
            }
            Err(e) => {
                error!(error = %e, "Failed to send plain text email");
                Err(e.into())
            }
        }
    }
}
