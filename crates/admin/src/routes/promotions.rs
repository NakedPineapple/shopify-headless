//! Promotions management route handlers.
//!
//! Manages which automatic discounts are surfaced on the storefront cart page
//! via the `custom.active_promotions` shop metafield.

use askama::Template;
use axum::{
    Form,
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use serde::Deserialize;
use tracing::{debug, info, instrument, warn};

use crate::{
    filters,
    middleware::auth::RequireAdminAuth,
    shopify::{
        ActivePromotions, ActivePromotionsWithDigest, AutomaticDiscount, ProgressTracking,
        PromotionBanner,
    },
    state::AppState,
};

use super::dashboard::AdminUserView;

// =============================================================================
// View Types
// =============================================================================

/// View for displaying an automatic discount with configuration status.
#[derive(Debug, Clone)]
pub struct AutomaticDiscountView {
    /// Discount node ID (GID).
    pub id: String,
    /// Short ID for forms (numeric portion).
    pub short_id: String,
    /// Display title.
    pub title: String,
    /// Discount type label.
    pub discount_type: String,
    /// Status badge.
    pub status: String,
    /// Status CSS class.
    pub status_class: String,
    /// Value description.
    pub value_description: String,
    /// Minimum requirement description.
    pub minimum_description: Option<String>,
    /// Usage count.
    pub usage_count: i64,
    /// Start date.
    pub starts_at: Option<String>,
    /// End date.
    pub ends_at: Option<String>,
    /// Whether this discount has a banner configured.
    pub has_banner: bool,
    /// Whether this discount has progress tracking configured.
    pub has_progress_tracking: bool,
}

impl AutomaticDiscountView {
    /// Create from an `AutomaticDiscount` with configuration status.
    fn from_discount(discount: &AutomaticDiscount, config: &ActivePromotions) -> Self {
        let short_id = discount
            .id
            .split('/')
            .next_back()
            .unwrap_or(&discount.id)
            .to_string();

        let (status, status_class) = match discount.status.as_str() {
            "ACTIVE" => (
                "Active",
                "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400",
            ),
            "SCHEDULED" => (
                "Scheduled",
                "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400",
            ),
            _ => (
                "Expired",
                "bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-400",
            ),
        };

        let has_banner = config.banners.iter().any(|b| b.id == short_id);
        let has_progress_tracking = config.progress_tracking.iter().any(|p| p.id == short_id);

        Self {
            id: discount.id.clone(),
            short_id,
            title: discount.title.clone(),
            discount_type: discount.discount_type.label().to_string(),
            status: status.to_string(),
            status_class: status_class.to_string(),
            value_description: discount.value_description.clone(),
            minimum_description: discount.minimum_description.clone(),
            usage_count: discount.usage_count,
            starts_at: discount.starts_at.clone(),
            ends_at: discount.ends_at.clone(),
            has_banner,
            has_progress_tracking,
        }
    }
}

// =============================================================================
// Templates
// =============================================================================

/// Promotions list page template.
#[derive(Template)]
#[template(path = "promotions/index.html")]
pub struct PromotionsIndexTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub discounts: Vec<AutomaticDiscountView>,
    pub current_config: ActivePromotions,
    pub error: Option<String>,
    pub success: Option<String>,
}

/// Promotion configuration form template.
#[derive(Template)]
#[template(path = "promotions/configure.html")]
pub struct PromotionConfigureTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub discount: AutomaticDiscountView,
    pub banner: Option<PromotionBanner>,
    pub progress_tracking: Option<ProgressTracking>,
    pub error: Option<String>,
}

// =============================================================================
// Form Types
// =============================================================================

/// Form for configuring a promotion banner (display only).
#[derive(Debug, Deserialize)]
pub struct BannerConfigForm {
    /// Discount short ID.
    pub discount_id: String,
    /// Whether to enable the banner.
    pub enable_banner: Option<bool>,
    /// Banner title.
    pub banner_title: Option<String>,
    /// Banner description.
    pub banner_description: Option<String>,
    /// Badge text.
    pub badge_text: Option<String>,
    /// Icon name.
    pub icon: Option<String>,
    /// Accent color.
    pub accent_color: Option<String>,
    /// CTA button text.
    pub cta_text: Option<String>,
    /// CTA button URL.
    pub cta_url: Option<String>,
    /// Priority (lower = higher).
    pub priority: Option<i32>,
}

/// Form for configuring progress tracking display (display only).
#[derive(Debug, Deserialize)]
pub struct ProgressTrackingConfigForm {
    /// Discount short ID.
    pub discount_id: String,
    /// Whether to enable progress tracking.
    pub enable_progress_tracking: Option<bool>,
    /// Icon name.
    pub icon: Option<String>,
    /// Accent color.
    pub accent_color: Option<String>,
    /// CTA button text.
    pub cta_text: Option<String>,
    /// CTA button URL.
    pub cta_url: Option<String>,
    /// Suggestion template message.
    pub suggestion_template: Option<String>,
    /// Badge text above suggestion.
    pub suggestion_badge_text: Option<String>,
    /// Qualified template message.
    pub qualified_template: Option<String>,
    /// Badge text above qualified message.
    pub qualified_badge_text: Option<String>,
    /// Priority (lower = higher).
    pub priority: Option<i32>,
    /// Whether to show progress bar.
    pub show_progress_bar: Option<bool>,
    /// Whether to hide when qualified.
    pub hide_when_qualified: Option<bool>,
}

// =============================================================================
// Handlers
// =============================================================================

/// Promotions list page handler.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
) -> Html<String> {
    debug!("Loading promotions management page");

    // Fetch automatic discounts and current config concurrently
    let (discounts_result, config_result) = tokio::join!(
        state.shopify().get_automatic_discounts(),
        state.shopify().get_active_promotions()
    );

    let discounts = discounts_result.unwrap_or_else(|e| {
        warn!(error = %e, "Failed to fetch automatic discounts");
        Vec::new()
    });

    let config = config_result
        .unwrap_or_else(|e| {
            warn!(error = %e, "Failed to fetch promotions config");
            None
        })
        .unwrap_or_default();

    let discount_views: Vec<AutomaticDiscountView> = discounts
        .iter()
        .map(|d| AutomaticDiscountView::from_discount(d, &config))
        .collect();

    let template = PromotionsIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/promotions".to_string(),
        discounts: discount_views,
        current_config: config,
        error: None,
        success: None,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// Configure a specific promotion.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn configure(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    debug!(discount_id = %id, "Loading promotion configuration");

    // Fetch automatic discounts and current config
    let (discounts_result, config_result) = tokio::join!(
        state.shopify().get_automatic_discounts(),
        state.shopify().get_active_promotions()
    );

    let discounts = discounts_result.unwrap_or_default();
    let config = config_result.unwrap_or(None).unwrap_or_default();

    // Find the discount
    let discount = discounts
        .iter()
        .find(|d| d.id.ends_with(&format!("/{id}")) || d.id == id);

    let Some(discount) = discount else {
        return (StatusCode::NOT_FOUND, "Discount not found").into_response();
    };

    let discount_view = AutomaticDiscountView::from_discount(discount, &config);
    let short_id = discount_view.short_id.clone();

    // Find existing banner and progress tracking configs
    let banner = config.banners.iter().find(|b| b.id == short_id).cloned();
    let progress_tracking = config
        .progress_tracking
        .iter()
        .find(|p| p.id == short_id)
        .cloned();

    let template = PromotionConfigureTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/promotions".to_string(),
        discount: discount_view,
        banner,
        progress_tracking,
        error: None,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
    .into_response()
}

/// Save banner configuration.
///
/// This saves display options and auto-populates the qualifying rule from the Shopify API.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn save_banner(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(form): Form<BannerConfigForm>,
) -> impl IntoResponse {
    debug!(discount_id = %form.discount_id, "Saving banner configuration");

    // Get current config with compareDigest for optimistic concurrency
    let ActivePromotionsWithDigest {
        promotions: mut config,
        compare_digest,
    } = state
        .shopify()
        .get_active_promotions_with_digest()
        .await
        .unwrap_or_default();

    // Remove existing banner for this discount
    config.banners.retain(|b| b.id != form.discount_id);

    // Remove existing qualifying rule for this discount (will re-add if banner enabled)
    config.qualifying_rules.retain(|r| r.id != form.discount_id);

    // Add new banner if enabled
    if form.enable_banner.unwrap_or(false) {
        // Extract rule data from Shopify API
        let rule_data = state
            .shopify()
            .extract_rule_data_from_discount(&form.discount_id)
            .await;

        match rule_data {
            Ok(Some(extracted)) => {
                // Add banner display config
                let banner = PromotionBanner {
                    id: form.discount_id.clone(),
                    title: form.banner_title.unwrap_or_default(),
                    description: form.banner_description.filter(|s| !s.is_empty()),
                    badge_text: form.badge_text.filter(|s| !s.is_empty()),
                    icon: form.icon.unwrap_or_else(|| "gift".to_string()),
                    accent_color: form.accent_color.unwrap_or_else(|| "honey".to_string()),
                    cta_text: form.cta_text.filter(|s| !s.is_empty()),
                    cta_url: form.cta_url.filter(|s| !s.is_empty()),
                    priority: form.priority.unwrap_or(0),
                };
                config.banners.push(banner);

                // Add qualifying rule from API data
                let qualifying_rule = extracted.into_qualifying_rule(form.discount_id.clone());
                config.qualifying_rules.push(qualifying_rule);
            }
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Html(format!(
                        "Discount {} not found in Shopify",
                        form.discount_id
                    )),
                )
                    .into_response();
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Html(format!("Failed to fetch discount data from Shopify: {e}")),
                )
                    .into_response();
            }
        }
    }

    // Save config with compareDigest for optimistic concurrency
    match state
        .shopify()
        .set_active_promotions(&config, compare_digest)
        .await
    {
        Ok(()) => {
            info!(discount_id = %form.discount_id, "Banner configuration saved");
            Redirect::to(&format!("/promotions/{}/configure", form.discount_id)).into_response()
        }
        Err(e) => {
            tracing::error!(discount_id = %form.discount_id, error = %e, "Failed to save banner");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("Error saving configuration: {e}")),
            )
                .into_response()
        }
    }
}

/// Save progress tracking configuration.
///
/// This saves display options and auto-populates the qualifying rule from the Shopify API.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn save_progress_tracking(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(form): Form<ProgressTrackingConfigForm>,
) -> impl IntoResponse {
    debug!(discount_id = %form.discount_id, "Saving progress tracking configuration");

    // Get current config with compareDigest for optimistic concurrency
    let ActivePromotionsWithDigest {
        promotions: mut config,
        compare_digest,
    } = state
        .shopify()
        .get_active_promotions_with_digest()
        .await
        .unwrap_or_default();

    // Remove existing progress tracking and qualifying rule for this discount
    config
        .progress_tracking
        .retain(|p| p.id != form.discount_id);
    config.qualifying_rules.retain(|r| r.id != form.discount_id);

    // Add new progress tracking if enabled
    if form.enable_progress_tracking.unwrap_or(false) {
        // Extract rule data from Shopify API
        let rule_data = state
            .shopify()
            .extract_rule_data_from_discount(&form.discount_id)
            .await;

        match rule_data {
            Ok(Some(extracted)) => {
                // Add progress tracking display config
                let progress_tracking = ProgressTracking {
                    id: form.discount_id.clone(),
                    icon: form.icon.unwrap_or_else(|| "gift".to_string()),
                    accent_color: form.accent_color.unwrap_or_else(|| "honey".to_string()),
                    cta_text: form.cta_text.filter(|s| !s.is_empty()),
                    cta_url: form.cta_url.filter(|s| !s.is_empty()),
                    suggestion_template: form
                        .suggestion_template
                        .unwrap_or_else(|| "Add {needed} more to qualify!".to_string()),
                    suggestion_badge_text: form.suggestion_badge_text.filter(|s| !s.is_empty()),
                    qualified_template: form
                        .qualified_template
                        .unwrap_or_else(|| "You qualify!".to_string()),
                    qualified_badge_text: form.qualified_badge_text.filter(|s| !s.is_empty()),
                    priority: form.priority.unwrap_or(0),
                    show_progress_bar: form.show_progress_bar.unwrap_or(true),
                    hide_when_qualified: form.hide_when_qualified.unwrap_or(false),
                };
                config.progress_tracking.push(progress_tracking);

                // Add qualifying rule from API data
                let qualifying_rule = extracted.into_qualifying_rule(form.discount_id.clone());
                config.qualifying_rules.push(qualifying_rule);
            }
            Ok(None) => {
                return (
                    StatusCode::NOT_FOUND,
                    Html(format!(
                        "Discount {} not found in Shopify",
                        form.discount_id
                    )),
                )
                    .into_response();
            }
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Html(format!("Failed to fetch discount data from Shopify: {e}")),
                )
                    .into_response();
            }
        }
    }

    // Save config with compareDigest for optimistic concurrency
    match state
        .shopify()
        .set_active_promotions(&config, compare_digest)
        .await
    {
        Ok(()) => {
            info!(discount_id = %form.discount_id, "Progress tracking configuration saved");
            Redirect::to(&format!("/promotions/{}/configure", form.discount_id)).into_response()
        }
        Err(e) => {
            tracing::error!(discount_id = %form.discount_id, error = %e, "Failed to save progress tracking");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("Error saving configuration: {e}")),
            )
                .into_response()
        }
    }
}

/// Build the promotions router.
pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/promotions", axum::routing::get(index))
        .route("/promotions/{id}/configure", axum::routing::get(configure))
        .route("/promotions/banner", axum::routing::post(save_banner))
        .route(
            "/promotions/progress-tracking",
            axum::routing::post(save_progress_tracking),
        )
}
