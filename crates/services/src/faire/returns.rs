//! Faire Returns API.

use serde::Serialize;
use tracing::instrument;

use super::FaireError;
use super::client::FaireClient;
use super::types::{FaireReturn, ReturnsPage};

/// Query parameters for listing returns.
#[derive(Serialize)]
struct ListReturnsQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    page: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<String>,
}

impl FaireClient {
    /// List returns with optional status filter.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self))]
    pub async fn list_returns(
        &self,
        page: Option<i32>,
        limit: Option<i32>,
        status: Option<String>,
    ) -> Result<ReturnsPage, FaireError> {
        let params = ListReturnsQuery {
            page,
            limit: Some(limit.unwrap_or(50)),
            state: status,
        };
        self.execute_get("/returns", Some(&params)).await
    }

    /// Get a specific return by token.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(return_token = %token))]
    pub async fn get_return(&self, token: &str) -> Result<FaireReturn, FaireError> {
        let path = format!("/returns/{token}");
        self.execute_get(&path, None::<&()>).await
    }

    /// Approve a return.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(return_token = %token))]
    pub async fn approve_return(&self, token: &str) -> Result<FaireReturn, FaireError> {
        let path = format!("/returns/{token}/approve");
        self.execute_put(&path, &serde_json::json!({})).await
    }

    /// Reject a return with a reason.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(return_token = %token))]
    pub async fn reject_return(
        &self,
        token: &str,
        reason: &str,
    ) -> Result<FaireReturn, FaireError> {
        let path = format!("/returns/{token}/reject");
        self.execute_put(&path, &serde_json::json!({ "reason": reason }))
            .await
    }
}
