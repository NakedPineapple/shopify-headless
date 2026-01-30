//! Order detail page handlers.

use std::collections::HashMap;

use askama::Template;
use axum::{
    Form,
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use naked_pineapple_core::{AdminUserId, InventoryLotId, LotAllocationId};
use serde::Deserialize;
use tracing::instrument;

use crate::{
    db::inventory_lot::InventoryLotRepository,
    filters,
    middleware::auth::RequireAdminAuth,
    models::inventory_lot::{AllocateLotInput, LotAllocation},
    state::AppState,
};

use super::super::dashboard::AdminUserView;
use super::types::{AvailableLotView, LineItemAllocationView, OrderDetailView};

/// Build allocation views for a line item from its allocations.
async fn build_allocation_views(
    allocations: &[LotAllocation],
    lot_repo: &InventoryLotRepository<'_>,
) -> (Vec<LineItemAllocationView>, i64) {
    let mut views = Vec::new();
    let mut total_allocated: i64 = 0;

    for alloc in allocations {
        if let Ok(Some(lot_with_batch)) = lot_repo.get_lot_with_batch_info(alloc.lot_id).await {
            total_allocated += i64::from(alloc.quantity);
            views.push(LineItemAllocationView {
                id: alloc.id.as_i32(),
                lot_id: alloc.lot_id.as_i32(),
                lot_number: lot_with_batch.lot.lot_number.clone(),
                batch_number: lot_with_batch.batch_number.clone(),
                quantity: alloc.quantity,
                cost_per_unit: format!("${:.2}", lot_with_batch.cost_per_unit),
                allocated_at: alloc.allocated_at.format("%Y-%m-%d").to_string(),
            });
        }
    }

    (views, total_allocated)
}

/// Order detail page template.
#[derive(Template)]
#[template(path = "orders/show.html")]
pub struct OrderShowTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub order: OrderDetailView,
    pub available_lots: Vec<AvailableLotView>,
    pub error: Option<String>,
}

/// Note update form input.
#[derive(Debug, Deserialize)]
pub struct NoteFormInput {
    pub note: Option<String>,
}

/// Cancel form input.
#[derive(Debug, Deserialize)]
pub struct CancelFormInput {
    pub reason: Option<String>,
    pub notify_customer: Option<String>,
    pub refund: Option<String>,
    pub restock: Option<String>,
}

/// Order detail page handler.
#[instrument(skip(admin, state))]
pub async fn show(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/Order/{id}")
    };

    match state.shopify().get_order(&order_id).await {
        Ok(Some(order)) => {
            let mut order_view = OrderDetailView::from(&order);

            // Fetch lot allocations for this order
            let lot_repo = InventoryLotRepository::new(state.pool());
            if let Ok(allocations) = lot_repo.get_allocations_for_order(&order_id).await {
                // Group allocations by line item ID
                let mut alloc_by_line_item: HashMap<String, Vec<_>> = HashMap::new();
                for alloc in allocations {
                    alloc_by_line_item
                        .entry(alloc.shopify_line_item_id.clone())
                        .or_default()
                        .push(alloc);
                }

                // Enrich line items with allocation data
                for line_item in &mut order_view.line_items {
                    if let Some(allocs) = alloc_by_line_item.get(&line_item.id) {
                        let (views, total_allocated) =
                            build_allocation_views(allocs, &lot_repo).await;
                        line_item.allocations = views;
                        line_item.allocated_quantity = total_allocated;
                        line_item.needed_quantity = line_item.quantity - total_allocated;
                        line_item.is_fully_allocated = total_allocated >= line_item.quantity;
                    }
                }

                // Also enrich fulfillment order line items with allocation data
                for fo in &mut order_view.fulfillment_orders {
                    for fo_line_item in &mut fo.line_items {
                        if let Some(allocs) = alloc_by_line_item.get(&fo_line_item.id) {
                            let (views, total_allocated) =
                                build_allocation_views(allocs, &lot_repo).await;
                            fo_line_item.allocations = views;
                            fo_line_item.allocated_quantity = total_allocated;
                            fo_line_item.needed_quantity = fo_line_item.quantity - total_allocated;
                            fo_line_item.is_fully_allocated =
                                total_allocated >= fo_line_item.quantity;
                        }
                    }
                }
            }

            // Collect unique product IDs for fetching available lots (from both regular
            // line items and fulfillment order line items)
            let mut product_ids_set = std::collections::HashSet::new();
            for li in &order_view.line_items {
                if let Some(ref pid) = li.product_id {
                    product_ids_set.insert(pid.clone());
                }
            }
            for fo in &order_view.fulfillment_orders {
                for li in &fo.line_items {
                    if let Some(ref pid) = li.product_id {
                        product_ids_set.insert(pid.clone());
                    }
                }
            }
            let product_ids: Vec<String> = product_ids_set.into_iter().collect();

            // Fetch available lots for all products in this order
            let mut available_lots: Vec<AvailableLotView> = Vec::new();
            for product_id in &product_ids {
                if let Ok(lots) = lot_repo.get_available_lots_for_product(product_id).await {
                    for lot in lots {
                        available_lots.push(AvailableLotView {
                            id: lot.lot.id.as_i32(),
                            lot_number: lot.lot.lot_number,
                            batch_number: lot.batch_number,
                            quantity_remaining: lot.quantity_remaining,
                            cost_per_unit: format!("${:.2}", lot.cost_per_unit),
                            received_date: lot.lot.received_date.format("%Y-%m-%d").to_string(),
                        });
                    }
                }
            }

            let template = OrderShowTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/orders".to_string(),
                order: order_view,
                available_lots,
                error: None,
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Order not found").into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch order: {f}", f = e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch order").into_response()
        }
    }
}

/// Update order note handler.
#[instrument(skip(admin, state))]
pub async fn update_note(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<NoteFormInput>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    match state
        .shopify()
        .update_order_note(&order_id, input.note.as_deref())
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %order_id, "Order note updated");
            // Redirect back to order detail
            let numeric_id = id.split('/').next_back().unwrap_or(&id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to update order note");
            // Fetch order again and show error
            match state.shopify().get_order(&order_id).await {
                Ok(Some(order)) => {
                    let template = OrderShowTemplate {
                        admin_user: AdminUserView::from(&admin),
                        current_path: "/orders".to_string(),
                        order: OrderDetailView::from(&order),
                        available_lots: vec![],
                        error: Some(format!("Failed to update note: {e}")),
                    };
                    Html(template.render().unwrap_or_else(|e| {
                        tracing::error!("Template render error: {}", e);
                        "Internal Server Error".to_string()
                    }))
                    .into_response()
                }
                _ => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to update order").into_response(),
            }
        }
    }
}

/// Mark order as paid handler.
#[instrument(skip(_admin, state))]
pub async fn mark_paid(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    match state.shopify().mark_order_as_paid(&order_id).await {
        Ok(()) => {
            tracing::info!(order_id = %order_id, "Order marked as paid");
            let numeric_id = id.split('/').next_back().unwrap_or(&id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to mark order as paid");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to mark as paid: {e}"),
            )
                .into_response()
        }
    }
}

/// Cancel order handler.
#[instrument(skip(_admin, state))]
pub async fn cancel(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<CancelFormInput>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    let notify = input.notify_customer.as_deref() == Some("on");
    let refund = input.refund.as_deref() == Some("on");
    let restock = input.restock.as_deref() == Some("on");

    match state
        .shopify()
        .cancel_order(&order_id, input.reason.as_deref(), notify, refund, restock)
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %order_id, reason = ?input.reason, "Order cancelled");
            Redirect::to("/orders").into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to cancel order");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to cancel order: {e}"),
            )
                .into_response()
        }
    }
}

// =============================================================================
// Lot Allocation Handlers
// =============================================================================

/// Form input for allocating a lot to a line item.
#[derive(Debug, Deserialize)]
pub struct AllocateLotFormInput {
    pub lot_id: i32,
    pub line_item_id: String,
    pub quantity: i32,
}

/// Allocate a lot to an order line item.
#[instrument(skip(admin, state))]
pub async fn allocate_lot(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<String>,
    Form(input): Form<AllocateLotFormInput>,
) -> impl IntoResponse {
    let full_order_id = if order_id.starts_with("gid://") {
        order_id.clone()
    } else {
        format!("gid://shopify/Order/{order_id}")
    };

    let lot_repo = InventoryLotRepository::new(state.pool());
    let alloc_input = AllocateLotInput {
        lot_id: InventoryLotId::new(input.lot_id),
        shopify_order_id: full_order_id.clone(),
        shopify_line_item_id: input.line_item_id,
        quantity: input.quantity,
    };

    match lot_repo
        .allocate(&alloc_input, Some(AdminUserId::new(admin.id.as_i32())))
        .await
    {
        Ok(allocation) => {
            tracing::info!(
                order_id = %full_order_id,
                lot_id = %input.lot_id,
                quantity = %allocation.quantity,
                "Lot allocated to order line item"
            );
            let numeric_id = order_id.split('/').next_back().unwrap_or(&order_id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(
                order_id = %full_order_id,
                lot_id = %input.lot_id,
                error = %e,
                "Failed to allocate lot"
            );
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to allocate lot: {e}"),
            )
                .into_response()
        }
    }
}

/// Remove a lot allocation.
#[instrument(skip(_admin, state))]
pub async fn deallocate_lot(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path((order_id, allocation_id)): Path<(String, i32)>,
) -> impl IntoResponse {
    let lot_repo = InventoryLotRepository::new(state.pool());

    match lot_repo
        .delete_allocation(LotAllocationId::new(allocation_id))
        .await
    {
        Ok(true) => {
            tracing::info!(allocation_id = %allocation_id, "Lot allocation removed");
            let numeric_id = order_id.split('/').next_back().unwrap_or(&order_id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Ok(false) => (StatusCode::NOT_FOUND, "Allocation not found").into_response(),
        Err(e) => {
            tracing::error!(allocation_id = %allocation_id, error = %e, "Failed to remove allocation");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to remove allocation: {e}"),
            )
                .into_response()
        }
    }
}

/// Auto-allocate lots to a line item using FIFO.
#[instrument(skip(admin, state))]
pub async fn auto_allocate_lot(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(order_id): Path<String>,
    Form(input): Form<AutoAllocateFormInput>,
) -> impl IntoResponse {
    let full_order_id = if order_id.starts_with("gid://") {
        order_id.clone()
    } else {
        format!("gid://shopify/Order/{order_id}")
    };

    let lot_repo = InventoryLotRepository::new(state.pool());

    match lot_repo
        .auto_allocate_fifo(
            &input.product_id,
            &full_order_id,
            &input.line_item_id,
            input.quantity,
            Some(AdminUserId::new(admin.id.as_i32())),
        )
        .await
    {
        Ok(allocations) => {
            let total_allocated: i32 = allocations.iter().map(|a| a.quantity).sum();
            tracing::info!(
                order_id = %full_order_id,
                product_id = %input.product_id,
                allocated = %total_allocated,
                lots_used = %allocations.len(),
                "Auto-allocated lots to order line item"
            );
            let numeric_id = order_id.split('/').next_back().unwrap_or(&order_id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(
                order_id = %full_order_id,
                product_id = %input.product_id,
                error = %e,
                "Failed to auto-allocate lots"
            );
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to auto-allocate: {e}"),
            )
                .into_response()
        }
    }
}

/// Form input for auto-allocating lots.
#[derive(Debug, Deserialize)]
pub struct AutoAllocateFormInput {
    pub product_id: String,
    pub line_item_id: String,
    pub quantity: i32,
}
