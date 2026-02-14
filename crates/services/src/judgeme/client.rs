//! Judge.me API client for product reviews.

use std::sync::Arc;

use secrecy::ExposeSecret;
use tracing::{debug, instrument};

use crate::config::JudgemeConfig;

use super::error::JudgemeError;
use super::types::{
    CreateReplyParams, CreateReviewParams, ModerateReviewParams, ProductResponse, ReviewsResponse,
};

const JUDGEME_API_URL: &str = "https://judge.me/api/v1";

/// Judge.me API client for reading and managing product reviews.
///
/// Uses `Arc` internally for cheap cloning across handlers.
#[derive(Clone)]
pub struct JudgemeClient {
    inner: Arc<JudgemeClientInner>,
}

struct JudgemeClientInner {
    client: reqwest::Client,
    api_token: String,
    shop_domain: String,
}

impl JudgemeClient {
    /// Create a new Judge.me client.
    ///
    /// # Panics
    ///
    /// Panics if the HTTP client cannot be built (should never happen with defaults).
    #[must_use]
    pub fn new(config: &JudgemeConfig) -> Self {
        let client = reqwest::Client::builder()
            .build()
            .expect("Failed to build HTTP client");

        Self {
            inner: Arc::new(JudgemeClientInner {
                client,
                api_token: config.api_token.expose_secret().to_string(),
                shop_domain: config.shop_domain.clone(),
            }),
        }
    }

    /// Build the auth query string portion: `api_token=...&shop_domain=...`
    fn auth_query(&self) -> String {
        format!(
            "api_token={}&shop_domain={}",
            urlencoding::encode(&self.inner.api_token),
            urlencoding::encode(&self.inner.shop_domain),
        )
    }

    /// Resolve a Shopify product's numeric external ID to a Judge.me internal product ID.
    ///
    /// # Errors
    ///
    /// Returns `ProductNotFound` if no Judge.me product exists for this Shopify ID.
    #[instrument(skip(self))]
    pub async fn resolve_product_id(&self, shopify_external_id: i64) -> Result<i64, JudgemeError> {
        let url = format!(
            "{JUDGEME_API_URL}/products/-1?{}&external_id={shopify_external_id}",
            self.auth_query(),
        );

        let response: reqwest::Response = self.inner.client.get(&url).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            if status == 404 {
                return Err(JudgemeError::ProductNotFound(shopify_external_id));
            }
            let body = response.text().await.unwrap_or_default();
            return Err(JudgemeError::Api {
                status,
                message: body,
            });
        }

        let product_response: ProductResponse = response
            .json()
            .await
            .map_err(|e| JudgemeError::Parse(format!("Failed to parse product response: {e}")))?;

        debug!(
            judgeme_id = product_response.product.id,
            shopify_external_id, "Resolved Judge.me product ID"
        );

        Ok(product_response.product.id)
    }

    /// Fetch paginated reviews for a specific Judge.me product.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_reviews(
        &self,
        judgeme_product_id: i64,
        page: i32,
        per_page: i32,
    ) -> Result<ReviewsResponse, JudgemeError> {
        let url = format!(
            "{JUDGEME_API_URL}/reviews?{}&product_id={judgeme_product_id}&page={page}&per_page={per_page}",
            self.auth_query(),
        );

        let response: reqwest::Response = self.inner.client.get(&url).send().await?;

        self.handle_response(response).await
    }

    /// Fetch paginated reviews across all products (for admin moderation).
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_all_reviews(
        &self,
        page: i32,
        per_page: i32,
    ) -> Result<ReviewsResponse, JudgemeError> {
        let url = format!(
            "{JUDGEME_API_URL}/reviews?{}&page={page}&per_page={per_page}",
            self.auth_query(),
        );

        let response: reqwest::Response = self.inner.client.get(&url).send().await?;

        self.handle_response(response).await
    }

    /// Submit a new review. Judge.me creates reviews asynchronously in the background.
    ///
    /// This endpoint is unauthenticated per Judge.me's design — customers submit
    /// reviews directly without needing an API token.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self, params), fields(product_id = params.id, rating = params.rating))]
    pub async fn create_review(&self, params: &CreateReviewParams) -> Result<(), JudgemeError> {
        let url = format!("{JUDGEME_API_URL}/reviews");
        let response = self.inner.client.post(&url).json(params).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(JudgemeError::Api {
                status,
                message: body,
            });
        }

        debug!("Review submitted successfully");
        Ok(())
    }

    /// Moderate a review by setting its curated status.
    ///
    /// # Arguments
    ///
    /// * `review_id` - Judge.me review ID
    /// * `curated` - New status: `"ok"` to approve, `"spam"` to reject
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn moderate_review(&self, review_id: i64, curated: &str) -> Result<(), JudgemeError> {
        let url = format!(
            "{JUDGEME_API_URL}/reviews/{review_id}?{}",
            self.auth_query(),
        );
        let params = ModerateReviewParams {
            curated: curated.to_string(),
        };

        let response: reqwest::Response = self.inner.client.put(&url).json(&params).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(JudgemeError::Api {
                status,
                message: body,
            });
        }

        debug!(review_id, curated, "Review moderated");
        Ok(())
    }

    /// Create a public reply to a review.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self, content))]
    pub async fn create_reply(
        &self,
        review_id: i64,
        content: &str,
        send_email: bool,
    ) -> Result<(), JudgemeError> {
        let url = format!(
            "{JUDGEME_API_URL}/reviews/{review_id}/replies?{}",
            self.auth_query(),
        );
        let params = CreateReplyParams {
            body: content.to_string(),
            send_reply_email: send_email,
        };

        let response: reqwest::Response = self.inner.client.post(&url).json(&params).send().await?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(JudgemeError::Api {
                status,
                message: body,
            });
        }

        debug!(review_id, "Reply created");
        Ok(())
    }

    /// Parse a JSON response body into the expected type.
    async fn handle_response<T: serde::de::DeserializeOwned>(
        &self,
        response: reqwest::Response,
    ) -> Result<T, JudgemeError> {
        let status = response.status();

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(JudgemeError::Api {
                status: status.as_u16(),
                message: body,
            });
        }

        response
            .json()
            .await
            .map_err(|e| JudgemeError::Parse(format!("Failed to parse response: {e}")))
    }
}
