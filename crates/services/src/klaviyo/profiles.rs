//! Klaviyo profile management for customer segmentation.
//!
//! Creates or updates customer profiles with segment properties
//! so Klaviyo flows and segments can target the right audience.

use serde_json::Map;
use tracing::{debug, instrument};

use super::{BASE_URL, KlaviyoClient, KlaviyoError};

/// Parameters for upserting a customer profile with segment data.
pub struct ProfileUpsertParams<'a> {
    /// Customer email address.
    pub email: &'a str,
    /// First name (optional).
    pub first_name: Option<&'a str>,
    /// Last name (optional).
    pub last_name: Option<&'a str>,
    /// Segment name (e.g., "vip", "repeat\_customer").
    pub segment: &'a str,
    /// Total number of orders.
    pub order_count: i32,
    /// Lifetime spend amount as a string.
    pub lifetime_value: &'a str,
    /// Date of last order (ISO 8601), if any.
    pub last_order_at: Option<&'a str>,
}

impl KlaviyoClient {
    /// Create or update a Klaviyo profile with segment properties.
    ///
    /// Uses the Klaviyo Create or Update Profile endpoint which is
    /// idempotent — profiles are matched by email address.
    ///
    /// # Errors
    ///
    /// Returns error if the Klaviyo API request fails.
    #[instrument(skip(self, params), fields(email = %params.email, segment = %params.segment))]
    pub async fn upsert_profile(
        &self,
        params: &ProfileUpsertParams<'_>,
    ) -> Result<(), KlaviyoError> {
        debug!("upserting customer profile in Klaviyo");

        let properties = build_properties(params);
        let attributes = build_attributes(params, properties);

        let body = serde_json::json!({
            "data": {
                "type": "profile",
                "attributes": attributes,
            }
        });

        let url = format!("{BASE_URL}/profile-import");
        let response = self.inner.client.post(&url).json(&body).send().await?;

        if response.status().is_success() {
            debug!("profile upserted successfully");
            Ok(())
        } else {
            let status = response.status().as_u16();
            let message = response.text().await.unwrap_or_default();
            Err(KlaviyoError::Api { status, message })
        }
    }
}

/// Build the properties object for the profile upsert.
fn build_properties(params: &ProfileUpsertParams<'_>) -> serde_json::Value {
    let mut props = Map::new();
    props.insert(
        "np_segment".to_string(),
        serde_json::Value::String(params.segment.to_string()),
    );
    props.insert(
        "np_order_count".to_string(),
        serde_json::json!(params.order_count),
    );
    props.insert(
        "np_lifetime_value".to_string(),
        serde_json::Value::String(params.lifetime_value.to_string()),
    );
    props.insert(
        "np_segmented_by".to_string(),
        serde_json::Value::String("automations".to_string()),
    );

    if let Some(last_order) = params.last_order_at {
        props.insert(
            "np_last_order_at".to_string(),
            serde_json::Value::String(last_order.to_string()),
        );
    }

    serde_json::Value::Object(props)
}

/// Build the attributes object for the profile upsert.
fn build_attributes(
    params: &ProfileUpsertParams<'_>,
    properties: serde_json::Value,
) -> serde_json::Value {
    let mut attrs = Map::new();
    attrs.insert(
        "email".to_string(),
        serde_json::Value::String(params.email.to_string()),
    );
    attrs.insert("properties".to_string(), properties);

    if let Some(first) = params.first_name {
        attrs.insert(
            "first_name".to_string(),
            serde_json::Value::String(first.to_string()),
        );
    }
    if let Some(last) = params.last_name {
        attrs.insert(
            "last_name".to_string(),
            serde_json::Value::String(last.to_string()),
        );
    }

    serde_json::Value::Object(attrs)
}
