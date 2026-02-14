//! Review moderation route handlers.
//!
//! Provides a moderation interface for Judge.me product reviews.

use askama::Template;
use axum::{
    Form, Router,
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use serde::Deserialize;
use tracing::{instrument, warn};

use naked_pineapple_services::judgeme::types::Review;

use crate::{error::AppError, filters, middleware::auth::RequireAdminAuth, state::AppState};

use super::dashboard::AdminUserView;

// =============================================================================
// Query / Form Types
// =============================================================================

/// Query parameters for the review list page.
#[derive(Debug, Deserialize)]
pub struct ReviewsQuery {
    /// Filter by moderation status: "pending", "approved", "spam".
    pub status: Option<String>,
    /// Page number (1-indexed).
    pub page: Option<i32>,
}

/// Form data for replying to a review.
#[derive(Debug, Deserialize)]
pub struct ReplyForm {
    pub body: String,
}

// =============================================================================
// View Types
// =============================================================================

/// A review formatted for admin display.
struct ReviewView {
    id: i64,
    title: String,
    body: String,
    rating: i32,
    reviewer_name: String,
    reviewer_email: String,
    curated: String,
    status_label: String,
    verified: bool,
    created_at: String,
    product_title: String,
    product_handle: String,
    pictures: Vec<String>,
}

// =============================================================================
// Templates
// =============================================================================

/// Review list page template.
#[derive(Template)]
#[template(path = "reviews/index.html")]
struct ReviewsIndexTemplate {
    admin_user: AdminUserView,
    current_path: String,
    reviews: Vec<ReviewView>,
    active_tab: String,
    current_page: i32,
    has_more: bool,
}

/// Review detail page template.
#[derive(Template)]
#[template(path = "reviews/show.html")]
struct ReviewShowTemplate {
    admin_user: AdminUserView,
    current_path: String,
    review: ReviewView,
}

// =============================================================================
// Conversions
// =============================================================================

fn build_review_view(review: &Review) -> ReviewView {
    let status_label = match review.curated.as_str() {
        "ok" => "Approved".to_string(),
        "spam" => "Spam".to_string(),
        _ => "Pending".to_string(),
    };

    ReviewView {
        id: review.id,
        title: review
            .title
            .clone()
            .unwrap_or_else(|| "(No title)".to_string()),
        body: review
            .body
            .clone()
            .unwrap_or_else(|| "(No body)".to_string()),
        rating: review.rating,
        reviewer_name: review
            .reviewer
            .name
            .clone()
            .unwrap_or_else(|| "Anonymous".to_string()),
        reviewer_email: review.reviewer.email.clone(),
        curated: review.curated.clone(),
        status_label,
        verified: review.verified == "buyer",
        created_at: review.created_at.clone(),
        product_title: review
            .product_title
            .clone()
            .unwrap_or_else(|| "Unknown Product".to_string()),
        product_handle: review.product_handle.clone().unwrap_or_default(),
        pictures: review
            .pictures
            .iter()
            .map(|p| p.urls.small.clone())
            .collect(),
    }
}

/// Filter reviews by moderation status tab.
fn filter_reviews(reviews: &[Review], status: &str) -> Vec<ReviewView> {
    reviews
        .iter()
        .filter(|r| match status {
            "approved" => r.curated == "ok",
            "spam" => r.curated == "spam",
            // "pending" — everything not yet moderated
            _ => r.curated != "ok" && r.curated != "spam",
        })
        .map(build_review_view)
        .collect()
}

// =============================================================================
// Router
// =============================================================================

/// Build review moderation routes.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/reviews", get(index))
        .route("/reviews/{id}", get(show))
        .route("/reviews/{id}/approve", post(approve))
        .route("/reviews/{id}/reject", post(reject))
        .route("/reviews/{id}/reply", post(reply))
}

// =============================================================================
// Handlers
// =============================================================================

/// Review list page with filter tabs.
#[instrument(skip_all, fields(admin_id = %admin.id.as_i32()))]
async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<ReviewsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let judgeme = state
        .judgeme()
        .ok_or_else(|| AppError::BadRequest("Judge.me is not configured".to_string()))?;

    let page = query.page.unwrap_or(1);
    let active_tab = query.status.unwrap_or_else(|| "pending".to_string());

    let response = judgeme
        .get_all_reviews(page, 50)
        .await
        .map_err(|e| AppError::Internal(format!("Judge.me API error: {e}")))?;

    let reviews = filter_reviews(&response.reviews, &active_tab);
    let per_page: usize = response.per_page.try_into().unwrap_or(50);
    let has_more = response.reviews.len() >= per_page;

    let template = ReviewsIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/reviews".to_string(),
        reviews,
        active_tab,
        current_page: page,
        has_more,
    };

    Ok(Html(
        template
            .render()
            .map_err(|e| AppError::Internal(e.to_string()))?,
    ))
}

/// Review detail page.
#[instrument(skip_all, fields(admin_id = %admin.id.as_i32(), review_id = id))]
async fn show(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let judgeme = state
        .judgeme()
        .ok_or_else(|| AppError::BadRequest("Judge.me is not configured".to_string()))?;

    // Fetch all reviews and find the one with the matching ID.
    // Judge.me doesn't have a single-review endpoint, so we paginate.
    let review = find_review_by_id(judgeme, id).await?;

    let template = ReviewShowTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/reviews".to_string(),
        review: build_review_view(&review),
    };

    Ok(Html(
        template
            .render()
            .map_err(|e| AppError::Internal(e.to_string()))?,
    ))
}

/// Approve a review (set curated = "ok").
#[instrument(skip_all, fields(admin_id = %admin.id.as_i32(), review_id = id))]
async fn approve(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let judgeme = state
        .judgeme()
        .ok_or_else(|| AppError::BadRequest("Judge.me is not configured".to_string()))?;

    judgeme
        .moderate_review(id, "ok")
        .await
        .map_err(|e| AppError::Internal(format!("Failed to approve review: {e}")))?;

    Ok(Redirect::to(&format!("/reviews/{id}")))
}

/// Reject a review (set curated = "spam").
#[instrument(skip_all, fields(admin_id = %admin.id.as_i32(), review_id = id))]
async fn reject(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<impl IntoResponse, AppError> {
    let judgeme = state
        .judgeme()
        .ok_or_else(|| AppError::BadRequest("Judge.me is not configured".to_string()))?;

    judgeme
        .moderate_review(id, "spam")
        .await
        .map_err(|e| AppError::Internal(format!("Failed to reject review: {e}")))?;

    Ok(Redirect::to(&format!("/reviews/{id}")))
}

/// Reply to a review.
#[instrument(skip_all, fields(admin_id = %admin.id.as_i32(), review_id = id))]
async fn reply(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<ReplyForm>,
) -> Result<impl IntoResponse, AppError> {
    let judgeme = state
        .judgeme()
        .ok_or_else(|| AppError::BadRequest("Judge.me is not configured".to_string()))?;

    if form.body.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Reply body cannot be empty".to_string(),
        ));
    }

    judgeme
        .create_reply(id, &form.body, true)
        .await
        .map_err(|e| AppError::Internal(format!("Failed to create reply: {e}")))?;

    Ok(Redirect::to(&format!("/reviews/{id}")))
}

// =============================================================================
// Helpers
// =============================================================================

/// Search through paginated reviews to find one by ID.
async fn find_review_by_id(
    judgeme: &naked_pineapple_services::judgeme::JudgemeClient,
    review_id: i64,
) -> Result<Review, AppError> {
    // Search through pages to find the review
    for page in 1..=10 {
        let response = judgeme
            .get_all_reviews(page, 50)
            .await
            .map_err(|e| AppError::Internal(format!("Judge.me API error: {e}")))?;

        if let Some(review) = response.reviews.into_iter().find(|r| r.id == review_id) {
            return Ok(review);
        }

        // No more pages
        if (response.current_page * response.per_page) >= 500 {
            break;
        }
    }

    Err(AppError::NotFound(format!("Review {review_id} not found")))
}
