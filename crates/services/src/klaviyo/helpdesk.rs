//! Klaviyo Helpdesk integration for routing support tickets.
//!
//! Creates events and profiles in Klaviyo that trigger helpdesk workflows
//! for emails that need human attention (complaints, praise, unknown).

use serde::Serialize;
use tracing::{debug, instrument};

use super::{KlaviyoClient, KlaviyoError};

/// Event to track in Klaviyo for helpdesk routing.
#[derive(Debug, Serialize)]
struct TrackEventRequest {
    data: TrackEventData,
}

#[derive(Debug, Serialize)]
struct TrackEventData {
    #[serde(rename = "type")]
    data_type: &'static str,
    attributes: TrackEventAttributes,
}

#[derive(Debug, Serialize)]
struct TrackEventAttributes {
    metric: EventMetric,
    profile: EventProfile,
    properties: serde_json::Value,
    time: String,
}

#[derive(Debug, Serialize)]
struct EventMetric {
    data: EventMetricData,
}

#[derive(Debug, Serialize)]
struct EventMetricData {
    #[serde(rename = "type")]
    data_type: &'static str,
    attributes: EventMetricAttributes,
}

#[derive(Debug, Serialize)]
struct EventMetricAttributes {
    name: String,
}

#[derive(Debug, Serialize)]
struct EventProfile {
    data: EventProfileData,
}

#[derive(Debug, Serialize)]
struct EventProfileData {
    #[serde(rename = "type")]
    data_type: &'static str,
    attributes: EventProfileAttributes,
}

#[derive(Debug, Serialize)]
struct EventProfileAttributes {
    email: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    first_name: Option<String>,
}

/// Parameters for creating a helpdesk event.
pub struct HelpdeskEventParams<'a> {
    /// Customer email address.
    pub email: &'a str,
    /// Customer name (if known).
    pub customer_name: Option<&'a str>,
    /// Email subject.
    pub subject: &'a str,
    /// Classification category.
    pub classification: &'a str,
    /// AI reasoning.
    pub reasoning: &'a str,
    /// Internal email ID for reference.
    pub email_id: i32,
}

impl KlaviyoClient {
    /// Track a helpdesk routing event in Klaviyo.
    ///
    /// Creates an event that can trigger Klaviyo flows for helpdesk ticket
    /// creation and follow-up workflows.
    ///
    /// # Errors
    ///
    /// Returns error if the Klaviyo API request fails.
    #[instrument(skip(self, params), fields(email = %params.email, classification = %params.classification))]
    pub async fn track_helpdesk_event(
        &self,
        params: &HelpdeskEventParams<'_>,
    ) -> Result<(), KlaviyoError> {
        debug!("tracking helpdesk event in Klaviyo");

        let properties = serde_json::json!({
            "subject": params.subject,
            "classification": params.classification,
            "reasoning": params.reasoning,
            "email_id": params.email_id,
            "source": "automations"
        });

        let request = TrackEventRequest {
            data: TrackEventData {
                data_type: "event",
                attributes: TrackEventAttributes {
                    metric: EventMetric {
                        data: EventMetricData {
                            data_type: "metric",
                            attributes: EventMetricAttributes {
                                name: "Support Email Received".to_string(),
                            },
                        },
                    },
                    profile: EventProfile {
                        data: EventProfileData {
                            data_type: "profile",
                            attributes: EventProfileAttributes {
                                email: params.email.to_string(),
                                first_name: params.customer_name.map(String::from),
                            },
                        },
                    },
                    properties,
                    time: chrono::Utc::now().to_rfc3339(),
                },
            },
        };

        // Klaviyo Create Event endpoint returns 202 with empty body
        let url = "/events";
        let response = self.post_raw(url, &request).await?;

        if response.status().is_success() {
            debug!("helpdesk event tracked successfully");
            Ok(())
        } else {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            Err(KlaviyoError::Api {
                status,
                message: body,
            })
        }
    }

    /// Execute a raw POST request and return the response (without parsing JSON).
    async fn post_raw<B: Serialize + Sync>(
        &self,
        path: &str,
        body: &B,
    ) -> Result<reqwest::Response, KlaviyoError> {
        let url = format!("https://a.klaviyo.com/api{path}");
        let response = self.inner.client.post(&url).json(body).send().await?;
        Ok(response)
    }
}
