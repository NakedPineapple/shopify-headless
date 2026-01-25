//! Collection route handlers.

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
};
use serde::Deserialize;

use crate::filters;
use crate::state::AppState;

pub use super::products::{ImageView, ProductView};

/// Collection display data for templates.
#[derive(Clone)]
pub struct CollectionView {
    pub handle: String,
    pub title: String,
    pub description: Option<String>,
    pub image: Option<ImageView>,
}

/// Pagination query parameters.
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<u32>,
    pub sort: Option<String>,
}

/// Collection listing page template.
#[derive(Template, WebTemplate)]
#[template(path = "collections/index.html")]
pub struct CollectionsIndexTemplate {
    pub collections: Vec<CollectionView>,
}

/// Collection detail page template.
#[derive(Template, WebTemplate)]
#[template(path = "collections/show.html")]
pub struct CollectionShowTemplate {
    pub collection: CollectionView,
    pub products: Vec<ProductView>,
    pub current_page: u32,
    pub total_pages: u32,
    pub has_more_pages: bool,
}

/// Display collection listing page.
pub async fn index(State(_state): State<AppState>) -> impl IntoResponse {
    // TODO: Fetch collections from Shopify Storefront API
    let collections = Vec::new();

    CollectionsIndexTemplate { collections }
}

/// Display collection detail page with products.
pub async fn show(
    State(_state): State<AppState>,
    Path(handle): Path<String>,
    Query(query): Query<PaginationQuery>,
) -> impl IntoResponse {
    let current_page = query.page.unwrap_or(1);

    // TODO: Fetch collection and products from Shopify Storefront API
    let collection = CollectionView {
        handle: handle.clone(),
        title: "Collection Not Found".to_string(),
        description: None,
        image: None,
    };

    let products = Vec::new();
    let total_pages = 1;

    CollectionShowTemplate {
        collection,
        products,
        current_page,
        total_pages,
        has_more_pages: current_page < total_pages,
    }
}
