//! Newsletter subscription route handlers.
//!
//! Handles email newsletter subscriptions via the Storefront API.
//! Creates a new customer with marketing consent, or handles existing
//! subscribers gracefully.

use askama::Template;
use askama_web::WebTemplate;
use axum::{Form, extract::State, response::IntoResponse};
use serde::Deserialize;
use tracing::instrument;

use crate::state::AppState;

/// Newsletter subscription form data.
#[derive(Debug, Deserialize)]
pub struct SubscribeForm {
    pub email: String,
}

/// Success fragment template (replaces the form via HTMX).
#[derive(Template, WebTemplate)]
#[template(path = "newsletter/subscribe_success.html")]
pub struct SubscribeSuccessTemplate {
    pub email: String,
}

/// Error fragment template (replaces the form via HTMX).
#[derive(Template, WebTemplate)]
#[template(path = "newsletter/subscribe_error.html")]
pub struct SubscribeErrorTemplate {
    pub message: String,
    pub email: String,
}

/// Subscribe to newsletter (HTMX).
///
/// Creates a new Shopify customer with `acceptsMarketing: true`.
/// If the email already exists, shows a success message (they're already
/// in the system and can manage preferences via their account).
#[instrument(skip(state), fields(email = %form.email))]
pub async fn subscribe(
    State(state): State<AppState>,
    Form(form): Form<SubscribeForm>,
) -> impl IntoResponse {
    let email = form.email.trim().to_lowercase();

    // Basic email validation
    if !is_valid_email(&email) {
        return SubscribeErrorTemplate {
            message: "Please enter a valid email address.".to_string(),
            email,
        }
        .into_response();
    }

    // Create a new customer with marketing consent
    // Using a random password since they won't use it (newsletter-only subscription)
    let password = generate_random_password();

    match state
        .storefront()
        .create_customer(&email, &password, None, None, true)
        .await
    {
        Ok(_customer) => {
            tracing::info!(email = %email, "Newsletter subscription successful");
            SubscribeSuccessTemplate {
                email: email.clone(),
            }
            .into_response()
        }
        Err(e) => {
            let error_message = e.to_string().to_lowercase();

            // Check if the error is because the email already exists
            // Shopify returns "Email has already been taken" for duplicates
            if error_message.contains("already been taken")
                || error_message.contains("already exists")
            {
                // Treat as success - they're already in the system
                tracing::info!(email = %email, "Email already exists - treating as success");
                SubscribeSuccessTemplate {
                    email: email.clone(),
                }
                .into_response()
            } else {
                tracing::warn!(email = %email, error = %e, "Newsletter subscription failed");
                SubscribeErrorTemplate {
                    message: "Something went wrong. Please try again.".to_string(),
                    email,
                }
                .into_response()
            }
        }
    }
}

/// Basic email validation.
fn is_valid_email(email: &str) -> bool {
    // Simple validation: contains @, has content before and after @
    let mut parts = email.splitn(2, '@');
    let Some(local) = parts.next() else {
        return false;
    };
    let Some(domain) = parts.next() else {
        return false;
    };
    !local.is_empty() && !domain.is_empty() && domain.contains('.')
}

/// Generate a random password for newsletter-only subscriptions.
///
/// These customers won't use the password (they'll need to use password reset
/// if they ever want to log in), but Shopify requires one for customer creation.
fn generate_random_password() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    // Generate a pseudo-random password that meets Shopify's requirements
    // (at least 5 characters)
    format!("NP{timestamp:x}!Glow")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_email() {
        assert!(is_valid_email("test@example.com"));
        assert!(is_valid_email("user.name@domain.co.uk"));
        assert!(is_valid_email("a@b.c"));

        assert!(!is_valid_email(""));
        assert!(!is_valid_email("@"));
        assert!(!is_valid_email("test@"));
        assert!(!is_valid_email("@example.com"));
        assert!(!is_valid_email("test@domain")); // no TLD
        assert!(!is_valid_email("test"));
    }

    #[test]
    fn test_generate_random_password() {
        let p1 = generate_random_password();
        let _p2 = generate_random_password();

        // Should be different each time (different timestamps)
        // Note: In very fast execution, they might be the same
        assert!(p1.starts_with("NP"));
        assert!(p1.ends_with("!Glow"));
        assert!(p1.len() >= 10);

        // Wait a tiny bit and verify they're different
        std::thread::sleep(std::time::Duration::from_nanos(1));
        let p3 = generate_random_password();
        // Even if p1 == p2 due to timing, p3 should be different
        assert_ne!(p1, p3);
    }
}
