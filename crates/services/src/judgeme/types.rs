//! Request and response types for the Judge.me API.

use serde::{Deserialize, Serialize};

/// A single review from Judge.me.
#[derive(Debug, Clone, Deserialize)]
pub struct Review {
    /// Judge.me review ID.
    pub id: i64,
    /// Review title.
    pub title: Option<String>,
    /// Review body text.
    pub body: Option<String>,
    /// Star rating (1-5).
    pub rating: i32,
    /// Reviewer information.
    pub reviewer: Reviewer,
    /// Moderation status: "ok", "spam", or "not-yet".
    pub curated: String,
    /// Whether the reviewer is a verified buyer.
    #[serde(default)]
    pub verified: String,
    /// Whether the review is hidden.
    #[serde(default)]
    pub hidden: bool,
    /// When the review was created.
    pub created_at: String,
    /// Attached pictures.
    #[serde(default)]
    pub pictures: Vec<ReviewPicture>,
    /// Shopify product external ID.
    #[serde(default)]
    pub product_external_id: Option<i64>,
    /// Product title.
    #[serde(default)]
    pub product_title: Option<String>,
    /// Product handle (URL slug).
    #[serde(default)]
    pub product_handle: Option<String>,
}

/// Reviewer information.
#[derive(Debug, Clone, Deserialize)]
pub struct Reviewer {
    /// Reviewer's Judge.me ID.
    pub id: i64,
    /// Reviewer's email address.
    pub email: String,
    /// Reviewer's display name.
    pub name: Option<String>,
}

/// A picture attached to a review.
#[derive(Debug, Clone, Deserialize)]
pub struct ReviewPicture {
    /// Picture URLs at different sizes.
    pub urls: PictureUrls,
}

/// Picture URLs at different sizes.
#[derive(Debug, Clone, Deserialize)]
pub struct PictureUrls {
    /// Original full-size URL.
    pub original: String,
    /// Small thumbnail URL.
    pub small: String,
    /// Compact thumbnail URL.
    pub compact: String,
}

/// Paginated response wrapper for reviews.
#[derive(Debug, Clone, Deserialize)]
pub struct ReviewsResponse {
    /// List of reviews on this page.
    pub reviews: Vec<Review>,
    /// Current page number (1-indexed).
    pub current_page: i32,
    /// Number of reviews per page.
    pub per_page: i32,
}

/// Judge.me internal product representation.
#[derive(Debug, Clone, Deserialize)]
pub struct JudgemeProduct {
    /// Judge.me internal product ID.
    pub id: i64,
    /// Shopify product external ID.
    pub external_id: Option<i64>,
    /// Product handle.
    pub handle: Option<String>,
}

/// Response wrapper for product lookup.
#[derive(Debug, Clone, Deserialize)]
pub struct ProductResponse {
    /// The product data.
    pub product: JudgemeProduct,
}

/// Parameters for creating a new review.
#[derive(Debug, Serialize)]
pub struct CreateReviewParams {
    /// Shop domain (e.g., "store.myshopify.com").
    pub shop_domain: String,
    /// E-commerce platform identifier.
    pub platform: String,
    /// Reviewer's name.
    pub name: String,
    /// Reviewer's email.
    pub email: String,
    /// Star rating (1-5).
    pub rating: i32,
    /// Review title (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Review body text.
    pub body: String,
    /// Shopify product ID (numeric, not GID).
    pub id: i64,
}

/// Parameters for moderating a review.
#[derive(Debug, Serialize)]
pub struct ModerateReviewParams {
    /// New moderation status: "ok" or "spam".
    pub curated: String,
}

/// Parameters for creating a reply to a review.
#[derive(Debug, Serialize)]
pub struct CreateReplyParams {
    /// Body of the reply.
    pub body: String,
    /// Whether to send a notification email to the reviewer.
    pub send_reply_email: bool,
}
