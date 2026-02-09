//! Klaviyo event tracking for automated workflows.
//!
//! Tracks custom events in Klaviyo that trigger flows for
//! abandoned cart recovery and other automated sequences.

use serde::Serialize;
use tracing::{debug, instrument};

use super::{KlaviyoClient, KlaviyoError};

/// Parameters for tracking an abandoned cart event in Klaviyo.
pub struct AbandonedCartEventParams<'a> {
    /// Customer email address.
    pub email: &'a str,
    /// Cart total as a string (e.g., "49.99").
    pub cart_total: &'a str,
    /// URL to resume the checkout.
    pub checkout_url: &'a str,
    /// Line items in the abandoned cart.
    pub line_items: &'a [CartLineItem],
}

/// A line item in an abandoned cart for Klaviyo event tracking.
#[derive(Debug, Clone, Serialize)]
pub struct CartLineItem {
    /// Product title.
    pub title: String,
    /// Quantity in the cart.
    pub quantity: i64,
    /// Variant title (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

impl KlaviyoClient {
    /// Track an abandoned cart event in Klaviyo.
    ///
    /// Creates an "Abandoned Cart Detected" event that triggers
    /// Klaviyo flows for the multi-step recovery email sequence
    /// (e.g., 1 hour reminder, 24 hour follow-up).
    ///
    /// # Errors
    ///
    /// Returns error if the Klaviyo API request fails.
    #[instrument(skip(self, params), fields(email = %params.email))]
    pub async fn track_abandoned_cart_event(
        &self,
        params: &AbandonedCartEventParams<'_>,
    ) -> Result<(), KlaviyoError> {
        debug!("tracking abandoned cart event in Klaviyo");

        let body = serde_json::json!({
            "data": {
                "type": "event",
                "attributes": {
                    "metric": {
                        "data": {
                            "type": "metric",
                            "attributes": {
                                "name": "Abandoned Cart Detected"
                            }
                        }
                    },
                    "profile": {
                        "data": {
                            "type": "profile",
                            "attributes": {
                                "email": params.email
                            }
                        }
                    },
                    "properties": {
                        "cart_total": params.cart_total,
                        "checkout_url": params.checkout_url,
                        "line_items": params.line_items,
                        "source": "email_automation"
                    },
                    "time": chrono::Utc::now().to_rfc3339()
                }
            }
        });

        let url = format!("{}/events", super::BASE_URL);
        let response = self.inner.client.post(&url).json(&body).send().await?;

        if response.status().is_success() {
            debug!("abandoned cart event tracked successfully");
            Ok(())
        } else {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            Err(KlaviyoError::Api { status, message })
        }
    }
}
