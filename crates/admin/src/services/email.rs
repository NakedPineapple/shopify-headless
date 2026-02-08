//! Email service for sending verification codes and notifications.
//!
//! Re-exports `EmailService` from `naked-pineapple-services` and extends it
//! with admin-specific template methods via the `AdminEmailExt` trait.

use askama::Template;
use thiserror::Error;
use tracing::debug;

pub use naked_pineapple_services::email::EmailService;

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

/// Errors that can occur when sending admin emails.
#[derive(Debug, Error)]
pub enum EmailError {
    /// Email delivery error (SMTP, address, message build).
    #[error(transparent)]
    Send(#[from] naked_pineapple_services::email::EmailError),

    /// Template rendering error.
    #[error("Template error: {0}")]
    Template(#[from] askama::Error),
}

/// Extension trait adding admin-specific email methods to `EmailService`.
pub trait AdminEmailExt {
    /// Send a verification code email for admin setup.
    fn send_verification_code(
        &self,
        to: &str,
        code: &str,
    ) -> impl std::future::Future<Output = Result<(), EmailError>> + Send;

    /// Send a welcome email after successful registration.
    fn send_welcome_email(
        &self,
        to: &str,
        name: &str,
    ) -> impl std::future::Future<Output = Result<(), EmailError>> + Send;
}

impl AdminEmailExt for EmailService {
    async fn send_verification_code(&self, to: &str, code: &str) -> Result<(), EmailError> {
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
        .await?;
        Ok(())
    }

    async fn send_welcome_email(&self, to: &str, name: &str) -> Result<(), EmailError> {
        debug!("Preparing welcome email");

        let admin_url = "https://admin.nakedpineapple.co";
        let html = WelcomeEmailHtml { name, admin_url }.render()?;
        let text = WelcomeEmailText { name, admin_url }.render()?;

        debug!("Templates rendered, sending welcome email");

        self.send_multipart_email(to, "Welcome to Naked Pineapple Admin", &text, &html)
            .await?;
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
