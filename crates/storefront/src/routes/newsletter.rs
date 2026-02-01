//! Newsletter subscription route handlers.
//!
//! Handles email newsletter subscriptions via the Storefront API.
//! Creates a new customer with marketing consent, or handles existing
//! subscribers gracefully. Also provides unsubscribe functionality via Klaviyo.

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    Form,
    extract::{Query, State},
    http::HeaderMap,
    response::{Html, IntoResponse},
};
use serde::Deserialize;
use tracing::{debug, error, info, instrument, warn};

use crate::filters;
use crate::services::KlaviyoClient;
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

/// Blog-specific success fragment template.
#[derive(Template, WebTemplate)]
#[template(path = "newsletter/blog_subscribe_success.html")]
pub struct BlogSubscribeSuccessTemplate {
    pub email: String,
}

/// Blog-specific error fragment template.
#[derive(Template, WebTemplate)]
#[template(path = "newsletter/blog_subscribe_error.html")]
pub struct BlogSubscribeErrorTemplate {
    pub message: String,
    pub email: String,
}

/// Unsubscribe page template.
#[derive(Template, WebTemplate)]
#[template(path = "newsletter/unsubscribe.html")]
pub struct UnsubscribeTemplate {
    pub email: String,
    pub unsubscribe_email: bool,
    pub unsubscribe_sms: bool,
    pub nonce: String,
}

/// Unsubscribe success template.
#[derive(Template, WebTemplate)]
#[template(path = "newsletter/unsubscribe_success.html")]
pub struct UnsubscribeSuccessTemplate {
    pub email: String,
    pub nonce: String,
}

/// Unsubscribe error template.
#[derive(Template, WebTemplate)]
#[template(path = "newsletter/unsubscribe_error.html")]
pub struct UnsubscribeErrorTemplate {
    pub message: String,
    pub email: String,
    pub nonce: String,
}

/// Query parameters for unsubscribe page.
#[derive(Debug, Deserialize)]
pub struct UnsubscribeQuery {
    pub email: Option<String>,
}

/// Form data for unsubscribe submission.
#[derive(Debug, Deserialize)]
pub struct UnsubscribeForm {
    pub email: String,
    #[serde(default)]
    pub unsubscribe_email: bool,
    #[serde(default)]
    pub unsubscribe_sms: bool,
}

/// Subscribe to newsletter (HTMX).
///
/// Creates a new Shopify customer with `acceptsMarketing: true` and
/// subscribes them to the Klaviyo email list for direct email marketing.
/// If the email already exists, shows a success message (they're already
/// in the system and can manage preferences via their account).
#[instrument(skip(state, headers), fields(email = %form.email))]
pub async fn subscribe(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<SubscribeForm>,
) -> impl IntoResponse {
    debug!("Processing newsletter subscription request");
    let email = form.email.trim().to_lowercase();

    // Check if request came from the blog (uses different templates)
    let from_blog = headers
        .get("HX-Current-URL")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|url| url.contains("/blog"));

    // Basic email validation
    if !is_valid_email(&email) {
        warn!(email = %email, "Invalid email format provided");
        return if from_blog {
            BlogSubscribeErrorTemplate {
                message: "Please enter a valid email address.".to_string(),
                email,
            }
            .into_response()
        } else {
            SubscribeErrorTemplate {
                message: "Please enter a valid email address.".to_string(),
                email,
            }
            .into_response()
        };
    }

    debug!(email = %email, "Email validation passed");

    // Subscribe to Klaviyo list (primary email marketing platform)
    if let Some(klaviyo_config) = state.config().klaviyo.as_ref() {
        debug!(email = %email, "Attempting Klaviyo subscription");
        match KlaviyoClient::new(klaviyo_config) {
            Ok(client) => {
                if let Err(e) = client.subscribe_email(&email).await {
                    warn!(email = %email, error = %e, "Klaviyo subscription failed");
                    // Continue anyway - Shopify sync will eventually add them
                } else {
                    info!(email = %email, "Klaviyo subscription successful");
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to create Klaviyo client");
            }
        }
    } else {
        debug!("Klaviyo not configured, skipping Klaviyo subscription");
    }

    // Create a new Shopify customer with marketing consent
    // Using a random password since they won't use it (newsletter-only subscription)
    let password = generate_random_password();
    debug!(email = %email, "Creating Shopify customer with marketing consent");

    match state
        .storefront()
        .create_customer(&email, &password, None, None, true)
        .await
    {
        Ok(_customer) => {
            info!(email = %email, "Shopify customer created with marketing consent");
            if from_blog {
                BlogSubscribeSuccessTemplate {
                    email: email.clone(),
                }
                .into_response()
            } else {
                SubscribeSuccessTemplate {
                    email: email.clone(),
                }
                .into_response()
            }
        }
        Err(e) => {
            let error_message = e.to_string().to_lowercase();

            // Check if the error is because the email already exists
            // Shopify returns "Email has already been taken" for duplicates
            if error_message.contains("already been taken")
                || error_message.contains("already exists")
            {
                // Treat as success - they're already in the system
                info!(email = %email, "Email already exists in Shopify - treating as success");
                if from_blog {
                    BlogSubscribeSuccessTemplate {
                        email: email.clone(),
                    }
                    .into_response()
                } else {
                    SubscribeSuccessTemplate {
                        email: email.clone(),
                    }
                    .into_response()
                }
            } else {
                warn!(email = %email, error = %e, "Shopify customer creation failed");
                if from_blog {
                    BlogSubscribeErrorTemplate {
                        message: "Something went wrong. Please try again.".to_string(),
                        email,
                    }
                    .into_response()
                } else {
                    SubscribeErrorTemplate {
                        message: "Something went wrong. Please try again.".to_string(),
                        email,
                    }
                    .into_response()
                }
            }
        }
    }
}

/// Show unsubscribe page.
///
/// GET /newsletter/unsubscribe?email=xxx
#[instrument(skip(state, nonce))]
pub async fn unsubscribe_page(
    State(state): State<AppState>,
    Query(query): Query<UnsubscribeQuery>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> impl IntoResponse {
    debug!("Rendering unsubscribe page");
    let email = query.email.unwrap_or_default();
    debug!(email = %email, "Unsubscribe page requested");

    // Check if Klaviyo is configured
    if state.config().klaviyo.is_none() {
        warn!("Klaviyo not configured, unsubscribe service unavailable");
        return Html(
            UnsubscribeErrorTemplate {
                message: "Unsubscribe service is not available. Please contact support."
                    .to_string(),
                email,
                nonce,
            }
            .render()
            .unwrap_or_else(|_| "Error".to_string()),
        )
        .into_response();
    }

    debug!(email = %email, "Rendering unsubscribe form");
    Html(
        UnsubscribeTemplate {
            email,
            unsubscribe_email: true,
            unsubscribe_sms: false,
            nonce,
        }
        .render()
        .unwrap_or_else(|_| "Error".to_string()),
    )
    .into_response()
}

/// Process unsubscribe request.
///
/// POST /newsletter/unsubscribe
#[instrument(skip(state, nonce), fields(email = %form.email))]
pub async fn unsubscribe(
    State(state): State<AppState>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    Form(form): Form<UnsubscribeForm>,
) -> impl IntoResponse {
    debug!("Processing unsubscribe request");
    let email = form.email.trim().to_lowercase();
    debug!(
        email = %email,
        unsubscribe_email = form.unsubscribe_email,
        unsubscribe_sms = form.unsubscribe_sms,
        "Unsubscribe options selected"
    );

    // Validate email
    if !is_valid_email(&email) {
        warn!(email = %email, "Invalid email format for unsubscribe");
        return render_unsubscribe_error("Please enter a valid email address.", &email, &nonce);
    }

    // Check if at least one option is selected
    if !form.unsubscribe_email && !form.unsubscribe_sms {
        warn!(email = %email, "No unsubscribe options selected");
        return render_unsubscribe_error(
            "Please select at least one option to unsubscribe from.",
            &email,
            &nonce,
        );
    }

    // Get Klaviyo config
    let Some(klaviyo_config) = state.config().klaviyo.as_ref() else {
        warn!("Klaviyo not configured for unsubscribe request");
        return render_unsubscribe_error(
            "Unsubscribe service is not available. Please contact support.",
            &email,
            &nonce,
        );
    };

    // Create Klaviyo client
    debug!("Creating Klaviyo client for unsubscribe");
    let client = match KlaviyoClient::new(klaviyo_config) {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Failed to create Klaviyo client");
            return render_unsubscribe_error(
                "Something went wrong. Please try again later.",
                &email,
                &nonce,
            );
        }
    };

    // Process the unsubscribe request
    debug!(email = %email, "Delegating to process_unsubscribe");
    process_unsubscribe(&client, &email, &form, &nonce).await
}

/// Process the unsubscribe request after validation.
async fn process_unsubscribe(
    client: &KlaviyoClient,
    email: &str,
    form: &UnsubscribeForm,
    nonce: &str,
) -> axum::response::Response {
    debug!(email = %email, "Looking up Klaviyo profile");
    // Find profile by email
    let profile = match client.find_profile_by_email(email).await {
        Ok(p) => {
            debug!(email = %email, profile_id = %p.id, "Found Klaviyo profile");
            p
        }
        Err(crate::services::KlaviyoError::ProfileNotFound(_)) => {
            // Profile not found - show success anyway (don't reveal if email exists)
            info!(email = %email, "Profile not found - showing success");
            return render_unsubscribe_success(email, nonce);
        }
        Err(e) => {
            error!(email = %email, error = %e, "Failed to find profile");
            return render_unsubscribe_error(
                "Something went wrong. Please try again later.",
                email,
                nonce,
            );
        }
    };

    // Unsubscribe from selected channels
    let mut errors = Vec::new();

    if form.unsubscribe_email {
        debug!(email = %email, profile_id = %profile.id, "Unsubscribing from email");
        if let Err(e) = client.unsubscribe_email(&profile.id).await {
            error!(email = %email, error = %e, "Failed to unsubscribe from email");
            errors.push("email");
        }
    }

    if form.unsubscribe_sms {
        debug!(email = %email, profile_id = %profile.id, "Unsubscribing from SMS");
        if let Err(e) = client.unsubscribe_sms(&profile.id).await {
            error!(email = %email, error = %e, "Failed to unsubscribe from SMS");
            errors.push("sms");
        }
    }

    if errors.is_empty() {
        info!(email = %email, "Unsubscribe successful");
        render_unsubscribe_success(email, nonce)
    } else {
        warn!(email = %email, failed_channels = ?errors, "Partial unsubscribe failure");
        render_unsubscribe_error(
            &format!(
                "Failed to unsubscribe from {}. Please try again or contact support.",
                errors.join(" and ")
            ),
            email,
            nonce,
        )
    }
}

/// Render the unsubscribe success page.
fn render_unsubscribe_success(email: &str, nonce: &str) -> axum::response::Response {
    Html(
        UnsubscribeSuccessTemplate {
            email: email.to_string(),
            nonce: nonce.to_string(),
        }
        .render()
        .unwrap_or_else(|_| "Error".to_string()),
    )
    .into_response()
}

/// Render the unsubscribe error page.
fn render_unsubscribe_error(message: &str, email: &str, nonce: &str) -> axum::response::Response {
    Html(
        UnsubscribeErrorTemplate {
            message: message.to_string(),
            email: email.to_string(),
            nonce: nonce.to_string(),
        }
        .render()
        .unwrap_or_else(|_| "Error".to_string()),
    )
    .into_response()
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
