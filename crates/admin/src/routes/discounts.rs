//! Discount management route handlers.

#![allow(clippy::used_underscore_binding)]

use askama::Template;
use axum::{
    Form, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use serde::{Deserialize, Serialize};
use tracing::instrument;

use crate::{
    components::data_table::{
        BulkAction, FilterType, TableColumn, TableFilter, discounts_table_config,
    },
    filters,
    middleware::auth::RequireAdminAuth,
    shopify::{
        DiscountCreateInput,
        types::{
            CustomerSegment, DiscountCode, DiscountCombinesWith, DiscountListItem, DiscountMethod,
            DiscountMinimumRequirement, DiscountSortKey, DiscountStatus, DiscountType,
            DiscountValue,
        },
    },
    state::AppState,
};

use super::dashboard::AdminUserView;

// =============================================================================
// Query and Form Types
// =============================================================================

/// Pagination and filtering query parameters.
#[derive(Debug, Deserialize)]
pub struct DiscountsQuery {
    pub cursor: Option<String>,
    pub query: Option<String>,
    pub status: Option<String>,
    #[serde(rename = "type")]
    pub discount_type: Option<String>,
    pub method: Option<String>,
    pub sort: Option<String>,
    pub dir: Option<String>,
}

/// Form input for creating/updating basic discounts.
#[derive(Debug, Deserialize)]
pub struct BasicDiscountFormInput {
    pub title: String,
    pub code: Option<String>,
    pub method: String,
    pub discount_type: String,
    pub value: String,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub usage_limit: Option<i64>,
    pub once_per_customer: Option<bool>,
    pub minimum_type: Option<String>,
    pub minimum_value: Option<String>,
    pub combines_order: Option<bool>,
    pub combines_product: Option<bool>,
    pub combines_shipping: Option<bool>,
}

/// Form input for BXGY discounts.
#[derive(Debug, Deserialize)]
pub struct BxgyDiscountFormInput {
    pub title: String,
    pub code: Option<String>,
    pub method: String,
    pub buy_quantity: i64,
    pub buy_products: Option<String>,
    pub get_quantity: i64,
    pub get_products: Option<String>,
    pub get_discount_type: String,
    pub get_discount_value: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub usage_limit: Option<i64>,
}

/// Form input for free shipping discounts.
#[derive(Debug, Deserialize)]
pub struct FreeShippingFormInput {
    pub title: String,
    pub code: Option<String>,
    pub method: String,
    pub minimum_type: Option<String>,
    pub minimum_value: Option<String>,
    pub destination: Option<String>,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub usage_limit: Option<i64>,
}

/// Bulk action form input.
#[derive(Debug, Deserialize)]
pub struct BulkActionInput {
    pub ids: String,
}

/// Search query for API endpoints.
#[derive(Debug, Deserialize)]
pub struct SearchQuery {
    pub q: Option<String>,
    pub limit: Option<i64>,
}

/// Codes input for adding codes to a discount.
#[derive(Debug, Deserialize)]
pub struct AddCodesInput {
    pub codes: String,
}

// =============================================================================
// Column Visibility
// =============================================================================

/// Column visibility state for discounts table.
// Allow: This struct represents toggleable UI columns - each needs an independent bool.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default)]
pub struct DiscountColumnVisibility {
    pub title: bool,
    pub code: bool,
    pub discount_type: bool,
    pub status: bool,
    pub value: bool,
    pub usage: bool,
    pub method: bool,
    pub minimum: bool,
    pub combines_with: bool,
    pub starts_at: bool,
    pub ends_at: bool,
    pub created_at: bool,
}

impl DiscountColumnVisibility {
    /// Create from a list of visible column keys.
    #[must_use]
    pub fn from_columns(columns: &[String]) -> Self {
        Self {
            title: columns.contains(&"title".to_string()),
            code: columns.contains(&"code".to_string()),
            discount_type: columns.contains(&"type".to_string()),
            status: columns.contains(&"status".to_string()),
            value: columns.contains(&"value".to_string()),
            usage: columns.contains(&"usage".to_string()),
            method: columns.contains(&"method".to_string()),
            minimum: columns.contains(&"minimum".to_string()),
            combines_with: columns.contains(&"combines_with".to_string()),
            starts_at: columns.contains(&"starts_at".to_string()),
            ends_at: columns.contains(&"ends_at".to_string()),
            created_at: columns.contains(&"created_at".to_string()),
        }
    }

    /// Check if a column is visible by key.
    #[must_use]
    pub fn is_visible(&self, key: &str) -> bool {
        match key {
            "title" => self.title,
            "code" => self.code,
            "type" => self.discount_type,
            "status" => self.status,
            "value" => self.value,
            "usage" => self.usage,
            "method" => self.method,
            "minimum" => self.minimum,
            "combines_with" => self.combines_with,
            "starts_at" => self.starts_at,
            "ends_at" => self.ends_at,
            "created_at" => self.created_at,
            _ => true,
        }
    }
}

// =============================================================================
// View Types
// =============================================================================

/// Discount table view for `DataTable` rows.
#[derive(Debug, Clone, Serialize)]
pub struct DiscountTableView {
    pub id: String,
    pub title: String,
    pub code: Option<String>,
    pub code_count: i64,
    pub method: String,
    pub discount_type: String,
    pub status: String,
    pub status_class: String,
    pub value: Option<String>,
    pub usage: String,
    pub usage_percentage: u8,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub minimum: Option<String>,
    pub combines_with: String,
}

impl From<&DiscountListItem> for DiscountTableView {
    fn from(item: &DiscountListItem) -> Self {
        let (status, status_class) = match item.status {
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

        let method = match item.method {
            DiscountMethod::Code => "Code",
            DiscountMethod::Automatic => "Automatic",
        };

        let discount_type = match item.discount_type {
            DiscountType::Basic => "Amount Off",
            DiscountType::BuyXGetY => "Buy X Get Y",
            DiscountType::FreeShipping => "Free Shipping",
        };

        let value = item.value.as_ref().map(|v| match v {
            DiscountValue::Percentage { percentage } => {
                format!("{}%", (percentage * 100.0).round())
            }
            DiscountValue::FixedAmount { amount, currency } => {
                format!("${amount} {currency}")
            }
        });

        let usage = item.usage_limit.map_or_else(
            || format!("{} uses", item.usage_count),
            |limit| format!("{}/{} uses", item.usage_count, limit),
        );

        let usage_percentage = item.usage_limit.map_or(0_u8, |limit| {
            if limit == 0 {
                0
            } else {
                // Calculate percentage, clamping to 0-100 range
                let pct = (item.usage_count * 100).checked_div(limit).unwrap_or(0);
                u8::try_from(pct.clamp(0, 100)).unwrap_or(100)
            }
        });

        let minimum = match &item.minimum_requirement {
            DiscountMinimumRequirement::None => None,
            DiscountMinimumRequirement::Quantity { quantity } => {
                Some(format!("Min {quantity} items"))
            }
            DiscountMinimumRequirement::Subtotal { amount, currency } => {
                Some(format!("Min ${amount} {currency}"))
            }
        };

        let combines = format_combines_with(&item.combines_with);

        Self {
            id: item.id.clone(),
            title: item.title.clone(),
            code: item.code.clone(),
            code_count: item.code_count,
            method: method.to_string(),
            discount_type: discount_type.to_string(),
            status: status.to_string(),
            status_class: status_class.to_string(),
            value,
            usage,
            usage_percentage,
            starts_at: item.starts_at.clone(),
            ends_at: item.ends_at.clone(),
            minimum,
            combines_with: combines,
        }
    }
}

/// Legacy discount view for templates (for backward compatibility).
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

/// Customer segment view for picker.
#[derive(Debug, Clone, Serialize)]
pub struct CustomerSegmentView {
    pub id: String,
    pub name: String,
}

impl From<&CustomerSegment> for CustomerSegmentView {
    fn from(seg: &CustomerSegment) -> Self {
        Self {
            id: seg.id.clone(),
            name: seg.name.clone(),
        }
    }
}

// =============================================================================
// Template Types
// =============================================================================

/// Discounts list page template using `DataTable`.
#[derive(Template)]
#[template(path = "discounts/index.html")]
pub struct DiscountsIndexTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    /// Data table ID.
    pub table_id: String,
    /// Column definitions.
    pub columns: Vec<TableColumn>,
    /// Filter definitions.
    pub filters: Vec<TableFilter>,
    /// Bulk action definitions.
    pub bulk_actions: Vec<BulkAction>,
    /// Default visible columns.
    pub default_columns: Vec<String>,
    /// Column visibility state.
    pub col_visible: DiscountColumnVisibility,
    /// Discounts to display.
    pub discounts: Vec<DiscountTableView>,
    /// Whether there are more pages.
    pub has_next_page: bool,
    /// Cursor for next page.
    pub next_cursor: Option<String>,
    /// Current search query value.
    pub search_value: Option<String>,
    /// Active filter values for highlighting.
    pub filter_values: std::collections::HashMap<String, String>,
    /// Current sort column.
    pub sort_column: Option<String>,
    /// Current sort direction.
    pub sort_direction: String,
    /// URL params to preserve in pagination.
    pub preserve_params: String,
    /// Method filter (for tabs).
    pub method_filter: Option<String>,
}

/// Discount detail page template.
#[derive(Template)]
#[template(path = "discounts/show.html")]
pub struct DiscountShowTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub discount: DiscountTableView,
}

/// Step 1: Choose discount method.
#[derive(Template)]
#[template(path = "discounts/new_step1.html")]
pub struct DiscountNewStep1Template {
    pub admin_user: AdminUserView,
    pub current_path: String,
}

/// Step 2: Choose discount type.
#[derive(Template)]
#[template(path = "discounts/new_step2.html")]
pub struct DiscountNewStep2Template {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub method: String,
}

/// Step 3: Full discount form.
#[derive(Template)]
#[template(path = "discounts/new_step3.html")]
pub struct DiscountNewStep3Template {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub method: String,
    pub discount_type: String,
    pub error: Option<String>,
}

/// Discount edit form template.
#[derive(Template)]
#[template(path = "discounts/edit.html")]
pub struct DiscountEditTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub discount: DiscountTableView,
    pub error: Option<String>,
}

// Legacy templates for backward compatibility
/// Discount create form template (legacy).
#[derive(Template)]
#[template(path = "discounts/new.html")]
pub struct DiscountNewTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub error: Option<String>,
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Format `combines_with` settings into a display string.
fn format_combines_with(cw: &DiscountCombinesWith) -> String {
    let mut parts = Vec::new();
    if cw.product_discounts {
        parts.push("Product");
    }
    if cw.order_discounts {
        parts.push("Order");
    }
    if cw.shipping_discounts {
        parts.push("Shipping");
    }
    if parts.is_empty() {
        "None".to_string()
    } else {
        parts.join(", ")
    }
}

/// Parse sort key from query string.
fn parse_sort_key(key: Option<&str>) -> Option<DiscountSortKey> {
    match key {
        Some("title") => Some(DiscountSortKey::Title),
        Some("created_at") => Some(DiscountSortKey::CreatedAt),
        Some("updated_at") => Some(DiscountSortKey::UpdatedAt),
        Some("starts_at") => Some(DiscountSortKey::StartsAt),
        Some("ends_at") => Some(DiscountSortKey::EndsAt),
        _ => None,
    }
}

/// Build Shopify query string from filters.
fn build_query_string(query: &DiscountsQuery) -> Option<String> {
    let mut parts = Vec::new();

    if let Some(ref q) = query.query
        && !q.is_empty()
    {
        parts.push(q.clone());
    }

    if let Some(ref status) = query.status
        && !status.is_empty()
    {
        parts.push(format!("status:{status}"));
    }

    if let Some(ref method) = query.method {
        match method.as_str() {
            "code" => parts.push("discount_type:code_discount".to_string()),
            "automatic" => parts.push("discount_type:automatic_discount".to_string()),
            _ => {}
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" AND "))
    }
}

/// Extract the numeric ID from a GID or return as-is if already numeric.
fn extract_numeric_id(id: &str) -> &str {
    id.rsplit('/').next().unwrap_or(id)
}

/// Find a discount by ID, fetching all discounts if necessary.
async fn find_discount_by_id(state: &AppState, discount_id: &str) -> Option<DiscountTableView> {
    let target_id = extract_numeric_id(discount_id);
    let mut cursor: Option<String> = None;

    loop {
        let result = state
            .shopify()
            .get_discounts_for_list(250, cursor.clone(), None, None, false)
            .await;

        match result {
            Ok(conn) => {
                // Search for matching ID (compare numeric parts)
                for item in &conn.discounts {
                    if extract_numeric_id(&item.id) == target_id {
                        return Some(DiscountTableView::from(item));
                    }
                }

                // Continue to next page if available
                if conn.page_info.has_next_page {
                    cursor = conn.page_info.end_cursor;
                } else {
                    break;
                }
            }
            Err(e) => {
                tracing::error!("Failed to fetch discounts: {e}");
                break;
            }
        }
    }

    None
}

/// Check if a sort key requires client-side sorting (not supported by Shopify API).
fn requires_client_sort(sort_key: Option<&str>) -> bool {
    match sort_key {
        // These are supported by Shopify's DiscountSortKey
        Some("title" | "starts_at" | "ends_at" | "created_at" | "updated_at") | None => false,
        // Everything else needs client-side sorting
        Some(_) => true,
    }
}

/// Sort discounts by the given column key.
fn sort_discounts(discounts: &mut [DiscountTableView], sort_key: Option<&str>, reverse: bool) {
    let Some(key) = sort_key else { return };

    discounts.sort_by(|a, b| {
        let cmp = match key {
            "title" => a.title.to_lowercase().cmp(&b.title.to_lowercase()),
            "code" => {
                let a_code = a.code.as_deref().unwrap_or("");
                let b_code = b.code.as_deref().unwrap_or("");
                a_code.to_lowercase().cmp(&b_code.to_lowercase())
            }
            "type" => a.discount_type.cmp(&b.discount_type),
            "status" => a.status.cmp(&b.status),
            "value" => {
                let a_val = a.value.as_deref().unwrap_or("");
                let b_val = b.value.as_deref().unwrap_or("");
                a_val.cmp(b_val)
            }
            "usage" => a.usage_percentage.cmp(&b.usage_percentage),
            "method" => a.method.cmp(&b.method),
            "minimum" => {
                let a_min = a.minimum.as_deref().unwrap_or("");
                let b_min = b.minimum.as_deref().unwrap_or("");
                a_min.cmp(b_min)
            }
            "combines_with" => a.combines_with.cmp(&b.combines_with),
            "starts_at" => {
                let a_date = a.starts_at.as_deref().unwrap_or("");
                let b_date = b.starts_at.as_deref().unwrap_or("");
                a_date.cmp(b_date)
            }
            "ends_at" => {
                let a_date = a.ends_at.as_deref().unwrap_or("");
                let b_date = b.ends_at.as_deref().unwrap_or("");
                a_date.cmp(b_date)
            }
            _ => std::cmp::Ordering::Equal,
        };

        if reverse { cmp.reverse() } else { cmp }
    });
}

/// Build URL parameters for preserving filters across pagination.
fn build_preserve_params(query: &DiscountsQuery) -> String {
    let mut params = Vec::new();

    if let Some(q) = &query.query
        && !q.is_empty()
    {
        params.push(format!("&query={q}"));
    }

    if let Some(status) = &query.status
        && !status.is_empty()
    {
        params.push(format!("&status={status}"));
    }

    if let Some(method) = &query.method
        && !method.is_empty()
    {
        params.push(format!("&method={method}"));
    }

    if let Some(dtype) = &query.discount_type
        && !dtype.is_empty()
    {
        params.push(format!("&type={dtype}"));
    }

    // Note: sort and dir are excluded because they're set fresh in sort links
    params.join("")
}

// =============================================================================
// List and Detail Handlers
// =============================================================================

/// Fetch all discounts and sort/paginate client-side.
async fn fetch_and_sort_discounts(
    state: &AppState,
    shopify_query: Option<String>,
    sort_key: Option<&str>,
    reverse: bool,
    page_cursor: Option<&str>,
) -> (Vec<DiscountTableView>, bool, Option<String>) {
    let mut all_discounts = Vec::new();
    let mut cursor: Option<String> = None;

    loop {
        let result = state
            .shopify()
            .get_discounts_for_list(250, cursor.clone(), shopify_query.clone(), None, false)
            .await;

        match result {
            Ok(conn) => {
                all_discounts.extend(conn.discounts.iter().map(DiscountTableView::from));
                if conn.page_info.has_next_page {
                    cursor = conn.page_info.end_cursor;
                } else {
                    break;
                }
            }
            Err(e) => {
                tracing::error!("Failed to fetch discounts: {e}");
                break;
            }
        }
    }

    sort_discounts(&mut all_discounts, sort_key, reverse);

    let page: usize = page_cursor.and_then(|c| c.parse().ok()).unwrap_or(0);
    let page_size = 25;
    let start = page * page_size;
    let end = (start + page_size).min(all_discounts.len());

    let has_next = end < all_discounts.len();
    let next_cursor = if has_next {
        Some((page + 1).to_string())
    } else {
        None
    };

    let page_discounts = all_discounts
        .get(start..end)
        .map(ToOwned::to_owned)
        .unwrap_or_default();

    (page_discounts, has_next, next_cursor)
}

/// Discounts list page handler.
#[instrument(skip(admin, state))]
pub async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<DiscountsQuery>,
) -> Html<String> {
    let config = discounts_table_config();
    let shopify_query = build_query_string(&query);
    let reverse = query.dir.as_deref() == Some("desc");

    let (discounts, has_next_page, next_cursor) = if requires_client_sort(query.sort.as_deref()) {
        fetch_and_sort_discounts(
            &state,
            shopify_query,
            query.sort.as_deref(),
            reverse,
            query.cursor.as_deref(),
        )
        .await
    } else {
        let sort_key = parse_sort_key(query.sort.as_deref());
        let result = state
            .shopify()
            .get_discounts_for_list(25, query.cursor.clone(), shopify_query, sort_key, reverse)
            .await;

        match result {
            Ok(conn) => {
                let discounts: Vec<DiscountTableView> =
                    conn.discounts.iter().map(DiscountTableView::from).collect();
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
        }
    };

    // Build column visibility from defaults
    let default_columns = config.default_columns();
    let col_visible = DiscountColumnVisibility::from_columns(&default_columns);

    // Build filter values map for highlighting active filters
    let mut filter_values = std::collections::HashMap::new();
    if let Some(ref status) = query.status {
        filter_values.insert("status".to_string(), status.clone());
    }
    if let Some(ref dtype) = query.discount_type {
        filter_values.insert("type".to_string(), dtype.clone());
    }
    if let Some(ref method) = query.method {
        filter_values.insert("method".to_string(), method.clone());
    }

    let preserve_params = build_preserve_params(&query);
    let sort_direction = if reverse { "desc" } else { "asc" }.to_string();

    let template = DiscountsIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/discounts".to_string(),
        table_id: config.table_id.clone(),
        columns: config.columns,
        filters: config.filters,
        bulk_actions: config.bulk_actions,
        default_columns,
        col_visible,
        discounts,
        has_next_page,
        next_cursor,
        search_value: query.query,
        filter_values,
        sort_column: query.sort,
        sort_direction,
        preserve_params,
        method_filter: query.method,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// Discount detail page handler.
#[instrument(skip(admin, state))]
pub async fn show(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let discount_id = normalize_discount_id(&id);

    find_discount_by_id(&state, &discount_id).await.map_or_else(
        || (StatusCode::NOT_FOUND, "Discount not found").into_response(),
        |discount| {
            let template = DiscountShowTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/discounts".to_string(),
                discount,
            };
            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        },
    )
}

// =============================================================================
// Create Flow Handlers
// =============================================================================

/// Step 1: Choose discount method (Code vs Automatic).
#[instrument(skip(admin))]
pub async fn new_step1(RequireAdminAuth(admin): RequireAdminAuth) -> Html<String> {
    let template = DiscountNewStep1Template {
        admin_user: AdminUserView::from(&admin),
        current_path: "/discounts".to_string(),
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// Step 2: Choose discount type.
#[instrument(skip(admin))]
pub async fn new_step2(
    RequireAdminAuth(admin): RequireAdminAuth,
    Path(method): Path<String>,
) -> Html<String> {
    let template = DiscountNewStep2Template {
        admin_user: AdminUserView::from(&admin),
        current_path: "/discounts".to_string(),
        method,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// Step 3: Full discount form.
#[instrument(skip(admin))]
pub async fn new_step3(
    RequireAdminAuth(admin): RequireAdminAuth,
    Path((method, discount_type)): Path<(String, String)>,
) -> Html<String> {
    let template = DiscountNewStep3Template {
        admin_user: AdminUserView::from(&admin),
        current_path: "/discounts".to_string(),
        method,
        discount_type,
        error: None,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// Legacy new discount form handler.
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

// =============================================================================
// Create Handlers
// =============================================================================

/// Create basic discount handler.
#[instrument(skip(admin, state))]
pub async fn create_basic(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<BasicDiscountFormInput>,
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

    let code = input.code.as_deref().unwrap_or("");

    match state
        .shopify()
        .create_discount(DiscountCreateInput {
            title: &input.title,
            code,
            percentage,
            amount,
            starts_at,
            ends_at: input.ends_at.as_deref(),
            usage_limit: input.usage_limit,
        })
        .await
    {
        Ok(discount_id) => {
            tracing::info!(discount_id = %discount_id, code = %code, "Discount created");
            Redirect::to("/discounts").into_response()
        }
        Err(e) => {
            tracing::error!(code = %code, error = %e, "Failed to create discount");
            let template = DiscountNewStep3Template {
                admin_user: AdminUserView::from(&admin),
                current_path: "/discounts".to_string(),
                method: input.method,
                discount_type: input.discount_type,
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

/// Legacy create discount handler.
#[instrument(skip(admin, state))]
pub async fn create(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<BasicDiscountFormInput>,
) -> impl IntoResponse {
    create_basic(RequireAdminAuth(admin), State(state), Form(input)).await
}

// =============================================================================
// Edit and Update Handlers
// =============================================================================

/// Normalize discount ID to full Shopify format.
fn normalize_discount_id(id: &str) -> String {
    if id.starts_with("gid://") {
        id.to_string()
    } else {
        format!("gid://shopify/DiscountNode/{id}")
    }
}

/// Edit discount form handler.
#[instrument(skip(admin, state))]
pub async fn edit(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let discount_id = normalize_discount_id(&id);

    find_discount_by_id(&state, &discount_id).await.map_or_else(
        || (StatusCode::NOT_FOUND, "Discount not found").into_response(),
        |discount| {
            let template = DiscountEditTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/discounts".to_string(),
                discount,
                error: None,
            };
            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        },
    )
}

/// Update discount handler.
#[instrument(skip(admin, state))]
pub async fn update(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<BasicDiscountFormInput>,
) -> impl IntoResponse {
    use crate::shopify::DiscountUpdateInput;

    let discount_id = normalize_discount_id(&id);

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
            find_discount_by_id(&state, &discount_id).await.map_or_else(
                || Redirect::to("/discounts").into_response(),
                |discount| {
                    let template = DiscountEditTemplate {
                        admin_user: AdminUserView::from(&admin),
                        current_path: "/discounts".to_string(),
                        discount,
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

// =============================================================================
// Action Handlers
// =============================================================================

/// Activate discount handler.
#[instrument(skip(_admin, state))]
pub async fn activate(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let discount_id = normalize_discount_id(&id);

    match state.shopify().activate_discount(&discount_id).await {
        Ok(()) => {
            tracing::info!(discount_id = %discount_id, "Discount activated");
            (
                StatusCode::OK,
                [("HX-Trigger", "discount-activated")],
                Html("<span class=\"text-green-600 dark:text-green-400\">Activated</span>"),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(discount_id = %discount_id, error = %e, "Failed to activate discount");
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

/// Deactivate discount handler (HTMX).
#[instrument(skip(_admin, state))]
pub async fn deactivate(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let discount_id = normalize_discount_id(&id);

    match state.shopify().deactivate_discount(&discount_id).await {
        Ok(()) => {
            tracing::info!(discount_id = %discount_id, "Discount deactivated");
            (
                StatusCode::OK,
                [("HX-Trigger", "discount-deactivated")],
                Html("<span class=\"text-gray-600 dark:text-gray-400\">Deactivated</span>"),
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

/// Delete discount handler.
#[instrument(skip(_admin, state))]
pub async fn delete(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let discount_id = normalize_discount_id(&id);

    match state.shopify().delete_discount(&discount_id).await {
        Ok(()) => {
            tracing::info!(discount_id = %discount_id, "Discount deleted");
            (
                StatusCode::OK,
                [
                    ("HX-Trigger", "discount-deleted"),
                    ("HX-Redirect", "/discounts"),
                ],
                Html("Deleted"),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(discount_id = %discount_id, error = %e, "Failed to delete discount");
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

/// Duplicate discount handler.
#[instrument(skip(_admin, _state))]
pub async fn duplicate(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // TODO: Implement duplicate functionality
    // This requires fetching the discount details and creating a copy
    tracing::warn!(discount_id = %id, "Duplicate discount not yet implemented");
    Redirect::to("/discounts").into_response()
}

// =============================================================================
// Bulk Action Handlers
// =============================================================================

/// Parse comma-separated IDs into a vector.
fn parse_ids(ids_str: &str) -> Vec<String> {
    ids_str
        .split(',')
        .map(|s| normalize_discount_id(s.trim()))
        .collect()
}

/// Bulk activate discounts handler.
#[instrument(skip(_admin, state))]
pub async fn bulk_activate(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<BulkActionInput>,
) -> impl IntoResponse {
    let ids = parse_ids(&input.ids);

    match state.shopify().bulk_activate_code_discounts(ids).await {
        Ok(()) => {
            tracing::info!("Bulk activated discounts");
            (
                StatusCode::OK,
                [("HX-Trigger", "discounts-bulk-activated")],
                Html("Activated"),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to bulk activate discounts");
            (StatusCode::BAD_REQUEST, Html(format!("Error: {e}"))).into_response()
        }
    }
}

/// Bulk deactivate discounts handler.
#[instrument(skip(_admin, state))]
pub async fn bulk_deactivate(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<BulkActionInput>,
) -> impl IntoResponse {
    let ids = parse_ids(&input.ids);

    match state.shopify().bulk_deactivate_code_discounts(ids).await {
        Ok(()) => {
            tracing::info!("Bulk deactivated discounts");
            (
                StatusCode::OK,
                [("HX-Trigger", "discounts-bulk-deactivated")],
                Html("Deactivated"),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to bulk deactivate discounts");
            (StatusCode::BAD_REQUEST, Html(format!("Error: {e}"))).into_response()
        }
    }
}

/// Bulk delete discounts handler.
#[instrument(skip(_admin, state))]
pub async fn bulk_delete(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<BulkActionInput>,
) -> impl IntoResponse {
    let ids = parse_ids(&input.ids);

    match state.shopify().bulk_delete_code_discounts(ids).await {
        Ok(()) => {
            tracing::info!("Bulk deleted discounts");
            (
                StatusCode::OK,
                [
                    ("HX-Trigger", "discounts-bulk-deleted"),
                    ("HX-Redirect", "/discounts"),
                ],
                Html("Deleted"),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to bulk delete discounts");
            (StatusCode::BAD_REQUEST, Html(format!("Error: {e}"))).into_response()
        }
    }
}

// =============================================================================
// API Handlers (for HTMX pickers)
// =============================================================================

/// Search products API handler.
#[instrument(skip(_admin, _state))]
pub async fn api_search_products(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(_state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Json<Vec<serde_json::Value>> {
    // TODO: Implement product search
    // This requires adding a search_products method to the Shopify client
    tracing::debug!(query = ?query.q, "Product search");
    Json(vec![])
}

/// Search collections API handler.
#[instrument(skip(_admin, _state))]
pub async fn api_search_collections(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(_state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Json<Vec<serde_json::Value>> {
    // TODO: Implement collection search
    tracing::debug!(query = ?query.q, "Collection search");
    Json(vec![])
}

/// Search customers API handler.
#[instrument(skip(_admin, _state))]
pub async fn api_search_customers(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(_state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Json<Vec<serde_json::Value>> {
    // TODO: Implement customer search
    tracing::debug!(query = ?query.q, "Customer search");
    Json(vec![])
}

/// Get customer segments API handler.
#[instrument(skip(_admin, state))]
pub async fn api_customer_segments(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
) -> Json<Vec<CustomerSegmentView>> {
    match state.shopify().get_customer_segments(50).await {
        Ok(segments) => {
            let views: Vec<CustomerSegmentView> =
                segments.iter().map(CustomerSegmentView::from).collect();
            Json(views)
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to fetch customer segments");
            Json(vec![])
        }
    }
}

/// Add codes to a discount API handler.
#[instrument(skip(_admin, _state))]
pub async fn api_add_codes(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(_state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<AddCodesInput>,
) -> impl IntoResponse {
    // TODO: Implement add codes functionality
    // This requires calling the discountRedeemCodeBulkAdd mutation
    tracing::warn!(discount_id = %id, codes = %input.codes, "Add codes not yet implemented");
    (StatusCode::OK, Html("Codes added")).into_response()
}
