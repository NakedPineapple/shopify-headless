//! Print handlers for order invoices and packing slips.

use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use serde::Deserialize;
use tracing::instrument;

use crate::{
    filters, middleware::auth::RequireAdminAuth, shopify::types::OrderLineItem, state::AppState,
};

use super::types::{AddressView, format_price};

/// Query params for print action.
#[derive(Debug, Deserialize)]
pub struct PrintQuery {
    /// Type of document: "invoice" or `packing_slip`.
    #[serde(rename = "type")]
    pub doc_type: Option<String>,
}

/// Print line item view (simpler than full line item view).
#[derive(Debug, Clone)]
pub struct PrintLineItemView {
    pub title: String,
    pub variant_title: Option<String>,
    pub sku: Option<String>,
    pub quantity: i64,
    pub price: String,
    pub total: String,
}

// Precision loss from i64 -> f64 is acceptable for display.
#[allow(clippy::cast_precision_loss)]
impl From<&OrderLineItem> for PrintLineItemView {
    fn from(item: &OrderLineItem) -> Self {
        let price = format_price(&item.discounted_unit_price);
        let total = item
            .discounted_unit_price
            .amount
            .parse::<f64>()
            .unwrap_or(0.0)
            * item.quantity as f64;
        Self {
            title: item.title.clone(),
            variant_title: item.variant_title.clone(),
            sku: item.sku.clone(),
            quantity: item.quantity,
            price,
            total: format!("${total:.2}"),
        }
    }
}

/// Order view for print templates.
#[derive(Debug, Clone)]
pub struct PrintOrderView {
    pub name: String,
    pub created_at: String,
    pub financial_status: String,
    pub subtotal: String,
    pub shipping: String,
    pub discount: String,
    pub tax: String,
    pub total: String,
    pub note: Option<String>,
    pub shipping_method: Option<String>,
    pub shipping_address: Option<AddressView>,
    pub billing_address: Option<AddressView>,
}

/// Invoice print template.
#[derive(Template)]
#[template(path = "orders/print_invoice.html")]
pub struct OrderInvoiceTemplate {
    pub order: PrintOrderView,
    pub line_items: Vec<PrintLineItemView>,
    pub printed_at: String,
}

/// Packing slip print template.
#[derive(Template)]
#[template(path = "orders/print_packing_slip.html")]
pub struct OrderPackingSlipTemplate {
    pub order: PrintOrderView,
    pub line_items: Vec<PrintLineItemView>,
    pub printed_at: String,
}

/// Print order invoice or packing slip.
#[instrument(skip(_admin, state))]
pub async fn print(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<PrintQuery>,
) -> impl IntoResponse {
    let order_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/Order/{id}")
    };

    match state.shopify().get_order(&order_id).await {
        Ok(Some(order)) => {
            let printed_at = chrono::Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();

            let print_order = PrintOrderView {
                name: order.name.clone(),
                created_at: order.created_at.clone(),
                financial_status: order
                    .financial_status
                    .as_ref()
                    .map_or_else(|| "Pending".to_string(), |s| format!("{s:?}")),
                subtotal: format_price(&order.subtotal_price),
                shipping: format_price(&order.total_shipping_price),
                discount: format_price(&order.total_discounts),
                tax: format_price(&order.total_tax),
                total: format_price(&order.total_price),
                note: order.note.clone(),
                shipping_method: None, // Would come from shipping lines
                shipping_address: order.shipping_address.as_ref().map(AddressView::from),
                billing_address: order.billing_address.as_ref().map(AddressView::from),
            };

            let line_items: Vec<PrintLineItemView> = order
                .line_items
                .iter()
                .map(PrintLineItemView::from)
                .collect();

            let doc_type = query.doc_type.as_deref().unwrap_or("invoice");

            if doc_type == "packing_slip" {
                let template = OrderPackingSlipTemplate {
                    order: print_order,
                    line_items,
                    printed_at,
                };
                Html(template.render().unwrap_or_else(|e| {
                    tracing::error!("Template render error: {}", e);
                    "Internal Server Error".to_string()
                }))
                .into_response()
            } else {
                let template = OrderInvoiceTemplate {
                    order: print_order,
                    line_items,
                    printed_at,
                };
                Html(template.render().unwrap_or_else(|e| {
                    tracing::error!("Template render error: {}", e);
                    "Internal Server Error".to_string()
                }))
                .into_response()
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Order not found").into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch order for printing: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch order").into_response()
        }
    }
}
