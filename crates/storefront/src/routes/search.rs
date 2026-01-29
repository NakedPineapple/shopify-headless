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
use tracing::instrument;

use crate::config::AnalyticsConfig;
use crate::filters;
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
}

/// Search suggestions endpoint (HTMX).
///
/// Returns HTML fragment with search results grouped by type.
#[instrument(skip(state))]
pub async fn suggest(
    State(state): State<AppState>,
    Query(query): Query<SuggestQuery>,
) -> impl IntoResponse {
    let query_str = query.q.trim();

    if query_str.is_empty() {
        return SearchResultsTemplate {
            results: SearchResults::default(),
            is_ready: state.search().is_ready(),
        }
        .into_response();
    }

    let results = state.search().search(query_str, 4).unwrap_or_default();

    SearchResultsTemplate {
        results,
        is_ready: state.search().is_ready(),
    }
    .into_response()
}

/// Full search page.
#[instrument(skip(state))]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub async fn search_page(
    State(state): State<AppState>,
    Query(query): Query<SearchPageQuery>,
) -> impl IntoResponse {
    let query_str = query.q.trim();
    let sort = SearchSort::parse(&query.sort_by);

    // Parse filters
    let filters = SearchFilters {
        available: query.availability.as_ref().map(|v| v == "1"),
        min_price_cents: query.price_gte.map(|p| (p * 100.0) as u64),
        max_price_cents: query.price_lte.map(|p| (p * 100.0) as u64),
    };

    let results = state
        .search()
        .search_filtered(query_str, &filters, sort, 100)
        .unwrap_or_default();

    SearchPageTemplate {
        query: query.q.clone(),
        results,
        sort_by: sort.as_str().to_string(),
        is_ready: state.search().is_ready(),
        filter_availability: query.availability.clone(),
        filter_price_gte: filters.min_price_cents,
        filter_price_lte: filters.max_price_cents,
        analytics: state.config().analytics.clone(),
    }
    .into_response()
}

/// Create the search routes router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(search_page))
        .route("/suggest", get(suggest))
}
