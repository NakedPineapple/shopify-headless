//! TikTok Shop Returns / Refunds API.
//!
//! Return listing, details, and approve/reject actions via the TikTok Shop
//! Open API.

use serde::Serialize;
use tracing::instrument;

use super::TikTokShopError;
use super::client::TikTokShopClient;
use super::types::{ReturnListData, TikTokReturn};

impl TikTokShopClient {
    /// Get returns with optional pagination.
    ///
    /// Calls `GET /api/return_refund/list`.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self))]
    pub async fn get_returns(
        &self,
        page_size: u32,
        page_token: Option<&str>,
    ) -> Result<ReturnListData, TikTokShopError> {
        let mut params = vec![("page_size".to_string(), page_size.min(50).to_string())];

        if let Some(token) = page_token {
            params.push(("page_token".to_string(), token.to_string()));
        }

        self.execute_get("/api/return_refund/list", &params).await
    }

    /// Get return details by ID.
    ///
    /// Calls `GET /api/return_refund/{return_id}`.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(return_id = %return_id))]
    pub async fn get_return_details(
        &self,
        return_id: &str,
    ) -> Result<TikTokReturn, TikTokShopError> {
        let path = format!("/api/return_refund/{return_id}");
        self.execute_get(&path, &[]).await
    }

    /// Approve a return request.
    ///
    /// Calls `POST /api/return_refund/{return_id}/approve`.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(return_id = %return_id))]
    pub async fn approve_return(&self, return_id: &str) -> Result<(), TikTokShopError> {
        #[derive(Serialize)]
        struct ApproveBody {
            decision: &'static str,
        }

        let path = format!("/api/return_refund/{return_id}/approve");
        let body = ApproveBody {
            decision: "APPROVE",
        };
        let _: serde_json::Value = self.execute_post(&path, &[], Some(&body)).await?;
        Ok(())
    }

    /// Reject a return request with a reason.
    ///
    /// Calls `POST /api/return_refund/{return_id}/reject`.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(return_id = %return_id))]
    pub async fn reject_return(
        &self,
        return_id: &str,
        reason: &str,
    ) -> Result<(), TikTokShopError> {
        #[derive(Serialize)]
        struct RejectBody<'a> {
            decision: &'static str,
            reject_reason: &'a str,
        }

        let path = format!("/api/return_refund/{return_id}/reject");
        let body = RejectBody {
            decision: "REJECT",
            reject_reason: reason,
        };
        let _: serde_json::Value = self.execute_post(&path, &[], Some(&body)).await?;
        Ok(())
    }
}
