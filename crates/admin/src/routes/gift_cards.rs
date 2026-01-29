//! Gift card management route handlers.

use askama::Template;
use axum::{
    Form,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{
    components::data_table::{DataTableConfig, gift_cards_table_config},
    filters,
    middleware::auth::RequireAdminAuth,
    shopify::types::{GiftCard, GiftCardDetail, GiftCardSortKey, GiftCardTransaction},
    state::AppState,
};

use super::dashboard::AdminUserView;

/// Query parameters for gift cards list.
#[derive(Debug, Deserialize)]
pub struct GiftCardsQuery {
    /// Pagination cursor.
    pub cursor: Option<String>,
    /// Search query (general search).
    pub q: Option<String>,
    /// Sort field.
    pub sort: Option<String>,
    /// Sort direction ("asc" or "desc").
    pub direction: Option<String>,
    /// Status filter (comma-separated: "enabled,disabled,expired,expiring").
    pub status: Option<String>,
    /// Balance status filter (comma-separated: "full,partial,empty").
    pub balance_status: Option<String>,
    /// Source filter (comma-separated: `manual,purchased,api_client`).
    pub source: Option<String>,
    /// Created at minimum date.
    pub created_at_min: Option<String>,
    /// Created at maximum date.
    pub created_at_max: Option<String>,
    /// Expires on minimum date.
    pub expires_on_min: Option<String>,
    /// Expires on maximum date.
    pub expires_on_max: Option<String>,
}

impl GiftCardsQuery {
    /// Build a Shopify-compatible query string from the filters.
    fn build_shopify_query(&self) -> Option<String> {
        let mut parts = Vec::new();

        // General search term
        if let Some(q) = &self.q
            && !q.is_empty()
        {
            parts.push(q.clone());
        }

        // Status filter
        if let Some(status) = &self.status {
            let statuses: Vec<&str> = status.split(',').collect();
            let status_parts: Vec<String> =
                statuses.iter().map(|s| format!("status:{s}")).collect();
            if !status_parts.is_empty() {
                parts.push(format!("({})", status_parts.join(" OR ")));
            }
        }

        // Balance status filter
        if let Some(balance) = &self.balance_status {
            let balances: Vec<&str> = balance.split(',').collect();
            let balance_parts: Vec<String> = balances
                .iter()
                .map(|b| format!("balance_status:{b}"))
                .collect();
            if !balance_parts.is_empty() {
                parts.push(format!("({})", balance_parts.join(" OR ")));
            }
        }

        // Source filter
        if let Some(source) = &self.source {
            let sources: Vec<&str> = source.split(',').collect();
            let source_parts: Vec<String> = sources.iter().map(|s| format!("source:{s}")).collect();
            if !source_parts.is_empty() {
                parts.push(format!("({})", source_parts.join(" OR ")));
            }
        }

        // Date range filters
        if let Some(min) = &self.created_at_min
            && !min.is_empty()
        {
            parts.push(format!("created_at:>={min}"));
        }
        if let Some(max) = &self.created_at_max
            && !max.is_empty()
        {
            parts.push(format!("created_at:<={max}"));
        }
        if let Some(min) = &self.expires_on_min
            && !min.is_empty()
        {
            parts.push(format!("expires_on:>={min}"));
        }
        if let Some(max) = &self.expires_on_max
            && !max.is_empty()
        {
            parts.push(format!("expires_on:<={max}"));
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" AND "))
        }
    }

    /// Parse sort key from query parameter.
    fn parse_sort_key(&self) -> Option<GiftCardSortKey> {
        self.sort.as_ref().map(|s| match s.as_str() {
            "code" => GiftCardSortKey::Code,
            "balance" => GiftCardSortKey::Balance,
            "initial_value" => GiftCardSortKey::InitialValue,
            "customer" => GiftCardSortKey::CustomerName,
            "expires_on" => GiftCardSortKey::ExpiresOn,
            "updated_at" => GiftCardSortKey::UpdatedAt,
            // Default to CreatedAt for "created_at" or any unknown sort key
            _ => GiftCardSortKey::CreatedAt,
        })
    }

    /// Check if sort should be reversed (descending).
    fn is_reverse(&self) -> bool {
        self.direction.as_ref().is_some_and(|d| d == "desc")
    }

    /// Count active filters.
    const fn active_filter_count(&self) -> usize {
        let mut count = 0;
        if self.status.is_some() {
            count += 1;
        }
        if self.balance_status.is_some() {
            count += 1;
        }
        if self.source.is_some() {
            count += 1;
        }
        if self.created_at_min.is_some() || self.created_at_max.is_some() {
            count += 1;
        }
        if self.expires_on_min.is_some() || self.expires_on_max.is_some() {
            count += 1;
        }
        count
    }
}

/// Column visibility for gift cards table.
///
/// Each boolean represents whether a column should be visible.
/// This is the natural representation for column toggles.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GiftCardColumnVisibility {
    pub code: bool,
    pub balance: bool,
    pub initial_value: bool,
    pub status: bool,
    pub customer: bool,
    pub expires_on: bool,
    pub created_at: bool,
    pub order: bool,
    pub note: bool,
    pub updated_at: bool,
}

impl Default for GiftCardColumnVisibility {
    fn default() -> Self {
        Self {
            code: true,
            balance: true,
            initial_value: true,
            status: true,
            customer: true,
            expires_on: true,
            created_at: true,
            order: false,
            note: false,
            updated_at: false,
        }
    }
}

/// Gift card view for templates.
///
/// Boolean fields represent card status flags (enabled, expired, etc.)
/// which are the natural representation for template conditionals.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct GiftCardView {
    pub id: String,
    pub short_id: String,
    pub last_characters: String,
    pub masked_code: String,
    pub balance: String,
    pub balance_raw: f64,
    pub initial_value: String,
    pub initial_value_raw: f64,
    pub expires_on: Option<String>,
    pub enabled: bool,
    pub is_expired: bool,
    pub is_expiring_soon: bool,
    pub balance_empty: bool,
    pub balance_partial: bool,
    pub created_at: String,
    pub customer_id: Option<String>,
    pub customer_name: Option<String>,
    pub customer_email: Option<String>,
    pub order_id: Option<String>,
    pub order_name: Option<String>,
    pub note: Option<String>,
}

impl From<&GiftCard> for GiftCardView {
    fn from(gc: &GiftCard) -> Self {
        let balance_amount: f64 = gc.balance.amount.parse().unwrap_or(0.0);
        let initial_amount: f64 = gc.initial_value.amount.parse().unwrap_or(0.0);

        // Extract short ID from Shopify GID
        let short_id = gc.id.split('/').next_back().unwrap_or(&gc.id).to_string();

        Self {
            id: gc.id.clone(),
            short_id,
            last_characters: gc.last_characters.clone(),
            masked_code: gc
                .masked_code
                .clone()
                .unwrap_or_else(|| format!("****{}", gc.last_characters)),
            balance: format!("${balance_amount:.2}"),
            balance_raw: balance_amount,
            initial_value: format!("${initial_amount:.2}"),
            initial_value_raw: initial_amount,
            expires_on: gc.expires_on.clone(),
            enabled: gc.enabled,
            is_expired: false,       // TODO: Check against current date
            is_expiring_soon: false, // TODO: Check if expires within 30 days
            balance_empty: balance_amount == 0.0,
            balance_partial: balance_amount > 0.0 && balance_amount < initial_amount,
            created_at: gc.created_at.clone(),
            customer_id: gc.customer_id.clone(),
            customer_name: gc.customer_name.clone(),
            customer_email: gc.customer_email.clone(),
            order_id: gc.order_id.clone(),
            order_name: gc.order_name.clone(),
            note: gc.note.clone(),
        }
    }
}

/// Gift cards list page template.
#[derive(Template)]
#[template(path = "gift_cards/index.html")]
pub struct GiftCardsIndexTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub gift_cards: Vec<GiftCardView>,
    pub has_next_page: bool,
    pub next_cursor: Option<String>,
    pub search_query: Option<String>,
    pub table_config: DataTableConfig,
    pub columns: GiftCardColumnVisibility,
    pub filter_count: usize,
    pub total_count: Option<i64>,
    pub sort_key: Option<String>,
    pub sort_direction: String,
}

/// Gift card create form template.
#[derive(Template)]
#[template(path = "gift_cards/new.html")]
pub struct GiftCardNewTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub error: Option<String>,
    pub success_code: Option<String>,
    pub issue_limit: Option<String>,
}

/// Form input for creating gift cards.
#[derive(Debug, Deserialize)]
pub struct GiftCardFormInput {
    pub initial_value: String,
    pub customer_id: Option<String>,
    pub expires_on: Option<String>,
    pub note: Option<String>,
    pub recipient_id: Option<String>,
    pub recipient_message: Option<String>,
}

/// Transaction view for templates.
#[derive(Debug, Clone)]
pub struct GiftCardTransactionView {
    pub id: String,
    pub amount: String,
    pub amount_raw: f64,
    pub processed_at: String,
    pub note: Option<String>,
    pub is_credit: bool,
}

impl From<&GiftCardTransaction> for GiftCardTransactionView {
    fn from(tx: &GiftCardTransaction) -> Self {
        let amount_raw: f64 = tx.amount.amount.parse().unwrap_or(0.0);
        Self {
            id: tx.id.clone(),
            amount: format!("${amount_raw:.2}"),
            amount_raw,
            processed_at: tx.processed_at.clone(),
            note: tx.note.clone(),
            is_credit: tx.is_credit,
        }
    }
}

/// Gift card detail view for templates.
///
/// Boolean fields represent card status flags (enabled, expired, etc.)
/// which are the natural representation for template conditionals.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct GiftCardDetailView {
    pub id: String,
    pub short_id: String,
    pub last_characters: String,
    pub masked_code: String,
    pub balance: String,
    pub balance_raw: f64,
    pub initial_value: String,
    pub initial_value_raw: f64,
    pub amount_spent: String,
    pub amount_spent_raw: f64,
    pub balance_percentage: f64,
    pub expires_on: Option<String>,
    pub enabled: bool,
    pub is_expired: bool,
    pub is_expiring_soon: bool,
    pub balance_empty: bool,
    pub balance_partial: bool,
    pub deactivated_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub note: Option<String>,
    pub template_suffix: Option<String>,
    pub customer_id: Option<String>,
    pub customer_short_id: Option<String>,
    pub customer_name: Option<String>,
    pub customer_email: Option<String>,
    pub customer_phone: Option<String>,
    pub recipient_id: Option<String>,
    pub recipient_name: Option<String>,
    pub recipient_email: Option<String>,
    pub recipient_preferred_name: Option<String>,
    pub recipient_message: Option<String>,
    pub recipient_send_notification_at: Option<String>,
    pub order_id: Option<String>,
    pub order_short_id: Option<String>,
    pub order_name: Option<String>,
    pub order_created_at: Option<String>,
    pub transactions: Vec<GiftCardTransactionView>,
}

impl From<&GiftCardDetail> for GiftCardDetailView {
    fn from(gc: &GiftCardDetail) -> Self {
        let balance_amount: f64 = gc.balance.amount.parse().unwrap_or(0.0);
        let initial_amount: f64 = gc.initial_value.amount.parse().unwrap_or(0.0);
        let amount_spent = initial_amount - balance_amount;
        let balance_percentage = if initial_amount > 0.0 {
            (balance_amount / initial_amount) * 100.0
        } else {
            0.0
        };

        // Extract short ID from Shopify GID
        let short_id = gc.id.split('/').next_back().unwrap_or(&gc.id).to_string();

        let customer_short_id = gc
            .customer_id
            .as_ref()
            .and_then(|id| id.split('/').next_back().map(String::from));

        let order_short_id = gc
            .order_id
            .as_ref()
            .and_then(|id| id.split('/').next_back().map(String::from));

        // Check expiration status
        let is_expired = gc.expires_on.as_ref().is_some_and(|exp| {
            chrono::NaiveDate::parse_from_str(exp, "%Y-%m-%d")
                .map(|date| date < chrono::Utc::now().date_naive())
                .unwrap_or(false)
        });

        let is_expiring_soon = gc.expires_on.as_ref().is_some_and(|exp| {
            chrono::NaiveDate::parse_from_str(exp, "%Y-%m-%d")
                .map(|date| {
                    let today = chrono::Utc::now().date_naive();
                    let thirty_days = today + chrono::Duration::days(30);
                    date >= today && date <= thirty_days
                })
                .unwrap_or(false)
        });

        let transactions: Vec<GiftCardTransactionView> = gc
            .transactions
            .iter()
            .map(GiftCardTransactionView::from)
            .collect();

        // Extract recipient details
        let (
            recipient_id,
            recipient_name,
            recipient_email,
            recipient_preferred_name,
            recipient_message,
            recipient_send_notification_at,
        ) = gc
            .recipient
            .as_ref()
            .map_or((None, None, None, None, None, None), |r| {
                (
                    r.recipient_id.clone(),
                    r.recipient_name.clone(),
                    r.recipient_email.clone(),
                    r.preferred_name.clone(),
                    r.message.clone(),
                    r.send_notification_at.clone(),
                )
            });

        Self {
            id: gc.id.clone(),
            short_id,
            last_characters: gc.last_characters.clone(),
            masked_code: gc.masked_code.clone(),
            balance: format!("${balance_amount:.2}"),
            balance_raw: balance_amount,
            initial_value: format!("${initial_amount:.2}"),
            initial_value_raw: initial_amount,
            amount_spent: format!("${amount_spent:.2}"),
            amount_spent_raw: amount_spent,
            balance_percentage,
            expires_on: gc.expires_on.clone(),
            enabled: gc.enabled,
            is_expired,
            is_expiring_soon,
            balance_empty: balance_amount == 0.0,
            balance_partial: balance_amount > 0.0 && balance_amount < initial_amount,
            deactivated_at: gc.deactivated_at.clone(),
            created_at: gc.created_at.clone(),
            updated_at: gc.updated_at.clone(),
            note: gc.note.clone(),
            template_suffix: gc.template_suffix.clone(),
            customer_id: gc.customer_id.clone(),
            customer_short_id,
            customer_name: gc.customer_name.clone(),
            customer_email: gc.customer_email.clone(),
            customer_phone: gc.customer_phone.clone(),
            recipient_id,
            recipient_name,
            recipient_email,
            recipient_preferred_name,
            recipient_message,
            recipient_send_notification_at,
            order_id: gc.order_id.clone(),
            order_short_id,
            order_name: gc.order_name.clone(),
            order_created_at: gc.order_created_at.clone(),
            transactions,
        }
    }
}

/// Gift card detail page template.
#[derive(Template)]
#[template(path = "gift_cards/show.html")]
pub struct GiftCardShowTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub gift_card: GiftCardDetailView,
    pub currency_code: String,
}

/// Form input for adjusting gift card balance.
#[derive(Debug, Deserialize)]
pub struct AdjustBalanceInput {
    pub adjustment_type: String,
    pub amount: String,
    pub note: Option<String>,
}

/// Form input for updating gift card note.
#[derive(Debug, Deserialize)]
pub struct UpdateNoteInput {
    pub note: String,
}

/// Gift card edit page template.
#[derive(Template)]
#[template(path = "gift_cards/edit.html")]
pub struct GiftCardEditTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub gift_card: GiftCardDetailView,
    pub error: Option<String>,
    pub success: bool,
}

/// Form input for updating gift card.
#[derive(Debug, Deserialize)]
pub struct GiftCardUpdateInput {
    pub note: Option<String>,
    pub expires_on: Option<String>,
    pub customer_id: Option<String>,
}

/// Gift cards list page handler.
#[instrument(skip(admin, state))]
pub async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<GiftCardsQuery>,
) -> Html<String> {
    let shopify_query = query.build_shopify_query();
    let sort_key = query.parse_sort_key();
    let reverse = query.is_reverse();
    let filter_count = query.active_filter_count();

    let result = state
        .shopify()
        .get_gift_cards(25, query.cursor.clone(), shopify_query, sort_key, reverse)
        .await;

    let (gift_cards, has_next_page, next_cursor, total_count) = match result {
        Ok(conn) => {
            let gift_cards: Vec<GiftCardView> =
                conn.gift_cards.iter().map(GiftCardView::from).collect();
            (
                gift_cards,
                conn.page_info.has_next_page,
                conn.page_info.end_cursor,
                conn.total_count,
            )
        }
        Err(e) => {
            tracing::error!("Failed to fetch gift cards: {e}");
            (vec![], false, None, None)
        }
    };

    let template = GiftCardsIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/gift-cards".to_string(),
        gift_cards,
        has_next_page,
        next_cursor,
        search_query: query.q,
        table_config: gift_cards_table_config(),
        columns: GiftCardColumnVisibility::default(),
        filter_count,
        total_count,
        sort_key: query.sort,
        sort_direction: query.direction.unwrap_or_else(|| "desc".to_string()),
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// New gift card form handler.
#[instrument(skip(admin, state))]
pub async fn new_gift_card(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
) -> Html<String> {
    // Fetch gift card configuration for limits
    let issue_limit = state
        .shopify()
        .get_gift_card_configuration()
        .await
        .ok()
        .and_then(|config| config.issue_limit)
        .map(|limit| format!("${}", limit.amount));

    let template = GiftCardNewTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/gift-cards".to_string(),
        error: None,
        success_code: None,
        issue_limit,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// Create gift card handler.
#[instrument(skip(admin, state))]
pub async fn create(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<GiftCardFormInput>,
) -> impl IntoResponse {
    match state
        .shopify()
        .create_gift_card(
            &input.initial_value,
            input.customer_id.as_deref(),
            input.expires_on.as_deref(),
            input.note.as_deref(),
            input.recipient_id.as_deref(),
            input.recipient_message.as_deref(),
        )
        .await
    {
        Ok((gift_card_id, code)) => {
            tracing::info!(gift_card_id = %gift_card_id, initial_value = %input.initial_value, "Gift card created");
            let template = GiftCardNewTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/gift-cards".to_string(),
                error: None,
                success_code: Some(code),
                issue_limit: None, // Not needed on success page
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
        Err(e) => {
            tracing::error!(initial_value = %input.initial_value, error = %e, "Failed to create gift card");

            // Fetch issue limit for error page
            let issue_limit = state
                .shopify()
                .get_gift_card_configuration()
                .await
                .ok()
                .and_then(|config| config.issue_limit)
                .map(|limit| format!("${}", limit.amount));

            let template = GiftCardNewTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/gift-cards".to_string(),
                error: Some(e.to_string()),
                success_code: None,
                issue_limit,
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
    }
}

/// Deactivate gift card handler (HTMX).
#[instrument(skip(_admin, state))]
pub async fn deactivate(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let gift_card_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/GiftCard/{id}")
    };

    match state.shopify().deactivate_gift_card(&gift_card_id).await {
        Ok(()) => {
            tracing::info!(gift_card_id = %gift_card_id, "Gift card deactivated");
            (
                StatusCode::OK,
                [("HX-Trigger", "gift-card-deactivated")],
                Html(
                    "<span class=\"inline-flex items-center px-2.5 py-0.5 rounded-full text-xs font-medium bg-red-500/10 text-red-400 border border-red-500/20\">Disabled</span>"
                        .to_string(),
                ),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(gift_card_id = %gift_card_id, error = %e, "Failed to deactivate gift card");
            (
                StatusCode::BAD_REQUEST,
                Html(format!("<span class=\"text-red-400\">Error: {e}</span>")),
            )
                .into_response()
        }
    }
}

/// Gift card detail page handler.
#[instrument(skip(admin, state))]
pub async fn show(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let gift_card_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/GiftCard/{id}")
    };

    match state.shopify().get_gift_card_detail(&gift_card_id).await {
        Ok(gift_card) => {
            let gift_card_view = GiftCardDetailView::from(&gift_card);
            let template = GiftCardShowTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/gift-cards".to_string(),
                gift_card: gift_card_view,
                currency_code: gift_card.balance.currency_code,
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
        Err(e) => {
            tracing::error!(gift_card_id = %gift_card_id, error = %e, "Failed to fetch gift card");
            (
                StatusCode::NOT_FOUND,
                Html("Gift card not found".to_string()),
            )
                .into_response()
        }
    }
}

/// Adjust gift card balance handler (HTMX).
#[instrument(skip(_admin, state))]
pub async fn adjust_balance(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<AdjustBalanceInput>,
) -> impl IntoResponse {
    let gift_card_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/GiftCard/{id}")
    };

    let result = if input.adjustment_type == "credit" {
        state
            .shopify()
            .credit_gift_card(&gift_card_id, &input.amount, "USD", input.note.as_deref())
            .await
    } else {
        state
            .shopify()
            .debit_gift_card(&gift_card_id, &input.amount, "USD", input.note.as_deref())
            .await
    };

    match result {
        Ok(tx) => {
            let amount: f64 = tx.amount.amount.parse().unwrap_or(0.0);
            tracing::info!(
                gift_card_id = %gift_card_id,
                amount = %amount,
                adjustment_type = %input.adjustment_type,
                "Gift card balance adjusted"
            );
            (
                StatusCode::OK,
                [("HX-Refresh", "true")],
                Html(String::new()),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(
                gift_card_id = %gift_card_id,
                error = %e,
                "Failed to adjust gift card balance"
            );
            (
                StatusCode::BAD_REQUEST,
                Html(format!(
                    r#"<div class="text-red-400 text-sm">Error: {e}</div>"#
                )),
            )
                .into_response()
        }
    }
}

/// Send notification to customer handler (HTMX).
#[instrument(skip(_admin, state))]
pub async fn notify_customer(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let gift_card_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/GiftCard/{id}")
    };

    match state
        .shopify()
        .send_gift_card_notification_to_customer(&gift_card_id)
        .await
    {
        Ok(()) => {
            tracing::info!(gift_card_id = %gift_card_id, "Gift card notification sent to customer");
            (
                StatusCode::OK,
                Html(
                    r#"<span class="text-green-400 text-sm"><i class="ph ph-check-circle mr-1"></i>Notification sent</span>"#
                        .to_string(),
                ),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(
                gift_card_id = %gift_card_id,
                error = %e,
                "Failed to send gift card notification to customer"
            );
            (
                StatusCode::BAD_REQUEST,
                Html(format!(
                    r#"<span class="text-red-400 text-sm">Error: {e}</span>"#
                )),
            )
                .into_response()
        }
    }
}

/// Send notification to recipient handler (HTMX).
#[instrument(skip(_admin, state))]
pub async fn notify_recipient(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let gift_card_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/GiftCard/{id}")
    };

    match state
        .shopify()
        .send_gift_card_notification_to_recipient(&gift_card_id)
        .await
    {
        Ok(()) => {
            tracing::info!(gift_card_id = %gift_card_id, "Gift card notification sent to recipient");
            (
                StatusCode::OK,
                Html(
                    r#"<span class="text-green-400 text-sm"><i class="ph ph-check-circle mr-1"></i>Notification sent</span>"#
                        .to_string(),
                ),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(
                gift_card_id = %gift_card_id,
                error = %e,
                "Failed to send gift card notification to recipient"
            );
            (
                StatusCode::BAD_REQUEST,
                Html(format!(
                    r#"<span class="text-red-400 text-sm">Error: {e}</span>"#
                )),
            )
                .into_response()
        }
    }
}

/// Update gift card note handler (HTMX).
#[instrument(skip(_admin, state))]
pub async fn update_note(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<UpdateNoteInput>,
) -> impl IntoResponse {
    let gift_card_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/GiftCard/{id}")
    };

    match state
        .shopify()
        .update_gift_card(&gift_card_id, Some(&input.note), None, None)
        .await
    {
        Ok(()) => {
            tracing::info!(gift_card_id = %gift_card_id, "Gift card note updated");
            (
                StatusCode::OK,
                [("HX-Trigger", "note-updated")],
                Html(
                    r#"<span class="text-green-400 text-sm"><i class="ph ph-check-circle mr-1"></i>Note saved</span>"#
                        .to_string(),
                ),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(
                gift_card_id = %gift_card_id,
                error = %e,
                "Failed to update gift card note"
            );
            (
                StatusCode::BAD_REQUEST,
                Html(format!(
                    r#"<span class="text-red-400 text-sm">Error: {e}</span>"#
                )),
            )
                .into_response()
        }
    }
}

/// Gift card edit page handler.
#[instrument(skip(admin, state))]
pub async fn edit(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let gift_card_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/GiftCard/{id}")
    };

    match state.shopify().get_gift_card_detail(&gift_card_id).await {
        Ok(gift_card) => {
            let template = GiftCardEditTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/gift-cards".to_string(),
                gift_card: GiftCardDetailView::from(&gift_card),
                error: None,
                success: false,
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
        Err(e) => {
            tracing::error!(gift_card_id = %gift_card_id, error = %e, "Failed to fetch gift card");
            (
                StatusCode::NOT_FOUND,
                Html("Gift card not found".to_string()),
            )
                .into_response()
        }
    }
}

/// Gift card update handler.
#[instrument(skip(admin, state))]
pub async fn update(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<GiftCardUpdateInput>,
) -> impl IntoResponse {
    let gift_card_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/GiftCard/{id}")
    };

    // Update the gift card
    let result = state
        .shopify()
        .update_gift_card(
            &gift_card_id,
            input.note.as_deref(),
            input.expires_on.as_deref(),
            input.customer_id.as_deref(),
        )
        .await;

    // Fetch the updated gift card
    let gift_card = match state.shopify().get_gift_card_detail(&gift_card_id).await {
        Ok(gc) => gc,
        Err(e) => {
            tracing::error!(gift_card_id = %gift_card_id, error = %e, "Failed to fetch gift card after update");
            return (
                StatusCode::NOT_FOUND,
                Html("Gift card not found".to_string()),
            )
                .into_response();
        }
    };

    let (error, success) = match result {
        Ok(()) => {
            tracing::info!(gift_card_id = %gift_card_id, "Gift card updated");
            (None, true)
        }
        Err(e) => {
            tracing::error!(gift_card_id = %gift_card_id, error = %e, "Failed to update gift card");
            (Some(e.to_string()), false)
        }
    };

    let template = GiftCardEditTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/gift-cards".to_string(),
        gift_card: GiftCardDetailView::from(&gift_card),
        error,
        success,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
    .into_response()
}

/// Form input for bulk deactivate.
#[derive(Debug, Deserialize)]
pub struct BulkDeactivateInput {
    pub ids: String,
}

/// Bulk deactivate gift cards handler.
#[instrument(skip(_admin, state))]
pub async fn bulk_deactivate(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<BulkDeactivateInput>,
) -> impl IntoResponse {
    let ids: Vec<&str> = input.ids.split(',').filter(|s| !s.is_empty()).collect();
    let mut success_count = 0;
    let mut error_count = 0;

    for id in &ids {
        let gift_card_id = if id.starts_with("gid://") {
            (*id).to_string()
        } else {
            format!("gid://shopify/GiftCard/{id}")
        };

        match state.shopify().deactivate_gift_card(&gift_card_id).await {
            Ok(()) => {
                success_count += 1;
                tracing::info!(gift_card_id = %gift_card_id, "Gift card deactivated (bulk)");
            }
            Err(e) => {
                error_count += 1;
                tracing::error!(gift_card_id = %gift_card_id, error = %e, "Failed to deactivate gift card (bulk)");
            }
        }
    }

    if error_count == 0 {
        (
            StatusCode::OK,
            [("HX-Redirect", "/gift-cards")],
            Html(format!("Deactivated {success_count} gift card(s)")),
        )
            .into_response()
    } else {
        (
            StatusCode::OK,
            [("HX-Redirect", "/gift-cards")],
            Html(format!(
                "Deactivated {success_count} gift card(s), {error_count} failed"
            )),
        )
            .into_response()
    }
}
