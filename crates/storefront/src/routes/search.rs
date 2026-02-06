//! Search route handlers.

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    Router,
    extract::{Query, State},
    response::IntoResponse,
    routing::get,
};
use serde::{Deserialize, Deserializer};
use tracing::{debug, info, instrument, warn};

use crate::config::{AnalyticsConfig, AnalyticsUserInfo};
use crate::error::add_breadcrumb;
use crate::filters;
use crate::middleware::OptionalAuth;
use crate::search::{SearchFilters, SearchResults, SearchSort};
use crate::state::AppState;

/// Deserialize empty strings as None for optional numeric fields.
fn empty_string_as_none<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        None => Ok(None),
        Some(s) if s.is_empty() => Ok(None),
        Some(s) => s.parse().map(Some).map_err(serde::de::Error::custom),
    }
}

/// Search suggestions query parameters.
#[derive(Debug, Deserialize)]
pub struct SuggestQuery {
    #[serde(default)]
    pub q: String,
}

/// Full search page query parameters.
#[derive(Debug, Deserialize)]
pub struct SearchPageQuery {
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub sort_by: String,
    /// Availability filter: "1" for in-stock only
    #[serde(rename = "filter.v.availability")]
    pub availability: Option<String>,
    /// Min price filter (dollars)
    #[serde(
        default,
        rename = "filter.v.price.gte",
        deserialize_with = "empty_string_as_none"
    )]
    pub price_gte: Option<f64>,
    /// Max price filter (dollars)
    #[serde(
        default,
        rename = "filter.v.price.lte",
        deserialize_with = "empty_string_as_none"
    )]
    pub price_lte: Option<f64>,
}

/// Search suggestions template (HTMX fragment).
#[derive(Template, WebTemplate)]
#[template(path = "partials/search_results.html")]
pub struct SearchResultsTemplate {
    pub results: SearchResults,
    pub is_ready: bool,
}

/// Full search page template.
#[derive(Template, WebTemplate)]
#[template(path = "pages/search.html")]
pub struct SearchPageTemplate {
    pub query: String,
    pub results: SearchResults,
    pub sort_by: String,
    pub is_ready: bool,
    // Active filters
    pub filter_availability: Option<String>,
    pub filter_price_gte: Option<u64>,
    pub filter_price_lte: Option<u64>,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub site: crate::middleware::SiteContext,
    pub nonce: String,
}

/// Search suggestions endpoint (HTMX).
///
/// Returns HTML fragment with search results grouped by type.
#[instrument(skip(state))]
pub async fn suggest(
    State(state): State<AppState>,
    Query(query): Query<SuggestQuery>,
) -> impl IntoResponse {
    debug!("Handling search suggestions request");
    let query_str = query.q.trim();

    if query_str.is_empty() {
        debug!("Empty search query, returning empty results");
        return SearchResultsTemplate {
            results: SearchResults::default(),
            is_ready: state.search().is_ready(),
        }
        .into_response();
    }

    debug!(query = %query_str, "Executing search suggestions query");
    let results = match state.search().search(query_str, 4) {
        Ok(results) => {
            info!(
                query = %query_str,
                product_count = results.products.len(),
                collection_count = results.collections.len(),
                "Search suggestions completed successfully"
            );
            results
        }
        Err(e) => {
            warn!(query = %query_str, error = %e, "Search suggestions query failed");
            SearchResults::default()
        }
    };

    SearchResultsTemplate {
        results,
        is_ready: state.search().is_ready(),
    }
    .into_response()
}

/// Full search page.
#[instrument(skip(state, nonce, customer, site))]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub async fn search_page(
    State(state): State<AppState>,
    OptionalAuth(customer): OptionalAuth,
    Query(query): Query<SearchPageQuery>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> impl IntoResponse {
    add_breadcrumb("search", "Searched", Some(&[("query", query.q.trim())]));
    debug!("Handling full search page request");
    let query_str = query.q.trim();
    let sort = SearchSort::parse(&query.sort_by);

    // Parse filters
    let filters = SearchFilters {
        available: query.availability.as_ref().map(|v| v == "1"),
        min_price_cents: query.price_gte.map(|p| (p * 100.0) as u64),
        max_price_cents: query.price_lte.map(|p| (p * 100.0) as u64),
    };

    debug!(
        query = %query_str,
        sort = %sort.as_str(),
        available_filter = ?filters.available,
        min_price_cents = ?filters.min_price_cents,
        max_price_cents = ?filters.max_price_cents,
        "Executing filtered search query"
    );

    let results = match state
        .search()
        .search_filtered(query_str, &filters, sort, 100)
    {
        Ok(results) => {
            info!(
                query = %query_str,
                product_count = results.products.len(),
                collection_count = results.collections.len(),
                "Search page query completed successfully"
            );
            results
        }
        Err(e) => {
            warn!(query = %query_str, error = %e, "Search page query failed");
            SearchResults::default()
        }
    };

    SearchPageTemplate {
        query: query.q.clone(),
        results,
        sort_by: sort.as_str().to_string(),
        is_ready: state.search().is_ready(),
        filter_availability: query.availability.clone(),
        filter_price_gte: filters.min_price_cents,
        filter_price_lte: filters.max_price_cents,
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::from_customer(customer.as_ref()),
        site,
        nonce,
    }
    .into_response()
}

/// Create the search routes router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(search_page))
        .route("/suggest", get(suggest))
}
