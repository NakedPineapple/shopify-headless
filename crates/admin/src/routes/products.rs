//! Products list route handler.

use askama::Template;
use axum::{
    extract::{Query, State},
    response::Html,
};
use serde::Deserialize;
use tracing::instrument;

use crate::{
    filters,
    middleware::auth::RequireAdminAuth,
    models::CurrentAdmin,
    shopify::types::{AdminProduct, Money, ProductStatus},
    state::AppState,
};

use naked_pineapple_core::AdminRole;

use super::dashboard::AdminUserView;

/// Pagination query parameters.
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub cursor: Option<String>,
    pub query: Option<String>,
}

/// Product view for templates.
#[derive(Debug, Clone)]
pub struct ProductView {
    pub id: String,
    pub title: String,
    pub status: String,
    pub status_class: String,
    pub inventory: i64,
    pub price: String,
    pub image_url: Option<String>,
    pub handle: String,
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

impl From<&AdminProduct> for ProductView {
    fn from(product: &AdminProduct) -> Self {
        let (status, status_class) = match product.status {
            ProductStatus::Active => ("Active", "bg-green-100 text-green-700"),
            ProductStatus::Draft => ("Draft", "bg-yellow-100 text-yellow-700"),
            ProductStatus::Archived => ("Archived", "bg-gray-100 text-gray-700"),
            ProductStatus::Unlisted => ("Unlisted", "bg-blue-100 text-blue-700"),
        };

        // Get price from first variant
        let price = product
            .variants
            .first()
            .map(|v| format_price(&v.price))
            .unwrap_or_else(|| "$0.00".to_string());

        Self {
            id: product.id.clone(),
            title: product.title.clone(),
            status: status.to_string(),
            status_class: status_class.to_string(),
            inventory: product.total_inventory,
            price,
            image_url: product.featured_image.as_ref().map(|img| img.url.clone()),
            handle: product.handle.clone(),
        }
    }
}

/// Products list page template.
#[derive(Template)]
#[template(path = "products/index.html")]
pub struct ProductsIndexTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub products: Vec<ProductView>,
    pub has_next_page: bool,
    pub next_cursor: Option<String>,
    pub search_query: Option<String>,
}

/// Products list page handler.
#[instrument(skip(admin, state))]
pub async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Html<String> {
    let result = state
        .shopify()
        .get_products(25, query.cursor.clone(), query.query.clone())
        .await;

    let (products, has_next_page, next_cursor) = match result {
        Ok(conn) => {
            let products: Vec<ProductView> = conn.products.iter().map(ProductView::from).collect();
            (
                products,
                conn.page_info.has_next_page,
                conn.page_info.end_cursor,
            )
        }
        Err(e) => {
            tracing::error!("Failed to fetch products: {e}");
            (vec![], false, None)
        }
    };

    let template = ProductsIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/products".to_string(),
        products,
        has_next_page,
        next_cursor,
        search_query: query.query,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}
