//! Product review route handlers.

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    Form,
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;
use tracing::{debug, instrument, warn};

use naked_pineapple_core::extract_shopify_numeric_id;

use crate::state::AppState;

/// Query parameters for fetching reviews.
#[derive(Debug, Deserialize)]
pub struct ReviewsQuery {
    pub page: Option<i32>,
}

/// Form data for submitting a review.
#[derive(Debug, Deserialize)]
pub struct ReviewSubmission {
    pub name: String,
    pub email: String,
    pub rating: i32,
    pub title: Option<String>,
    pub body: String,
}

/// A single review for template display.
#[derive(Clone)]
pub struct ReviewItemView {
    pub reviewer_name: String,
    pub rating: i32,
    pub full_stars: u8,
    pub empty_stars: u8,
    pub title: String,
    pub body: String,
    pub verified: bool,
    pub date: String,
    pub pictures: Vec<ReviewPictureView>,
}

/// A review picture for template display.
#[derive(Clone)]
pub struct ReviewPictureView {
    pub compact_url: String,
    pub original_url: String,
}

/// A single row in the star distribution bar chart.
#[derive(Clone)]
pub struct StarBar {
    pub stars: i32,
    pub percent: f64,
}

/// Rating distribution data for the summary bar chart.
#[derive(Clone, Default)]
pub struct RatingDistribution {
    pub bars: Vec<StarBar>,
    pub total_reviews: usize,
    pub average_rating: f64,
    pub full_stars: u8,
    pub has_half_star: bool,
    pub empty_stars: u8,
}

/// Reviews fragment template (loaded via HTMX).
#[derive(Template, WebTemplate)]
#[template(path = "products/_reviews.html")]
pub struct ReviewsFragmentTemplate {
    pub handle: String,
    pub reviews: Vec<ReviewItemView>,
    pub distribution: RatingDistribution,
    pub current_page: i32,
    pub has_next_page: bool,
    pub has_reviews: bool,
}

/// Success message after review submission.
#[derive(Template, WebTemplate)]
#[template(path = "products/_review_submitted.html")]
pub struct ReviewSubmittedTemplate {}

/// Display reviews for a product (HTMX fragment).
#[instrument(skip(state))]
pub async fn product_reviews(
    State(state): State<AppState>,
    Path(handle): Path<String>,
    Query(query): Query<ReviewsQuery>,
) -> Response {
    let page = query.page.unwrap_or(1);
    let per_page = 10;

    let Some(judgeme) = state.judgeme() else {
        return empty_reviews_fragment(&handle);
    };

    // Fetch product from Shopify to get its numeric ID
    let shopify_product = match state.storefront().get_product_by_handle(&handle).await {
        Ok(p) => p,
        Err(e) => {
            warn!(handle = %handle, error = %e, "Failed to fetch product for reviews");
            return empty_reviews_fragment(&handle);
        }
    };

    let Some(shopify_numeric_id) = extract_shopify_numeric_id(&shopify_product.id) else {
        warn!(handle = %handle, gid = %shopify_product.id, "Failed to extract numeric ID from GID");
        return empty_reviews_fragment(&handle);
    };

    // Resolve Judge.me product ID (cached for 24 hours)
    let cache = state.judgeme_product_id_cache();
    let cache_key = shopify_numeric_id.to_string();

    let judgeme_product_id = if let Some(cached) = cache.get(&cache_key).await {
        cached
    } else {
        match judgeme.resolve_product_id(shopify_numeric_id).await {
            Ok(id) => {
                cache.insert(cache_key, id).await;
                id
            }
            Err(e) => {
                debug!(handle = %handle, error = %e, "No Judge.me product found");
                return empty_reviews_fragment(&handle);
            }
        }
    };

    // Fetch reviews (cached for 5 minutes)
    let reviews_cache = state.judgeme_reviews_cache();
    let reviews_cache_key = format!("{judgeme_product_id}:{page}:{per_page}");

    let reviews_response = if let Some(cached) = reviews_cache.get(&reviews_cache_key).await {
        cached
    } else {
        match judgeme
            .get_reviews(judgeme_product_id, page, per_page)
            .await
        {
            Ok(r) => {
                reviews_cache.insert(reviews_cache_key, r.clone()).await;
                r
            }
            Err(e) => {
                warn!(handle = %handle, error = %e, "Failed to fetch reviews from Judge.me");
                return empty_reviews_fragment(&handle);
            }
        }
    };

    // Filter to approved, non-hidden reviews
    let approved_reviews: Vec<_> = reviews_response
        .reviews
        .iter()
        .filter(|r| r.curated == "ok" && !r.hidden)
        .collect();

    // Build rating distribution
    let distribution = build_distribution(&approved_reviews);

    // Convert to view types
    let review_views: Vec<ReviewItemView> = approved_reviews
        .iter()
        .map(|r| build_review_view(r))
        .collect();

    let per_page_usize: usize = per_page.try_into().unwrap_or(10);
    let has_next_page = reviews_response.reviews.len() >= per_page_usize;

    ReviewsFragmentTemplate {
        handle,
        reviews: review_views,
        distribution,
        current_page: page,
        has_next_page,
        has_reviews: !approved_reviews.is_empty(),
    }
    .into_response()
}

/// Submit a new review for a product.
#[instrument(skip(state, form))]
pub async fn submit_review(
    State(state): State<AppState>,
    Path(handle): Path<String>,
    Form(form): Form<ReviewSubmission>,
) -> Response {
    // Validate inputs
    if form.rating < 1 || form.rating > 5 {
        return Html("<p class=\"text-red-600 text-sm\">Rating must be between 1 and 5.</p>")
            .into_response();
    }
    if form.name.trim().is_empty() || form.email.trim().is_empty() || form.body.trim().is_empty() {
        return Html("<p class=\"text-red-600 text-sm\">Please fill in all required fields.</p>")
            .into_response();
    }

    let Some(judgeme) = state.judgeme() else {
        return Html("<p class=\"text-red-600 text-sm\">Reviews are currently unavailable.</p>")
            .into_response();
    };

    // Get the Shopify product numeric ID
    let shopify_product = match state.storefront().get_product_by_handle(&handle).await {
        Ok(p) => p,
        Err(e) => {
            warn!(handle = %handle, error = %e, "Failed to fetch product for review submission");
            return Html(
                "<p class=\"text-red-600 text-sm\">Something went wrong. Please try again.</p>",
            )
            .into_response();
        }
    };

    let Some(shopify_numeric_id) = extract_shopify_numeric_id(&shopify_product.id) else {
        return Html(
            "<p class=\"text-red-600 text-sm\">Something went wrong. Please try again.</p>",
        )
        .into_response();
    };

    let params = naked_pineapple_services::judgeme::types::CreateReviewParams {
        shop_domain: state.config().shopify.store.clone(),
        platform: "shopify".to_string(),
        name: form.name.trim().to_string(),
        email: form.email.trim().to_string(),
        rating: form.rating,
        title: form
            .title
            .as_deref()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty()),
        body: form.body.trim().to_string(),
        id: shopify_numeric_id,
    };

    if let Err(e) = judgeme.create_review(&params).await {
        warn!(handle = %handle, error = %e, "Failed to submit review to Judge.me");
        return Html(
            "<p class=\"text-red-600 text-sm\">Something went wrong. Please try again.</p>",
        )
        .into_response();
    }

    ReviewSubmittedTemplate {}.into_response()
}

/// Return an empty reviews fragment for graceful degradation.
fn empty_reviews_fragment(handle: &str) -> Response {
    ReviewsFragmentTemplate {
        handle: handle.to_string(),
        reviews: Vec::new(),
        distribution: RatingDistribution::default(),
        current_page: 1,
        has_next_page: false,
        has_reviews: false,
    }
    .into_response()
}

/// Build a `ReviewItemView` from a Judge.me review.
fn build_review_view(review: &naked_pineapple_services::judgeme::types::Review) -> ReviewItemView {
    let rating = review.rating.clamp(1, 5);
    let full_stars = u8::try_from(rating).unwrap_or(5);
    let empty_stars = 5 - full_stars;

    let date = review
        .created_at
        .split('T')
        .next()
        .unwrap_or(&review.created_at)
        .to_string();

    ReviewItemView {
        reviewer_name: review
            .reviewer
            .name
            .clone()
            .unwrap_or_else(|| "Anonymous".to_string()),
        rating,
        full_stars,
        empty_stars,
        title: review.title.clone().unwrap_or_default(),
        body: review.body.clone().unwrap_or_default(),
        verified: review.verified == "buyer",
        date,
        pictures: review
            .pictures
            .iter()
            .map(|p| ReviewPictureView {
                compact_url: p.urls.compact.clone(),
                original_url: p.urls.original.clone(),
            })
            .collect(),
    }
}

/// Build rating distribution from a list of approved reviews.
fn build_distribution(
    reviews: &[&naked_pineapple_services::judgeme::types::Review],
) -> RatingDistribution {
    if reviews.is_empty() {
        return RatingDistribution::default();
    }

    let total = reviews.len();
    let mut counts = [0usize; 5]; // index 0 = 1-star, index 4 = 5-star

    let mut sum = 0i64;
    for review in reviews {
        let rating = review.rating.clamp(1, 5);
        if let Some(bucket) = counts.get_mut(usize::try_from(rating - 1).unwrap_or(0)) {
            *bucket += 1;
        }
        sum += i64::from(rating);
    }

    #[expect(clippy::cast_precision_loss, reason = "review counts are small")]
    let total_f = total as f64;
    #[expect(clippy::cast_precision_loss, reason = "review counts are small")]
    let average = sum as f64 / total_f;

    let rounded = (average * 2.0).round() / 2.0;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "average is clamped to 0..=5"
    )]
    let full_stars = rounded.floor() as u8;
    let has_half_star = (rounded - rounded.floor()) >= 0.5;
    let empty_stars = 5 - full_stars - u8::from(has_half_star);

    #[expect(clippy::cast_precision_loss, reason = "review counts are small")]
    let percentages: Vec<f64> = counts
        .iter()
        .map(|&c| (c as f64 / total_f) * 100.0)
        .collect();
    let bars = (1..=5_i32)
        .rev()
        .map(|s| {
            let idx = usize::try_from(s - 1).unwrap_or(0);
            StarBar {
                stars: s,
                percent: percentages.get(idx).copied().unwrap_or(0.0),
            }
        })
        .collect();

    RatingDistribution {
        bars,
        total_reviews: total,
        average_rating: (average * 10.0).round() / 10.0,
        full_stars,
        has_half_star,
        empty_stars,
    }
}

#[cfg(test)]
mod tests {
    use naked_pineapple_services::judgeme::types::{Review, Reviewer};

    use super::*;

    fn make_review(rating: i32) -> Review {
        Review {
            id: 1,
            title: Some("Great".to_string()),
            body: Some("Love it".to_string()),
            rating,
            reviewer: Reviewer {
                id: 1,
                email: "test@example.com".to_string(),
                name: Some("Test User".to_string()),
            },
            curated: "ok".to_string(),
            verified: "buyer".to_string(),
            hidden: false,
            created_at: "2025-01-15T10:30:00Z".to_string(),
            pictures: Vec::new(),
            product_external_id: None,
            product_title: None,
            product_handle: None,
        }
    }

    #[test]
    fn review_view_star_computation() {
        let review = make_review(4);
        let view = build_review_view(&review);
        assert_eq!(view.full_stars, 4);
        assert_eq!(view.empty_stars, 1);
        assert_eq!(view.rating, 4);
        assert!(view.verified);
    }

    #[test]
    fn review_view_clamps_out_of_range() {
        let view = build_review_view(&make_review(10));
        assert_eq!(view.full_stars, 5);
        assert_eq!(view.empty_stars, 0);

        let view = build_review_view(&make_review(0));
        assert_eq!(view.full_stars, 1);
        assert_eq!(view.empty_stars, 4);
    }

    #[test]
    fn review_view_anonymous_reviewer() {
        let mut review = make_review(3);
        review.reviewer.name = None;
        let view = build_review_view(&review);
        assert_eq!(view.reviewer_name, "Anonymous");
    }

    #[test]
    fn review_view_not_verified() {
        let mut review = make_review(5);
        review.verified = "not_buyer".to_string();
        let view = build_review_view(&review);
        assert!(!view.verified);
    }

    #[test]
    fn review_view_date_extraction() {
        let review = make_review(5);
        let view = build_review_view(&review);
        assert_eq!(view.date, "2025-01-15");
    }

    #[test]
    fn distribution_empty() {
        let dist = build_distribution(&[]);
        assert_eq!(dist.total_reviews, 0);
        assert!((dist.average_rating - 0.0).abs() < f64::EPSILON);
        assert!(dist.bars.is_empty());
    }

    #[test]
    fn distribution_all_five_stars() {
        let reviews = [make_review(5), make_review(5), make_review(5)];
        let refs: Vec<&Review> = reviews.iter().collect();
        let dist = build_distribution(&refs);

        assert_eq!(dist.total_reviews, 3);
        assert!((dist.average_rating - 5.0).abs() < f64::EPSILON);
        assert_eq!(dist.full_stars, 5);
        assert!(!dist.has_half_star);
        assert_eq!(dist.empty_stars, 0);

        // 5-star bar should be 100%
        let bar_5 = dist.bars.first().expect("should have bars");
        assert_eq!(bar_5.stars, 5);
        assert!((bar_5.percent - 100.0).abs() < f64::EPSILON);
    }

    #[test]
    fn distribution_mixed_ratings() {
        let reviews = [
            make_review(5),
            make_review(5),
            make_review(4),
            make_review(3),
        ];
        let refs: Vec<&Review> = reviews.iter().collect();
        let dist = build_distribution(&refs);

        assert_eq!(dist.total_reviews, 4);
        // Average: (5+5+4+3)/4 = 17/4 = 4.25 -> rounded to 1 decimal = 4.3
        assert!((dist.average_rating - 4.3).abs() < f64::EPSILON);
        // 4.25 rounded to nearest 0.5 = 4.5 -> full_stars=4, half=true, empty=0
        assert_eq!(dist.full_stars, 4);
        assert!(dist.has_half_star);
        assert_eq!(dist.empty_stars, 0);

        // Bars should be 5-star down to 1-star
        assert_eq!(dist.bars.len(), 5);
        let bar_5 = dist.bars.first().expect("bar 5");
        let bar_4 = dist.bars.get(1).expect("bar 4");
        let bar_3 = dist.bars.get(2).expect("bar 3");
        assert_eq!(bar_5.stars, 5);
        assert!((bar_5.percent - 50.0).abs() < f64::EPSILON); // 2/4
        assert_eq!(bar_4.stars, 4);
        assert!((bar_4.percent - 25.0).abs() < f64::EPSILON); // 1/4
        assert_eq!(bar_3.stars, 3);
        assert!((bar_3.percent - 25.0).abs() < f64::EPSILON); // 1/4
    }

    #[test]
    fn distribution_single_review() {
        let reviews = [make_review(3)];
        let refs: Vec<&Review> = reviews.iter().collect();
        let dist = build_distribution(&refs);

        assert_eq!(dist.total_reviews, 1);
        assert!((dist.average_rating - 3.0).abs() < f64::EPSILON);
        assert_eq!(dist.full_stars, 3);
        assert!(!dist.has_half_star);
        assert_eq!(dist.empty_stars, 2);
    }
}
