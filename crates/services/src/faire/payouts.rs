//! Faire Payouts API.

use serde::Serialize;
use tracing::instrument;

use super::FaireError;
use super::client::FaireClient;
use super::types::{FairePayout, PayoutsPage};

/// Query parameters for listing payouts.
#[derive(Serialize)]
struct ListPayoutsQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<String>,
}

impl FaireClient {
    /// List payouts with optional status filter.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self))]
    pub async fn list_payouts(
        &self,
        page: Option<i32>,
        limit: Option<i32>,
        status: Option<String>,
    ) -> Result<PayoutsPage, FaireError> {
        let params = ListPayoutsQuery {
            page,
            limit: Some(limit.unwrap_or(50)),
            state: status,
        };
        self.execute_get("/payouts", Some(&params)).await
    }

    /// Get a specific payout by token.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(payout_token = %token))]
    pub async fn get_payout(&self, token: &str) -> Result<FairePayout, FaireError> {
        let path = format!("/payouts/{token}");
        self.execute_get(&path, None::<&()>).await
    }
}
