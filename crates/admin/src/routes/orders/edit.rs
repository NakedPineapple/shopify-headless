//! Order edit handlers.

use askama::Template;
use axum::{
    Form,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use serde::Deserialize;
use tracing::instrument;

use crate::{
    filters,
    middleware::auth::RequireAdminAuth,
    shopify::types::{
        Money, OrderEditAddShippingLineInput, OrderEditAppliedDiscountInput,
        OrderEditUpdateShippingLineInput,
    },
    state::AppState,
};

use super::super::dashboard::AdminUserView;
use super::types::OrderEditView;

// =============================================================================
// Templates
// =============================================================================

/// Template for the order edit page.
#[derive(Template)]
#[template(path = "orders/edit.html")]
pub struct OrderEditTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub edit: OrderEditView,
}

/// Template for the edit line items partial (HTMX swap).
#[derive(Template)]
#[template(path = "orders/_edit_line_items.html")]
pub struct EditLineItemsPartial {
    pub edit: OrderEditView,
}

/// Template for the edit summary partial (HTMX swap).
#[derive(Template)]
#[template(path = "orders/_edit_summary.html")]
pub struct EditSummaryPartial {
    pub edit: OrderEditView,
}

// =============================================================================
// Input Types
// =============================================================================

/// Input for adding a variant to the order.
#[derive(Debug, Deserialize)]
pub struct AddVariantInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Variant ID to add.
    pub variant_id: String,
    /// Quantity to add.
    pub quantity: i64,
}

/// Input for adding a custom item.
#[derive(Debug, Deserialize)]
pub struct AddCustomItemInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Item title.
    pub title: String,
    /// Quantity.
    pub quantity: i64,
    /// Price amount (decimal string).
    pub price: String,
    /// Whether taxable.
    #[serde(default)]
    pub taxable: bool,
    /// Whether requires shipping.
    #[serde(default)]
    pub requires_shipping: bool,
}

/// Input for setting line item quantity.
#[derive(Debug, Deserialize)]
pub struct SetQuantityInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Line item ID.
    pub line_item_id: String,
    /// New quantity.
    pub quantity: i64,
    /// Whether to restock.
    #[serde(default)]
    pub restock: bool,
}

/// Input for adding a discount.
#[derive(Debug, Deserialize)]
pub struct AddDiscountInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Line item ID.
    pub line_item_id: String,
    /// Discount type (percent or fixed).
    pub discount_type: String,
    /// Discount value.
    pub value: String,
    /// Optional description.
    pub description: Option<String>,
}

/// Input for updating a discount.
#[derive(Debug, Deserialize)]
pub struct UpdateDiscountInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Discount application ID.
    pub discount_application_id: String,
    /// Discount type (percent or fixed).
    pub discount_type: String,
    /// Discount value.
    pub value: String,
    /// Optional description.
    pub description: Option<String>,
}

/// Input for removing a discount.
#[derive(Debug, Deserialize)]
pub struct RemoveDiscountInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Discount application ID.
    pub discount_application_id: String,
}

/// Input for adding a shipping line.
#[derive(Debug, Deserialize)]
pub struct AddShippingInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Shipping method title.
    pub title: String,
    /// Price amount.
    pub price: String,
}

/// Input for updating a shipping line.
#[derive(Debug, Deserialize)]
pub struct UpdateShippingInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Shipping line ID.
    pub shipping_line_id: String,
    /// New title.
    pub title: Option<String>,
    /// New price.
    pub price: Option<String>,
}

/// Input for removing a shipping line.
#[derive(Debug, Deserialize)]
pub struct RemoveShippingInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Shipping line ID.
    pub shipping_line_id: String,
}

/// Input for committing order edit.
#[derive(Debug, Deserialize)]
pub struct CommitEditInput {
    /// Calculated order ID.
    pub calculated_order_id: String,
    /// Whether to notify customer.
    #[serde(default)]
    pub notify_customer: bool,
    /// Optional staff note.
    pub staff_note: Option<String>,
}

/// Input for product search.
#[derive(Debug, Deserialize)]
pub struct ProductSearchQuery {
    /// Search query.
    pub q: Option<String>,
}

// =============================================================================
// Handlers
// =============================================================================

/// Start editing an order.
#[instrument(skip(admin, state))]
pub async fn edit(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Extract short ID for URL paths
    let short_id = if id.starts_with("gid://") {
        id.split('/').next_back().unwrap_or(&id).to_string()
    } else {
        id.clone()
    };

    let order_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/Order/{id}")
    };

    match state.shopify().order_edit_begin(&order_id).await {
        Ok(calculated_order) => {
            let edit = OrderEditView::from(&calculated_order);
            let template = OrderEditTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: format!("/orders/{short_id}/edit"),
                edit,
            };
            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
        Err(e) => {
            tracing::error!(order_id = %order_id, error = %e, "Failed to begin order edit");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to begin order edit: {e}"),
            )
                .into_response()
        }
    }
}

/// Add a variant to the order edit (HTMX).
#[instrument(skip(_admin, state))]
pub async fn edit_add_variant(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<AddVariantInput>,
) -> impl IntoResponse {
    let variant_id = if input.variant_id.starts_with("gid://") {
        input.variant_id
    } else {
        format!("gid://shopify/ProductVariant/{}", input.variant_id)
    };

    match state
        .shopify()
        .order_edit_add_variant(&input.calculated_order_id, &variant_id, input.quantity)
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %id, variant_id = %variant_id, "Added variant to order edit");
            // Redirect to refresh the edit page
            Redirect::to(&format!("/orders/{id}/edit")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to add variant");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to add variant: {e}"),
            )
                .into_response()
        }
    }
}

/// Add a custom item to the order edit (HTMX).
#[instrument(skip(_admin, state))]
pub async fn edit_add_custom_item(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<AddCustomItemInput>,
) -> impl IntoResponse {
    let price = Money {
        amount: input.price,
        currency_code: "USD".to_string(),
    };

    match state
        .shopify()
        .order_edit_add_custom_item(
            &input.calculated_order_id,
            &input.title,
            input.quantity,
            &price,
            input.taxable,
            input.requires_shipping,
        )
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %id, title = %input.title, "Added custom item to order edit");
            Redirect::to(&format!("/orders/{id}/edit")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to add custom item");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to add custom item: {e}"),
            )
                .into_response()
        }
    }
}

/// Set line item quantity in order edit (HTMX).
#[instrument(skip(_admin, state))]
pub async fn edit_set_quantity(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<SetQuantityInput>,
) -> impl IntoResponse {
    match state
        .shopify()
        .order_edit_set_quantity(
            &input.calculated_order_id,
            &input.line_item_id,
            input.quantity,
            input.restock,
        )
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %id, line_item_id = %input.line_item_id, quantity = %input.quantity, "Updated quantity");
            Redirect::to(&format!("/orders/{id}/edit")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to set quantity");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to set quantity: {e}"),
            )
                .into_response()
        }
    }
}

/// Add a discount to a line item (HTMX).
#[instrument(skip(_admin, state))]
pub async fn edit_add_discount(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<AddDiscountInput>,
) -> impl IntoResponse {
    let discount = if input.discount_type == "percent" {
        let percent: f64 = input.value.parse().unwrap_or(0.0);
        OrderEditAppliedDiscountInput::percentage(percent, input.description)
    } else {
        let amount: f64 = input.value.parse().unwrap_or(0.0);
        let money = Money {
            amount: format!("{amount:.2}"),
            currency_code: "USD".to_string(),
        };
        OrderEditAppliedDiscountInput::fixed_amount(money, input.description)
    };

    match state
        .shopify()
        .order_edit_add_line_item_discount(
            &input.calculated_order_id,
            &input.line_item_id,
            &discount,
        )
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %id, "Added discount to line item");
            Redirect::to(&format!("/orders/{id}/edit")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to add discount");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to add discount: {e}"),
            )
                .into_response()
        }
    }
}

/// Update a discount (HTMX).
#[instrument(skip(_admin, state))]
pub async fn edit_update_discount(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<UpdateDiscountInput>,
) -> impl IntoResponse {
    let discount = if input.discount_type == "percent" {
        let percent: f64 = input.value.parse().unwrap_or(0.0);
        OrderEditAppliedDiscountInput::percentage(percent, input.description)
    } else {
        let amount: f64 = input.value.parse().unwrap_or(0.0);
        let money = Money {
            amount: format!("{amount:.2}"),
            currency_code: "USD".to_string(),
        };
        OrderEditAppliedDiscountInput::fixed_amount(money, input.description)
    };

    match state
        .shopify()
        .order_edit_update_discount(
            &input.calculated_order_id,
            &input.discount_application_id,
            &discount,
        )
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %id, "Updated discount");
            Redirect::to(&format!("/orders/{id}/edit")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to update discount");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to update discount: {e}"),
            )
                .into_response()
        }
    }
}

/// Remove a discount (HTMX).
#[instrument(skip(_admin, state))]
pub async fn edit_remove_discount(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<RemoveDiscountInput>,
) -> impl IntoResponse {
    match state
        .shopify()
        .order_edit_remove_discount(&input.calculated_order_id, &input.discount_application_id)
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %id, "Removed discount");
            Redirect::to(&format!("/orders/{id}/edit")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to remove discount");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to remove discount: {e}"),
            )
                .into_response()
        }
    }
}

/// Add a shipping line (HTMX).
#[instrument(skip(_admin, state))]
pub async fn edit_add_shipping(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<AddShippingInput>,
) -> impl IntoResponse {
    let shipping_input = OrderEditAddShippingLineInput {
        title: input.title,
        price: Money {
            amount: input.price,
            currency_code: "USD".to_string(),
        },
    };

    match state
        .shopify()
        .order_edit_add_shipping_line(&input.calculated_order_id, &shipping_input)
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %id, "Added shipping line");
            Redirect::to(&format!("/orders/{id}/edit")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to add shipping line");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to add shipping line: {e}"),
            )
                .into_response()
        }
    }
}

/// Update a shipping line (HTMX).
#[instrument(skip(_admin, state))]
pub async fn edit_update_shipping(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<UpdateShippingInput>,
) -> impl IntoResponse {
    let shipping_input = OrderEditUpdateShippingLineInput {
        title: input.title,
        price: input.price.map(|p| Money {
            amount: p,
            currency_code: "USD".to_string(),
        }),
    };

    match state
        .shopify()
        .order_edit_update_shipping_line(
            &input.calculated_order_id,
            &input.shipping_line_id,
            &shipping_input,
        )
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %id, "Updated shipping line");
            Redirect::to(&format!("/orders/{id}/edit")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to update shipping line");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to update shipping line: {e}"),
            )
                .into_response()
        }
    }
}

/// Remove a shipping line (HTMX).
#[instrument(skip(_admin, state))]
pub async fn edit_remove_shipping(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<RemoveShippingInput>,
) -> impl IntoResponse {
    match state
        .shopify()
        .order_edit_remove_shipping_line(&input.calculated_order_id, &input.shipping_line_id)
        .await
    {
        Ok(()) => {
            tracing::info!(order_id = %id, "Removed shipping line");
            Redirect::to(&format!("/orders/{id}/edit")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to remove shipping line");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to remove shipping line: {e}"),
            )
                .into_response()
        }
    }
}

/// Commit the order edit.
#[instrument(skip(_admin, state))]
pub async fn edit_commit(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<CommitEditInput>,
) -> impl IntoResponse {
    match state
        .shopify()
        .order_edit_commit(
            &input.calculated_order_id,
            input.notify_customer,
            input.staff_note.as_deref(),
        )
        .await
    {
        Ok(order_id) => {
            tracing::info!(order_id = %order_id, "Order edit committed");
            // Extract short ID for redirect
            let short_id = order_id.strip_prefix("gid://shopify/Order/").unwrap_or(&id);
            Redirect::to(&format!("/orders/{short_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to commit order edit");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to commit order edit: {e}"),
            )
                .into_response()
        }
    }
}

/// Discard the order edit and return to order detail.
#[instrument(skip(_admin))]
pub async fn edit_discard(
    RequireAdminAuth(_admin): RequireAdminAuth,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Simply redirect back to the order detail page
    // The calculated order session will expire automatically
    Redirect::to(&format!("/orders/{id}"))
}

/// Search products for adding to order (HTMX).
#[instrument(skip(state))]
pub async fn edit_search_products(
    RequireAdminAuth(_): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<ProductSearchQuery>,
) -> impl IntoResponse {
    use std::fmt::Write;

    let search_query = query.q.filter(|q| !q.is_empty());

    match state.shopify().get_products(20, None, search_query).await {
        Ok(products) => {
            // Build a simple HTML list of products with variants
            let mut html = String::new();
            for product in products.products {
                let _ = write!(
                    html,
                    r#"<div class="p-3 border-b border-border hover:bg-muted/50">
                        <div class="font-medium">{}</div>
                        <div class="mt-2 space-y-1">"#,
                    product.title
                );
                for variant in &product.variants {
                    let price: f64 = variant.price.amount.parse().unwrap_or(0.0);
                    let variant_title = &variant.title;
                    let sku_display = variant
                        .sku
                        .as_ref()
                        .filter(|s| !s.is_empty())
                        .map_or(String::new(), |s| format!("({s})"));
                    let _ = write!(
                        html,
                        r##"<button type="button"
                            class="w-full text-left px-2 py-1 rounded hover:bg-primary/10 text-sm flex justify-between items-center"
                            hx-post="/orders/{id}/edit/add-variant"
                            hx-vals='{{"variant_id": "{}", "quantity": 1}}'
                            hx-target="#edit-content"
                            hx-swap="outerHTML">
                            <span>{variant_title} {sku_display}</span>
                            <span class="text-muted-foreground">${price:.2}</span>
                        </button>"##,
                        variant.id,
                    );
                }
                html.push_str("</div></div>");
            }
            if html.is_empty() {
                html =
                    r#"<div class="p-4 text-center text-muted-foreground">No products found</div>"#
                        .to_string();
            }
            Html(html).into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to search products");
            Html(
                r#"<div class="p-4 text-center text-destructive">Failed to search products</div>"#
                    .to_string(),
            )
            .into_response()
        }
    }
}
