//! Financials route handlers.
//!
//! Handles manufacturing batches, inventory lots, and cost tracking.

use askama::Template;
use axum::{
    Form, Router,
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::Deserialize;
use tracing::instrument;

use crate::{
    db::{InventoryLotRepository, ManufacturingRepository, RepositoryError},
    filters,
    middleware::auth::RequireAdminAuth,
    models::inventory_lot::{CreateLotInput, UpdateLotInput},
    models::manufacturing::{BatchFilter, CreateBatchInput, UpdateBatchInput},
    state::AppState,
};

use super::dashboard::AdminUserView;

// =============================================================================
// Query Parameters
// =============================================================================

/// Query parameters for batch list.
#[derive(Debug, Deserialize)]
pub struct BatchesQuery {
    pub product_id: Option<String>,
    pub batch_number: Option<String>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub page: Option<i64>,
}

/// Query parameters for lot list.
#[derive(Debug, Deserialize)]
pub struct LotsQuery {
    pub has_remaining: Option<bool>,
    pub page: Option<i64>,
}

// =============================================================================
// Form Inputs
// =============================================================================

/// Form data for creating a batch.
#[derive(Debug, Deserialize)]
pub struct CreateBatchForm {
    pub batch_number: String,
    pub shopify_product_id: String,
    pub shopify_variant_id: Option<String>,
    pub quantity: i32,
    pub manufacture_date: NaiveDate,
    pub raw_cost_per_item: Decimal,
    pub label_cost_per_item: Decimal,
    pub outer_carton_cost_per_item: Decimal,
    pub currency_code: Option<String>,
    pub notes: Option<String>,
}

/// Form data for updating a batch.
#[derive(Debug, Deserialize)]
pub struct UpdateBatchForm {
    pub batch_number: Option<String>,
    pub quantity: Option<i32>,
    pub manufacture_date: Option<NaiveDate>,
    pub raw_cost_per_item: Option<Decimal>,
    pub label_cost_per_item: Option<Decimal>,
    pub outer_carton_cost_per_item: Option<Decimal>,
    pub currency_code: Option<String>,
    pub notes: Option<String>,
}

/// Form data for creating a lot.
#[derive(Debug, Deserialize)]
pub struct CreateLotForm {
    pub lot_number: String,
    pub quantity: i32,
    pub received_date: NaiveDate,
    pub shopify_location_id: Option<String>,
    pub notes: Option<String>,
}

/// Form data for updating a lot.
#[derive(Debug, Deserialize)]
pub struct UpdateLotForm {
    pub lot_number: Option<String>,
    pub quantity: Option<i32>,
    pub received_date: Option<NaiveDate>,
    pub shopify_location_id: Option<String>,
    pub notes: Option<String>,
}

// =============================================================================
// View Types
// =============================================================================

/// Batch view for templates.
#[derive(Debug, Clone)]
pub struct BatchView {
    pub id: i32,
    pub batch_number: String,
    pub shopify_product_id: String,
    pub shopify_variant_id: Option<String>,
    /// Product title from Shopify.
    pub product_title: Option<String>,
    /// Product image URL from Shopify.
    pub product_image: Option<String>,
    /// Variant title from Shopify (if variant selected).
    pub variant_title: Option<String>,
    /// Short numeric product ID for links.
    pub product_short_id: String,
    pub quantity: i32,
    pub manufacture_date: String,
    pub raw_cost_per_item: String,
    pub label_cost_per_item: String,
    pub outer_carton_cost_per_item: String,
    pub cost_per_unit: String,
    pub total_batch_cost: String,
    pub currency_code: String,
    pub notes: Option<String>,
    pub lots_received: i64,
}

/// Lot view for templates.
#[derive(Debug, Clone)]
pub struct LotView {
    pub id: i32,
    pub lot_number: String,
    pub quantity: i32,
    pub quantity_remaining: i64,
    pub received_date: String,
    pub shopify_location_id: Option<String>,
    /// Location name from Shopify.
    pub location_name: Option<String>,
    /// Short numeric location ID for display.
    pub location_short_id: Option<String>,
    pub notes: Option<String>,
}

/// Location view for dropdown selection.
#[derive(Debug, Clone)]
pub struct LocationView {
    pub id: String,
    pub name: String,
    pub is_active: bool,
}

/// Product search result for picker.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProductSearchResult {
    pub id: String,
    pub short_id: String,
    pub title: String,
    pub image_url: Option<String>,
    pub variants: Vec<VariantSearchResult>,
}

/// Variant search result for picker.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VariantSearchResult {
    pub id: String,
    pub short_id: String,
    pub title: String,
    pub sku: Option<String>,
    pub price: String,
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Extract numeric ID from Shopify GID.
fn extract_short_id(gid: &str) -> String {
    gid.split('/').next_back().unwrap_or(gid).to_string()
}

/// Fetch product info from Shopify and create `BatchView` with product details.
async fn enrich_batch_view(
    shopify: &crate::shopify::AdminClient,
    batch: &crate::models::manufacturing::ManufacturingBatchWithDetails,
) -> BatchView {
    let mut product_title = None;
    let mut product_image = None;
    let mut variant_title = None;

    // Try to fetch product info from Shopify
    if let Ok(Some(product)) = shopify.get_product(&batch.batch.shopify_product_id).await {
        product_title = Some(product.title.clone());
        product_image = product.images.first().map(|img| img.url.clone());

        // If variant is specified, find its title
        if let Some(ref variant_id) = batch.batch.shopify_variant_id {
            variant_title = product
                .variants
                .iter()
                .find(|v| &v.id == variant_id)
                .map(|v| v.title.clone());
        }
    }

    BatchView {
        id: batch.batch.id.as_i32(),
        batch_number: batch.batch.batch_number.clone(),
        shopify_product_id: batch.batch.shopify_product_id.clone(),
        shopify_variant_id: batch.batch.shopify_variant_id.clone(),
        product_title,
        product_image,
        variant_title,
        product_short_id: extract_short_id(&batch.batch.shopify_product_id),
        quantity: batch.batch.quantity,
        manufacture_date: batch.batch.manufacture_date.format("%Y-%m-%d").to_string(),
        raw_cost_per_item: format!("{:.4}", batch.batch.raw_cost_per_item),
        label_cost_per_item: format!("{:.4}", batch.batch.label_cost_per_item),
        outer_carton_cost_per_item: format!("{:.4}", batch.batch.outer_carton_cost_per_item),
        cost_per_unit: format!("{:.4}", batch.batch.cost_per_unit),
        total_batch_cost: format!("{:.2}", batch.batch.total_batch_cost),
        currency_code: batch.batch.currency_code.clone(),
        notes: batch.batch.notes.clone(),
        lots_received: batch.lots_received,
    }
}

/// Create a simple `BatchView` without Shopify enrichment (for error cases).
fn simple_batch_view(
    batch: &crate::models::manufacturing::ManufacturingBatchWithDetails,
) -> BatchView {
    BatchView {
        id: batch.batch.id.as_i32(),
        batch_number: batch.batch.batch_number.clone(),
        shopify_product_id: batch.batch.shopify_product_id.clone(),
        shopify_variant_id: batch.batch.shopify_variant_id.clone(),
        product_title: None,
        product_image: None,
        variant_title: None,
        product_short_id: extract_short_id(&batch.batch.shopify_product_id),
        quantity: batch.batch.quantity,
        manufacture_date: batch.batch.manufacture_date.format("%Y-%m-%d").to_string(),
        raw_cost_per_item: format!("{:.4}", batch.batch.raw_cost_per_item),
        label_cost_per_item: format!("{:.4}", batch.batch.label_cost_per_item),
        outer_carton_cost_per_item: format!("{:.4}", batch.batch.outer_carton_cost_per_item),
        cost_per_unit: format!("{:.4}", batch.batch.cost_per_unit),
        total_batch_cost: format!("{:.2}", batch.batch.total_batch_cost),
        currency_code: batch.batch.currency_code.clone(),
        notes: batch.batch.notes.clone(),
        lots_received: batch.lots_received,
    }
}

/// Fetch locations from Shopify.
async fn fetch_locations(shopify: &crate::shopify::AdminClient) -> Vec<LocationView> {
    match shopify.get_locations().await {
        Ok(connection) => connection
            .locations
            .into_iter()
            .filter(|l| l.is_active)
            .map(|l| LocationView {
                id: l.id,
                name: l.name,
                is_active: l.is_active,
            })
            .collect(),
        Err(e) => {
            tracing::warn!(?e, "Failed to fetch locations from Shopify");
            vec![]
        }
    }
}

/// Find location name by ID.
fn find_location_name(locations: &[LocationView], location_id: &str) -> Option<String> {
    locations
        .iter()
        .find(|l| l.id == location_id)
        .map(|l| l.name.clone())
}

/// Convert products to search results.
fn products_to_search_results(
    products: Vec<crate::shopify::types::AdminProduct>,
) -> Vec<ProductSearchResult> {
    products
        .into_iter()
        .map(|p| ProductSearchResult {
            short_id: extract_short_id(&p.id),
            id: p.id,
            title: p.title,
            image_url: p.images.first().map(|img| img.url.clone()),
            variants: p
                .variants
                .into_iter()
                .map(|v| VariantSearchResult {
                    short_id: extract_short_id(&v.id),
                    id: v.id,
                    title: v.title,
                    sku: v.sku,
                    price: format!("${}", v.price.amount),
                })
                .collect(),
        })
        .collect()
}

// =============================================================================
// Templates
// =============================================================================

/// Manufacturing batches index page.
#[derive(Template)]
#[template(path = "financials/manufacturing/index.html")]
struct ManufacturingIndexTemplate {
    admin_user: AdminUserView,
    current_path: String,
    batches: Vec<BatchView>,
    query: BatchesQuery,
    page: i64,
    total_count: i64,
    has_next: bool,
    has_prev: bool,
}

/// New batch form page.
#[derive(Template)]
#[template(path = "financials/manufacturing/new.html")]
struct ManufacturingNewTemplate {
    admin_user: AdminUserView,
    current_path: String,
    /// Recent products for quick selection.
    recent_products: Vec<ProductSearchResult>,
}

/// Batch detail page.
#[derive(Template)]
#[template(path = "financials/manufacturing/show.html")]
struct ManufacturingShowTemplate {
    admin_user: AdminUserView,
    current_path: String,
    batch: BatchView,
    lots: Vec<LotView>,
}

/// Edit batch form page.
#[derive(Template)]
#[template(path = "financials/manufacturing/edit.html")]
struct ManufacturingEditTemplate {
    admin_user: AdminUserView,
    current_path: String,
    batch: BatchView,
}

/// New lot form page.
#[derive(Template)]
#[template(path = "financials/manufacturing/lots/new.html")]
struct LotNewTemplate {
    admin_user: AdminUserView,
    current_path: String,
    batch: BatchView,
    /// Available locations for dropdown.
    locations: Vec<LocationView>,
}

/// Edit lot form page.
#[derive(Template)]
#[template(path = "financials/manufacturing/lots/edit.html")]
struct LotEditTemplate {
    admin_user: AdminUserView,
    current_path: String,
    batch: BatchView,
    lot: LotView,
    /// Available locations for dropdown.
    locations: Vec<LocationView>,
}

/// Product search results template (HTMX partial).
#[derive(Template)]
#[template(path = "financials/manufacturing/_product_search_results.html")]
struct ProductSearchResultsTemplate {
    products: Vec<ProductSearchResult>,
}

// =============================================================================
// Route Handlers
// =============================================================================

/// Financials landing page - redirects to manufacturing.
#[instrument(skip_all)]
pub async fn index() -> Redirect {
    Redirect::to("/financials/manufacturing")
}

/// Manufacturing batches index.
#[instrument(skip_all)]
pub async fn manufacturing_index(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Query(query): Query<BatchesQuery>,
) -> impl IntoResponse {
    let repo = ManufacturingRepository::new(state.pool());

    let page = query.page.unwrap_or(1).max(1);
    let limit = 25_i64;
    let offset = (page - 1) * limit;

    let filter = BatchFilter {
        shopify_product_id: query.product_id.clone(),
        batch_number: query.batch_number.clone(),
        start_date: query.start_date,
        end_date: query.end_date,
        limit: Some(limit),
        offset: Some(offset),
    };

    let batches = match repo.list_batches(&filter).await {
        Ok(batches) => batches,
        Err(e) => {
            tracing::error!(?e, "Failed to list batches");
            return Html(format!("Error: {e}")).into_response();
        }
    };

    let total_count = match repo.count_batches(&filter).await {
        Ok(count) => count,
        Err(e) => {
            tracing::error!(?e, "Failed to count batches");
            0
        }
    };

    // Convert to views with lots_received counts and product info
    let mut batch_views = Vec::with_capacity(batches.len());
    for batch in &batches {
        let lots_received = repo.get_lots_received(batch.id).await.unwrap_or(0);

        // Try to fetch product info from Shopify
        let (product_title, product_image, variant_title) = if let Ok(Some(product)) =
            state.shopify().get_product(&batch.shopify_product_id).await
        {
            let title = Some(product.title.clone());
            let image = product.images.first().map(|img| img.url.clone());
            let var_title = batch.shopify_variant_id.as_ref().and_then(|vid| {
                product
                    .variants
                    .iter()
                    .find(|v| &v.id == vid)
                    .map(|v| v.title.clone())
            });
            (title, image, var_title)
        } else {
            (None, None, None)
        };

        batch_views.push(BatchView {
            id: batch.id.as_i32(),
            batch_number: batch.batch_number.clone(),
            shopify_product_id: batch.shopify_product_id.clone(),
            shopify_variant_id: batch.shopify_variant_id.clone(),
            product_title,
            product_image,
            variant_title,
            product_short_id: extract_short_id(&batch.shopify_product_id),
            quantity: batch.quantity,
            manufacture_date: batch.manufacture_date.to_string(),
            raw_cost_per_item: format!("{:.4}", batch.raw_cost_per_item),
            label_cost_per_item: format!("{:.4}", batch.label_cost_per_item),
            outer_carton_cost_per_item: format!("{:.4}", batch.outer_carton_cost_per_item),
            cost_per_unit: format!("{:.4}", batch.cost_per_unit),
            total_batch_cost: format!("{:.2}", batch.total_batch_cost),
            currency_code: batch.currency_code.clone(),
            notes: batch.notes.clone(),
            lots_received,
        });
    }

    let has_next = (page * limit) < total_count;
    let has_prev = page > 1;

    let template = ManufacturingIndexTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/financials/manufacturing".to_string(),
        batches: batch_views,
        query,
        page,
        total_count,
        has_next,
        has_prev,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
    .into_response()
}

/// New batch form.
#[instrument(skip_all)]
pub async fn manufacturing_new(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
) -> impl IntoResponse {
    // Fetch recent products for quick selection
    let recent_products = match state.shopify().get_products(10, None, None).await {
        Ok(conn) => products_to_search_results(conn.products),
        Err(e) => {
            tracing::warn!(?e, "Failed to fetch products");
            vec![]
        }
    };

    let template = ManufacturingNewTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/financials/manufacturing".to_string(),
        recent_products,
    };
    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
}

/// Create batch.
#[instrument(skip_all)]
pub async fn manufacturing_create(
    State(state): State<AppState>,
    RequireAdminAuth(_user): RequireAdminAuth,
    Form(form): Form<CreateBatchForm>,
) -> impl IntoResponse {
    let repo = ManufacturingRepository::new(state.pool());

    let input = CreateBatchInput {
        batch_number: form.batch_number,
        shopify_product_id: form.shopify_product_id,
        shopify_variant_id: form.shopify_variant_id,
        quantity: form.quantity,
        manufacture_date: form.manufacture_date,
        raw_cost_per_item: form.raw_cost_per_item,
        label_cost_per_item: form.label_cost_per_item,
        outer_carton_cost_per_item: form.outer_carton_cost_per_item,
        currency_code: form.currency_code.unwrap_or_else(|| "USD".to_string()),
        notes: form.notes,
    };

    match repo.create_batch(&input).await {
        Ok(batch) => Redirect::to(&format!("/financials/manufacturing/{}", batch.id.as_i32()))
            .into_response(),
        Err(RepositoryError::Conflict(msg)) => Html(format!("Error: {msg}")).into_response(),
        Err(e) => {
            tracing::error!(?e, "Failed to create batch");
            Html(format!("Error: {e}")).into_response()
        }
    }
}

/// Batch detail page.
#[instrument(skip_all)]
pub async fn manufacturing_show(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    let mfg_repo = ManufacturingRepository::new(state.pool());
    let lot_repo = InventoryLotRepository::new(state.pool());

    let batch_id = naked_pineapple_core::ManufacturingBatchId::new(id);

    let batch = match mfg_repo.get_batch(batch_id).await {
        Ok(Some(batch)) => batch,
        Ok(None) => return Redirect::to("/financials/manufacturing").into_response(),
        Err(e) => {
            tracing::error!(?e, "Failed to get batch");
            return Html(format!("Error: {e}")).into_response();
        }
    };

    let lots_received = mfg_repo.get_lots_received(batch_id).await.unwrap_or(0);
    let lots = match lot_repo.list_lots_for_batch(batch_id).await {
        Ok(lots) => lots,
        Err(e) => {
            tracing::error!(?e, "Failed to list lots");
            Vec::new()
        }
    };

    // Fetch product info from Shopify
    let (product_title, product_image, variant_title) =
        if let Ok(Some(product)) = state.shopify().get_product(&batch.shopify_product_id).await {
            let title = Some(product.title.clone());
            let image = product.images.first().map(|img| img.url.clone());
            let var_title = batch.shopify_variant_id.as_ref().and_then(|vid| {
                product
                    .variants
                    .iter()
                    .find(|v| &v.id == vid)
                    .map(|v| v.title.clone())
            });
            (title, image, var_title)
        } else {
            (None, None, None)
        };

    // Fetch locations for lot names
    let locations = fetch_locations(state.shopify()).await;

    let batch_view = BatchView {
        id: batch.id.as_i32(),
        batch_number: batch.batch_number.clone(),
        shopify_product_id: batch.shopify_product_id.clone(),
        shopify_variant_id: batch.shopify_variant_id.clone(),
        product_title,
        product_image,
        variant_title,
        product_short_id: extract_short_id(&batch.shopify_product_id),
        quantity: batch.quantity,
        manufacture_date: batch.manufacture_date.to_string(),
        raw_cost_per_item: format!("{:.4}", batch.raw_cost_per_item),
        label_cost_per_item: format!("{:.4}", batch.label_cost_per_item),
        outer_carton_cost_per_item: format!("{:.4}", batch.outer_carton_cost_per_item),
        cost_per_unit: format!("{:.4}", batch.cost_per_unit),
        total_batch_cost: format!("{:.2}", batch.total_batch_cost),
        currency_code: batch.currency_code,
        notes: batch.notes,
        lots_received,
    };

    let lot_views: Vec<LotView> = lots
        .into_iter()
        .map(|lwr| {
            let location_name = lwr
                .lot
                .shopify_location_id
                .as_ref()
                .and_then(|lid| find_location_name(&locations, lid));
            let location_short_id = lwr
                .lot
                .shopify_location_id
                .as_ref()
                .map(|lid| extract_short_id(lid));
            LotView {
                id: lwr.lot.id.as_i32(),
                lot_number: lwr.lot.lot_number,
                quantity: lwr.lot.quantity,
                quantity_remaining: lwr.quantity_remaining,
                received_date: lwr.lot.received_date.to_string(),
                shopify_location_id: lwr.lot.shopify_location_id,
                location_name,
                location_short_id,
                notes: lwr.lot.notes,
            }
        })
        .collect();

    let template = ManufacturingShowTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/financials/manufacturing".to_string(),
        batch: batch_view,
        lots: lot_views,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
    .into_response()
}

/// Edit batch form.
#[instrument(skip_all)]
pub async fn manufacturing_edit(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    let repo = ManufacturingRepository::new(state.pool());
    let batch_id = naked_pineapple_core::ManufacturingBatchId::new(id);

    let batch = match repo.get_batch(batch_id).await {
        Ok(Some(batch)) => batch,
        Ok(None) => return Redirect::to("/financials/manufacturing").into_response(),
        Err(e) => {
            tracing::error!(?e, "Failed to get batch");
            return Html(format!("Error: {e}")).into_response();
        }
    };

    let lots_received = repo.get_lots_received(batch_id).await.unwrap_or(0);

    // Fetch product info from Shopify
    let (product_title, product_image, variant_title) =
        if let Ok(Some(product)) = state.shopify().get_product(&batch.shopify_product_id).await {
            let title = Some(product.title.clone());
            let image = product.images.first().map(|img| img.url.clone());
            let var_title = batch.shopify_variant_id.as_ref().and_then(|vid| {
                product
                    .variants
                    .iter()
                    .find(|v| &v.id == vid)
                    .map(|v| v.title.clone())
            });
            (title, image, var_title)
        } else {
            (None, None, None)
        };

    let batch_view = BatchView {
        id: batch.id.as_i32(),
        batch_number: batch.batch_number.clone(),
        shopify_product_id: batch.shopify_product_id.clone(),
        shopify_variant_id: batch.shopify_variant_id.clone(),
        product_title,
        product_image,
        variant_title,
        product_short_id: extract_short_id(&batch.shopify_product_id),
        quantity: batch.quantity,
        manufacture_date: batch.manufacture_date.to_string(),
        raw_cost_per_item: format!("{:.4}", batch.raw_cost_per_item),
        label_cost_per_item: format!("{:.4}", batch.label_cost_per_item),
        outer_carton_cost_per_item: format!("{:.4}", batch.outer_carton_cost_per_item),
        cost_per_unit: format!("{:.4}", batch.cost_per_unit),
        total_batch_cost: format!("{:.2}", batch.total_batch_cost),
        currency_code: batch.currency_code,
        notes: batch.notes,
        lots_received,
    };

    let template = ManufacturingEditTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/financials/manufacturing".to_string(),
        batch: batch_view,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
    .into_response()
}

/// Update batch.
#[instrument(skip_all)]
pub async fn manufacturing_update(
    State(state): State<AppState>,
    RequireAdminAuth(_user): RequireAdminAuth,
    Path(id): Path<i32>,
    Form(form): Form<UpdateBatchForm>,
) -> impl IntoResponse {
    let repo = ManufacturingRepository::new(state.pool());
    let batch_id = naked_pineapple_core::ManufacturingBatchId::new(id);

    let input = UpdateBatchInput {
        batch_number: form.batch_number,
        quantity: form.quantity,
        manufacture_date: form.manufacture_date,
        raw_cost_per_item: form.raw_cost_per_item,
        label_cost_per_item: form.label_cost_per_item,
        outer_carton_cost_per_item: form.outer_carton_cost_per_item,
        currency_code: form.currency_code,
        notes: form.notes,
    };

    match repo.update_batch(batch_id, &input).await {
        Ok(_) => Redirect::to(&format!("/financials/manufacturing/{id}")).into_response(),
        Err(RepositoryError::NotFound) => Redirect::to("/financials/manufacturing").into_response(),
        Err(e) => {
            tracing::error!(?e, "Failed to update batch");
            Html(format!("Error: {e}")).into_response()
        }
    }
}

/// Delete batch.
#[instrument(skip_all)]
pub async fn manufacturing_delete(
    State(state): State<AppState>,
    RequireAdminAuth(_user): RequireAdminAuth,
    Path(id): Path<i32>,
) -> impl IntoResponse {
    let repo = ManufacturingRepository::new(state.pool());
    let batch_id = naked_pineapple_core::ManufacturingBatchId::new(id);

    match repo.delete_batch(batch_id).await {
        Ok(_) => Redirect::to("/financials/manufacturing"),
        Err(e) => {
            tracing::error!(?e, "Failed to delete batch");
            Redirect::to(&format!("/financials/manufacturing/{id}"))
        }
    }
}

// =============================================================================
// Lot Handlers
// =============================================================================

/// New lot form.
#[instrument(skip_all)]
pub async fn lot_new(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Path(batch_id): Path<i32>,
) -> impl IntoResponse {
    let repo = ManufacturingRepository::new(state.pool());
    let id = naked_pineapple_core::ManufacturingBatchId::new(batch_id);

    let batch = match repo.get_batch(id).await {
        Ok(Some(batch)) => batch,
        Ok(None) => return Redirect::to("/financials/manufacturing").into_response(),
        Err(e) => {
            tracing::error!(?e, "Failed to get batch");
            return Html(format!("Error: {e}")).into_response();
        }
    };

    let lots_received = repo.get_lots_received(id).await.unwrap_or(0);

    // Fetch product info from Shopify
    let (product_title, product_image, variant_title) =
        if let Ok(Some(product)) = state.shopify().get_product(&batch.shopify_product_id).await {
            let title = Some(product.title.clone());
            let image = product.images.first().map(|img| img.url.clone());
            let var_title = batch.shopify_variant_id.as_ref().and_then(|vid| {
                product
                    .variants
                    .iter()
                    .find(|v| &v.id == vid)
                    .map(|v| v.title.clone())
            });
            (title, image, var_title)
        } else {
            (None, None, None)
        };

    // Fetch locations for dropdown
    let locations = fetch_locations(state.shopify()).await;

    let batch_view = BatchView {
        id: batch.id.as_i32(),
        batch_number: batch.batch_number.clone(),
        shopify_product_id: batch.shopify_product_id.clone(),
        shopify_variant_id: batch.shopify_variant_id.clone(),
        product_title,
        product_image,
        variant_title,
        product_short_id: extract_short_id(&batch.shopify_product_id),
        quantity: batch.quantity,
        manufacture_date: batch.manufacture_date.to_string(),
        raw_cost_per_item: format!("{:.4}", batch.raw_cost_per_item),
        label_cost_per_item: format!("{:.4}", batch.label_cost_per_item),
        outer_carton_cost_per_item: format!("{:.4}", batch.outer_carton_cost_per_item),
        cost_per_unit: format!("{:.4}", batch.cost_per_unit),
        total_batch_cost: format!("{:.2}", batch.total_batch_cost),
        currency_code: batch.currency_code,
        notes: batch.notes,
        lots_received,
    };

    let template = LotNewTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/financials/manufacturing".to_string(),
        batch: batch_view,
        locations,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
    .into_response()
}

/// Create lot.
#[instrument(skip_all)]
pub async fn lot_create(
    State(state): State<AppState>,
    RequireAdminAuth(_user): RequireAdminAuth,
    Path(batch_id): Path<i32>,
    Form(form): Form<CreateLotForm>,
) -> impl IntoResponse {
    let repo = InventoryLotRepository::new(state.pool());
    let id = naked_pineapple_core::ManufacturingBatchId::new(batch_id);

    let input = CreateLotInput {
        batch_id: id,
        lot_number: form.lot_number,
        quantity: form.quantity,
        received_date: form.received_date,
        shopify_location_id: form.shopify_location_id,
        notes: form.notes,
    };

    match repo.create_lot(&input).await {
        Ok(_) => Redirect::to(&format!("/financials/manufacturing/{batch_id}")).into_response(),
        Err(e) => {
            tracing::error!(?e, "Failed to create lot");
            Html(format!("Error: {e}")).into_response()
        }
    }
}

/// Edit lot form.
#[instrument(skip_all)]
pub async fn lot_edit(
    State(state): State<AppState>,
    RequireAdminAuth(user): RequireAdminAuth,
    Path((batch_id, lot_id)): Path<(i32, i32)>,
) -> impl IntoResponse {
    let mfg_repo = ManufacturingRepository::new(state.pool());
    let lot_repo = InventoryLotRepository::new(state.pool());

    let b_id = naked_pineapple_core::ManufacturingBatchId::new(batch_id);
    let l_id = naked_pineapple_core::InventoryLotId::new(lot_id);

    let batch = match mfg_repo.get_batch(b_id).await {
        Ok(Some(batch)) => batch,
        Ok(None) => return Redirect::to("/financials/manufacturing").into_response(),
        Err(e) => {
            tracing::error!(?e, "Failed to get batch");
            return Html(format!("Error: {e}")).into_response();
        }
    };

    let lot_with_remaining = match lot_repo.get_lot_with_remaining(l_id).await {
        Ok(Some(lot)) => lot,
        Ok(None) => {
            return Redirect::to(&format!("/financials/manufacturing/{batch_id}")).into_response();
        }
        Err(e) => {
            tracing::error!(?e, "Failed to get lot");
            return Html(format!("Error: {e}")).into_response();
        }
    };

    let lots_received = mfg_repo.get_lots_received(b_id).await.unwrap_or(0);

    // Fetch product info from Shopify
    let (product_title, product_image, variant_title) =
        if let Ok(Some(product)) = state.shopify().get_product(&batch.shopify_product_id).await {
            let title = Some(product.title.clone());
            let image = product.images.first().map(|img| img.url.clone());
            let var_title = batch.shopify_variant_id.as_ref().and_then(|vid| {
                product
                    .variants
                    .iter()
                    .find(|v| &v.id == vid)
                    .map(|v| v.title.clone())
            });
            (title, image, var_title)
        } else {
            (None, None, None)
        };

    // Fetch locations for dropdown
    let locations = fetch_locations(state.shopify()).await;

    // Resolve location name for current lot
    let location_name = lot_with_remaining
        .lot
        .shopify_location_id
        .as_ref()
        .and_then(|lid| find_location_name(&locations, lid));
    let location_short_id = lot_with_remaining
        .lot
        .shopify_location_id
        .as_ref()
        .map(|lid| extract_short_id(lid));

    let batch_view = BatchView {
        id: batch.id.as_i32(),
        batch_number: batch.batch_number.clone(),
        shopify_product_id: batch.shopify_product_id.clone(),
        shopify_variant_id: batch.shopify_variant_id.clone(),
        product_title,
        product_image,
        variant_title,
        product_short_id: extract_short_id(&batch.shopify_product_id),
        quantity: batch.quantity,
        manufacture_date: batch.manufacture_date.to_string(),
        raw_cost_per_item: format!("{:.4}", batch.raw_cost_per_item),
        label_cost_per_item: format!("{:.4}", batch.label_cost_per_item),
        outer_carton_cost_per_item: format!("{:.4}", batch.outer_carton_cost_per_item),
        cost_per_unit: format!("{:.4}", batch.cost_per_unit),
        total_batch_cost: format!("{:.2}", batch.total_batch_cost),
        currency_code: batch.currency_code,
        notes: batch.notes,
        lots_received,
    };

    let lot_view = LotView {
        id: lot_with_remaining.lot.id.as_i32(),
        lot_number: lot_with_remaining.lot.lot_number,
        quantity: lot_with_remaining.lot.quantity,
        quantity_remaining: lot_with_remaining.quantity_remaining,
        received_date: lot_with_remaining.lot.received_date.to_string(),
        shopify_location_id: lot_with_remaining.lot.shopify_location_id,
        location_name,
        location_short_id,
        notes: lot_with_remaining.lot.notes,
    };

    let template = LotEditTemplate {
        admin_user: AdminUserView::from(&user),
        current_path: "/financials/manufacturing".to_string(),
        batch: batch_view,
        lot: lot_view,
        locations,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|e| format!("Template error: {e}")),
    )
    .into_response()
}

/// Update lot.
#[instrument(skip_all)]
pub async fn lot_update(
    State(state): State<AppState>,
    RequireAdminAuth(_user): RequireAdminAuth,
    Path((batch_id, lot_id)): Path<(i32, i32)>,
    Form(form): Form<UpdateLotForm>,
) -> impl IntoResponse {
    let repo = InventoryLotRepository::new(state.pool());
    let l_id = naked_pineapple_core::InventoryLotId::new(lot_id);

    let input = UpdateLotInput {
        lot_number: form.lot_number,
        quantity: form.quantity,
        received_date: form.received_date,
        shopify_location_id: form.shopify_location_id,
        notes: form.notes,
    };

    match repo.update_lot(l_id, &input).await {
        Ok(_) => Redirect::to(&format!("/financials/manufacturing/{batch_id}")).into_response(),
        Err(RepositoryError::NotFound) => {
            Redirect::to(&format!("/financials/manufacturing/{batch_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(?e, "Failed to update lot");
            Html(format!("Error: {e}")).into_response()
        }
    }
}

/// Delete lot.
#[instrument(skip_all)]
pub async fn lot_delete(
    State(state): State<AppState>,
    RequireAdminAuth(_user): RequireAdminAuth,
    Path((batch_id, lot_id)): Path<(i32, i32)>,
) -> impl IntoResponse {
    let repo = InventoryLotRepository::new(state.pool());
    let l_id = naked_pineapple_core::InventoryLotId::new(lot_id);

    match repo.delete_lot(l_id).await {
        Ok(_) => Redirect::to(&format!("/financials/manufacturing/{batch_id}")),
        Err(e) => {
            tracing::error!(?e, "Failed to delete lot");
            Redirect::to(&format!("/financials/manufacturing/{batch_id}"))
        }
    }
}

// =============================================================================
// Router
// =============================================================================

/// Build the financials router.
pub fn router() -> Router<AppState> {
    Router::new()
        // Financials landing
        .route("/financials", get(index))
        // Manufacturing batches
        .route(
            "/financials/manufacturing",
            get(manufacturing_index).post(manufacturing_create),
        )
        .route("/financials/manufacturing/new", get(manufacturing_new))
        .route(
            "/financials/manufacturing/{id}",
            get(manufacturing_show).post(manufacturing_update),
        )
        .route(
            "/financials/manufacturing/{id}/edit",
            get(manufacturing_edit),
        )
        .route(
            "/financials/manufacturing/{id}/delete",
            post(manufacturing_delete),
        )
        // Inventory lots (nested under batches)
        .route(
            "/financials/manufacturing/{batch_id}/lots/new",
            get(lot_new),
        )
        .route(
            "/financials/manufacturing/{batch_id}/lots",
            post(lot_create),
        )
        .route(
            "/financials/manufacturing/{batch_id}/lots/{id}/edit",
            get(lot_edit),
        )
        .route(
            "/financials/manufacturing/{batch_id}/lots/{id}",
            post(lot_update),
        )
        .route(
            "/financials/manufacturing/{batch_id}/lots/{id}/delete",
            post(lot_delete),
        )
}
