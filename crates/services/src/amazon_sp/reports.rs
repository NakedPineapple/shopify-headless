//! Amazon SP-API Reports API (v2021-06-30).
//!
//! Foundation for async report generation and download. Reports are created
//! asynchronously; callers must poll for completion before downloading.

use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::AmazonSpError;
use super::client::AmazonSpClient;

// =============================================================================
// Types (camelCase JSON)
// =============================================================================

/// Request body for `POST /reports/2021-06-30/reports`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateReportRequest {
    pub report_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_start_time: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_end_time: Option<String>,
    pub marketplace_ids: Vec<String>,
}

/// Response from `POST /reports/2021-06-30/reports`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CreateReportResponse {
    pub report_id: String,
}

/// Report metadata from `GET /reports/2021-06-30/reports/{id}`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub report_id: String,
    pub report_type: String,
    pub processing_status: String,
    pub data_start_time: Option<String>,
    pub data_end_time: Option<String>,
    pub report_document_id: Option<String>,
    pub created_time: Option<String>,
    pub processing_start_time: Option<String>,
    pub processing_end_time: Option<String>,
    pub marketplace_ids: Option<Vec<String>>,
}

/// Report document with download URL from `GET /reports/2021-06-30/documents/{id}`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportDocument {
    pub report_document_id: String,
    pub url: String,
    pub compression_algorithm: Option<String>,
}

// =============================================================================
// Client Implementation
// =============================================================================

impl AmazonSpClient {
    /// Create a new report request.
    ///
    /// Returns the `report_id` which can be polled via [`get_report`](Self::get_report).
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self, request), fields(report_type = %request.report_type))]
    pub async fn create_report(
        &self,
        request: &CreateReportRequest,
    ) -> Result<String, AmazonSpError> {
        let response: CreateReportResponse = self
            .execute_with_retry(
                reqwest::Method::POST,
                "/reports/2021-06-30/reports",
                Option::<&()>::None,
                Some(request),
            )
            .await?;

        Ok(response.report_id)
    }

    /// Get report status and metadata.
    ///
    /// Poll this until `processing_status` is `DONE`, `CANCELLED`, or `FATAL`.
    /// When `DONE`, use `report_document_id` to download via [`get_report_document`](Self::get_report_document).
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(report_id = %report_id))]
    pub async fn get_report(&self, report_id: &str) -> Result<Report, AmazonSpError> {
        let path = format!("/reports/2021-06-30/reports/{report_id}");
        self.execute(&path, Option::<&()>::None).await
    }

    /// Get report document download URL.
    ///
    /// The returned URL is pre-signed and expires after a short window.
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self), fields(document_id = %document_id))]
    pub async fn get_report_document(
        &self,
        document_id: &str,
    ) -> Result<ReportDocument, AmazonSpError> {
        let path = format!("/reports/2021-06-30/documents/{document_id}");
        self.execute(&path, Option::<&()>::None).await
    }
}
