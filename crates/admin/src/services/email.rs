//! Email service for sending verification codes and notifications.
//!
//! Uses SMTP via lettre for delivery with Askama HTML templates.

use std::time::Instant;

use askama::Template;
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{MultiPart, SinglePart, header::ContentType},
    transport::smtp::{Error as SmtpError, authentication::Credentials},
};
use secrecy::ExposeSecret;
use thiserror::Error;
use tracing::{debug, error, info, instrument, warn};

use crate::config::EmailConfig;

/// HTML template for verification code email.
#[derive(Template)]
#[template(path = "email/verification_code.html")]
struct VerificationCodeEmailHtml<'a> {
    code: &'a str,
}

/// Plain text template for verification code email.
#[derive(Template)]
#[template(path = "email/verification_code.txt")]
struct VerificationCodeEmailText<'a> {
    code: &'a str,
}

/// HTML template for welcome email.
#[derive(Template)]
#[template(path = "email/welcome.html")]
struct WelcomeEmailHtml<'a> {
    name: &'a str,
    admin_url: &'a str,
}

/// Plain text template for welcome email.
#[derive(Template)]
#[template(path = "email/welcome.txt")]
struct WelcomeEmailText<'a> {
    name: &'a str,
    admin_url: &'a str,
}

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

    /// Template rendering error.
    #[error("Template error: {0}")]
    Template(#[from] askama::Error),
}

/// Email service for sending transactional emails.
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

    /// Send a verification code email for admin setup.
    ///
    /// # Errors
    ///
    /// Returns error if email fails to send or template fails to render.
    #[instrument(skip(self, code), fields(recipient = %to))]
    pub async fn send_verification_code(&self, to: &str, code: &str) -> Result<(), EmailError> {
        debug!("Preparing verification code email");

        let html = VerificationCodeEmailHtml { code }.render()?;
        let text = VerificationCodeEmailText { code }.render()?;

        debug!("Templates rendered, sending verification code email");

        self.send_multipart_email(
            to,
            "Your Naked Pineapple Admin Verification Code",
            &text,
            &html,
        )
        .await
    }

    /// Send a welcome email after successful registration.
    ///
    /// # Errors
    ///
    /// Returns error if email fails to send or template fails to render.
    #[instrument(skip(self), fields(recipient = %to, user_name = %name))]
    pub async fn send_welcome_email(&self, to: &str, name: &str) -> Result<(), EmailError> {
        debug!("Preparing welcome email");

        let admin_url = "https://admin.nakedpineapple.co";
        let html = WelcomeEmailHtml { name, admin_url }.render()?;
        let text = WelcomeEmailText { name, admin_url }.render()?;

        debug!("Templates rendered, sending welcome email");

        self.send_multipart_email(to, "Welcome to Naked Pineapple Admin", &text, &html)
            .await
    }

    /// Send a multipart email with both plain text and HTML versions.
    #[instrument(skip(self, text_body, html_body), fields(recipient = %to, subject = %subject))]
    async fn send_multipart_email(
        &self,
        to: &str,
        subject: &str,
        text_body: &str,
        html_body: &str,
    ) -> Result<(), EmailError> {
        debug!("Building email message");
        let start = Instant::now();

        let email = Message::builder()
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
}

/// Generate a 6-digit verification code.
#[must_use]
pub fn generate_verification_code() -> String {
    use rand::Rng;
    let code: u32 = rand::rng().random_range(100_000..1_000_000);
    code.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_verification_code_format() {
        let code = generate_verification_code();
        assert_eq!(code.len(), 6);
        assert!(code.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn test_generate_verification_code_range() {
        for _ in 0..100 {
            let code: u32 = generate_verification_code().parse().expect("valid number");
            assert!(code >= 100_000);
            assert!(code < 1_000_000);
        }
    }
}
