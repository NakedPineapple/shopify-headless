//! TikTok Shop finance / settlement routes.
//!
//! Provides a view of TikTok Shop settlements cached locally from
//! the TikTok Shop Open API. Only `super_admin` users can access
//! these features.

use askama::Template;
use axum::{
    Router,
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use serde::Deserialize;
use tower_sessions::Session;
use tracing::instrument;

use crate::db::{
    CachedTikTokSettlement, CachedTikTokSettlementLineItem, TikTokSettlementRepository,
};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// TikTok settlements list page.
#[derive(Template)]
#[template(path = "tiktok/finance/index.html")]
struct FinanceIndexTemplate {
    admin_user: AdminUserView,
    current_path: String,
    connected: bool,
    settlements: Vec<CachedTikTokSettlement>,
    total_count: i64,
    page: i64,
    page_size: i64,
    status_filter: Option<String>,
    sync_error: Option<String>,
}

/// TikTok settlement detail page.
#[derive(Template)]
#[template(path = "tiktok/finance/show.html")]
struct FinanceShowTemplate {
    admin_user: AdminUserView,
    current_path: String,
    settlement: CachedTikTokSettlement,
    line_items: Vec<CachedTikTokSettlementLineItem>,
}

// =============================================================================
// Query Parameters
// =============================================================================

#[derive(Debug, Deserialize)]
struct FinanceQuery {
    page: Option<i64>,
    status: Option<String>,
}

// =============================================================================
// Router
// =============================================================================

/// Build the TikTok finance router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tiktok/finance", get(finance_index))
        .route("/tiktok/finance/sync", get(finance_sync))
        .route("/tiktok/finance/{id}", get(finance_show))
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /tiktok/finance -- Settlements list.
#[instrument(skip(state, session))]
async fn finance_index(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<FinanceQuery>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let connected = state.tiktok().is_some();
    if !connected {
        return render(FinanceIndexTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/tiktok/finance".to_string(),
            connected: false,
            settlements: vec![],
            total_count: 0,
            page: 1,
            page_size: 50,
            status_filter: None,
            sync_error: None,
        });
    }

    let page = query.page.unwrap_or(1).max(1);
    let page_size: i64 = 50;
    let offset = (page - 1) * page_size;
    let status_filter = query.status.as_deref();

    let repo = TikTokSettlementRepository::new(state.pool());
    let settlements = repo.list(page_size, offset, status_filter).await;
    let count = repo.count(status_filter).await;

    match (settlements, count) {
        (Ok(settlements), Ok(total_count)) => render(FinanceIndexTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/tiktok/finance".to_string(),
            connected: true,
            settlements,
            total_count,
            page,
            page_size,
            status_filter: query.status,
            sync_error: None,
        }),
        (Err(e), _) | (_, Err(e)) => render(FinanceIndexTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/tiktok/finance".to_string(),
            connected: true,
            settlements: vec![],
            total_count: 0,
            page: 1,
            page_size: 50,
            status_filter: None,
            sync_error: Some(format!("Failed to load settlements: {e}")),
        }),
    }
}

/// GET /tiktok/finance/sync -- Trigger manual settlement sync.
#[instrument(skip(state, session))]
async fn finance_sync(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(client) = state.tiktok() else {
        return Redirect::to("/tiktok/finance").into_response();
    };

    match client.get_settlements(50, None).await {
        Ok(data) => {
            let repo = TikTokSettlementRepository::new(state.pool());
            let mut synced = 0;
            if let Some(settlements) = &data.settlements {
                for settlement in settlements {
                    if upsert_settlement_from_api(&repo, settlement).await.is_ok() {
                        synced += 1;
                    }
                }
            }
            tracing::info!(count = synced, "Manual TikTok settlement sync complete");
        }
        Err(e) => {
            tracing::error!(error = %e, "Manual TikTok settlement sync failed");
        }
    }

    Redirect::to("/tiktok/finance").into_response()
}

/// GET /tiktok/finance/{id} -- Settlement detail.
#[instrument(skip(state, session))]
async fn finance_show(
    State(state): State<AppState>,
    session: Session,
    Path(id): Path<i32>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let repo = TikTokSettlementRepository::new(state.pool());
    let settlement = repo.get_by_id(id).await;

    match settlement {
        Ok(Some(settlement)) => {
            let line_items = repo
                .get_line_items(&settlement.settlement_id)
                .await
                .unwrap_or_default();
            render(FinanceShowTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/tiktok/finance".to_string(),
                settlement,
                line_items,
            })
        }
        _ => Redirect::to("/tiktok/finance").into_response(),
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Upsert a single settlement from the TikTok API response into the DB.
async fn upsert_settlement_from_api(
    repo: &TikTokSettlementRepository<'_>,
    settlement: &naked_pineapple_services::tiktok_shop::TikTokSettlement,
) -> Result<(), crate::db::RepositoryError> {
    use crate::db::tiktok_settlements::{
        UpsertSettlementFinancials, UpsertTikTokSettlementLineItemParams,
        UpsertTikTokSettlementParams,
    };

    let settlement_id = settlement.id.as_deref().unwrap_or("unknown");

    let period_start = settlement
        .period_start
        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0));
    let period_end = settlement
        .period_end
        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0));
    let payout_date = settlement
        .payout_date
        .and_then(|ts| chrono::DateTime::from_timestamp(ts, 0));

    let params = UpsertTikTokSettlementParams {
        settlement_id,
        settlement_period_start: period_start,
        settlement_period_end: period_end,
        status: settlement.status.as_deref().unwrap_or("UNKNOWN"),
        total_revenue: settlement.total_revenue.as_deref(),
        total_refunds: settlement.total_refunds.as_deref(),
        financials: UpsertSettlementFinancials {
            total_platform_fees: settlement.total_platform_fees.as_deref(),
            total_affiliate_commission: settlement.total_affiliate_commission.as_deref(),
            net_payout: settlement.net_payout.as_deref(),
            currency: settlement.currency.as_deref(),
        },
        payout_date,
    };

    repo.upsert(&params).await?;

    // Upsert line items
    if let Some(items) = &settlement.line_items {
        for item in items {
            let item_params = UpsertTikTokSettlementLineItemParams {
                settlement_id,
                tiktok_order_id: item.order_id.as_deref().unwrap_or("unknown"),
                order_amount: item.order_amount.as_deref(),
                refund_amount: item.refund_amount.as_deref(),
                referral_fee: item.referral_fee.as_deref(),
                affiliate_commission: item.affiliate_commission.as_deref(),
                shipping_fee_subsidy: item.shipping_fee_subsidy.as_deref(),
                net_amount: item.net_amount.as_deref(),
            };
            let _ = repo.upsert_line_item(&item_params).await;
        }
    }

    Ok(())
}

/// Get the current admin from the session.
async fn get_admin(session: &Session) -> Option<CurrentAdmin> {
    session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
}

/// Render an Askama template into an HTML response.
fn render(template: impl Template) -> Response {
    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
    .into_response()
}
