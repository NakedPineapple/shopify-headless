//! Single order action handlers.

use axum::{
    Form,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use serde::Deserialize;
use tracing::instrument;

use crate::{
    middleware::auth::RequireAdminAuth,
    shopify::types::{
        FulfillmentHoldInput, FulfillmentHoldReason, RefundCreateInput, RefundLineItemInput,
        RefundRestockType, ReturnCreateInput, ReturnLineItemCreateInput,
    },
    state::AppState,
};

// =============================================================================
// Input Types
// =============================================================================

/// Input for adding/removing a single tag.
#[derive(Debug, Deserialize)]
pub struct TagInput {
    /// Tag to add or remove.
    pub tag: String,
    /// Action: "add" or "remove".
    pub action: String,
}

/// Input for creating a fulfillment.
#[derive(Debug, Deserialize)]
pub struct FulfillInput {
    /// Fulfillment order ID to fulfill.
    pub fulfillment_order_id: String,
    /// Optional tracking company.
    pub tracking_company: Option<String>,
    /// Optional tracking number.
    pub tracking_number: Option<String>,
    /// Optional tracking URL.
    pub tracking_url: Option<String>,
}

/// Input for holding a fulfillment order.
#[derive(Debug, Deserialize)]
pub struct HoldInput {
    /// Reason for the hold.
    pub reason: String,
    /// Additional notes.
    pub reason_notes: Option<String>,
}

/// Input for creating a refund.
#[derive(Debug, Deserialize)]
pub struct RefundInput {
    /// Refund note.
    pub note: Option<String>,
    /// Whether to notify customer.
    pub notify: Option<String>,
    /// Comma-separated line item IDs and quantities (format: "id:qty,id:qty").
    pub line_items: Option<String>,
    /// Shipping refund amount.
    pub shipping_amount: Option<String>,
    /// Whether to do full shipping refund.
    pub full_shipping_refund: Option<String>,
}

/// Input for creating a return.
#[derive(Debug, Deserialize)]
pub struct ReturnInput {
    /// Comma-separated fulfillment line item IDs and quantities (format: "id:qty,id:qty").
    pub line_items: String,
    /// Return reason note.
    pub reason_note: Option<String>,
}

/// Input for capturing payment.
#[derive(Debug, Deserialize)]
pub struct CaptureInput {
    /// Transaction ID to capture.
    pub transaction_id: String,
    /// Amount to capture.
    pub amount: String,
}

/// Query params for archive action.
#[derive(Debug, Deserialize)]
pub struct ArchiveParams {
    /// If true, unarchive instead of archive.
    pub unarchive: Option<bool>,
}

// =============================================================================
// Tag Handlers
// =============================================================================

/// Add or remove a tag from an order (HTMX).
#[instrument(skip(_admin, state))]
pub async fn update_tags(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<TagInput>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    let result = if input.action == "remove" {
        state
            .shopify()
            .remove_tags_from_order(&order_id, std::slice::from_ref(&input.tag))
            .await
    } else {
        state
            .shopify()
            .add_tags_to_order(&order_id, std::slice::from_ref(&input.tag))
            .await
    };

    match result {
        Ok(tags) => {
            tracing::info!(order_id = %order_id, action = %input.action, tag = %input.tag, "Tag updated");
            // Return updated tags list as HTML for HTMX swap
            let tags_html: String = tags
                .iter()
                .map(|t| format!(r#"<span class="badge badge-secondary">{t}</span>"#))
                .collect::<Vec<_>>()
                .join(" ");
            Html(tags_html).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to update tag");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to update tag: {e}"),
            )
                .into_response()
        }
    }
}

// =============================================================================
// Fulfillment Handlers
// =============================================================================

/// Create a fulfillment for an order.
#[instrument(skip(_admin, state))]
pub async fn fulfill(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<FulfillInput>,
) -> impl IntoResponse {
    let fulfillment_order_id = if input.fulfillment_order_id.starts_with("gid://") {
        input.fulfillment_order_id.clone()
    } else {
        format!(
            "gid://shopify/FulfillmentOrder/{}",
            input.fulfillment_order_id
        )
    };

    match state
        .shopify()
        .create_fulfillment(
            &fulfillment_order_id,
            input.tracking_company.as_deref(),
            input.tracking_number.as_deref(),
            input.tracking_url.as_deref(),
        )
        .await
    {
        Ok(fulfillment_id) => {
            tracing::info!(order_id = %id, fulfillment_id = %fulfillment_id, "Fulfillment created");
            let numeric_id = id.split('/').next_back().unwrap_or(&id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %id, error = %e, "Failed to create fulfillment");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to create fulfillment: {e}"),
            )
                .into_response()
        }
    }
}

/// Hold a fulfillment order.
#[instrument(skip(_admin, state))]
pub async fn hold_fulfillment(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path((order_id, fo_id)): Path<(String, String)>,
    Form(input): Form<HoldInput>,
) -> impl IntoResponse {
    let fulfillment_order_id = if fo_id.starts_with("gid://") {
        fo_id.clone()
    } else {
        format!("gid://shopify/FulfillmentOrder/{fo_id}")
    };

    let reason = match input.reason.to_uppercase().as_str() {
        "AWAITING_PAYMENT" => FulfillmentHoldReason::AwaitingPayment,
        "HIGH_RISK_OF_FRAUD" => FulfillmentHoldReason::HighRiskOfFraud,
        "INCORRECT_ADDRESS" => FulfillmentHoldReason::IncorrectAddress,
        "INVENTORY_OUT_OF_STOCK" => FulfillmentHoldReason::InventoryOutOfStock,
        "UNKNOWN_DELIVERY_DATE" => FulfillmentHoldReason::UnknownDeliveryDate,
        "AWAITING_RETURN_ITEMS" => FulfillmentHoldReason::AwaitingReturnItems,
        _ => FulfillmentHoldReason::Other,
    };

    let hold_input = FulfillmentHoldInput {
        reason,
        reason_notes: input.reason_notes,
        notify_merchant: false,
    };

    match state
        .shopify()
        .hold_fulfillment_order(&fulfillment_order_id, hold_input)
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %order_id, fulfillment_order_id = %fulfillment_order_id, "Fulfillment order held");
            let numeric_id = order_id.split('/').next_back().unwrap_or(&order_id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(fulfillment_order_id = %fulfillment_order_id, error = %e, "Failed to hold fulfillment order");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to hold fulfillment: {e}"),
            )
                .into_response()
        }
    }
}

/// Release a hold on a fulfillment order.
#[instrument(skip(_admin, state))]
pub async fn release_hold(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path((order_id, fo_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let fulfillment_order_id = if fo_id.starts_with("gid://") {
        fo_id.clone()
    } else {
        format!("gid://shopify/FulfillmentOrder/{fo_id}")
    };

    match state
        .shopify()
        .release_fulfillment_order_hold(&fulfillment_order_id)
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %order_id, fulfillment_order_id = %fulfillment_order_id, "Fulfillment hold released");
            let numeric_id = order_id.split('/').next_back().unwrap_or(&order_id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(fulfillment_order_id = %fulfillment_order_id, error = %e, "Failed to release hold");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to release hold: {e}"),
            )
                .into_response()
        }
    }
}

// =============================================================================
// Refund Handlers
// =============================================================================

/// Calculate suggested refund for an order (HTMX).
///
/// Returns HTML partial with refund calculation details.
#[instrument(skip(_admin, state))]
pub async fn calculate_refund(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    match state.shopify().get_suggested_refund(&order_id).await {
        Ok(suggested) => {
            use std::fmt::Write;

            // Return HTML partial for HTMX swap
            let mut line_items_html = String::new();
            for item in &suggested.line_items {
                let _ = write!(
                    line_items_html,
                    r#"<div class="flex justify-between py-2 border-b border-muted">
                        <span>{} (qty: {})</span>
                        <span>Refund: {}</span>
                    </div>"#,
                    item.title, item.original_quantity, item.refund_quantity
                );
            }

            let html = format!(
                r#"<div class="space-y-4">
                    <div class="text-sm text-muted-foreground">Suggested refund calculation</div>
                    <div class="space-y-2">{line_items_html}</div>
                    <div class="border-t border-muted pt-4 space-y-2">
                        <div class="flex justify-between">
                            <span>Subtotal</span>
                            <span>{} {}</span>
                        </div>
                        <div class="flex justify-between">
                            <span>Tax</span>
                            <span>{} {}</span>
                        </div>
                        <div class="flex justify-between font-semibold">
                            <span>Total refund</span>
                            <span>{} {}</span>
                        </div>
                    </div>
                </div>"#,
                suggested.subtotal,
                suggested.currency_code,
                suggested.total_tax,
                suggested.currency_code,
                suggested.amount,
                suggested.currency_code
            );
            Html(html).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to calculate refund");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to calculate refund: {e}"),
            )
                .into_response()
        }
    }
}

/// Create a refund for an order.
#[instrument(skip(_admin, state))]
pub async fn refund(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<RefundInput>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    // Parse line items from "id:qty,id:qty" format
    let line_items: Vec<RefundLineItemInput> = input
        .line_items
        .as_deref()
        .unwrap_or("")
        .split(',')
        .filter_map(|item| {
            let mut parts = item.trim().split(':');
            let id_part = parts.next()?;
            let qty_part = parts.next()?;
            // Ensure no extra parts
            if parts.next().is_some() {
                return None;
            }
            let line_item_id = if id_part.starts_with("gid://") {
                id_part.to_string()
            } else {
                format!("gid://shopify/LineItem/{id_part}")
            };
            let quantity = qty_part.parse().ok()?;
            Some(RefundLineItemInput {
                line_item_id,
                quantity,
                restock_type: RefundRestockType::Return,
                location_id: None,
            })
        })
        .collect();

    let refund_input = RefundCreateInput {
        note: input.note,
        notify: input.notify.as_deref() == Some("on"),
        line_items,
        shipping_amount: input.shipping_amount,
        full_shipping_refund: input.full_shipping_refund.as_deref() == Some("on"),
    };

    match state.shopify().create_refund(&order_id, refund_input).await {
        Ok(refund_id) => {
            tracing::info!(order_id = %order_id, refund_id = %refund_id, "Refund created");
            let numeric_id = id.split('/').next_back().unwrap_or(&id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to create refund");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to create refund: {e}"),
            )
                .into_response()
        }
    }
}

// =============================================================================
// Return Handlers
// =============================================================================

/// Create a return for an order.
#[instrument(skip(_admin, state))]
pub async fn create_return(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<ReturnInput>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    // Parse line items from "id:qty,id:qty" format
    let line_items: Vec<ReturnLineItemCreateInput> = input
        .line_items
        .split(',')
        .filter_map(|item| {
            let mut parts = item.trim().split(':');
            let id_part = parts.next()?;
            let qty_part = parts.next()?;
            // Ensure no extra parts
            if parts.next().is_some() {
                return None;
            }
            let fulfillment_line_item_id = if id_part.starts_with("gid://") {
                id_part.to_string()
            } else {
                format!("gid://shopify/FulfillmentLineItem/{id_part}")
            };
            let quantity = qty_part.parse().ok()?;
            Some(ReturnLineItemCreateInput {
                fulfillment_line_item_id,
                quantity,
                return_reason_note: input.reason_note.clone(),
            })
        })
        .collect();

    if line_items.is_empty() {
        return (StatusCode::BAD_REQUEST, "No valid line items specified").into_response();
    }

    let return_input = ReturnCreateInput {
        line_items,
        requested_at: None,
    };

    match state.shopify().create_return(&order_id, return_input).await {
        Ok(return_id) => {
            tracing::info!(order_id = %order_id, return_id = %return_id, "Return created");
            let numeric_id = id.split('/').next_back().unwrap_or(&id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to create return");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to create return: {e}"),
            )
                .into_response()
        }
    }
}

// =============================================================================
// Payment Handlers
// =============================================================================

/// Capture payment on an order.
#[instrument(skip(_admin, state))]
pub async fn capture(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<CaptureInput>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    let transaction_id = if input.transaction_id.starts_with("gid://") {
        input.transaction_id.clone()
    } else {
        format!("gid://shopify/OrderTransaction/{}", input.transaction_id)
    };

    match state
        .shopify()
        .capture_order_payment(&order_id, &transaction_id, &input.amount)
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %order_id, "Payment captured");
            let numeric_id = id.split('/').next_back().unwrap_or(&id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to capture payment");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to capture payment: {e}"),
            )
                .into_response()
        }
    }
}

// =============================================================================
// Archive Handlers
// =============================================================================

/// Archive or unarchive an order.
#[instrument(skip(_admin, state))]
pub async fn archive(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(params): Query<ArchiveParams>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Order/{id}")
    };

    let result = if params.unarchive.unwrap_or(false) {
        state.shopify().unarchive_order(&order_id).await
    } else {
        state.shopify().archive_order(&order_id).await
    };

    match result {
        Ok(()) => {
            let action = if params.unarchive.unwrap_or(false) {
                "unarchived"
            } else {
                "archived"
            };
            tracing::info!(order_id = %order_id, action = action, "Order archive status updated");
            let numeric_id = id.split('/').next_back().unwrap_or(&id);
            Redirect::to(&format!("/orders/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to update archive status");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to update archive status: {e}"),
            )
                .into_response()
        }
    }
}
