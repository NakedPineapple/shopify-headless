//! Bulk action handlers for orders.

use axum::{
    Form,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Redirect},
};
use serde::Deserialize;
use tracing::instrument;

use crate::{middleware::auth::RequireAdminAuth, state::AppState};

/// Input for bulk tag operations.
#[derive(Debug, Deserialize)]
pub struct BulkTagsInput {
    /// Comma-separated list of order IDs.
    pub order_ids: String,
    /// Tags to add or remove (comma-separated).
    pub tags: String,
}

/// Input for bulk archive/cancel operations.
#[derive(Debug, Deserialize)]
pub struct BulkOrdersInput {
    /// Comma-separated list of order IDs.
    pub order_ids: String,
}

/// Bulk add tags to orders.
#[instrument(skip(_admin, state))]
pub async fn bulk_add_tags(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<BulkTagsInput>,
) -> impl IntoResponse {
    let order_ids: Vec<&str> = input.order_ids.split(',').map(str::trim).collect();
    let tags: Vec<String> = input
        .tags
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();

    if order_ids.is_empty() || tags.is_empty() {
        return (StatusCode::BAD_REQUEST, "No orders or tags specified").into_response();
    }

    let mut success_count = 0;
    let mut error_messages = Vec::new();

    for id in &order_ids {
        let order_id = if id.starts_with("gid://") {
            (*id).to_string()
        } else {
            format!("gid://shopify/Order/{id}")
        };

        match state.shopify().add_tags_to_order(&order_id, &tags).await {
            Ok(_) => success_count += 1,
            Err(e) => {
                error_messages.push(format!("{id}: {e}"));
            }
        }
    }

    if error_messages.is_empty() {
        tracing::info!(count = success_count, "Bulk add tags completed");
        Redirect::to("/orders").into_response()
    } else {
        tracing::warn!(
            success = success_count,
            errors = ?error_messages,
            "Bulk add tags completed with errors"
        );
        (
            StatusCode::MULTI_STATUS,
            format!(
                "Added tags to {success_count} orders. Errors: {}",
                error_messages.join("; ")
            ),
        )
            .into_response()
    }
}

/// Bulk remove tags from orders.
#[instrument(skip(_admin, state))]
pub async fn bulk_remove_tags(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<BulkTagsInput>,
) -> impl IntoResponse {
    let order_ids: Vec<&str> = input.order_ids.split(',').map(str::trim).collect();
    let tags: Vec<String> = input
        .tags
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();

    if order_ids.is_empty() || tags.is_empty() {
        return (StatusCode::BAD_REQUEST, "No orders or tags specified").into_response();
    }

    let mut success_count = 0;
    let mut error_messages = Vec::new();

    for id in &order_ids {
        let order_id = if id.starts_with("gid://") {
            (*id).to_string()
        } else {
            format!("gid://shopify/Order/{id}")
        };

        match state
            .shopify()
            .remove_tags_from_order(&order_id, &tags)
            .await
        {
            Ok(_) => success_count += 1,
            Err(e) => {
                error_messages.push(format!("{id}: {e}"));
            }
        }
    }

    if error_messages.is_empty() {
        tracing::info!(count = success_count, "Bulk remove tags completed");
        Redirect::to("/orders").into_response()
    } else {
        tracing::warn!(
            success = success_count,
            errors = ?error_messages,
            "Bulk remove tags completed with errors"
        );
        (
            StatusCode::MULTI_STATUS,
            format!(
                "Removed tags from {success_count} orders. Errors: {}",
                error_messages.join("; ")
            ),
        )
            .into_response()
    }
}

/// Bulk archive orders.
#[instrument(skip(_admin, state))]
pub async fn bulk_archive(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<BulkOrdersInput>,
) -> impl IntoResponse {
    let order_ids: Vec<&str> = input.order_ids.split(',').map(str::trim).collect();

    if order_ids.is_empty() {
        return (StatusCode::BAD_REQUEST, "No orders specified").into_response();
    }

    let mut success_count = 0;
    let mut error_messages = Vec::new();

    for id in &order_ids {
        let order_id = if id.starts_with("gid://") {
            (*id).to_string()
        } else {
            format!("gid://shopify/Order/{id}")
        };

        match state.shopify().archive_order(&order_id).await {
            Ok(()) => success_count += 1,
            Err(e) => {
                error_messages.push(format!("{id}: {e}"));
            }
        }
    }

    if error_messages.is_empty() {
        tracing::info!(count = success_count, "Bulk archive completed");
        Redirect::to("/orders").into_response()
    } else {
        tracing::warn!(
            success = success_count,
            errors = ?error_messages,
            "Bulk archive completed with errors"
        );
        (
            StatusCode::MULTI_STATUS,
            format!(
                "Archived {success_count} orders. Errors: {}",
                error_messages.join("; ")
            ),
        )
            .into_response()
    }
}

/// Bulk cancel orders.
#[instrument(skip(_admin, state))]
pub async fn bulk_cancel(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<BulkOrdersInput>,
) -> impl IntoResponse {
    let order_ids: Vec<&str> = input.order_ids.split(',').map(str::trim).collect();

    if order_ids.is_empty() {
        return (StatusCode::BAD_REQUEST, "No orders specified").into_response();
    }

    let mut success_count = 0;
    let mut error_messages = Vec::new();

    for id in &order_ids {
        let order_id = if id.starts_with("gid://") {
            (*id).to_string()
        } else {
            format!("gid://shopify/Order/{id}")
        };

        // Cancel with default settings: no notification, no refund, restock items
        match state
            .shopify()
            .cancel_order(&order_id, Some("OTHER"), false, false, true)
            .await
        {
            Ok(()) => success_count += 1,
            Err(e) => {
                error_messages.push(format!("{id}: {e}"));
            }
        }
    }

    if error_messages.is_empty() {
        tracing::info!(count = success_count, "Bulk cancel completed");
        Redirect::to("/orders").into_response()
    } else {
        tracing::warn!(
            success = success_count,
            errors = ?error_messages,
            "Bulk cancel completed with errors"
        );
        (
            StatusCode::MULTI_STATUS,
            format!(
                "Cancelled {success_count} orders. Errors: {}",
                error_messages.join("; ")
            ),
        )
            .into_response()
    }
}
