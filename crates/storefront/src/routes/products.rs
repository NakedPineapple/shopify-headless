//! Product route handlers.

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
};
use serde::Deserialize;

use crate::filters;
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

/// Product listing page template.
#[derive(Template, WebTemplate)]
#[template(path = "products/index.html")]
pub struct ProductsIndexTemplate {
    pub products: Vec<ProductView>,
    pub current_page: u32,
    pub total_pages: u32,
    pub has_more_pages: bool,
}

/// Product detail page template.
#[derive(Template, WebTemplate)]
#[template(path = "products/show.html")]
pub struct ProductShowTemplate {
    pub product: ProductView,
    pub related_products: Vec<ProductView>,
}

/// Quick view fragment template.
#[derive(Template, WebTemplate)]
#[template(path = "partials/quick_view.html")]
pub struct QuickViewTemplate {
    pub product: ProductView,
}

/// Display product listing page.
pub async fn index(
    State(_state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> impl IntoResponse {
    let current_page = query.page.unwrap_or(1);

    // TODO: Fetch products from Shopify Storefront API
    let products = Vec::new();
    let total_pages = 1;

    ProductsIndexTemplate {
        products,
        current_page,
        total_pages,
        has_more_pages: current_page < total_pages,
    }
}

/// Display product detail page.
pub async fn show(
    State(_state): State<AppState>,
    Path(handle): Path<String>,
) -> impl IntoResponse {
    // TODO: Fetch product from Shopify Storefront API
    let product = ProductView {
        handle: handle.clone(),
        title: "Product Not Found".to_string(),
        description: "This product could not be found.".to_string(),
        price: "$0.00".to_string(),
        compare_at_price: None,
        featured_image: None,
        images: Vec::new(),
        variants: vec![VariantView {
            id: "default".to_string(),
            title: "Default".to_string(),
            price: "$0.00".to_string(),
        }],
        ingredients: None,
    };

    let related_products = Vec::new();

    ProductShowTemplate {
        product,
        related_products,
    }
}

/// Display quick view fragment (for HTMX).
pub async fn quick_view(
    State(_state): State<AppState>,
    Path(handle): Path<String>,
) -> impl IntoResponse {
    // TODO: Fetch product from Shopify Storefront API
    let product = ProductView {
        handle: handle.clone(),
        title: "Product".to_string(),
        description: String::new(),
        price: "$0.00".to_string(),
        compare_at_price: None,
        featured_image: None,
        images: Vec::new(),
        variants: vec![VariantView {
            id: "default".to_string(),
            title: "Default".to_string(),
            price: "$0.00".to_string(),
        }],
        ingredients: None,
    };

    QuickViewTemplate { product }
}
