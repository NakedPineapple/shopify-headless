//! Product route handlers.

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::instrument;

use crate::config::AnalyticsConfig;
use crate::filters;
use crate::shopify::ShopifyError;
use crate::shopify::types::{
    Money, Product as ShopifyProduct, ProductRecommendationIntent, SellingPlanPriceAdjustmentValue,
};
use crate::state::AppState;

/// Product rating display data for templates.
#[derive(Clone)]
pub struct RatingView {
    /// Average rating value (e.g., 4.5).
    pub value: f64,
    /// Number of full stars to display.
    pub full_stars: u8,
    /// Whether to display a half star.
    pub has_half_star: bool,
    /// Number of empty stars to display.
    pub empty_stars: u8,
    /// Total number of reviews.
    pub count: i64,
}

/// A single selling plan (subscription option) for templates.
#[derive(Clone)]
pub struct SellingPlanView {
    /// Selling plan ID (pass to cart).
    pub id: String,
    /// Display name (e.g., "Delivery every 30 days").
    pub name: String,
    /// Discount percentage (e.g., 15 for 15% off), if applicable.
    pub discount_percentage: Option<i64>,
    /// Formatted discount text (e.g., "Save 15%").
    pub discount_text: Option<String>,
}

/// A group of selling plans for templates.
#[derive(Clone)]
pub struct SellingPlanGroupView {
    /// Group name (e.g., "Subscribe & Save").
    pub name: String,
    /// Selling plans in this group.
    pub selling_plans: Vec<SellingPlanView>,
}

/// Product display data for templates.
#[derive(Clone)]
pub struct ProductView {
    pub handle: String,
    pub title: String,
    pub description: String,
    pub price: String,
    pub compare_at_price: Option<String>,
    pub featured_image: Option<ImageView>,
    pub images: Vec<ImageView>,
    pub variants: Vec<VariantView>,
    pub ingredients: Option<String>,
    pub rating: Option<RatingView>,
    /// Whether product requires a subscription (can't be purchased one-time).
    pub requires_selling_plan: bool,
    /// Subscription options available for this product.
    pub selling_plan_groups: Vec<SellingPlanGroupView>,
}

/// Image display data for templates.
#[derive(Clone)]
pub struct ImageView {
    pub url: String,
    pub alt: String,
}

/// Shop Pay installments display data for templates.
#[derive(Clone)]
pub struct ShopPayInstallmentsView {
    /// Whether the variant is eligible for Shop Pay installments.
    pub eligible: bool,
    /// Price per payment term (formatted, e.g., "$51.75").
    pub price_per_term: Option<String>,
    /// Number of installments.
    pub installments_count: Option<i64>,
}

/// Variant display data for templates.
#[derive(Clone)]
pub struct VariantView {
    pub id: String,
    pub title: String,
    pub price: String,
    pub available_for_sale: bool,
    pub quantity_available: Option<i64>,
    pub shop_pay_installments: Option<ShopPayInstallmentsView>,
}

/// Breadcrumb item for SEO structured data.
#[derive(Clone)]
pub struct BreadcrumbItem {
    pub name: String,
    pub url: Option<String>,
}

/// Pagination query parameters.
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<u32>,
    pub sort: Option<String>,
}

// =============================================================================
// Type Conversions
// =============================================================================

/// Format a Shopify Money type as a price string.
fn format_price(money: &Money) -> String {
    // Parse the amount string to format it properly
    money.amount.parse::<f64>().map_or_else(
        |_| format!("${}", money.amount),
        |amount| format!("${amount:.2}"),
    )
}

impl From<&ShopifyProduct> for ProductView {
    fn from(product: &ShopifyProduct) -> Self {
        Self {
            handle: product.handle.clone(),
            title: product.title.clone(),
            description: product.description_html.clone(),
            price: format_price(&product.price_range.min_variant_price),
            compare_at_price: product
                .compare_at_price_range
                .as_ref()
                .filter(|r| r.min_variant_price.amount != "0.0")
                .map(|r| format_price(&r.min_variant_price)),
            featured_image: product.featured_image.as_ref().map(|img| ImageView {
                url: img.url.clone(),
                alt: img.alt_text.clone().unwrap_or_default(),
            }),
            images: product
                .images
                .iter()
                .map(|img| ImageView {
                    url: img.url.clone(),
                    alt: img.alt_text.clone().unwrap_or_default(),
                })
                .collect(),
            variants: product
                .variants
                .iter()
                .map(|v| VariantView {
                    id: v.id.clone(),
                    title: v.title.clone(),
                    price: format_price(&v.price),
                    available_for_sale: v.available_for_sale,
                    quantity_available: v.quantity_available,
                    shop_pay_installments: v.shop_pay_installments.as_ref().map(|sp| {
                        ShopPayInstallmentsView {
                            eligible: sp.eligible,
                            price_per_term: sp.price_per_term.as_ref().map(format_price),
                            installments_count: sp.installments_count.as_ref().map(|c| c.count),
                        }
                    }),
                })
                .collect(),
            ingredients: None, // Could parse from metafields if available
            rating: product.rating.as_ref().map(|r| {
                // Calculate star display: round to nearest 0.5, clamped to valid range
                let clamped = r.value.clamp(0.0, 5.0);
                let rounded = (clamped * 2.0).round() / 2.0;
                // SAFETY: rounded is clamped to 0.0-5.0, floor gives 0-5, fits in u8
                #[expect(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "value is clamped to 0.0-5.0 range"
                )]
                let full_stars = rounded.floor() as u8;
                let has_half_star = (rounded - rounded.floor()) >= 0.5;
                let empty_stars = 5 - full_stars - u8::from(has_half_star);

                RatingView {
                    value: r.value,
                    full_stars,
                    has_half_star,
                    empty_stars,
                    count: r.count,
                }
            }),
            requires_selling_plan: product.requires_selling_plan,
            selling_plan_groups: product
                .selling_plan_groups
                .iter()
                .map(|group| SellingPlanGroupView {
                    name: group.name.clone(),
                    selling_plans: group
                        .selling_plans
                        .iter()
                        .map(|sp| {
                            // Extract discount percentage from first price adjustment
                            let discount_percentage = sp.price_adjustments.first().and_then(|adj| {
                                if let SellingPlanPriceAdjustmentValue::Percentage(p) = &adj.adjustment_value {
                                    #[expect(
                                        clippy::cast_possible_truncation,
                                        reason = "discount percentages are small integers"
                                    )]
                                    Some(*p as i64)
                                } else {
                                    None
                                }
                            });

                            let discount_text = discount_percentage.map(|p| format!("Save {p}%"));

                            SellingPlanView {
                                id: sp.id.clone(),
                                name: sp.name.clone(),
                                discount_percentage,
                                discount_text,
                            }
                        })
                        .collect(),
                })
                .collect(),
        }
    }
}

/// Product listing page template.
#[derive(Template, WebTemplate)]
#[template(path = "products/index.html")]
pub struct ProductsIndexTemplate {
    pub products: Vec<ProductView>,
    pub current_page: u32,
    pub total_pages: u32,
    pub has_more_pages: bool,
    pub analytics: AnalyticsConfig,
    pub nonce: String,
    /// Base URL for canonical links.
    pub base_url: String,
}

/// Product detail page template.
#[derive(Template, WebTemplate)]
#[template(path = "products/show.html")]
pub struct ProductShowTemplate {
    pub product: ProductView,
    pub related_products: Vec<ProductView>,
    pub analytics: AnalyticsConfig,
    pub nonce: String,
    /// Base URL for canonical links and structured data.
    pub base_url: String,
    /// Breadcrumb trail for SEO.
    pub breadcrumbs: Vec<BreadcrumbItem>,
    /// Shopify store URL for Shop Pay button (e.g., "your-store.myshopify.com").
    pub store_url: String,
}

/// Quick view fragment template.
#[derive(Template, WebTemplate)]
#[template(path = "partials/quick_view.html")]
pub struct QuickViewTemplate {
    pub product: ProductView,
    /// Shopify store URL for Shop Pay button (e.g., "your-store.myshopify.com")
    pub store_url: String,
}

/// Products per page for pagination.
const PRODUCTS_PER_PAGE: i64 = 12;

/// Display product listing page.
#[instrument(skip(state, nonce))]
pub async fn index(
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> Response {
    let current_page = query.page.unwrap_or(1);

    // Fetch products from Shopify Storefront API
    let result = state
        .storefront()
        .get_products(Some(PRODUCTS_PER_PAGE), None, None, None, None)
        .await;

    match result {
        Ok(connection) => {
            let products: Vec<ProductView> =
                connection.products.iter().map(ProductView::from).collect();

            // Estimate total pages (Shopify doesn't give total count easily)
            let has_more = connection.page_info.has_next_page;

            ProductsIndexTemplate {
                products,
                current_page,
                total_pages: if has_more {
                    current_page + 1
                } else {
                    current_page
                },
                has_more_pages: has_more,
                analytics: state.config().analytics.clone(),
                nonce,
                base_url: state.config().base_url.clone(),
            }
            .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to fetch products: {e}");
            // Return empty products page on error
            ProductsIndexTemplate {
                products: Vec::new(),
                current_page: 1,
                total_pages: 1,
                has_more_pages: false,
                analytics: state.config().analytics.clone(),
                nonce,
                base_url: state.config().base_url.clone(),
            }
            .into_response()
        }
    }
}

/// Display product detail page.
#[instrument(skip(state, nonce))]
pub async fn show(
    State(state): State<AppState>,
    Path(handle): Path<String>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> Response {
    // Fetch product from Shopify Storefront API
    let result = state.storefront().get_product_by_handle(&handle).await;

    match result {
        Ok(shopify_product) => {
            let product = ProductView::from(&shopify_product);

            // Fetch related products
            let related_products = state
                .storefront()
                .get_product_recommendations(
                    &shopify_product.id,
                    Some(ProductRecommendationIntent::Related),
                )
                .await
                .map(|products| products.iter().take(4).map(ProductView::from).collect())
                .unwrap_or_default();

            // SEO breadcrumbs
            let breadcrumbs = vec![
                BreadcrumbItem {
                    name: "Home".to_string(),
                    url: Some("/".to_string()),
                },
                BreadcrumbItem {
                    name: "Products".to_string(),
                    url: Some("/products".to_string()),
                },
                BreadcrumbItem {
                    name: product.title.clone(),
                    url: None,
                },
            ];

            ProductShowTemplate {
                product,
                related_products,
                analytics: state.config().analytics.clone(),
                nonce,
                base_url: state.config().base_url.clone(),
                breadcrumbs,
                store_url: state.config().shopify.store.clone(),
            }
            .into_response()
        }
        Err(ShopifyError::NotFound(_)) => {
            // Return 404 for missing products
            (
                StatusCode::NOT_FOUND,
                ProductShowTemplate {
                    product: ProductView {
                        handle: handle.clone(),
                        title: "Product Not Found".to_string(),
                        description: "This product could not be found.".to_string(),
                        price: "$0.00".to_string(),
                        compare_at_price: None,
                        featured_image: None,
                        images: Vec::new(),
                        variants: Vec::new(),
                        ingredients: None,
                        rating: None,
                        requires_selling_plan: false,
                        selling_plan_groups: Vec::new(),
                    },
                    related_products: Vec::new(),
                    analytics: state.config().analytics.clone(),
                    nonce,
                    base_url: state.config().base_url.clone(),
                    breadcrumbs: Vec::new(),
                    store_url: state.config().shopify.store.clone(),
                },
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to fetch product {handle}: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                ProductShowTemplate {
                    product: ProductView {
                        handle,
                        title: "Error".to_string(),
                        description: "An error occurred loading this product.".to_string(),
                        price: "$0.00".to_string(),
                        compare_at_price: None,
                        featured_image: None,
                        images: Vec::new(),
                        variants: Vec::new(),
                        ingredients: None,
                        rating: None,
                        requires_selling_plan: false,
                        selling_plan_groups: Vec::new(),
                    },
                    related_products: Vec::new(),
                    analytics: state.config().analytics.clone(),
                    nonce,
                    base_url: state.config().base_url.clone(),
                    breadcrumbs: Vec::new(),
                    store_url: state.config().shopify.store.clone(),
                },
            )
                .into_response()
        }
    }
}

/// Display quick view fragment (for HTMX).
#[instrument(skip(state))]
pub async fn quick_view(State(state): State<AppState>, Path(handle): Path<String>) -> Response {
    // Fetch product from Shopify Storefront API
    let result = state.storefront().get_product_by_handle(&handle).await;

    // Store URL for Shop Pay button (e.g., "your-store.myshopify.com")
    let store_url = state.config().shopify.store.clone();

    match result {
        Ok(shopify_product) => {
            let product = ProductView::from(&shopify_product);
            QuickViewTemplate { product, store_url }.into_response()
        }
        Err(e) => {
            tracing::error!("Failed to fetch product for quick view {handle}: {e}");
            // Return a minimal error fragment
            let product = ProductView {
                handle,
                title: "Product Not Found".to_string(),
                description: String::new(),
                price: "$0.00".to_string(),
                compare_at_price: None,
                featured_image: None,
                images: Vec::new(),
                variants: Vec::new(),
                ingredients: None,
                rating: None,
                requires_selling_plan: false,
                selling_plan_groups: Vec::new(),
            };
            QuickViewTemplate { product, store_url }.into_response()
        }
    }
}
