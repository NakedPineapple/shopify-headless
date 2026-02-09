//! Klaviyo event tracking for automated workflows.
//!
//! Tracks custom events in Klaviyo that trigger flows for
//! abandoned cart recovery, transactional emails, and
//! subscription lifecycle sequences.

use serde::Serialize;
use tracing::{debug, instrument};

use super::{KlaviyoClient, KlaviyoError};

// ---------------------------------------------------------------------------
// Shared event types
// ---------------------------------------------------------------------------

/// A line item in an order for Klaviyo event tracking.
#[derive(Debug, Clone, Serialize)]
pub struct OrderLineItemEvent {
    /// Product title.
    pub title: String,
    /// Variant title (if applicable).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// Quantity ordered.
    pub quantity: i64,
    /// Formatted price (e.g., "$29.99").
    pub price: String,
}

/// A shipping address for Klaviyo event tracking.
#[derive(Debug, Clone, Serialize)]
pub struct ShippingAddressEvent {
    pub name: String,
    pub address1: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address2: Option<String>,
    pub city: String,
    pub province: String,
    pub zip: String,
    pub country: String,
}

// ---------------------------------------------------------------------------
// Abandoned cart
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Order events
// ---------------------------------------------------------------------------

/// Parameters for an "Order Confirmed" event.
pub struct OrderConfirmedEventParams<'a> {
    pub email: &'a str,
    pub customer_name: &'a str,
    pub order_name: &'a str,
    pub order_date: &'a str,
    pub line_items: &'a [OrderLineItemEvent],
    pub subtotal: &'a str,
    pub shipping: &'a str,
    pub tax: &'a str,
    pub total: &'a str,
    pub shipping_address: Option<&'a ShippingAddressEvent>,
}

/// Parameters for an "Order Shipped" event.
pub struct OrderShippedEventParams<'a> {
    pub email: &'a str,
    pub customer_name: &'a str,
    pub order_name: &'a str,
    pub carrier: Option<&'a str>,
    pub tracking_number: Option<&'a str>,
    pub tracking_url: Option<&'a str>,
    pub items: &'a [String],
}

/// Parameters for an "Order Delivered" event.
///
/// This event triggers both the delivery notification and, after a
/// configurable delay in the Klaviyo flow, a review request email.
pub struct OrderDeliveredEventParams<'a> {
    pub email: &'a str,
    pub customer_name: &'a str,
    pub order_name: &'a str,
    pub product_names: &'a [String],
    pub store_url: &'a str,
}

// ---------------------------------------------------------------------------
// Subscription events
// ---------------------------------------------------------------------------

/// Parameters for a "Subscription Renewal Reminder" event.
pub struct SubscriptionRenewalReminderEventParams<'a> {
    pub email: &'a str,
    pub customer_name: &'a str,
    pub renewal_date: &'a str,
    pub product_names: &'a [String],
}

/// Parameters for a "Subscription Payment Failed" event.
pub struct SubscriptionPaymentFailedEventParams<'a> {
    pub email: &'a str,
    pub customer_name: &'a str,
    pub product_names: &'a [String],
}

/// Parameters for a "Subscription Cancelled" event.
///
/// The Klaviyo flow handles the win-back delay (e.g., 14 days).
pub struct SubscriptionCancelledEventParams<'a> {
    pub email: &'a str,
    pub customer_name: &'a str,
    pub product_names: &'a [String],
    pub store_url: &'a str,
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

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

        self.post_event(&body).await
    }

    /// Track an "Order Confirmed" event.
    ///
    /// # Errors
    ///
    /// Returns error if the Klaviyo API request fails.
    #[instrument(skip(self, params), fields(email = %params.email, order = %params.order_name))]
    pub async fn track_order_confirmed_event(
        &self,
        params: &OrderConfirmedEventParams<'_>,
    ) -> Result<(), KlaviyoError> {
        debug!("tracking order confirmed event in Klaviyo");

        let body = serde_json::json!({
            "data": {
                "type": "event",
                "attributes": {
                    "metric": {
                        "data": {
                            "type": "metric",
                            "attributes": { "name": "Order Confirmed" }
                        }
                    },
                    "profile": {
                        "data": {
                            "type": "profile",
                            "attributes": { "email": params.email }
                        }
                    },
                    "properties": {
                        "customer_name": params.customer_name,
                        "order_name": params.order_name,
                        "order_date": params.order_date,
                        "line_items": params.line_items,
                        "subtotal": params.subtotal,
                        "shipping": params.shipping,
                        "tax": params.tax,
                        "total": params.total,
                        "shipping_address": params.shipping_address,
                        "source": "email_automation"
                    },
                    "time": chrono::Utc::now().to_rfc3339()
                }
            }
        });

        self.post_event(&body).await
    }

    /// Track an "Order Shipped" event.
    ///
    /// # Errors
    ///
    /// Returns error if the Klaviyo API request fails.
    #[instrument(skip(self, params), fields(email = %params.email, order = %params.order_name))]
    pub async fn track_order_shipped_event(
        &self,
        params: &OrderShippedEventParams<'_>,
    ) -> Result<(), KlaviyoError> {
        debug!("tracking order shipped event in Klaviyo");

        let body = serde_json::json!({
            "data": {
                "type": "event",
                "attributes": {
                    "metric": {
                        "data": {
                            "type": "metric",
                            "attributes": { "name": "Order Shipped" }
                        }
                    },
                    "profile": {
                        "data": {
                            "type": "profile",
                            "attributes": { "email": params.email }
                        }
                    },
                    "properties": {
                        "customer_name": params.customer_name,
                        "order_name": params.order_name,
                        "carrier": params.carrier,
                        "tracking_number": params.tracking_number,
                        "tracking_url": params.tracking_url,
                        "items": params.items,
                        "source": "email_automation"
                    },
                    "time": chrono::Utc::now().to_rfc3339()
                }
            }
        });

        self.post_event(&body).await
    }

    /// Track an "Order Delivered" event.
    ///
    /// This event triggers both the delivery notification and, after a
    /// configurable delay in the Klaviyo flow, a review request email.
    ///
    /// # Errors
    ///
    /// Returns error if the Klaviyo API request fails.
    #[instrument(skip(self, params), fields(email = %params.email, order = %params.order_name))]
    pub async fn track_order_delivered_event(
        &self,
        params: &OrderDeliveredEventParams<'_>,
    ) -> Result<(), KlaviyoError> {
        debug!("tracking order delivered event in Klaviyo");

        let body = serde_json::json!({
            "data": {
                "type": "event",
                "attributes": {
                    "metric": {
                        "data": {
                            "type": "metric",
                            "attributes": { "name": "Order Delivered" }
                        }
                    },
                    "profile": {
                        "data": {
                            "type": "profile",
                            "attributes": { "email": params.email }
                        }
                    },
                    "properties": {
                        "customer_name": params.customer_name,
                        "order_name": params.order_name,
                        "product_names": params.product_names,
                        "store_url": params.store_url,
                        "source": "email_automation"
                    },
                    "time": chrono::Utc::now().to_rfc3339()
                }
            }
        });

        self.post_event(&body).await
    }

    /// Track a "Subscription Renewal Reminder" event.
    ///
    /// # Errors
    ///
    /// Returns error if the Klaviyo API request fails.
    #[instrument(skip(self, params), fields(email = %params.email))]
    pub async fn track_subscription_renewal_reminder_event(
        &self,
        params: &SubscriptionRenewalReminderEventParams<'_>,
    ) -> Result<(), KlaviyoError> {
        debug!("tracking subscription renewal reminder event in Klaviyo");

        let body = serde_json::json!({
            "data": {
                "type": "event",
                "attributes": {
                    "metric": {
                        "data": {
                            "type": "metric",
                            "attributes": { "name": "Subscription Renewal Reminder" }
                        }
                    },
                    "profile": {
                        "data": {
                            "type": "profile",
                            "attributes": { "email": params.email }
                        }
                    },
                    "properties": {
                        "customer_name": params.customer_name,
                        "renewal_date": params.renewal_date,
                        "product_names": params.product_names,
                        "source": "email_automation"
                    },
                    "time": chrono::Utc::now().to_rfc3339()
                }
            }
        });

        self.post_event(&body).await
    }

    /// Track a "Subscription Payment Failed" event.
    ///
    /// # Errors
    ///
    /// Returns error if the Klaviyo API request fails.
    #[instrument(skip(self, params), fields(email = %params.email))]
    pub async fn track_subscription_payment_failed_event(
        &self,
        params: &SubscriptionPaymentFailedEventParams<'_>,
    ) -> Result<(), KlaviyoError> {
        debug!("tracking subscription payment failed event in Klaviyo");

        let body = serde_json::json!({
            "data": {
                "type": "event",
                "attributes": {
                    "metric": {
                        "data": {
                            "type": "metric",
                            "attributes": { "name": "Subscription Payment Failed" }
                        }
                    },
                    "profile": {
                        "data": {
                            "type": "profile",
                            "attributes": { "email": params.email }
                        }
                    },
                    "properties": {
                        "customer_name": params.customer_name,
                        "product_names": params.product_names,
                        "source": "email_automation"
                    },
                    "time": chrono::Utc::now().to_rfc3339()
                }
            }
        });

        self.post_event(&body).await
    }

    /// Track a "Subscription Cancelled" event.
    ///
    /// The Klaviyo flow handles the win-back delay (e.g., 14 days).
    ///
    /// # Errors
    ///
    /// Returns error if the Klaviyo API request fails.
    #[instrument(skip(self, params), fields(email = %params.email))]
    pub async fn track_subscription_cancelled_event(
        &self,
        params: &SubscriptionCancelledEventParams<'_>,
    ) -> Result<(), KlaviyoError> {
        debug!("tracking subscription cancelled event in Klaviyo");

        let body = serde_json::json!({
            "data": {
                "type": "event",
                "attributes": {
                    "metric": {
                        "data": {
                            "type": "metric",
                            "attributes": { "name": "Subscription Cancelled" }
                        }
                    },
                    "profile": {
                        "data": {
                            "type": "profile",
                            "attributes": { "email": params.email }
                        }
                    },
                    "properties": {
                        "customer_name": params.customer_name,
                        "product_names": params.product_names,
                        "store_url": params.store_url,
                        "source": "email_automation"
                    },
                    "time": chrono::Utc::now().to_rfc3339()
                }
            }
        });

        self.post_event(&body).await
    }

    /// Post an event to the Klaviyo Create Event endpoint.
    async fn post_event(&self, body: &serde_json::Value) -> Result<(), KlaviyoError> {
        let url = format!("{}/events", super::BASE_URL);
        let response = self.inner.client.post(&url).json(body).send().await?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            Err(KlaviyoError::Api { status, message })
        }
    }
}
