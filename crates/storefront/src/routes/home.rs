//! Home page route handler.

use askama::Template;
use askama_web::WebTemplate;
use axum::{extract::State, response::IntoResponse};

use crate::filters;
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

/// Home page template.
#[derive(Template, WebTemplate)]
#[template(path = "home.html")]
pub struct HomeTemplate {
    pub featured_products: Vec<ProductView>,
}

/// Display the home page.
pub async fn home(State(_state): State<AppState>) -> impl IntoResponse {
    // TODO: Fetch featured products from Shopify Storefront API
    // For now, return empty products list
    let featured_products = Vec::new();

    HomeTemplate { featured_products }
}
