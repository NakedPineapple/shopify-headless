//! Payout history route handlers.
//!
//! Handles payouts list, payout detail, disputes, bank accounts, and settings.

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
    components::data_table::{
        DataTableConfig, FilterType, disputes_table_config, payouts_table_config,
        transactions_table_config,
    },
    filters,
    middleware::auth::RequireAdminAuth,
    shopify::types::{
        BalanceTransaction, BalanceTransactionSourceType, BalanceTransactionType, BankAccount,
        BankAccountStatus, Dispute, DisputeDetail, DisputeStatus, DisputeType, Payout,
        PayoutDetail, PayoutSchedule, PayoutScheduleInterval, PayoutSortKey, PayoutStatus,
        PayoutSummary, PayoutTransactionType,
    },
    state::AppState,
};

use super::dashboard::AdminUserView;

// =============================================================================
// Query Parameters
// =============================================================================

/// Pagination and sorting query parameters for payouts list.
#[derive(Debug, Deserialize)]
pub struct PayoutsQuery {
    pub cursor: Option<String>,
    pub query: Option<String>,
    pub status: Option<String>,
    pub transaction_type: Option<String>,
    pub sort: Option<String>,
    pub dir: Option<String>,
}

/// Pagination and sorting query parameters for disputes list.
#[derive(Debug, Deserialize)]
pub struct DisputesQuery {
    pub cursor: Option<String>,
    pub query: Option<String>,
    pub status: Option<String>,
    pub sort: Option<String>,
    pub dir: Option<String>,
}

/// Pagination and sorting query parameters for transactions.
#[derive(Debug, Deserialize)]
pub struct TransactionsQuery {
    pub cursor: Option<String>,
    pub type_filter: Option<String>,
    pub sort: Option<String>,
    pub dir: Option<String>,
    /// The payout date for filtering (YYYY-MM-DD or ISO 8601 datetime)
    pub payout_date: Option<String>,
}

// =============================================================================
// Payout Views
// =============================================================================

/// Payout view for list templates.
#[derive(Debug, Clone)]
pub struct PayoutView {
    pub id: String,
    pub short_id: String,
    pub status: String,
    pub status_class: String,
    pub transaction_type: String,
    pub gross: String,
    pub fees: String,
    pub net: String,
    pub amount: String, // Alias for net - for template compatibility
    pub issued_at: Option<String>,
}

impl From<&Payout> for PayoutView {
    fn from(p: &Payout) -> Self {
        let (status, status_class) = status_badge(p.status);
        let net = format!("${}", p.net.amount);
        Self {
            id: p.id.clone(),
            short_id: extract_short_id(&p.id),
            status: status.to_string(),
            status_class: status_class.to_string(),
            transaction_type: "Deposit".to_string(),
            gross: format!("${}", p.net.amount),
            fees: "$0.00".to_string(),
            net: net.clone(),
            amount: net,
            issued_at: p.issued_at.clone(),
        }
    }
}

/// Payout detail view with summary breakdown.
#[derive(Debug, Clone)]
pub struct PayoutDetailView {
    pub id: String,
    pub short_id: String,
    pub status: String,
    pub status_class: String,
    pub transaction_type: String,
    pub gross: String,
    pub net: String,
    pub amount: String, // Alias for net - for template compatibility
    pub issued_at: Option<String>,
    pub summary: Option<PayoutSummaryView>,
}

impl From<&PayoutDetail> for PayoutDetailView {
    fn from(p: &PayoutDetail) -> Self {
        let (status, status_class) = status_badge(p.status);
        let tx_type = match p.transaction_type {
            PayoutTransactionType::Deposit => "Deposit",
            PayoutTransactionType::Withdrawal => "Withdrawal",
        };
        let net = format!("${}", p.net.amount);
        Self {
            id: p.id.clone(),
            short_id: extract_short_id(&p.id),
            status: status.to_string(),
            status_class: status_class.to_string(),
            transaction_type: tx_type.to_string(),
            gross: format!("${}", p.gross.amount),
            net: net.clone(),
            amount: net,
            issued_at: p.issued_at.clone(),
            summary: p.summary.as_ref().map(PayoutSummaryView::from),
        }
    }
}

/// Payout summary breakdown view.
#[derive(Debug, Clone)]
pub struct PayoutSummaryView {
    pub charges_gross: String,
    pub charges_fee: String,
    pub refunds_gross: String,
    pub refunds_fee: String,
    pub adjustments_gross: String,
    pub adjustments_fee: String,
    pub reserved_funds_gross: String,
    pub reserved_funds_fee: String,
}

impl From<&PayoutSummary> for PayoutSummaryView {
    fn from(s: &PayoutSummary) -> Self {
        Self {
            charges_gross: format!("${}", s.charges_gross.amount),
            charges_fee: format!("-${}", s.charges_fee.amount),
            refunds_gross: format!("-${}", s.refunds_fee_gross.amount),
            refunds_fee: format!("${}", s.refunds_fee.amount),
            adjustments_gross: format!("${}", s.adjustments_gross.amount),
            adjustments_fee: format!("-${}", s.adjustments_fee.amount),
            reserved_funds_gross: format!("-${}", s.reserved_funds_gross.amount),
            reserved_funds_fee: format!("${}", s.reserved_funds_fee.amount),
        }
    }
}

// =============================================================================
// Transaction Views
// =============================================================================

/// Balance transaction view.
#[derive(Debug, Clone)]
pub struct TransactionView {
    pub id: String,
    pub date: String,
    pub transaction_type: String,
    pub type_class: String,
    pub source: String,
    pub order_id: Option<String>,
    pub order_name: Option<String>,
    pub amount: String,
    pub fee: String,
    pub net: String,
}

impl From<&BalanceTransaction> for TransactionView {
    fn from(t: &BalanceTransaction) -> Self {
        let (tx_type, type_class) = transaction_type_badge(t.transaction_type);
        let source = match t.source_type {
            BalanceTransactionSourceType::Charge => "Sale",
            BalanceTransactionSourceType::Refund => "Refund",
            BalanceTransactionSourceType::Adjustment => "Adjustment",
            BalanceTransactionSourceType::Dispute => "Dispute",
            BalanceTransactionSourceType::Payout => "Payout",
            BalanceTransactionSourceType::ReservedFunds => "Reserved",
            BalanceTransactionSourceType::Unknown => "Other",
        };
        Self {
            id: t.id.clone(),
            date: t.transaction_date.clone(),
            transaction_type: tx_type.to_string(),
            type_class: type_class.to_string(),
            source: source.to_string(),
            order_id: t.order_id.clone(),
            order_name: t.order_name.clone(),
            amount: format!("${}", t.amount.amount),
            fee: format!("-${}", t.fee.amount),
            net: format!("${}", t.net.amount),
        }
    }
}

// =============================================================================
// Dispute Views
// =============================================================================

/// Dispute view for list templates.
#[derive(Debug, Clone)]
pub struct DisputeView {
    pub id: String,
    pub short_id: String,
    pub status: String,
    pub status_class: String,
    pub dispute_type: String,
    pub type_class: String,
    pub amount: String,
    pub initiated_at: String,
    pub evidence_due_by: Option<String>,
    pub days_until_due: Option<i64>,
    pub order_id: Option<String>,
    pub order_name: Option<String>,
    pub reason: String,
    pub is_urgent: bool,
}

impl From<&Dispute> for DisputeView {
    fn from(d: &Dispute) -> Self {
        let (status, status_class, is_urgent) = dispute_status_badge(d.status);
        let (dtype, type_class) = dispute_type_badge(d.kind);
        let reason = d
            .reason_details
            .as_ref()
            .map_or_else(|| "Unknown".to_string(), |r| r.reason.clone());
        Self {
            id: d.id.clone(),
            short_id: extract_short_id(&d.id),
            status: status.to_string(),
            status_class: status_class.to_string(),
            dispute_type: dtype.to_string(),
            type_class: type_class.to_string(),
            amount: format!("${}", d.amount.amount),
            initiated_at: d.initiated_at.clone(),
            evidence_due_by: d.evidence_due_by.clone(),
            days_until_due: None, // Would need date calculation
            order_id: d.order_id.clone(),
            order_name: d.order_name.clone(),
            reason,
            is_urgent,
        }
    }
}

/// Dispute detail view with evidence.
#[derive(Debug, Clone)]
pub struct DisputeDetailView {
    pub dispute: DisputeView,
    pub customer_email: Option<String>,
    pub customer_name: Option<String>,
    pub order_total: Option<String>,
    pub evidence_submitted: bool,
    pub evidence_sent_on: Option<String>,
    pub product_description: Option<String>,
}

impl From<&DisputeDetail> for DisputeDetailView {
    fn from(d: &DisputeDetail) -> Self {
        let evidence_submitted = d.evidence.as_ref().is_some_and(|e| e.submitted);
        Self {
            dispute: DisputeView::from(&d.dispute),
            customer_email: d.customer_email.clone(),
            customer_name: d.customer_name.clone(),
            order_total: d.order_total.as_ref().map(|m| format!("${}", m.amount)),
            evidence_submitted,
            evidence_sent_on: d.evidence_sent_on.clone(),
            product_description: d
                .evidence
                .as_ref()
                .and_then(|e| e.product_description.clone()),
        }
    }
}

// =============================================================================
// Bank Account & Schedule Views
// =============================================================================

/// Bank account view.
#[derive(Debug, Clone)]
pub struct BankAccountView {
    pub id: String,
    pub bank_name: String,
    pub last_digits: String,
    pub country: String,
    pub currency: String,
    pub status: String,
    pub status_class: String,
}

impl From<&BankAccount> for BankAccountView {
    fn from(b: &BankAccount) -> Self {
        let (status, status_class) = bank_account_status_badge(b.status);
        Self {
            id: b.id.clone(),
            bank_name: b
                .bank_name
                .clone()
                .unwrap_or_else(|| "Unknown Bank".to_string()),
            last_digits: b.account_number_last_digits.clone(),
            country: b.country.clone(),
            currency: b.currency.clone(),
            status: status.to_string(),
            status_class: status_class.to_string(),
        }
    }
}

/// Payout schedule view.
#[derive(Debug, Clone)]
pub struct PayoutScheduleView {
    pub interval: String,
    pub interval_description: String,
    pub monthly_anchor: Option<i64>,
    pub weekly_anchor: Option<String>,
}

impl From<&PayoutSchedule> for PayoutScheduleView {
    fn from(s: &PayoutSchedule) -> Self {
        let (interval, description) = match s.interval {
            PayoutScheduleInterval::Daily => ("Daily", "Payouts are sent every business day"),
            PayoutScheduleInterval::Weekly => ("Weekly", "Payouts are sent once per week"),
            PayoutScheduleInterval::Monthly => ("Monthly", "Payouts are sent once per month"),
            PayoutScheduleInterval::Manual => ("Manual", "Payouts are sent on request"),
        };
        Self {
            interval: interval.to_string(),
            interval_description: description.to_string(),
            monthly_anchor: s.monthly_anchor,
            weekly_anchor: s.weekly_anchor.clone(),
        }
    }
}

// =============================================================================
// Templates
// =============================================================================

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
    pub table_config: DataTableConfig,
    pub current_sort: Option<String>,
    pub current_dir: String,
}

/// Payout detail page template.
#[derive(Template)]
#[template(path = "payouts/show.html")]
pub struct PayoutShowTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub payout: PayoutDetailView,
    pub transactions: Vec<TransactionView>,
    pub has_more_transactions: bool,
    pub transactions_cursor: Option<String>,
    pub table_config: DataTableConfig,
}

/// Disputes list page template.
#[derive(Template)]
#[template(path = "payouts/disputes/index.html")]
pub struct DisputesIndexTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub disputes: Vec<DisputeView>,
    pub has_next_page: bool,
    pub next_cursor: Option<String>,
    pub needs_response_count: usize,
    pub table_config: DataTableConfig,
}

/// Dispute detail page template.
#[derive(Template)]
#[template(path = "payouts/disputes/show.html")]
pub struct DisputeShowTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub dispute: DisputeDetailView,
}

/// Bank accounts page template.
#[derive(Template)]
#[template(path = "payouts/bank_accounts.html")]
pub struct BankAccountsTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub accounts: Vec<BankAccountView>,
}

/// Payout settings page template.
#[derive(Template)]
#[template(path = "payouts/settings.html")]
pub struct PayoutSettingsTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub schedule: PayoutScheduleView,
}

/// Transactions partial template for HTMX.
#[derive(Template)]
#[template(path = "payouts/_transactions_table.html")]
pub struct TransactionsPartialTemplate {
    pub transactions: Vec<TransactionView>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
    pub payout_id: String,
}

// =============================================================================
// Route Handlers
// =============================================================================

/// Payouts list page handler.
#[instrument(skip(admin, state))]
pub async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<PayoutsQuery>,
) -> Html<String> {
    // Parse sort parameters
    tracing::debug!(
        sort = ?query.sort,
        dir = ?query.dir,
        cursor = ?query.cursor,
        "Payouts index request"
    );

    let sort_key = query
        .sort
        .as_deref()
        .and_then(PayoutSortKey::from_str_param);

    tracing::debug!(
        sort_key = ?sort_key,
        "Parsed sort key from query param {:?}",
        query.sort
    );

    let reverse = query.dir.as_deref() == Some("desc");

    let result = state
        .shopify()
        .get_payouts(25, query.cursor.clone(), sort_key, reverse)
        .await;

    tracing::debug!(success = result.is_ok(), "Shopify get_payouts result");

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
        table_config: payouts_table_config(),
        current_sort: query.sort.clone(),
        current_dir: query.dir.clone().unwrap_or_else(|| "asc".to_string()),
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
    let payout_id = normalize_payout_id(&id);

    match state.shopify().get_payout_detail(&payout_id).await {
        Ok(payout) => {
            // Fetch transactions for this payout using payout_date filter + client-side payout_id filter
            tracing::info!(
                payout_id = %payout_id,
                issued_at = ?payout.issued_at,
                "Fetching transactions for payout"
            );
            let transactions_result = state
                .shopify()
                .get_payout_transactions(
                    100,
                    None,
                    Some(payout_id.clone()),
                    payout.issued_at.clone(),
                )
                .await;

            let (transactions, has_more, cursor) = match transactions_result {
                Ok(conn) => (
                    conn.transactions
                        .iter()
                        .map(TransactionView::from)
                        .collect(),
                    conn.page_info.has_next_page,
                    conn.page_info.end_cursor,
                ),
                Err(e) => {
                    tracing::warn!("Failed to fetch transactions: {e}");
                    (vec![], false, None)
                }
            };

            let template = PayoutShowTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/payouts".to_string(),
                payout: PayoutDetailView::from(&payout),
                transactions,
                has_more_transactions: has_more,
                transactions_cursor: cursor,
                table_config: transactions_table_config(),
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

/// Disputes list page handler.
#[instrument(skip(admin, state))]
pub async fn disputes_index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<DisputesQuery>,
) -> Html<String> {
    let result = state
        .shopify()
        .get_disputes(25, query.cursor.clone(), query.query.clone())
        .await;

    let (mut disputes, has_next_page, next_cursor, needs_response_count) = match result {
        Ok(conn) => {
            let needs_response = conn
                .disputes
                .iter()
                .filter(|d| matches!(d.status, DisputeStatus::NeedsResponse))
                .count();
            let disputes: Vec<DisputeView> = conn.disputes.iter().map(DisputeView::from).collect();
            (
                disputes,
                conn.page_info.has_next_page,
                conn.page_info.end_cursor,
                needs_response,
            )
        }
        Err(e) => {
            tracing::error!("Failed to fetch disputes: {e}");
            (vec![], false, None, 0)
        }
    };

    // Apply Rust-side sorting (Shopify API doesn't support dispute sorting)
    if let Some(ref sort_col) = query.sort {
        let reverse = query.dir.as_deref() == Some("desc");
        sort_disputes(&mut disputes, sort_col, reverse);
    }

    let template = DisputesIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/payouts/disputes".to_string(),
        disputes,
        has_next_page,
        next_cursor,
        needs_response_count,
        table_config: disputes_table_config(),
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// Dispute detail page handler.
#[instrument(skip(admin, state))]
pub async fn dispute_show(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let dispute_id = normalize_dispute_id(&id);

    match state.shopify().get_dispute(&dispute_id).await {
        Ok(dispute) => {
            let template = DisputeShowTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/payouts/disputes".to_string(),
                dispute: DisputeDetailView::from(&dispute),
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to fetch dispute: {e}");
            (StatusCode::NOT_FOUND, format!("Dispute not found: {e}")).into_response()
        }
    }
}

/// Bank accounts page handler.
#[instrument(skip(admin, state))]
pub async fn bank_accounts(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
) -> Html<String> {
    let accounts = match state.shopify().get_bank_accounts().await {
        Ok(accounts) => accounts.iter().map(BankAccountView::from).collect(),
        Err(e) => {
            tracing::error!("Failed to fetch bank accounts: {e}");
            vec![]
        }
    };

    let template = BankAccountsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/payouts/bank-accounts".to_string(),
        accounts,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// Payout settings page handler.
#[instrument(skip(admin, state))]
pub async fn settings(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
) -> Html<String> {
    let schedule = match state.shopify().get_payout_schedule().await {
        Ok(schedule) => PayoutScheduleView::from(&schedule),
        Err(e) => {
            tracing::error!("Failed to fetch payout schedule: {e}");
            PayoutScheduleView {
                interval: "Unknown".to_string(),
                interval_description: "Could not load schedule".to_string(),
                monthly_anchor: None,
                weekly_anchor: None,
            }
        }
    };

    let template = PayoutSettingsTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/payouts/settings".to_string(),
        schedule,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// HTMX partial handler for loading more transactions.
///
/// # Errors
///
/// Returns 500 if transactions cannot be fetched or template fails to render.
#[instrument(skip(state))]
pub async fn transactions(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<TransactionsQuery>,
) -> impl IntoResponse {
    let payout_id = normalize_payout_id(&id);

    match state
        .shopify()
        .get_payout_transactions(
            100,
            query.cursor,
            Some(payout_id.clone()),
            query.payout_date,
        )
        .await
    {
        Ok(conn) => {
            let template = TransactionsPartialTemplate {
                transactions: conn
                    .transactions
                    .iter()
                    .map(TransactionView::from)
                    .collect(),
                has_more: conn.page_info.has_next_page,
                next_cursor: conn.page_info.end_cursor,
                payout_id: id,
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Error loading transactions".to_string()
            }))
            .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to fetch transactions: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to load transactions",
            )
                .into_response()
        }
    }
}

/// CSV export handler for payout transactions.
///
/// # Errors
///
/// Returns 500 if transactions cannot be fetched.
#[instrument(skip(state))]
pub async fn export_csv(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let payout_id = normalize_payout_id(&id);

    // Get payout details to extract the issued_at date for filtering
    let payout_date = match state.shopify().get_payout_detail(&payout_id).await {
        Ok(payout) => payout.issued_at,
        Err(e) => {
            tracing::warn!("Could not fetch payout for date: {e}");
            None
        }
    };

    // Fetch all transactions for this payout (up to 250 for export)
    match state
        .shopify()
        .get_payout_transactions(250, None, Some(payout_id.clone()), payout_date)
        .await
    {
        Ok(conn) => {
            use std::fmt::Write;
            let mut csv = String::from("Date,Type,Source,Order,Amount,Fee,Net\n");
            for t in &conn.transactions {
                let view = TransactionView::from(t);
                let _ = writeln!(
                    csv,
                    "{},{},{},{},{},{},{}",
                    view.date,
                    view.transaction_type,
                    view.source,
                    view.order_name.as_deref().unwrap_or("-"),
                    view.amount,
                    view.fee,
                    view.net
                );
            }

            let filename = format!("payout-{}-transactions.csv", extract_short_id(&payout_id));
            (
                StatusCode::OK,
                [
                    ("Content-Type", "text/csv"),
                    (
                        "Content-Disposition",
                        &format!("attachment; filename=\"{filename}\""),
                    ),
                ],
                csv,
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to export transactions: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to export transactions",
            )
                .into_response()
        }
    }
}

/// Dispute evidence submission handler.
///
/// Redirects to Shopify admin for evidence submission since the API
/// requires file uploads which are complex to handle in this context.
#[instrument]
pub async fn dispute_submit_evidence(
    RequireAdminAuth(_admin): RequireAdminAuth,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // The Shopify Payments API for dispute evidence primarily handles file uploads
    // which requires a more complex flow. Redirect to Shopify admin for now.
    let shopify_url =
        format!("https://admin.shopify.com/store/naked-pineapple/settings/payments/disputes/{id}");
    axum::response::Redirect::temporary(&shopify_url)
}

// =============================================================================
// Sorting Functions
// =============================================================================

/// Sort disputes by the specified column.
///
/// Shopify API does not support sorting for disputes, so we handle it client-side.
fn sort_disputes(disputes: &mut [DisputeView], column: &str, reverse: bool) {
    disputes.sort_by(|a, b| {
        let cmp = match column {
            "initiated_at" => a.initiated_at.cmp(&b.initiated_at),
            "order" => a.order_name.cmp(&b.order_name),
            "type" => a.dispute_type.cmp(&b.dispute_type),
            "status" => a.status.cmp(&b.status),
            "reason" => a.reason.cmp(&b.reason),
            "amount" => compare_money(&a.amount, &b.amount),
            "evidence_due_by" => a.evidence_due_by.cmp(&b.evidence_due_by),
            _ => std::cmp::Ordering::Equal,
        };
        if reverse { cmp.reverse() } else { cmp }
    });
}

/// Sort transactions by the specified column.
fn sort_transactions(transactions: &mut [TransactionView], column: &str, reverse: bool) {
    transactions.sort_by(|a, b| {
        let cmp = match column {
            "transaction_date" => a.date.cmp(&b.date),
            "type" => a.transaction_type.cmp(&b.transaction_type),
            "source" => a.source.cmp(&b.source),
            "order" => a.order_name.cmp(&b.order_name),
            "amount" => compare_money(&a.amount, &b.amount),
            "fee" => compare_money(&a.fee, &b.fee),
            "net" => compare_money(&a.net, &b.net),
            _ => std::cmp::Ordering::Equal,
        };
        if reverse { cmp.reverse() } else { cmp }
    });
}

/// Compare money strings (e.g., "$123.45").
fn compare_money(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> f64 {
        s.trim_start_matches('$')
            .trim_start_matches('-')
            .replace(',', "")
            .parse()
            .unwrap_or(0.0)
    };
    let a_neg = a.starts_with("-$") || a.starts_with("$-");
    let b_neg = b.starts_with("-$") || b.starts_with("$-");
    let a_val = if a_neg { -parse(a) } else { parse(a) };
    let b_val = if b_neg { -parse(b) } else { parse(b) };
    a_val
        .partial_cmp(&b_val)
        .unwrap_or(std::cmp::Ordering::Equal)
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Get status badge classes for payout status.
const fn status_badge(status: PayoutStatus) -> (&'static str, &'static str) {
    match status {
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
    }
}

/// Get status badge classes for dispute status, with urgency indicator.
const fn dispute_status_badge(status: DisputeStatus) -> (&'static str, &'static str, bool) {
    match status {
        DisputeStatus::NeedsResponse => (
            "Needs Response",
            "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400 animate-pulse",
            true,
        ),
        DisputeStatus::UnderReview => (
            "Under Review",
            "bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400",
            false,
        ),
        DisputeStatus::ChargeRefunded => (
            "Charge Refunded",
            "bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-400",
            false,
        ),
        DisputeStatus::Accepted => (
            "Accepted",
            "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400",
            false,
        ),
        DisputeStatus::Won => (
            "Won",
            "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400",
            false,
        ),
        DisputeStatus::Lost => (
            "Lost",
            "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400",
            false,
        ),
    }
}

/// Get type badge classes for dispute type.
const fn dispute_type_badge(dtype: DisputeType) -> (&'static str, &'static str) {
    match dtype {
        DisputeType::Chargeback => (
            "Chargeback",
            "bg-red-50 text-red-700 dark:bg-red-900/20 dark:text-red-400",
        ),
        DisputeType::Inquiry => (
            "Inquiry",
            "bg-blue-50 text-blue-700 dark:bg-blue-900/20 dark:text-blue-400",
        ),
    }
}

/// Get type badge classes for transaction type.
const fn transaction_type_badge(ttype: BalanceTransactionType) -> (&'static str, &'static str) {
    match ttype {
        BalanceTransactionType::Charge => (
            "Charge",
            "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400",
        ),
        BalanceTransactionType::Refund => (
            "Refund",
            "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400",
        ),
        BalanceTransactionType::Adjustment => (
            "Adjustment",
            "bg-blue-100 text-blue-700 dark:bg-blue-900/30 dark:text-blue-400",
        ),
        BalanceTransactionType::Dispute => (
            "Dispute",
            "bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400",
        ),
        BalanceTransactionType::Payout => (
            "Payout",
            "bg-purple-100 text-purple-700 dark:bg-purple-900/30 dark:text-purple-400",
        ),
        BalanceTransactionType::ReservedFunds => (
            "Reserved",
            "bg-gray-100 text-gray-700 dark:bg-gray-800 dark:text-gray-400",
        ),
    }
}

/// Get status badge classes for bank account status.
const fn bank_account_status_badge(status: BankAccountStatus) -> (&'static str, &'static str) {
    match status {
        BankAccountStatus::Pending => (
            "Pending",
            "bg-yellow-100 text-yellow-700 dark:bg-yellow-900/30 dark:text-yellow-400",
        ),
        BankAccountStatus::Verified => (
            "Verified",
            "bg-green-100 text-green-700 dark:bg-green-900/30 dark:text-green-400",
        ),
        BankAccountStatus::Deleted => (
            "Deleted",
            "bg-red-100 text-red-700 dark:bg-red-900/30 dark:text-red-400",
        ),
    }
}

/// Extract short ID from Shopify GID.
fn extract_short_id(gid: &str) -> String {
    gid.rsplit('/').next().unwrap_or(gid).to_string()
}

/// Extract legacy ID number from Shopify GID.
fn extract_legacy_id(gid: &str) -> String {
    gid.rsplit('/').next().unwrap_or(gid).to_string()
}

/// Normalize payout ID to full Shopify GID format.
fn normalize_payout_id(id: &str) -> String {
    if id.starts_with("gid://") {
        id.to_string()
    } else {
        format!("gid://shopify/ShopifyPaymentsPayout/{id}")
    }
}

/// Normalize dispute ID to full Shopify GID format.
fn normalize_dispute_id(id: &str) -> String {
    if id.starts_with("gid://") {
        id.to_string()
    } else {
        format!("gid://shopify/ShopifyPaymentsDispute/{id}")
    }
}
