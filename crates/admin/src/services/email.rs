//! Email service for sending verification codes and notifications.
//!
//! Uses SMTP via lettre for delivery.

use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::header::ContentType,
    transport::smtp::{authentication::Credentials, Error as SmtpError},
};
use secrecy::ExposeSecret;
use thiserror::Error;

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
    pub fn new(config: &EmailConfig) -> Result<Self, SmtpError> {
        let credentials = Credentials::new(
            config.smtp_username.clone(),
            config.smtp_password.expose_secret().to_string(),
        );

        let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&config.smtp_host)?
            .port(config.smtp_port)
            .credentials(credentials)
            .build();

        Ok(Self {
            mailer,
            from_address: config.from_address.clone(),
        })
    }

    /// Send a verification code email for admin setup.
    ///
    /// # Errors
    ///
    /// Returns error if email fails to send.
    pub async fn send_verification_code(&self, to: &str, code: &str) -> Result<(), EmailError> {
        let subject = "Your Naked Pineapple Admin Verification Code";
        let body = format!(
            r#"Welcome to Naked Pineapple Admin!

Your verification code is: {code}

This code expires in 10 minutes.

Enter this code on the setup page to verify your email and create your passkey.

If you didn't request this code, you can safely ignore this email.

— The Naked Pineapple Team"#
        );

        self.send_email(to, subject, &body).await
    }

    /// Send a welcome email after successful registration.
    ///
    /// # Errors
    ///
    /// Returns error if email fails to send.
    pub async fn send_welcome_email(&self, to: &str, name: &str) -> Result<(), EmailError> {
        let subject = "Welcome to Naked Pineapple Admin";
        let body = format!(
            r#"Hi {name}!

Your admin account has been set up successfully. You can now log in using your passkey.

Admin Panel: https://admin.nakedpineapple.co

Your passkey is the only way to access your account — keep your device secure!

If you have any questions, reach out to the team.

— The Naked Pineapple Team"#
        );

        self.send_email(to, subject, &body).await
    }

    /// Send a generic email.
    async fn send_email(&self, to: &str, subject: &str, body: &str) -> Result<(), EmailError> {
        let email = Message::builder()
            .from(
                self.from_address
                    .parse()
                    .map_err(|_| EmailError::InvalidAddress(self.from_address.clone()))?,
            )
            .to(to
                .parse()
                .map_err(|_| EmailError::InvalidAddress(to.to_string()))?)
            .subject(subject)
            .header(ContentType::TEXT_PLAIN)
            .body(body.to_string())?;

        self.mailer.send(email).await?;

        tracing::info!(to = %to, subject = %subject, "Email sent successfully");
        Ok(())
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
