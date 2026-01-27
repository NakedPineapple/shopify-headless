//! Discount management route handlers.

#![allow(clippy::used_underscore_binding)]

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
    shopify::{
        DiscountCreateInput,
        types::{DiscountCode, DiscountStatus, DiscountValue},
    },
    state::AppState,
};

use super::dashboard::AdminUserView;

/// Pagination query parameters.
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub cursor: Option<String>,
    pub query: Option<String>,
}

/// Discount view for templates.
#[derive(Debug, Clone)]
pub struct DiscountView {
    pub id: String,
    pub title: String,
    pub code: String,
    pub status: String,
    pub status_class: String,
    pub discount_type: String,
    pub value: String,
    pub usage: String,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
}

impl From<&DiscountCode> for DiscountView {
    fn from(dc: &DiscountCode) -> Self {
        let (status, status_class) = match dc.status {
            DiscountStatus::Active => (
                "Active",
                "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400",
            ),
            DiscountStatus::Expired => (
                "Expired",
                "bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-400",
            ),
            DiscountStatus::Scheduled => (
                "Scheduled",
                "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400",
            ),
        };

        let (discount_type, value) = match &dc.value {
            Some(DiscountValue::Percentage { percentage }) => (
                "Percentage".to_string(),
                format!("{}%", (percentage * 100.0).round()),
            ),
            Some(DiscountValue::FixedAmount { amount, currency }) => {
                ("Fixed Amount".to_string(), format!("${amount} {currency}"))
            }
            None => ("Other".to_string(), "—".to_string()),
        };

        let usage = dc.usage_limit.map_or_else(
            || format!("{} uses", dc.usage_count),
            |limit| format!("{}/{} uses", dc.usage_count, limit),
        );

        Self {
            id: dc.id.clone(),
            title: dc.title.clone(),
            code: dc.code.clone(),
            status: status.to_string(),
            status_class: status_class.to_string(),
            discount_type,
            value,
            usage,
            starts_at: dc.starts_at.clone(),
            ends_at: dc.ends_at.clone(),
        }
    }
}

/// Discounts list page template.
#[derive(Template)]
#[template(path = "discounts/index.html")]
pub struct DiscountsIndexTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub discounts: Vec<DiscountView>,
    pub has_next_page: bool,
    pub next_cursor: Option<String>,
    pub search_query: Option<String>,
}

/// Discount create form template.
#[derive(Template)]
#[template(path = "discounts/new.html")]
pub struct DiscountNewTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub error: Option<String>,
}

/// Discount edit form template.
#[derive(Template)]
#[template(path = "discounts/edit.html")]
pub struct DiscountEditTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub discount: DiscountView,
    pub error: Option<String>,
}

/// Form input for creating/updating discounts.
#[derive(Debug, Deserialize)]
pub struct DiscountFormInput {
    pub title: String,
    pub code: String,
    pub discount_type: String,
    pub value: String,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub usage_limit: Option<i64>,
}

/// Discounts list page handler.
#[instrument(skip(admin, state))]
pub async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Html<String> {
    let result = state
        .shopify()
        .get_discounts(25, query.cursor.clone(), query.query.clone())
        .await;

    let (discounts, has_next_page, next_cursor) = match result {
        Ok(conn) => {
            let discounts: Vec<DiscountView> =
                conn.discount_codes.iter().map(DiscountView::from).collect();
            (
                discounts,
                conn.page_info.has_next_page,
                conn.page_info.end_cursor,
            )
        }
        Err(e) => {
            tracing::error!("Failed to fetch discounts: {e}");
            (vec![], false, None)
        }
    };

    let template = DiscountsIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/discounts".to_string(),
        discounts,
        has_next_page,
        next_cursor,
        search_query: query.query,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// New discount form handler.
#[instrument(skip(admin))]
pub async fn new_discount(RequireAdminAuth(admin): RequireAdminAuth) -> Html<String> {
    let template = DiscountNewTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/discounts".to_string(),
        error: None,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// Create discount handler.
#[instrument(skip(admin, state))]
pub async fn create(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<DiscountFormInput>,
) -> impl IntoResponse {
    // Parse discount value
    let (percentage, amount) = if input.discount_type == "percentage" {
        let pct = input.value.parse::<f64>().unwrap_or(0.0) / 100.0;
        (Some(pct), None)
    } else {
        (None, Some((input.value.as_str(), "USD")))
    };

    // Default starts_at to now if not provided
    let default_starts_at = chrono::Utc::now().to_rfc3339();
    let starts_at = input.starts_at.as_deref().unwrap_or(&default_starts_at);

    match state
        .shopify()
        .create_discount(DiscountCreateInput {
            title: &input.title,
            code: &input.code,
            percentage,
            amount,
            starts_at,
            ends_at: input.ends_at.as_deref(),
            usage_limit: input.usage_limit,
        })
        .await
    {
        Ok(discount_id) => {
            tracing::info!(discount_id = %discount_id, code = %input.code, "Discount created");
            Redirect::to("/discounts").into_response()
        }
        Err(e) => {
            tracing::error!(code = %input.code, error = %e, "Failed to create discount");
            let template = DiscountNewTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/discounts".to_string(),
                error: Some(e.to_string()),
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
    }
}

/// Edit discount form handler.
#[instrument(skip(admin, state))]
pub async fn edit(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Ensure ID has the proper Shopify format
    let discount_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/DiscountCodeNode/{id}")
    };

    match state.shopify().get_discount(&discount_id).await {
        Ok(discount) => {
            let template = DiscountEditTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/discounts".to_string(),
                discount: DiscountView::from(&discount),
                error: None,
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to fetch discount: {e}");
            (StatusCode::NOT_FOUND, format!("Discount not found: {e}")).into_response()
        }
    }
}

/// Update discount handler.
#[instrument(skip(admin, state))]
pub async fn update(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<DiscountFormInput>,
) -> impl IntoResponse {
    use crate::shopify::DiscountUpdateInput;

    // Ensure ID has the proper Shopify format
    let discount_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/DiscountCodeNode/{id}")
    };

    let update_input = DiscountUpdateInput {
        title: Some(&input.title),
        starts_at: input.starts_at.as_deref(),
        ends_at: input.ends_at.as_deref(),
    };

    match state
        .shopify()
        .update_discount(&discount_id, update_input)
        .await
    {
        Ok(()) => {
            tracing::info!(discount_id = %discount_id, "Discount updated");
            Redirect::to("/discounts").into_response()
        }
        Err(e) => {
            tracing::error!(discount_id = %discount_id, error = %e, "Failed to update discount");
            let error_msg = e.to_string();
            // Re-fetch the discount to show the edit form with error
            let discount = state.shopify().get_discount(&discount_id).await.ok();
            discount.map_or_else(
                || Redirect::to("/discounts").into_response(),
                |d| {
                    let template = DiscountEditTemplate {
                        admin_user: AdminUserView::from(&admin),
                        current_path: "/discounts".to_string(),
                        discount: DiscountView::from(&d),
                        error: Some(error_msg),
                    };

                    Html(template.render().unwrap_or_else(|e| {
                        tracing::error!("Template render error: {}", e);
                        "Internal Server Error".to_string()
                    }))
                    .into_response()
                },
            )
        }
    }
}

/// Deactivate discount handler (HTMX).
#[instrument(skip(_admin, state))]
pub async fn deactivate(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Ensure ID has the proper Shopify format
    let discount_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/DiscountCodeNode/{id}")
    };

    match state.shopify().deactivate_discount(&discount_id).await {
        Ok(()) => {
            tracing::info!(discount_id = %discount_id, "Discount deactivated");
            (
                StatusCode::OK,
                [("HX-Trigger", "discount-deactivated")],
                Html(
                    "<span class=\"text-gray-600 dark:text-gray-400\">Deactivated</span>"
                        .to_string(),
                ),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(discount_id = %discount_id, error = %e, "Failed to deactivate discount");
            (
                StatusCode::BAD_REQUEST,
                Html(format!(
                    "<span class=\"text-red-600 dark:text-red-400\">Error: {e}</span>"
                )),
            )
                .into_response()
        }
    }
}
