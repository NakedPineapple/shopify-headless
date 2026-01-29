//! Collection route handlers.

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
use crate::shopify::types::Collection as ShopifyCollection;
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

// =============================================================================
// Type Conversions
// =============================================================================

impl From<&ShopifyCollection> for CollectionView {
    fn from(collection: &ShopifyCollection) -> Self {
        Self {
            handle: collection.handle.clone(),
            title: collection.title.clone(),
            description: if collection.description.is_empty() {
                None
            } else {
                Some(collection.description.clone())
            },
            image: collection.image.as_ref().map(|img| ImageView {
                url: img.url.clone(),
                alt: img.alt_text.clone().unwrap_or_default(),
            }),
        }
    }
}

/// Collection listing page template.
#[derive(Template, WebTemplate)]
#[template(path = "collections/index.html")]
pub struct CollectionsIndexTemplate {
    pub collections: Vec<CollectionView>,
    pub analytics: AnalyticsConfig,
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
    pub analytics: AnalyticsConfig,
}

/// Products per page for collection view.
const PRODUCTS_PER_PAGE: usize = 12;

/// Display collection listing page.
#[instrument(skip(state))]
pub async fn index(State(state): State<AppState>) -> Response {
    // Fetch collections from Shopify Storefront API
    let result = state
        .storefront()
        .get_collections(Some(50), None, None)
        .await;

    match result {
        Ok(connection) => {
            let collections: Vec<CollectionView> = connection
                .collections
                .iter()
                .map(CollectionView::from)
                .collect();

            CollectionsIndexTemplate {
                collections,
                analytics: state.config().analytics.clone(),
            }
            .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to fetch collections: {e}");
            CollectionsIndexTemplate {
                collections: Vec::new(),
                analytics: state.config().analytics.clone(),
            }
            .into_response()
        }
    }
}

/// Display collection detail page with products.
#[instrument(skip(state))]
pub async fn show(
    State(state): State<AppState>,
    Path(handle): Path<String>,
    Query(query): Query<PaginationQuery>,
) -> Response {
    let current_page = query.page.unwrap_or(1);

    // Fetch collection and products from Shopify Storefront API
    #[allow(clippy::cast_possible_wrap)]
    let products_per_page = PRODUCTS_PER_PAGE as i64;
    let result = state
        .storefront()
        .get_collection_by_handle(&handle, Some(products_per_page), None)
        .await;

    match result {
        Ok(shopify_collection) => {
            let collection = CollectionView::from(&shopify_collection);
            let products: Vec<ProductView> = shopify_collection
                .products
                .iter()
                .map(ProductView::from)
                .collect();

            // Note: For proper pagination, we'd need to track page info
            // For now, assume single page of products
            let has_more = products.len() >= PRODUCTS_PER_PAGE;

            CollectionShowTemplate {
                collection,
                products,
                current_page,
                total_pages: if has_more {
                    current_page + 1
                } else {
                    current_page
                },
                has_more_pages: has_more,
                analytics: state.config().analytics.clone(),
            }
            .into_response()
        }
        Err(ShopifyError::NotFound(_)) => (
            StatusCode::NOT_FOUND,
            CollectionShowTemplate {
                collection: CollectionView {
                    handle: handle.clone(),
                    title: "Collection Not Found".to_string(),
                    description: None,
                    image: None,
                },
                products: Vec::new(),
                current_page: 1,
                total_pages: 1,
                has_more_pages: false,
                analytics: state.config().analytics.clone(),
            },
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch collection {handle}: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                CollectionShowTemplate {
                    collection: CollectionView {
                        handle,
                        title: "Error".to_string(),
                        description: Some("An error occurred loading this collection.".to_string()),
                        image: None,
                    },
                    products: Vec::new(),
                    current_page: 1,
                    total_pages: 1,
                    has_more_pages: false,
                    analytics: state.config().analytics.clone(),
                },
            )
                .into_response()
        }
    }
}
