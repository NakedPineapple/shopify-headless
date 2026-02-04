//! Server-side Mixpanel HTTP API client.
//!
//! Used for tracking events that can't fire from the browser,
//! primarily purchase events from Shopify webhooks.

use reqwest::Client;
use serde_json::{Value, json};
use tracing::{debug, warn};

const MIXPANEL_TRACK_URL: &str = "https://api.mixpanel.com/track";
const MIXPANEL_ENGAGE_URL: &str = "https://api.mixpanel.com/engage";

/// A lightweight Mixpanel server-side client.
pub struct MixpanelClient {
    http: Client,
    token: String,
}

impl MixpanelClient {
    /// Create a new client with the given project token.
    #[must_use]
    pub fn new(token: String) -> Self {
        Self {
            http: Client::new(),
            token,
        }
    }

    /// Track an event for a specific user.
    pub async fn track(&self, distinct_id: &str, event: &str, properties: Value) {
        let mut props = match properties {
            Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        props.insert("token".to_string(), json!(self.token));
        props.insert("distinct_id".to_string(), json!(distinct_id));

        let payload = json!([{
            "event": event,
            "properties": Value::Object(props),
        }]);

        debug!(event, distinct_id, "Sending Mixpanel track event");

        match self
            .http
            .post(MIXPANEL_TRACK_URL)
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                debug!(event, "Mixpanel track event sent successfully");
            }
            Ok(resp) => {
                warn!(
                    event,
                    status = %resp.status(),
                    "Mixpanel track event returned non-success status"
                );
            }
            Err(e) => {
                warn!(event, error = %e, "Failed to send Mixpanel track event");
            }
        }
    }

    /// Record a revenue charge for a user (people analytics).
    pub async fn track_charge(&self, distinct_id: &str, amount: f64) {
        let payload = json!([{
            "$token": self.token,
            "$distinct_id": distinct_id,
            "$append": {
                "$transactions": {
                    "$amount": amount,
                }
            }
        }]);

        debug!(distinct_id, amount, "Sending Mixpanel revenue charge");

        match self
            .http
            .post(MIXPANEL_ENGAGE_URL)
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                debug!("Mixpanel revenue charge sent successfully");
            }
            Ok(resp) => {
                warn!(
                    status = %resp.status(),
                    "Mixpanel revenue charge returned non-success status"
                );
            }
            Err(e) => {
                warn!(error = %e, "Failed to send Mixpanel revenue charge");
            }
        }
    }
}
