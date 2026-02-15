//! Pinterest Conversions API (CAPI).
//!
//! Reports server-side conversion events to Pinterest for attribution.
//! Complements the client-side Pinterest Tag for deduplication via `event_id`.
//!
//! Rate limit: 5,000 calls/minute per ad account.
//! Events must be sent within 1 hour of occurrence.

use tracing::instrument;

use super::PinterestError;
use super::client::PinterestClient;
use super::types::{ConversionEvent, ConversionEventsRequest, ConversionEventsResponse};

impl PinterestClient {
    /// Send conversion events to the Pinterest Conversions API.
    ///
    /// Events are batched in a single request. Each event should include
    /// an `event_id` that matches the client-side Pinterest Tag event for
    /// deduplication.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails or the ad account is not configured.
    #[instrument(skip(self, events), fields(event_count = events.len()))]
    pub async fn send_conversion_events(
        &self,
        events: Vec<ConversionEvent>,
    ) -> Result<ConversionEventsResponse, PinterestError> {
        if events.is_empty() {
            return Ok(ConversionEventsResponse {
                num_events_received: Some(0),
                num_events_processed: Some(0),
                events: None,
            });
        }

        let ad_account_id = self.ad_account_id();
        let path = format!("/ad_accounts/{ad_account_id}/events");
        let body = ConversionEventsRequest { data: events };

        self.execute_post(&path, &body).await
    }
}
