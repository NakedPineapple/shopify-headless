//! Promotions management route handlers.
//!
//! Manages which automatic discounts are surfaced on the storefront cart page
//! via the `custom.active_promotions` shop metafield, and related product
//! recommendations via the `custom.cart_recommendations` metafield.

use askama::Template;
use axum::{
    Form,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use serde::Deserialize;
use tracing::{debug, info, instrument, warn};

use crate::{
    filters,
    middleware::auth::RequireAdminAuth,
    shopify::{
        ActivePromotions, ActivePromotionsWithDigest, AutomaticDiscount, CartRecommendations,
        CartRecommendationsWithDigest, ProductRelation, ProgressTracking, PromotionBanner,
        RelatedProduct, ShopifyRecommendation,
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

/// View for displaying a product with its related products configuration.
#[derive(Debug, Clone)]
pub struct ProductWithRelatedView {
    /// Shopify product GID.
    pub id: String,
    /// Short ID for URLs (numeric portion).
    pub short_id: String,
    /// Product title.
    pub title: String,
    /// Product image URL.
    pub image_url: Option<String>,
    /// Number of related products configured.
    pub related_count: usize,
    /// Related products (for display).
    pub related_products: Vec<RelatedProductView>,
}

/// View for displaying a related product.
#[derive(Debug, Clone)]
pub struct RelatedProductView {
    /// Shopify product GID.
    pub product_id: String,
    /// Variant GID.
    pub variant_id: String,
    /// Product title.
    pub title: String,
    /// Product image URL.
    pub image_url: Option<String>,
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
    /// Currently active tab: "banners" or "products".
    pub active_tab: String,
    /// Products with related products (for the products tab).
    pub products: Vec<ProductWithRelatedView>,
    /// Whether there are more products to load.
    pub has_more_products: bool,
    /// Cursor for loading more products.
    pub next_cursor: Option<String>,
}

/// Query parameters for the promotions index page.
#[derive(Debug, Deserialize)]
pub struct PromotionsIndexQuery {
    /// Active tab selection.
    #[serde(default = "default_tab")]
    pub tab: String,
}

fn default_tab() -> String {
    "banners".to_string()
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

/// Related products configuration template.
#[derive(Template)]
#[template(path = "promotions/edit_related.html")]
pub struct EditRelatedProductsTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    /// The source product being configured.
    pub product: ProductWithRelatedView,
    /// All products available for selection (excluding the source product).
    pub available_products: Vec<ProductForPickerView>,
    /// Shopify ML recommendations for this product (if any).
    pub shopify_recommendations: Vec<ShopifyRecommendation>,
    /// Error message if any.
    pub error: Option<String>,
    /// Success message if any.
    pub success: Option<String>,
}

/// Product view for the picker (minimal data needed for selection).
#[derive(Debug, Clone)]
pub struct ProductForPickerView {
    /// Shopify product GID.
    pub id: String,
    /// Short ID for checkboxes.
    pub short_id: String,
    /// Product title.
    pub title: String,
    /// Product image URL.
    pub image_url: Option<String>,
    /// Default variant GID (for add-to-cart).
    pub default_variant_id: String,
    /// Whether this product is currently selected as related.
    pub is_selected: bool,
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

/// Form for saving related products configuration.
#[derive(Debug, Deserialize)]
pub struct RelatedProductsForm {
    /// Product GID (the source product).
    pub product_id: String,
    /// Comma-separated list of `product_id|variant_id` pairs (uses `|` since GIDs contain `:`).
    #[serde(default)]
    pub related_products: String,
}

/// Search query parameters.
#[derive(Debug, Deserialize)]
pub struct ProductSearchQuery {
    /// Search query string.
    #[serde(default)]
    pub q: String,
    /// Product ID to exclude from results.
    pub exclude: Option<String>,
}

/// Query parameters for the edit related products page.
#[derive(Debug, Deserialize)]
pub struct EditRelatedQuery {
    /// Success indicator.
    #[serde(default)]
    pub success: Option<String>,
    /// Error message.
    pub error: Option<String>,
}

// =============================================================================
// Handlers
// =============================================================================

/// Promotions list page handler.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<PromotionsIndexQuery>,
) -> Html<String> {
    debug!(tab = %query.tab, "Loading promotions management page");

    // Validate tab parameter
    let active_tab = if query.tab == "products" {
        "products".to_string()
    } else {
        "banners".to_string()
    };

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

    // Fetch products and cart recommendations if on the products tab
    let (products, has_more_products, next_cursor) = if active_tab == "products" {
        fetch_products_with_related(&state).await
    } else {
        (Vec::new(), false, None)
    };

    let template = PromotionsIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/promotions".to_string(),
        discounts: discount_views,
        current_config: config,
        error: None,
        success: None,
        active_tab,
        products,
        has_more_products,
        next_cursor,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// Fetch products with their related products configuration.
async fn fetch_products_with_related(
    state: &AppState,
) -> (Vec<ProductWithRelatedView>, bool, Option<String>) {
    // Fetch products and cart recommendations concurrently
    let (products_result, recommendations_result) = tokio::join!(
        state.shopify().get_products(50, None, None),
        state.shopify().get_cart_recommendations()
    );

    let recommendations = recommendations_result.unwrap_or_else(|e| {
        warn!(error = %e, "Failed to fetch cart recommendations");
        None
    });

    let (products, has_next_page, next_cursor) = match products_result {
        Ok(conn) => (
            conn.products,
            conn.page_info.has_next_page,
            conn.page_info.end_cursor,
        ),
        Err(e) => {
            warn!(error = %e, "Failed to fetch products");
            return (Vec::new(), false, None);
        }
    };

    // Build a lookup map for resolving product titles and images
    let product_lookup: std::collections::HashMap<&str, &crate::shopify::types::AdminProduct> =
        products.iter().map(|p| (p.id.as_str(), p)).collect();

    // Build product views with related products info
    let product_views: Vec<ProductWithRelatedView> = products
        .iter()
        .map(|p| {
            let short_id = p.id.split('/').next_back().unwrap_or(&p.id).to_string();

            // Find related products for this product
            let related = recommendations
                .as_ref()
                .and_then(|r| {
                    r.product_relations
                        .iter()
                        .find(|rel| rel.product_id == p.id)
                })
                .map_or(&[] as &[RelatedProduct], |rel| {
                    rel.related_products.as_slice()
                });

            // Build related product views with resolved titles and images
            let related_views: Vec<RelatedProductView> = related
                .iter()
                .map(|rp| {
                    let (title, image_url) =
                        product_lookup.get(rp.product_id.as_str()).map_or_else(
                            || (extract_short_id(&rp.product_id), None),
                            |prod| {
                                (
                                    prod.title.clone(),
                                    prod.featured_image.as_ref().map(|img| img.url.clone()),
                                )
                            },
                        );

                    RelatedProductView {
                        product_id: rp.product_id.clone(),
                        variant_id: rp.variant_id.clone(),
                        title,
                        image_url,
                    }
                })
                .collect();

            ProductWithRelatedView {
                id: p.id.clone(),
                short_id,
                title: p.title.clone(),
                image_url: p.featured_image.as_ref().map(|img| img.url.clone()),
                related_count: related.len(),
                related_products: related_views,
            }
        })
        .collect();

    (product_views, has_next_page, next_cursor)
}

/// Extract the short ID from a GID.
fn extract_short_id(gid: &str) -> String {
    gid.split('/').next_back().unwrap_or(gid).to_string()
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

// =============================================================================
// Related Products Handlers
// =============================================================================

/// Build the product view with related products for the edit page.
fn build_product_with_related_view(
    product: &crate::shopify::types::AdminProduct,
    recommendations: Option<&CartRecommendations>,
    all_products: &crate::shopify::types::AdminProductConnection,
) -> ProductWithRelatedView {
    let short_id = product
        .id
        .split('/')
        .next_back()
        .unwrap_or(&product.id)
        .to_string();

    let current_related = recommendations
        .and_then(|r| {
            r.product_relations
                .iter()
                .find(|rel| rel.product_id == product.id)
        })
        .map(|rel| rel.related_products.clone())
        .unwrap_or_default();

    // Build a lookup map for product details
    let product_lookup: std::collections::HashMap<&str, &crate::shopify::types::AdminProduct> =
        all_products
            .products
            .iter()
            .map(|p| (p.id.as_str(), p))
            .collect();

    let related_views: Vec<RelatedProductView> = current_related
        .iter()
        .map(|rp| {
            // Look up the product to get title and image
            let (title, image_url) = product_lookup.get(rp.product_id.as_str()).map_or_else(
                || (extract_short_id(&rp.product_id), None),
                |p| {
                    (
                        p.title.clone(),
                        p.featured_image.as_ref().map(|img| img.url.clone()),
                    )
                },
            );

            RelatedProductView {
                product_id: rp.product_id.clone(),
                variant_id: rp.variant_id.clone(),
                title,
                image_url,
            }
        })
        .collect();

    ProductWithRelatedView {
        id: product.id.clone(),
        short_id,
        title: product.title.clone(),
        image_url: product.featured_image.as_ref().map(|img| img.url.clone()),
        related_count: current_related.len(),
        related_products: related_views,
    }
}

/// Build the available products list for the picker (excluding the source product).
fn build_available_products(
    all_products: &crate::shopify::types::AdminProductConnection,
    source_product_id: &str,
    recommendations: Option<&CartRecommendations>,
) -> Vec<ProductForPickerView> {
    let current_related_ids: std::collections::HashSet<_> = recommendations
        .and_then(|r| {
            r.product_relations
                .iter()
                .find(|rel| rel.product_id == source_product_id)
        })
        .map(|rel| rel.related_products.iter().map(|r| &r.product_id).collect())
        .unwrap_or_default();

    all_products
        .products
        .iter()
        .filter(|p| p.id != source_product_id)
        .map(|p| {
            let p_short_id = p.id.split('/').next_back().unwrap_or(&p.id).to_string();
            let default_variant_id = p.variants.first().map(|v| v.id.clone()).unwrap_or_default();
            let is_selected = current_related_ids.contains(&p.id);

            ProductForPickerView {
                id: p.id.clone(),
                short_id: p_short_id,
                title: p.title.clone(),
                image_url: p.featured_image.as_ref().map(|img| img.url.clone()),
                default_variant_id,
                is_selected,
            }
        })
        .collect()
}

/// Edit related products for a specific product.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32(), product_id = %id))]
pub async fn edit_related_products(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<EditRelatedQuery>,
) -> impl IntoResponse {
    debug!("Loading related products editor");

    // Ensure ID has the proper Shopify format
    let product_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Product/{id}")
    };

    // Fetch the product, all products, cart recommendations, and Shopify ML recommendations concurrently
    let (product_result, all_products_result, recommendations_result, shopify_recs_result) = tokio::join!(
        state.shopify().get_product(&product_id),
        state.shopify().get_products(100, None, None),
        state.shopify().get_cart_recommendations(),
        state.shopify().get_shopify_recommendations(&product_id)
    );

    // Get Shopify ML recommendations (may be empty for new stores)
    let shopify_recommendations = shopify_recs_result.unwrap_or_else(|e| {
        warn!(error = %e, "Failed to fetch Shopify ML recommendations");
        Vec::new()
    });

    // Get the source product
    let product = match product_result {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, Html("Product not found".to_string())).into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("Failed to fetch product: {e}")),
            )
                .into_response();
        }
    };

    // Get current related products
    let recommendations = recommendations_result.unwrap_or_else(|e| {
        warn!(error = %e, "Failed to fetch cart recommendations");
        None
    });

    // Get all products (needed for both the product view and available products picker)
    let all_products = all_products_result.unwrap_or_else(|e| {
        warn!(error = %e, "Failed to fetch products");
        crate::shopify::types::AdminProductConnection {
            products: Vec::new(),
            page_info: crate::shopify::types::PageInfo {
                has_next_page: false,
                has_previous_page: false,
                start_cursor: None,
                end_cursor: None,
            },
        }
    });

    let product_view =
        build_product_with_related_view(&product, recommendations.as_ref(), &all_products);

    let available_products =
        build_available_products(&all_products, &product.id, recommendations.as_ref());

    // Determine success/error from query params
    let success = query
        .success
        .map(|_| "Related products saved successfully!".to_string());
    let error = query.error;

    let template = EditRelatedProductsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/promotions".to_string(),
        product: product_view,
        available_products,
        shopify_recommendations,
        error,
        success,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
    .into_response()
}

/// Save related products configuration for a product.
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn save_related_products(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(form): Form<RelatedProductsForm>,
) -> impl IntoResponse {
    debug!(product_id = %form.product_id, "Saving related products");

    // Parse related products from form (format: "product_id|variant_id,product_id|variant_id")
    // Uses | as delimiter since Shopify GIDs contain colons
    let related_products: Vec<RelatedProduct> = form
        .related_products
        .split(',')
        .filter(|s| !s.is_empty())
        .filter_map(|pair| {
            let parts: Vec<&str> = pair.split('|').collect();
            match (parts.first(), parts.get(1)) {
                (Some(product_id), Some(variant_id)) if parts.len() == 2 => Some(RelatedProduct {
                    product_id: (*product_id).to_string(),
                    variant_id: (*variant_id).to_string(),
                }),
                _ => {
                    warn!(pair = %pair, "Invalid related product pair format");
                    None
                }
            }
        })
        .collect();

    // Get current config with compareDigest for optimistic concurrency
    let CartRecommendationsWithDigest {
        recommendations: mut config,
        compare_digest,
    } = state
        .shopify()
        .get_cart_recommendations_with_digest()
        .await
        .unwrap_or_default();

    // Update the related products for this product
    config.set_related_products(form.product_id.clone(), related_products);

    // Save config with compareDigest for optimistic concurrency
    match state
        .shopify()
        .set_cart_recommendations(&config, compare_digest)
        .await
    {
        Ok(()) => {
            info!(product_id = %form.product_id, "Related products saved");
            let short_id = extract_short_id(&form.product_id);
            Redirect::to(&format!("/promotions/products/{short_id}/edit?success=1")).into_response()
        }
        Err(e) => {
            tracing::error!(product_id = %form.product_id, error = %e, "Failed to save related products");
            let short_id = extract_short_id(&form.product_id);
            Redirect::to(&format!(
                "/promotions/products/{short_id}/edit?error={}",
                urlencoding::encode(&e.to_string())
            ))
            .into_response()
        }
    }
}

/// Remove all related products for a product (HTMX).
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32(), product_id = %id))]
pub async fn remove_related_products(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!("Removing related products");

    let product_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/Product/{id}")
    };

    // Get current config with compareDigest
    let CartRecommendationsWithDigest {
        recommendations: mut config,
        compare_digest,
    } = state
        .shopify()
        .get_cart_recommendations_with_digest()
        .await
        .unwrap_or_default();

    // Remove related products for this product
    config.remove_related_products(&product_id);

    // Save config
    match state
        .shopify()
        .set_cart_recommendations(&config, compare_digest)
        .await
    {
        Ok(()) => {
            info!(product_id = %product_id, "Related products removed");
            (
                StatusCode::OK,
                [("HX-Trigger", "related-products-updated")],
                Html("<span class=\"text-muted-foreground\">No related products</span>"),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(product_id = %product_id, error = %e, "Failed to remove related products");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("Error: {e}")),
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
        // Related products routes
        .route(
            "/promotions/products/{id}/edit",
            axum::routing::get(edit_related_products),
        )
        .route(
            "/promotions/products/related",
            axum::routing::post(save_related_products),
        )
        .route(
            "/promotions/products/{id}/related",
            axum::routing::delete(remove_related_products),
        )
}
