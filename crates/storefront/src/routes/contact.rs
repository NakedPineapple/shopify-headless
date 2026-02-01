//! Contact form route handlers.
//!
//! Handles product question submissions via Klaviyo event tracking.

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::services::KlaviyoClient;
use crate::state::AppState;

/// Product question form data.
#[derive(Debug, Deserialize)]
pub struct ProductQuestionForm {
    pub product: String,
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub phone: Option<String>,
    pub message: String,
}

/// Response for form submission.
#[derive(Debug, Serialize)]
pub struct ContactResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Submit a product question.
///
/// POST /contact/product-question
///
/// Tracks the question as a Klaviyo event, which can trigger flows
/// to notify the support team and send an auto-response.
#[instrument(skip(state), fields(email = %form.email, product = %form.product))]
pub async fn product_question(
    State(state): State<AppState>,
    Json(form): Json<ProductQuestionForm>,
) -> impl IntoResponse {
    let email = form.email.trim().to_lowercase();

    // Basic email validation
    if !is_valid_email(&email) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ContactResponse {
                success: false,
                message: Some("Please enter a valid email address.".to_string()),
            }),
        );
    }

    // Validate required fields
    if form.name.trim().is_empty() || form.message.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ContactResponse {
                success: false,
                message: Some("Name and message are required.".to_string()),
            }),
        );
    }

    // Get Klaviyo config
    let Some(klaviyo_config) = state.config().klaviyo.as_ref() else {
        tracing::error!("Klaviyo not configured");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ContactResponse {
                success: false,
                message: Some("Service temporarily unavailable.".to_string()),
            }),
        );
    };

    // Create Klaviyo client
    let client = match KlaviyoClient::new(klaviyo_config) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create Klaviyo client");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ContactResponse {
                    success: false,
                    message: Some("Service temporarily unavailable.".to_string()),
                }),
            );
        }
    };

    // Build event properties
    let properties = serde_json::json!({
        "product": form.product.trim(),
        "customer_name": form.name.trim(),
        "phone": form.phone.as_deref().unwrap_or("").trim(),
        "message": form.message.trim(),
        "source": "Product Page - Ask a Question"
    });

    // Track the event in Klaviyo
    match client
        .track_event(&email, "Asked Product Question", properties)
        .await
    {
        Ok(()) => {
            tracing::info!(email = %email, product = %form.product, "Product question tracked");
            (
                StatusCode::OK,
                Json(ContactResponse {
                    success: true,
                    message: None,
                }),
            )
        }
        Err(e) => {
            tracing::error!(email = %email, error = %e, "Failed to track product question");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ContactResponse {
                    success: false,
                    message: Some("Something went wrong. Please try again.".to_string()),
                }),
            )
        }
    }
}

/// Basic email validation.
fn is_valid_email(email: &str) -> bool {
    let mut parts = email.splitn(2, '@');
    let Some(local) = parts.next() else {
        return false;
    };
    let Some(domain) = parts.next() else {
        return false;
    };
    !local.is_empty() && !domain.is_empty() && domain.contains('.')
}
