//! Home page route handler.

use askama::Template;
use askama_web::WebTemplate;
use axum::{extract::State, response::IntoResponse};
use tracing::instrument;

use crate::filters;
use crate::shopify::types::{Money, Product as ShopifyProduct};
use crate::state::AppState;

/// Product display data for templates.
#[derive(Clone)]
pub struct ProductView {
    pub handle: String,
    pub title: String,
    pub price: String,
    pub compare_at_price: Option<String>,
    pub featured_image: Option<ImageView>,
}

/// Image display data for templates.
#[derive(Clone)]
pub struct ImageView {
    pub url: String,
    pub alt: String,
}

// =============================================================================
// Type Conversions
// =============================================================================

/// Format a Shopify Money type as a price string.
fn format_price(money: &Money) -> String {
    if let Ok(amount) = money.amount.parse::<f64>() {
        format!("${amount:.2}")
    } else {
        format!("${}", money.amount)
    }
}

impl From<&ShopifyProduct> for ProductView {
    fn from(product: &ShopifyProduct) -> Self {
        Self {
            handle: product.handle.clone(),
            title: product.title.clone(),
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
        }
    }
}

/// Home page template.
#[derive(Template, WebTemplate)]
#[template(path = "home.html")]
pub struct HomeTemplate {
    pub featured_products: Vec<ProductView>,
}

/// Number of featured products to show on home page.
const FEATURED_PRODUCTS_COUNT: i64 = 8;

/// Display the home page.
#[instrument(skip(state))]
pub async fn home(State(state): State<AppState>) -> impl IntoResponse {
    // Fetch featured products from Shopify Storefront API
    // Try to get products tagged as "featured", fall back to latest products
    let featured_products = state
        .storefront()
        .get_products(
            Some(FEATURED_PRODUCTS_COUNT),
            None,
            None, // Could use Some("tag:featured".to_string()) if products are tagged
            None,
            None,
        )
        .await
        .map(|conn| conn.products.iter().map(ProductView::from).collect())
        .unwrap_or_else(|e| {
            tracing::error!("Failed to fetch featured products: {e}");
            Vec::new()
        });

    HomeTemplate { featured_products }
}
