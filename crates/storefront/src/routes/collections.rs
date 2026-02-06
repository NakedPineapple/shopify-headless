//! Collection route handlers.

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tracing::{debug, info, instrument, warn};

use crate::config::{AnalyticsConfig, AnalyticsUserInfo};
use crate::error::add_breadcrumb;
use crate::filters;
use crate::middleware::OptionalAuth;
use crate::shopify::types::Collection as ShopifyCollection;
use crate::shopify::{PriceRangeFilter, ProductCollectionSortKeys, ProductFilter, ShopifyError};
use crate::state::AppState;

pub use super::products::{BreadcrumbItem, ImageView, ProductView};

/// Collection display data for templates.
#[derive(Clone)]
pub struct CollectionView {
    pub handle: String,
    pub title: String,
    pub description: Option<String>,
    pub description_html: Option<String>,
    pub image: Option<ImageView>,
}

/// Pagination and filter query parameters.
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<u32>,
    pub sort: Option<String>,
    /// Filter to show only in-stock products.
    pub available: Option<bool>,
    /// Minimum price filter (in dollars).
    pub price_min: Option<f64>,
    /// Maximum price filter (in dollars).
    pub price_max: Option<f64>,
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
            description_html: if collection.description_html.is_empty() {
                None
            } else {
                Some(collection.description_html.clone())
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
    pub analytics_user_info: AnalyticsUserInfo,
    pub site: crate::middleware::SiteContext,
    pub nonce: String,
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
    pub analytics_user_info: AnalyticsUserInfo,
    pub site: crate::middleware::SiteContext,
    pub nonce: String,
    /// Breadcrumb trail for SEO.
    pub breadcrumbs: Vec<BreadcrumbItem>,
    /// Current sort option value.
    pub current_sort: String,
    /// Filter: show only in-stock products.
    pub filter_available: bool,
    /// Filter: minimum price.
    pub filter_price_min: Option<f64>,
    /// Filter: maximum price.
    pub filter_price_max: Option<f64>,
    /// Whether a price filter is actively applied (not at default 0-200 range).
    pub has_price_filter: bool,
}

/// Products per page for collection view.
const PRODUCTS_PER_PAGE: usize = 12;

/// Display collection listing page.
#[instrument(skip_all)]
pub async fn index(
    State(state): State<AppState>,
    OptionalAuth(customer): OptionalAuth,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> Response {
    debug!("Fetching all collections from Shopify Storefront API");

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

            info!(
                collection_count = collections.len(),
                "Successfully loaded collections index"
            );

            CollectionsIndexTemplate {
                collections,
                analytics: state.config().analytics.clone(),
                analytics_user_info: AnalyticsUserInfo::from_customer(customer.as_ref()),
                site,
                nonce,
            }
            .into_response()
        }
        Err(e) => {
            warn!(error = %e, "Failed to fetch collections from Shopify");
            CollectionsIndexTemplate {
                collections: Vec::new(),
                analytics: state.config().analytics.clone(),
                analytics_user_info: AnalyticsUserInfo::from_customer(customer.as_ref()),
                site,
                nonce,
            }
            .into_response()
        }
    }
}

/// Parse sort query parameter into Shopify sort key and reverse flag.
fn parse_sort(sort: Option<&str>) -> (Option<ProductCollectionSortKeys>, Option<bool>) {
    match sort {
        Some("price-asc") => (Some(ProductCollectionSortKeys::PRICE), Some(false)),
        Some("price-desc") => (Some(ProductCollectionSortKeys::PRICE), Some(true)),
        Some("newest") => (Some(ProductCollectionSortKeys::CREATED), Some(true)),
        Some("title-asc") => (Some(ProductCollectionSortKeys::TITLE), Some(false)),
        Some("title-desc") => (Some(ProductCollectionSortKeys::TITLE), Some(true)),
        // "best-selling" or default
        _ => (Some(ProductCollectionSortKeys::BEST_SELLING), None),
    }
}

/// Build Shopify product filters from query parameters.
fn build_filters(query: &PaginationQuery) -> Option<Vec<ProductFilter>> {
    let mut filters = Vec::new();

    // In-stock filter
    if query.available == Some(true) {
        debug!("Adding availability filter: available=true");
        filters.push(ProductFilter {
            available: Some(true),
            category: None,
            price: None,
            product_metafield: None,
            product_type: None,
            product_vendor: None,
            tag: None,
            taxonomy_metafield: None,
            variant_metafield: None,
            variant_option: None,
        });
    }

    // Price range filter - only apply if not at default values
    // Default slider range is 0-200, so ignore those values
    let has_min_filter = query.price_min.is_some_and(|v| v > 0.0);
    let has_max_filter = query.price_max.is_some_and(|v| v < 200.0);

    debug!(
        price_min = ?query.price_min,
        price_max = ?query.price_max,
        has_min_filter,
        has_max_filter,
        "Evaluating price filter"
    );

    if has_min_filter || has_max_filter {
        let min_val = if has_min_filter {
            query.price_min
        } else {
            None
        };
        let max_val = if has_max_filter {
            query.price_max
        } else {
            None
        };
        debug!(?min_val, ?max_val, "Adding price filter");
        filters.push(ProductFilter {
            available: None,
            category: None,
            price: Some(PriceRangeFilter {
                min: min_val,
                max: max_val,
            }),
            product_metafield: None,
            product_type: None,
            product_vendor: None,
            tag: None,
            taxonomy_metafield: None,
            variant_metafield: None,
            variant_option: None,
        });
    }

    let result = if filters.is_empty() {
        None
    } else {
        Some(filters)
    };

    debug!(
        filter_count = result.as_ref().map_or(0, Vec::len),
        "build_filters complete"
    );

    result
}

/// Active filter state for collection display.
#[derive(Clone)]
struct CollectionFilterState {
    current_sort: String,
    filter_available: bool,
    filter_price_min: Option<f64>,
    filter_price_max: Option<f64>,
}

impl CollectionFilterState {
    /// Check if a non-default price filter is actively applied.
    fn has_price_filter(&self) -> bool {
        self.filter_price_min.is_some_and(|v| v > 0.0)
            || self.filter_price_max.is_some_and(|v| v < 200.0)
    }
}

/// Parameters for building an error collection template.
struct ErrorParams {
    status: StatusCode,
    handle: String,
    title: &'static str,
    description: Option<&'static str>,
    filters: CollectionFilterState,
}

/// Build SEO breadcrumbs for a collection page.
fn build_breadcrumbs(title: &str) -> Vec<BreadcrumbItem> {
    vec![
        BreadcrumbItem {
            name: "Home".to_string(),
            url: Some("/".to_string()),
        },
        BreadcrumbItem {
            name: "Collections".to_string(),
            url: Some("/collections".to_string()),
        },
        BreadcrumbItem {
            name: title.to_string(),
            url: None,
        },
    ]
}

/// Create an error response for collection pages.
fn error_template(
    params: ErrorParams,
    state: &AppState,
    nonce: String,
    analytics_user_info: AnalyticsUserInfo,
    site: crate::middleware::SiteContext,
) -> Response {
    let has_price_filter = params.filters.has_price_filter();
    (
        params.status,
        CollectionShowTemplate {
            collection: CollectionView {
                handle: params.handle,
                title: params.title.to_string(),
                description: params.description.map(String::from),
                description_html: None,
                image: None,
            },
            products: Vec::new(),
            current_page: 1,
            total_pages: 1,
            has_more_pages: false,
            analytics: state.config().analytics.clone(),
            analytics_user_info,
            site,
            nonce,
            breadcrumbs: Vec::new(),
            current_sort: params.filters.current_sort,
            filter_available: params.filters.filter_available,
            filter_price_min: params.filters.filter_price_min,
            filter_price_max: params.filters.filter_price_max,
            has_price_filter,
        },
    )
        .into_response()
}

/// Log collection result including product prices for debugging.
fn log_collection_result(result: &Result<ShopifyCollection, ShopifyError>) {
    match result {
        Ok(collection) => {
            let prices: Vec<String> = collection
                .products
                .iter()
                .map(|p| format!("{}: {}", p.title, p.price_range.min_variant_price.amount))
                .collect();
            debug!(
                product_count = collection.products.len(),
                ?prices,
                "Received collection response"
            );
        }
        Err(e) => {
            debug!(error = %e, "Shopify collection request failed");
        }
    }
}

/// Build the success response template for collection show.
fn build_collection_response(
    shopify_collection: &ShopifyCollection,
    current_page: u32,
    filters: CollectionFilterState,
    state: &AppState,
    nonce: String,
    analytics_user_info: AnalyticsUserInfo,
    site: crate::middleware::SiteContext,
) -> Response {
    let collection = CollectionView::from(shopify_collection);
    let products: Vec<ProductView> = shopify_collection
        .products
        .iter()
        .map(ProductView::from)
        .collect();
    let has_more = products.len() >= PRODUCTS_PER_PAGE;
    let has_price_filter = filters.has_price_filter();

    info!(
        collection_title = %collection.title,
        product_count = products.len(),
        current_page,
        has_more_pages = has_more,
        "Successfully loaded collection detail page"
    );

    CollectionShowTemplate {
        breadcrumbs: build_breadcrumbs(&collection.title),
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
        analytics_user_info,
        site,
        nonce,
        current_sort: filters.current_sort,
        filter_available: filters.filter_available,
        filter_price_min: filters.filter_price_min,
        filter_price_max: filters.filter_price_max,
        has_price_filter,
    }
    .into_response()
}

/// Display collection detail page with products.
#[instrument(skip(state, nonce, customer, site), fields(handle = %handle))]
pub async fn show(
    State(state): State<AppState>,
    Path(handle): Path<String>,
    OptionalAuth(customer): OptionalAuth,
    Query(query): Query<PaginationQuery>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> Response {
    add_breadcrumb(
        "collection",
        "Viewed collection",
        Some(&[("handle", &handle)]),
    );
    debug!(?query, "Fetching collection detail page with products");

    let current_page = query.page.unwrap_or(1);
    let current_sort = query
        .sort
        .clone()
        .unwrap_or_else(|| "best-selling".to_string());
    let (sort_key, reverse) = parse_sort(query.sort.as_deref());
    let filter_available = query.available.unwrap_or(false);
    let filter_price_min = query.price_min;
    let filter_price_max = query.price_max;
    let filters = build_filters(&query);

    debug!(
        filter_available,
        ?filter_price_min,
        ?filter_price_max,
        has_filters = filters.is_some(),
        filter_count = filters.as_ref().map_or(0, Vec::len),
        "Built filters for Shopify API request"
    );

    #[allow(clippy::cast_possible_wrap)]
    let result = state
        .storefront()
        .get_collection_by_handle(
            &handle,
            Some(PRODUCTS_PER_PAGE as i64),
            None,
            sort_key,
            reverse,
            filters,
        )
        .await;

    log_collection_result(&result);

    let filter_state = CollectionFilterState {
        current_sort,
        filter_available,
        filter_price_min,
        filter_price_max,
    };

    let analytics_user_info = AnalyticsUserInfo::from_customer(customer.as_ref());

    let err_params = |status, title, desc| ErrorParams {
        status,
        handle: handle.clone(),
        title,
        description: desc,
        filters: filter_state.clone(),
    };

    match result {
        Ok(shopify_collection) => build_collection_response(
            &shopify_collection,
            current_page,
            filter_state.clone(),
            &state,
            nonce,
            analytics_user_info,
            site,
        ),
        Err(ShopifyError::NotFound(_)) => {
            warn!(handle = %handle, "Collection not found");
            error_template(
                err_params(StatusCode::NOT_FOUND, "Collection Not Found", None),
                &state,
                nonce,
                analytics_user_info,
                site,
            )
        }
        Err(e) => {
            warn!(error = %e, "Failed to fetch collection from Shopify");
            error_template(
                err_params(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Error",
                    Some("An error occurred loading this collection."),
                ),
                &state,
                nonce,
                analytics_user_info,
                site,
            )
        }
    }
}
