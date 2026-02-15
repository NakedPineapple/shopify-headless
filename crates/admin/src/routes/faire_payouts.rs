//! Faire payout / settlement routes.
//!
//! Provides a view of Faire payouts cached locally from
//! the Faire Brand API v2. Only `super_admin` users can access
//! these features.

use askama::Template;
use axum::{
    Router,
    extract::{Path, Query, State},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tower_sessions::Session;
use tracing::instrument;

use crate::db::{CachedFairePayout, CachedFairePayoutLineItem, FairePayoutRepository};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::dashboard::AdminUserView;

// =============================================================================
// Templates
// =============================================================================

/// Faire payouts list page.
#[derive(Template)]
#[template(path = "faire/payouts/index.html")]
struct FairePayoutsTemplate {
    admin_user: AdminUserView,
    current_path: String,
    connected: bool,
    payouts: Vec<CachedFairePayout>,
    total_count: i64,
    page: i64,
    page_size: i64,
    status_filter: Option<String>,
    sync_error: Option<String>,
}

/// Faire payout detail page.
#[derive(Template)]
#[template(path = "faire/payouts/show.html")]
struct FairePayoutDetailTemplate {
    admin_user: AdminUserView,
    current_path: String,
    payout: CachedFairePayout,
    line_items: Vec<CachedFairePayoutLineItem>,
}

// =============================================================================
// Query Parameters
// =============================================================================

#[derive(Debug, Deserialize)]
struct PayoutsQuery {
    page: Option<i64>,
    status: Option<String>,
}

// =============================================================================
// Router
// =============================================================================

/// Build the Faire payouts router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/faire/payouts", get(payouts_index))
        .route("/faire/payouts/sync", post(payouts_sync))
        .route("/faire/payouts/{id}", get(payouts_show))
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /faire/payouts -- Payouts list.
#[instrument(skip(state, session))]
async fn payouts_index(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<PayoutsQuery>,
) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let connected = build_client_from_db(&state).await.is_some();
    if !connected {
        return render(FairePayoutsTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/faire/payouts".to_string(),
            connected: false,
            payouts: vec![],
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

    let repo = FairePayoutRepository::new(state.pool());
    let payouts = repo.list(page_size, offset, status_filter).await;
    let count = repo.count(status_filter).await;

    match (payouts, count) {
        (Ok(payouts), Ok(total_count)) => render(FairePayoutsTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/faire/payouts".to_string(),
            connected: true,
            payouts,
            total_count,
            page,
            page_size,
            status_filter: query.status,
            sync_error: None,
        }),
        (Err(e), _) | (_, Err(e)) => render(FairePayoutsTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/faire/payouts".to_string(),
            connected: true,
            payouts: vec![],
            total_count: 0,
            page: 1,
            page_size: 50,
            status_filter: None,
            sync_error: Some(format!("Failed to load payouts: {e}")),
        }),
    }
}

/// POST /faire/payouts/sync -- Trigger manual payout sync.
#[instrument(skip(state, session))]
async fn payouts_sync(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(client) = build_client_from_db(&state).await else {
        return Redirect::to("/faire/payouts").into_response();
    };

    match client.list_payouts(None, Some(50), None).await {
        Ok(data) => {
            let repo = FairePayoutRepository::new(state.pool());
            let mut synced = 0;
            if let Some(payouts) = &data.payouts {
                for payout in payouts {
                    if upsert_payout_from_api(&repo, payout).await.is_ok() {
                        synced += 1;
                    }
                }
            }
            tracing::info!(count = synced, "Manual Faire payout sync complete");
        }
        Err(e) => {
            tracing::error!(error = %e, "Manual Faire payout sync failed");
        }
    }

    Redirect::to("/faire/payouts").into_response()
}

/// GET /faire/payouts/{id} -- Payout detail.
#[instrument(skip(state, session))]
async fn payouts_show(
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

    let repo = FairePayoutRepository::new(state.pool());
    let payout = repo.get_by_id(id).await;

    match payout {
        Ok(Some(payout)) => {
            let line_items = repo
                .get_line_items(&payout.faire_payout_token)
                .await
                .unwrap_or_default();
            render(FairePayoutDetailTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/faire/payouts".to_string(),
                payout,
                line_items,
            })
        }
        _ => Redirect::to("/faire/payouts").into_response(),
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Convert cents to a dollar string (e.g., 1250 -> "12.50").
fn cents_to_dollars(cents: i64) -> String {
    #[allow(
        clippy::cast_precision_loss,
        reason = "monetary cent values will never approach 2^52"
    )]
    let dollars = cents as f64 / 100.0;
    format!("{dollars:.2}")
}

/// Upsert a single payout from the Faire API response into the DB.
async fn upsert_payout_from_api(
    repo: &FairePayoutRepository<'_>,
    payout: &naked_pineapple_services::faire::FairePayout,
) -> Result<(), crate::db::RepositoryError> {
    use crate::db::faire_payouts::{
        UpsertFairePayoutLineItemParams, UpsertFairePayoutParams, UpsertPayoutFinancials,
    };

    let raw_json = serde_json::to_value(payout).ok();
    let payout_token = payout.token.as_deref().unwrap_or("unknown");

    let total_revenue_str = payout.total_order_cents.map(cents_to_dollars);
    let total_refunds_str = payout.total_refund_cents.map(cents_to_dollars);
    let total_commission_str = payout.total_commission_cents.map(cents_to_dollars);
    let total_shipping_str = payout.total_shipping_cents.map(cents_to_dollars);
    let net_payout_str = payout.net_payout_cents.map(cents_to_dollars);

    let params = UpsertFairePayoutParams {
        faire_payout_token: payout_token,
        payout_period_start: None,
        payout_period_end: None,
        financials: UpsertPayoutFinancials {
            total_revenue: total_revenue_str.as_deref(),
            total_refunds: total_refunds_str.as_deref(),
            total_commission: total_commission_str.as_deref(),
            total_shipping_fees: total_shipping_str.as_deref(),
            net_payout: net_payout_str.as_deref(),
            currency: payout.currency.as_deref(),
        },
        payout_status: payout.state.as_deref().unwrap_or("UNKNOWN"),
        payout_date: None,
        raw_json: raw_json.as_ref(),
    };

    repo.upsert(&params).await?;

    // Upsert line items
    if let Some(items) = &payout.line_items {
        for item in items {
            let order_amount_str = item.order_amount_cents.map(cents_to_dollars);
            let refund_amount_str = item.refund_amount_cents.map(cents_to_dollars);
            let commission_str = item.commission_cents.map(cents_to_dollars);
            let shipping_str = item.shipping_fee_cents.map(cents_to_dollars);
            let net_str = item.net_amount_cents.map(cents_to_dollars);

            let item_params = UpsertFairePayoutLineItemParams {
                faire_payout_token: payout_token,
                faire_order_token: item.order_token.as_deref().unwrap_or("unknown"),
                order_amount: order_amount_str.as_deref(),
                refund_amount: refund_amount_str.as_deref(),
                commission_amount: commission_str.as_deref(),
                shipping_fee: shipping_str.as_deref(),
                net_amount: net_str.as_deref(),
            };
            let _ = repo.upsert_line_item(&item_params).await;
        }
    }

    Ok(())
}

/// Build a `FaireClient` from stored DB credentials.
async fn build_client_from_db(
    state: &AppState,
) -> Option<naked_pineapple_services::faire::FaireClient> {
    use naked_pineapple_services::faire::FaireCredentials as ApiCredentials;

    let repo = crate::db::FaireCredentialsRepository::new(state.pool());
    let creds = repo.get_default().await.ok().flatten()?;

    Some(naked_pineapple_services::faire::FaireClient::new(
        ApiCredentials {
            brand_id: creds.brand_id,
            api_token: creds.api_token,
        },
    ))
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
