//! Payout history route handlers.

#![allow(clippy::used_underscore_binding)]

use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use serde::Deserialize;
use tracing::instrument;

use crate::{
    filters,
    middleware::auth::RequireAdminAuth,
    shopify::types::{Payout, PayoutStatus},
    state::AppState,
};

use super::dashboard::AdminUserView;

/// Pagination query parameters.
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub cursor: Option<String>,
}

/// Payout view for templates.
#[derive(Debug, Clone)]
pub struct PayoutView {
    pub id: String,
    pub status: String,
    pub status_class: String,
    pub amount: String,
    pub issued_at: Option<String>,
}

impl From<&Payout> for PayoutView {
    fn from(p: &Payout) -> Self {
        let (status, status_class) = match p.status {
            PayoutStatus::Scheduled => (
                "Scheduled",
                "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400",
            ),
            PayoutStatus::InTransit => (
                "In Transit",
                "bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400",
            ),
            PayoutStatus::Paid => (
                "Paid",
                "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400",
            ),
            PayoutStatus::Failed => (
                "Failed",
                "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400",
            ),
            PayoutStatus::Canceled => (
                "Canceled",
                "bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-400",
            ),
        };

        Self {
            id: p.id.clone(),
            status: status.to_string(),
            status_class: status_class.to_string(),
            amount: format!("${}", p.net.amount),
            issued_at: p.issued_at.clone(),
        }
    }
}

/// Payouts list page template.
#[derive(Template)]
#[template(path = "payouts/index.html")]
pub struct PayoutsIndexTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub payouts: Vec<PayoutView>,
    pub has_next_page: bool,
    pub next_cursor: Option<String>,
    pub balance: Option<String>,
}

/// Payout detail page template.
#[derive(Template)]
#[template(path = "payouts/show.html")]
pub struct PayoutShowTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub payout: PayoutView,
}

/// Payouts list page handler.
#[instrument(skip(admin, state))]
pub async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Html<String> {
    let result = state.shopify().get_payouts(25, query.cursor.clone()).await;

    let (payouts, has_next_page, next_cursor, balance) = match result {
        Ok(conn) => {
            let payouts: Vec<PayoutView> = conn.payouts.iter().map(PayoutView::from).collect();
            let balance = conn.balance.as_ref().map(|b| format!("${}", b.amount));
            (
                payouts,
                conn.page_info.has_next_page,
                conn.page_info.end_cursor,
                balance,
            )
        }
        Err(e) => {
            tracing::error!("Failed to fetch payouts: {e}");
            (vec![], false, None, None)
        }
    };

    let template = PayoutsIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/payouts".to_string(),
        payouts,
        has_next_page,
        next_cursor,
        balance,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// Payout detail page handler.
#[instrument(skip(admin, state))]
pub async fn show(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Ensure ID has the proper Shopify format
    let payout_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/ShopifyPaymentsPayout/{id}")
    };

    match state.shopify().get_payout(&payout_id).await {
        Ok(payout) => {
            let template = PayoutShowTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/payouts".to_string(),
                payout: PayoutView::from(&payout),
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to fetch payout: {e}");
            (StatusCode::NOT_FOUND, format!("Payout not found: {e}")).into_response()
        }
    }
}
