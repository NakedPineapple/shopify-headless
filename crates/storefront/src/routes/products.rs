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
use crate::shopify::types::{Money, Product as ShopifyProduct, ProductRecommendationIntent};
use crate::state::AppState;

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
}

/// Image display data for templates.
#[derive(Clone)]
pub struct ImageView {
    pub url: String,
    pub alt: String,
}

/// Variant display data for templates.
#[derive(Clone)]
pub struct VariantView {
    pub id: String,
    pub title: String,
    pub price: String,
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
                })
                .collect(),
            ingredients: None, // Could parse from metafields if available
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
}

/// Product detail page template.
#[derive(Template, WebTemplate)]
#[template(path = "products/show.html")]
pub struct ProductShowTemplate {
    pub product: ProductView,
    pub related_products: Vec<ProductView>,
    pub analytics: AnalyticsConfig,
    pub nonce: String,
}

/// Quick view fragment template.
#[derive(Template, WebTemplate)]
#[template(path = "partials/quick_view.html")]
pub struct QuickViewTemplate {
    pub product: ProductView,
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

            ProductShowTemplate {
                product,
                related_products,
                analytics: state.config().analytics.clone(),
                nonce,
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
                    },
                    related_products: Vec::new(),
                    analytics: state.config().analytics.clone(),
                    nonce,
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
                    },
                    related_products: Vec::new(),
                    analytics: state.config().analytics.clone(),
                    nonce,
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

    match result {
        Ok(shopify_product) => {
            let product = ProductView::from(&shopify_product);
            QuickViewTemplate { product }.into_response()
        }
        Err(e) => {
            tracing::error!("Failed to fetch product for quick view {handle}: {e}");
            // Return a minimal error fragment
            QuickViewTemplate {
                product: ProductView {
                    handle,
                    title: "Product Not Found".to_string(),
                    description: String::new(),
                    price: "$0.00".to_string(),
                    compare_at_price: None,
                    featured_image: None,
                    images: Vec::new(),
                    variants: Vec::new(),
                    ingredients: None,
                },
            }
            .into_response()
        }
    }
}
