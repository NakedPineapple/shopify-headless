//! Customer management route handlers.

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
    middleware::auth::{RequireAdminAuth, RequireSuperAdmin},
    models::CurrentAdmin,
    shopify::types::{Address, Customer, CustomerOrder, CustomerState, Money},
    state::AppState,
};

use naked_pineapple_core::AdminRole;

use super::dashboard::AdminUserView;

// =============================================================================
// Query Parameters
// =============================================================================

/// Pagination and filter query parameters.
#[derive(Debug, Clone, Deserialize)]
pub struct CustomersQuery {
    /// Pagination cursor.
    pub cursor: Option<String>,
    /// Text search query (name/email).
    pub query: Option<String>,
    /// Filter by state (comma-separated: ENABLED,INVITED,DISABLED,DECLINED).
    pub state: Option<String>,
    /// Filter by tags (comma-separated).
    pub tags: Option<String>,
    /// Filter by country code (e.g., US, CA).
    pub country: Option<String>,
    /// Filter by email marketing subscription (true/false).
    pub accepts_marketing: Option<String>,
    /// Filter by has orders (true/false).
    pub has_orders: Option<String>,
    /// Filter by created date from (YYYY-MM-DD).
    pub created_from: Option<String>,
    /// Filter by created date to (YYYY-MM-DD).
    pub created_to: Option<String>,
    /// Sort column.
    pub sort: Option<String>,
    /// Sort direction (asc/desc).
    pub dir: Option<String>,
}

// =============================================================================
// Data Table Types
// =============================================================================

/// Column definition for data tables.
#[derive(Debug, Clone)]
pub struct DataTableColumn {
    /// Column identifier (used for data attributes and visibility tracking).
    pub key: &'static str,
    /// Display label for the column header.
    pub label: &'static str,
    /// Whether this column is sortable.
    pub sortable: bool,
    /// Sort key for URL (if different from key).
    pub sort_key: &'static str,
}

/// Filter option for select/multiselect filters.
#[derive(Debug, Clone)]
pub struct FilterOption {
    /// Option value (submitted in form).
    pub value: &'static str,
    /// Display label.
    pub label: &'static str,
}

/// Filter type for data table filters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterType {
    /// Text input filter.
    Text,
    /// Single-select dropdown filter.
    Select,
    /// Multi-select checkbox filter.
    MultiSelect,
    /// Date range filter (from/to).
    DateRange,
}

/// Filter definition for data tables.
#[derive(Debug, Clone)]
pub struct DataTableFilter {
    /// Filter parameter name (for URL).
    pub key: &'static str,
    /// Display label.
    pub label: &'static str,
    /// Filter type.
    pub filter_type: FilterType,
    /// Options for select/multiselect filters.
    pub options: Vec<FilterOption>,
}

/// Bulk action definition for data tables.
#[derive(Debug, Clone)]
pub struct BulkAction {
    /// Action identifier.
    pub key: &'static str,
    /// Display label.
    pub label: &'static str,
    /// Phosphor icon class (e.g., "ph-tag").
    pub icon: &'static str,
    /// Whether this is a destructive action.
    pub destructive: bool,
}

/// Get the default columns for the customers data table.
fn get_customer_columns() -> Vec<DataTableColumn> {
    vec![
        DataTableColumn {
            key: "customer",
            label: "Customer",
            sortable: true,
            sort_key: "name",
        },
        DataTableColumn {
            key: "phone",
            label: "Phone",
            sortable: false,
            sort_key: "",
        },
        DataTableColumn {
            key: "location",
            label: "Location",
            sortable: true,
            sort_key: "location",
        },
        DataTableColumn {
            key: "orders",
            label: "Orders",
            sortable: true,
            sort_key: "orders_count",
        },
        DataTableColumn {
            key: "spent",
            label: "Spent",
            sortable: true,
            sort_key: "amount_spent",
        },
        DataTableColumn {
            key: "state",
            label: "State",
            sortable: false,
            sort_key: "",
        },
        DataTableColumn {
            key: "tags",
            label: "Tags",
            sortable: false,
            sort_key: "",
        },
        DataTableColumn {
            key: "marketing",
            label: "Marketing",
            sortable: false,
            sort_key: "",
        },
        DataTableColumn {
            key: "created",
            label: "Created",
            sortable: true,
            sort_key: "created_at",
        },
        DataTableColumn {
            key: "updated",
            label: "Updated",
            sortable: true,
            sort_key: "updated_at",
        },
    ]
}

/// Get the default visible columns for the customers data table.
fn get_default_visible_columns() -> Vec<&'static str> {
    vec!["customer", "location", "orders", "spent", "state"]
}

/// Column visibility flags for templates.
///
/// Using explicit booleans avoids Askama template issues with `contains()` on string literals.
// Allow: This struct exists specifically to provide boolean visibility flags for each column,
// where each flag is independent and maps directly to a UI checkbox state.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct ColumnVisibility {
    pub customer: bool,
    pub phone: bool,
    pub location: bool,
    pub orders: bool,
    pub spent: bool,
    pub state: bool,
    pub tags: bool,
    pub marketing: bool,
    pub created: bool,
    pub updated: bool,
}

impl ColumnVisibility {
    /// Create visibility flags from a list of visible column keys.
    fn from_visible(visible: &[&str]) -> Self {
        Self {
            customer: visible.contains(&"customer"),
            phone: visible.contains(&"phone"),
            location: visible.contains(&"location"),
            orders: visible.contains(&"orders"),
            spent: visible.contains(&"spent"),
            state: visible.contains(&"state"),
            tags: visible.contains(&"tags"),
            marketing: visible.contains(&"marketing"),
            created: visible.contains(&"created"),
            updated: visible.contains(&"updated"),
        }
    }

    /// Check if a column is visible by key.
    #[must_use]
    pub fn is_visible(&self, key: &str) -> bool {
        match key {
            "customer" => self.customer,
            "phone" => self.phone,
            "location" => self.location,
            "orders" => self.orders,
            "spent" => self.spent,
            "state" => self.state,
            "tags" => self.tags,
            "marketing" => self.marketing,
            "created" => self.created,
            "updated" => self.updated,
            _ => false,
        }
    }
}

/// Get the filters for the customers data table.
fn get_customer_filters() -> Vec<DataTableFilter> {
    vec![
        DataTableFilter {
            key: "state",
            label: "State",
            filter_type: FilterType::MultiSelect,
            options: vec![
                FilterOption {
                    value: "ENABLED",
                    label: "Enabled",
                },
                FilterOption {
                    value: "INVITED",
                    label: "Invited",
                },
                FilterOption {
                    value: "DISABLED",
                    label: "Disabled",
                },
                FilterOption {
                    value: "DECLINED",
                    label: "Declined",
                },
            ],
        },
        DataTableFilter {
            key: "country",
            label: "Country",
            filter_type: FilterType::Select,
            options: vec![
                FilterOption {
                    value: "US",
                    label: "United States",
                },
                FilterOption {
                    value: "CA",
                    label: "Canada",
                },
                FilterOption {
                    value: "GB",
                    label: "United Kingdom",
                },
                FilterOption {
                    value: "AU",
                    label: "Australia",
                },
                FilterOption {
                    value: "DE",
                    label: "Germany",
                },
                FilterOption {
                    value: "FR",
                    label: "France",
                },
            ],
        },
        DataTableFilter {
            key: "accepts_marketing",
            label: "Marketing",
            filter_type: FilterType::Select,
            options: vec![
                FilterOption {
                    value: "true",
                    label: "Subscribed",
                },
                FilterOption {
                    value: "false",
                    label: "Not Subscribed",
                },
            ],
        },
        DataTableFilter {
            key: "has_orders",
            label: "Has Orders",
            filter_type: FilterType::Select,
            options: vec![
                FilterOption {
                    value: "true",
                    label: "Yes",
                },
                FilterOption {
                    value: "false",
                    label: "No",
                },
            ],
        },
    ]
}

/// Get the bulk actions for the customers data table.
fn get_customer_bulk_actions() -> Vec<BulkAction> {
    vec![
        BulkAction {
            key: "add_tags",
            label: "Add Tags",
            icon: "ph-tag",
            destructive: false,
        },
        BulkAction {
            key: "remove_tags",
            label: "Remove Tags",
            icon: "ph-tag-simple-slash",
            destructive: false,
        },
        BulkAction {
            key: "subscribe_marketing",
            label: "Subscribe to Marketing",
            icon: "ph-envelope",
            destructive: false,
        },
        BulkAction {
            key: "unsubscribe_marketing",
            label: "Unsubscribe from Marketing",
            icon: "ph-envelope-simple-open",
            destructive: false,
        },
    ]
}

// =============================================================================
// View Types
// =============================================================================

/// Customer view for list templates.
#[derive(Debug, Clone)]
pub struct CustomerView {
    pub id: String,
    pub short_id: String,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub orders_count: i64,
    pub total_spent: String,
    pub created_at: String,
    pub updated_at: String,
    pub location: Option<String>,
    pub state: String,
    pub state_class: String,
    pub tags: Vec<String>,
    pub accepts_marketing: bool,
}

/// Detailed customer view for show/edit pages.
// Allow: View model mirrors Shopify API which has independent boolean properties
// (tax_exempt, can_delete, is_mergeable, accepts_marketing) with no logical grouping.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct CustomerDetailView {
    pub id: String,
    pub short_id: String,
    pub display_name: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub locale: Option<String>,
    pub state: String,
    pub state_class: String,
    pub orders_count: i64,
    pub total_spent: String,
    pub lifetime_duration: Option<String>,
    pub tax_exempt: bool,
    pub tax_exemptions: Vec<String>,
    pub note: Option<String>,
    pub tags: Vec<String>,
    pub tags_string: String,
    pub can_delete: bool,
    pub is_mergeable: bool,
    pub accepts_marketing: bool,
    pub default_address: Option<AddressView>,
    pub addresses: Vec<AddressView>,
    pub recent_orders: Vec<OrderView>,
    pub created_at: String,
    pub updated_at: String,
}

/// Address view for templates.
#[derive(Debug, Clone)]
pub struct AddressView {
    pub id: Option<String>,
    pub formatted: String,
    pub address1: Option<String>,
    pub address2: Option<String>,
    pub city: Option<String>,
    pub province_code: Option<String>,
    pub country_code: Option<String>,
    pub zip: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub company: Option<String>,
    pub phone: Option<String>,
    pub is_default: bool,
}

/// Order view for customer detail.
#[derive(Debug, Clone)]
pub struct OrderView {
    pub id: String,
    pub short_id: String,
    pub name: String,
    pub created_at: String,
    pub financial_status: Option<String>,
    pub financial_status_class: String,
    pub fulfillment_status: Option<String>,
    pub fulfillment_status_class: String,
    pub total: String,
}

// =============================================================================
// Type Conversions
// =============================================================================

/// Format a Shopify Money type as a price string.
fn format_price(money: &Money) -> String {
    money.amount.parse::<f64>().map_or_else(
        |_| format!("${}", money.amount),
        |amount| format!("${amount:.2}"),
    )
}

/// Extract short ID from GID (e.g., `gid://shopify/Customer/123` -> `123`).
fn extract_short_id(gid: &str) -> String {
    gid.rsplit('/').next().unwrap_or(gid).to_string()
}

/// Get status class for customer state.
const fn state_class(state: CustomerState) -> &'static str {
    match state {
        CustomerState::Enabled => {
            "bg-green-500/10 text-green-600 dark:text-green-400 ring-1 ring-inset ring-green-500/20"
        }
        CustomerState::Invited => {
            "bg-amber-500/10 text-amber-600 dark:text-amber-400 ring-1 ring-inset ring-amber-500/20"
        }
        CustomerState::Declined => {
            "bg-red-500/10 text-red-600 dark:text-red-400 ring-1 ring-inset ring-red-500/20"
        }
        CustomerState::Disabled => {
            "bg-zinc-500/10 text-zinc-600 dark:text-zinc-400 ring-1 ring-inset ring-zinc-500/20"
        }
    }
}

/// Get financial status class.
fn financial_status_class(status: Option<&str>) -> &'static str {
    match status {
        Some("PAID") => "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400",
        Some("PARTIALLY_PAID") => {
            "bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-400"
        }
        Some("PENDING") => {
            "bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400"
        }
        Some("REFUNDED" | "PARTIALLY_REFUNDED") => {
            "bg-blue-100 text-blue-800 dark:bg-blue-900/30 dark:text-blue-400"
        }
        _ => "bg-zinc-100 text-zinc-800 dark:bg-zinc-900/30 dark:text-zinc-400",
    }
}

/// Get fulfillment status class.
fn fulfillment_status_class(status: Option<&str>) -> &'static str {
    match status {
        Some("FULFILLED") => "bg-green-100 text-green-800 dark:bg-green-900/30 dark:text-green-400",
        Some("PARTIALLY_FULFILLED" | "IN_PROGRESS") => {
            "bg-amber-100 text-amber-800 dark:bg-amber-900/30 dark:text-amber-400"
        }
        Some("UNFULFILLED") => {
            "bg-yellow-100 text-yellow-800 dark:bg-yellow-900/30 dark:text-yellow-400"
        }
        _ => "bg-zinc-100 text-zinc-800 dark:bg-zinc-900/30 dark:text-zinc-400",
    }
}

/// Format an address as a single line.
fn format_address(addr: &Address) -> String {
    let parts: Vec<&str> = [
        addr.address1.as_deref(),
        addr.city.as_deref(),
        addr.province_code.as_deref(),
        addr.zip.as_deref(),
        addr.country_code.as_deref(),
    ]
    .into_iter()
    .flatten()
    .filter(|s| !s.is_empty())
    .collect();

    parts.join(", ")
}

/// Get location string from address.
fn get_location(addr: &Address) -> Option<String> {
    let city = addr.city.as_deref().unwrap_or("");
    let province = addr.province_code.as_deref().unwrap_or("");
    let country = addr.country_code.as_deref().unwrap_or("");

    if !city.is_empty() && !province.is_empty() {
        Some(format!("{city}, {province}"))
    } else if !city.is_empty() && !country.is_empty() {
        Some(format!("{city}, {country}"))
    } else if !province.is_empty() && !country.is_empty() {
        Some(format!("{province}, {country}"))
    } else if !country.is_empty() {
        Some(country.to_string())
    } else {
        None
    }
}

impl From<&Customer> for CustomerView {
    fn from(customer: &Customer) -> Self {
        let location = customer.default_address.as_ref().and_then(get_location);

        Self {
            id: customer.id.clone(),
            short_id: extract_short_id(&customer.id),
            name: customer.display_name.clone(),
            email: customer.email.clone(),
            phone: customer.phone.clone(),
            orders_count: customer.orders_count,
            total_spent: format_price(&customer.total_spent),
            created_at: customer.created_at.clone(),
            updated_at: customer.updated_at.clone(),
            location,
            state: format!("{:?}", customer.state),
            state_class: state_class(customer.state).to_string(),
            tags: customer.tags.clone(),
            accepts_marketing: customer.accepts_marketing,
        }
    }
}

impl From<&Customer> for CustomerDetailView {
    fn from(customer: &Customer) -> Self {
        let default_address = customer.default_address.as_ref().map(|a| AddressView {
            id: a.id.clone(),
            formatted: format_address(a),
            address1: a.address1.clone(),
            address2: a.address2.clone(),
            city: a.city.clone(),
            province_code: a.province_code.clone(),
            country_code: a.country_code.clone(),
            zip: a.zip.clone(),
            first_name: a.first_name.clone(),
            last_name: a.last_name.clone(),
            company: a.company.clone(),
            phone: a.phone.clone(),
            is_default: true,
        });

        let default_id = customer.default_address.as_ref().and_then(|a| a.id.clone());

        let addresses: Vec<AddressView> = customer
            .addresses
            .iter()
            .map(|a| AddressView {
                id: a.id.clone(),
                formatted: format_address(a),
                address1: a.address1.clone(),
                address2: a.address2.clone(),
                city: a.city.clone(),
                province_code: a.province_code.clone(),
                country_code: a.country_code.clone(),
                zip: a.zip.clone(),
                first_name: a.first_name.clone(),
                last_name: a.last_name.clone(),
                company: a.company.clone(),
                phone: a.phone.clone(),
                is_default: a.id == default_id,
            })
            .collect();

        let recent_orders: Vec<OrderView> = customer
            .recent_orders
            .iter()
            .map(|o| OrderView {
                id: o.id.clone(),
                short_id: extract_short_id(&o.id),
                name: o.name.clone(),
                created_at: o.created_at.clone(),
                financial_status: o.financial_status.clone(),
                financial_status_class: financial_status_class(o.financial_status.as_deref())
                    .to_string(),
                fulfillment_status: o.fulfillment_status.clone(),
                fulfillment_status_class: fulfillment_status_class(o.fulfillment_status.as_deref())
                    .to_string(),
                total: format_price(&o.total_price),
            })
            .collect();

        Self {
            id: customer.id.clone(),
            short_id: extract_short_id(&customer.id),
            display_name: customer.display_name.clone(),
            first_name: customer.first_name.clone(),
            last_name: customer.last_name.clone(),
            email: customer.email.clone(),
            phone: customer.phone.clone(),
            locale: customer.locale.clone(),
            state: format!("{:?}", customer.state),
            state_class: state_class(customer.state).to_string(),
            orders_count: customer.orders_count,
            total_spent: format_price(&customer.total_spent),
            lifetime_duration: customer.lifetime_duration.clone(),
            tax_exempt: customer.tax_exempt,
            tax_exemptions: customer.tax_exemptions.clone(),
            note: customer.note.clone(),
            tags: customer.tags.clone(),
            tags_string: customer.tags.join(", "),
            can_delete: customer.can_delete,
            is_mergeable: customer.is_mergeable,
            accepts_marketing: customer.accepts_marketing,
            default_address,
            addresses,
            recent_orders,
            created_at: customer.created_at.clone(),
            updated_at: customer.updated_at.clone(),
        }
    }
}

// =============================================================================
// Templates
// =============================================================================

/// Customers list page template.
#[derive(Template)]
#[template(path = "customers/index.html")]
pub struct CustomersIndexTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    // Data table data
    pub customers: Vec<CustomerView>,
    pub has_next_page: bool,
    pub next_cursor: Option<String>,
    // Data table configuration
    pub table_id: &'static str,
    pub columns: Vec<DataTableColumn>,
    pub default_columns: Vec<&'static str>,
    pub visible_columns: Vec<&'static str>,
    pub col_visible: ColumnVisibility,
    pub filters: Vec<DataTableFilter>,
    pub bulk_actions: Vec<BulkAction>,
    // Current filter/sort state
    pub search_value: Option<String>,
    pub filter_values: std::collections::HashMap<String, String>,
    pub sort_column: Option<String>,
    pub sort_direction: String,
    // Preserve URL params for links
    pub preserve_params: String,
}

/// Customer detail page template.
#[derive(Template)]
#[template(path = "customers/show.html")]
pub struct CustomerShowTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub customer: CustomerDetailView,
}

/// Customer create form template.
#[derive(Template)]
#[template(path = "customers/new.html")]
pub struct CustomerNewTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub error: Option<String>,
}

/// Customer edit form template.
#[derive(Template)]
#[template(path = "customers/edit.html")]
pub struct CustomerEditTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub customer: CustomerDetailView,
    pub error: Option<String>,
}

// =============================================================================
// Form Inputs
// =============================================================================

/// Form input for creating/updating customers.
#[derive(Debug, Deserialize)]
pub struct CustomerFormInput {
    pub email: String,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub note: Option<String>,
    pub tags: Option<String>,
}

/// Form input for updating tags.
#[derive(Debug, Deserialize)]
pub struct TagsFormInput {
    pub tags: String,
    pub action: String, // "add" or "remove"
}

/// Form input for updating note.
#[derive(Debug, Deserialize)]
pub struct NoteFormInput {
    pub note: String,
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /customers - List customers with search and pagination.
#[instrument(skip(admin, state))]
pub async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<CustomersQuery>,
) -> Html<String> {
    // Build Shopify query string from filters
    let shopify_query = build_shopify_query(&query);

    // Parse sort parameters
    let sort_key = query
        .sort
        .as_deref()
        .and_then(crate::shopify::types::CustomerSortKey::from_str_param);
    let reverse = query.dir.as_deref() == Some("desc");

    let params = crate::shopify::types::CustomerListParams {
        first: Some(25),
        after: query.cursor.clone(),
        query: shopify_query,
        sort_key,
        reverse,
    };

    let result = state.shopify().get_customers(params).await;

    let (customers, has_next_page, next_cursor) = match result {
        Ok(conn) => {
            let customers: Vec<CustomerView> =
                conn.customers.iter().map(CustomerView::from).collect();
            (
                customers,
                conn.page_info.has_next_page,
                conn.page_info.end_cursor,
            )
        }
        Err(e) => {
            tracing::error!("Failed to fetch customers: {e}");
            (vec![], false, None)
        }
    };

    // Build filter values map from query params
    let mut filter_values = std::collections::HashMap::new();
    if let Some(ref v) = query.state {
        filter_values.insert("state".to_string(), v.clone());
    }
    if let Some(ref v) = query.tags {
        filter_values.insert("tags".to_string(), v.clone());
    }
    if let Some(ref v) = query.country {
        filter_values.insert("country".to_string(), v.clone());
    }
    if let Some(ref v) = query.accepts_marketing {
        filter_values.insert("accepts_marketing".to_string(), v.clone());
    }
    if let Some(ref v) = query.has_orders {
        filter_values.insert("has_orders".to_string(), v.clone());
    }
    if let Some(ref v) = query.created_from {
        filter_values.insert("created_from".to_string(), v.clone());
    }
    if let Some(ref v) = query.created_to {
        filter_values.insert("created_to".to_string(), v.clone());
    }

    // Build preserve_params for pagination/sort links
    let preserve_params = build_preserve_params(&query);

    let template = CustomersIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/customers".to_string(),
        // Data
        customers,
        has_next_page,
        next_cursor,
        // Configuration
        table_id: "customers",
        columns: get_customer_columns(),
        default_columns: get_default_visible_columns(),
        visible_columns: get_default_visible_columns(), // TODO: Load from user prefs
        col_visible: ColumnVisibility::from_visible(&get_default_visible_columns()),
        filters: get_customer_filters(),
        bulk_actions: get_customer_bulk_actions(),
        // Current state
        search_value: query.query,
        filter_values,
        sort_column: query.sort,
        sort_direction: query.dir.unwrap_or_else(|| "asc".to_string()),
        preserve_params,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// GET /customers/:id - Show customer detail.
#[instrument(skip(admin, state))]
pub async fn show(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let gid = format!("gid://shopify/Customer/{id}");

    match state.shopify().get_customer(&gid).await {
        Ok(Some(customer)) => {
            let template = CustomerShowTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: format!("/customers/{id}"),
                customer: CustomerDetailView::from(&customer),
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Customer not found").into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch customer: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load customer").into_response()
        }
    }
}

/// GET /customers/new - Show create customer form.
#[instrument(skip(admin))]
pub async fn new(RequireAdminAuth(admin): RequireAdminAuth) -> Html<String> {
    let template = CustomerNewTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/customers/new".to_string(),
        error: None,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// POST /customers - Create new customer.
#[instrument(skip(admin, state, input))]
pub async fn create(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<CustomerFormInput>,
) -> impl IntoResponse {
    let tags: Vec<String> = input
        .tags
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    match state
        .shopify()
        .create_customer(
            &input.email,
            input.first_name.as_deref(),
            input.last_name.as_deref(),
            input.phone.as_deref(),
            input.note.as_deref(),
            tags,
        )
        .await
    {
        Ok(id) => {
            let short_id = extract_short_id(&id);
            Redirect::to(&format!("/customers/{short_id}")).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to create customer: {e}");
            let template = CustomerNewTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/customers/new".to_string(),
                error: Some(format!("Failed to create customer: {e}")),
            };
            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
    }
}

/// GET /customers/:id/edit - Show edit customer form.
#[instrument(skip(admin, state))]
pub async fn edit(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let gid = format!("gid://shopify/Customer/{id}");

    match state.shopify().get_customer(&gid).await {
        Ok(Some(customer)) => {
            let template = CustomerEditTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: format!("/customers/{id}/edit"),
                customer: CustomerDetailView::from(&customer),
                error: None,
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Customer not found").into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch customer: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load customer").into_response()
        }
    }
}

/// POST /customers/:id - Update customer.
#[instrument(skip(admin, state, input))]
pub async fn update(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<CustomerFormInput>,
) -> impl IntoResponse {
    let gid = format!("gid://shopify/Customer/{id}");

    let tags: Option<Vec<String>> = input.tags.as_ref().map(|t| {
        t.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    });

    let params = crate::shopify::types::CustomerUpdateParams {
        email: Some(input.email.clone()),
        first_name: input.first_name.clone(),
        last_name: input.last_name.clone(),
        phone: input.phone.clone(),
        note: input.note.clone(),
        tags,
    };

    match state.shopify().update_customer(&gid, params).await {
        Ok(_) => Redirect::to(&format!("/customers/{id}")).into_response(),
        Err(e) => {
            tracing::error!("Failed to update customer: {e}");

            // Re-fetch customer to show form with error
            if let Ok(Some(customer)) = state.shopify().get_customer(&gid).await {
                let template = CustomerEditTemplate {
                    admin_user: AdminUserView::from(&admin),
                    current_path: format!("/customers/{id}/edit"),
                    customer: CustomerDetailView::from(&customer),
                    error: Some(format!("Failed to update customer: {e}")),
                };
                return Html(template.render().unwrap_or_else(|e| {
                    tracing::error!("Template render error: {}", e);
                    "Internal Server Error".to_string()
                }))
                .into_response();
            }

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to update customer",
            )
                .into_response()
        }
    }
}

/// POST /customers/:id/delete - Delete customer (super admin only).
#[instrument(skip(state))]
pub async fn delete(
    RequireSuperAdmin(_): RequireSuperAdmin,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let gid = format!("gid://shopify/Customer/{id}");

    match state.shopify().delete_customer(&gid).await {
        Ok(_) => Redirect::to("/customers").into_response(),
        Err(e) => {
            tracing::error!("Failed to delete customer: {e}");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to delete customer: {e}"),
            )
                .into_response()
        }
    }
}

/// POST /customers/:id/tags - Add or remove tags (HTMX partial).
#[instrument(skip(state, input))]
pub async fn update_tags(
    RequireAdminAuth(_): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<TagsFormInput>,
) -> impl IntoResponse {
    let gid = format!("gid://shopify/Customer/{id}");

    let tags: Vec<String> = input
        .tags
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let result = if input.action == "add" {
        state.shopify().add_customer_tags(&gid, tags).await
    } else {
        state.shopify().remove_customer_tags(&gid, tags).await
    };

    match result {
        Ok(_) => {
            // Return updated tags partial
            if let Ok(Some(customer)) = state.shopify().get_customer(&gid).await {
                let tags_html = customer
                    .tags
                    .iter()
                    .map(|tag| {
                        format!(
                            r##"<span class="inline-flex items-center gap-1 px-2 py-1 bg-muted rounded text-sm">
                                {tag}
                                <button type="button" class="text-muted-foreground hover:text-foreground"
                                        hx-post="/customers/{id}/tags"
                                        hx-vals='{{"tags": "{tag}", "action": "remove"}}'
                                        hx-target="#tags-container"
                                        hx-swap="innerHTML">
                                    <i class="ph ph-x text-xs"></i>
                                </button>
                            </span>"##
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");

                return Html(tags_html).into_response();
            }
            Html("Tags updated").into_response()
        }
        Err(e) => {
            tracing::error!("Failed to update tags: {e}");
            (StatusCode::BAD_REQUEST, format!("Failed: {e}")).into_response()
        }
    }
}

/// POST /customers/:id/note - Update customer note (HTMX partial).
#[instrument(skip(state, input))]
pub async fn update_note(
    RequireAdminAuth(_): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<NoteFormInput>,
) -> impl IntoResponse {
    let gid = format!("gid://shopify/Customer/{id}");

    let params = crate::shopify::types::CustomerUpdateParams {
        note: Some(input.note.clone()),
        ..Default::default()
    };

    match state.shopify().update_customer(&gid, params).await {
        Ok(_) => Html("Note saved").into_response(),
        Err(e) => {
            tracing::error!("Failed to update note: {e}");
            (StatusCode::BAD_REQUEST, format!("Failed: {e}")).into_response()
        }
    }
}

/// POST /customers/:id/send-invite - Send account invitation email.
#[instrument(skip(state))]
pub async fn send_invite(
    RequireAdminAuth(_): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let gid = format!("gid://shopify/Customer/{id}");

    match state.shopify().send_customer_invite(&gid).await {
        Ok(()) => Html(
            r#"<span class="text-green-600 dark:text-green-400">
                <i class="ph ph-check mr-1"></i>Invitation sent
            </span>"#,
        )
        .into_response(),
        Err(e) => {
            tracing::error!("Failed to send invite: {e}");
            (StatusCode::BAD_REQUEST, format!("Failed: {e}")).into_response()
        }
    }
}

/// POST /customers/:id/activation-url - Generate activation URL.
#[instrument(skip(state))]
pub async fn activation_url(
    RequireAdminAuth(_): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let gid = format!("gid://shopify/Customer/{id}");

    match state.shopify().generate_customer_activation_url(&gid).await {
        Ok(url) => Html(format!(
            r#"<div class="p-3 bg-muted rounded-lg">
                <p class="text-xs text-muted-foreground mb-1">Activation URL (expires in 30 days)</p>
                <input type="text" value="{url}" readonly
                       class="w-full p-2 bg-input rounded text-sm font-mono"
                       onclick="this.select(); navigator.clipboard.writeText(this.value);">
                <p class="text-xs text-muted-foreground mt-1">Click to copy</p>
            </div>"#
        ))
        .into_response(),
        Err(e) => {
            tracing::error!("Failed to generate activation URL: {e}");
            (StatusCode::BAD_REQUEST, format!("Failed: {e}")).into_response()
        }
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Build Shopify query string from filter parameters.
///
/// Shopify Admin API query syntax:
/// - Text search: `{text}` (searches name/email)
/// - State filter: `state:ENABLED` or `(state:ENABLED OR state:INVITED)`
/// - Tag filter: `tag:vip` or `(tag:vip AND tag:wholesale)`
/// - Country filter: `country:US`
/// - Marketing filter: `accepts_marketing:true`
/// - Orders filter: `orders_count:>0`
/// - Date filter: `created_at:>=2024-01-01`
fn build_shopify_query(query: &CustomersQuery) -> Option<String> {
    let mut parts = Vec::new();

    // Text search (name/email)
    if let Some(ref q) = query.query {
        let trimmed = q.trim();
        if !trimmed.is_empty() {
            parts.push(trimmed.to_string());
        }
    }

    // State filter (multi-select with OR)
    if let Some(ref states) = query.state {
        let state_values: Vec<&str> = states
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        match state_values.as_slice() {
            [] => {}
            [single] => parts.push(format!("state:{single}")),
            multiple => {
                let state_parts: Vec<String> =
                    multiple.iter().map(|s| format!("state:{s}")).collect();
                parts.push(format!("({})", state_parts.join(" OR ")));
            }
        }
    }

    // Tags filter (multi-select with AND)
    if let Some(ref tags) = query.tags {
        let tag_values: Vec<&str> = tags
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        match tag_values.as_slice() {
            [] => {}
            [single] => parts.push(format!("tag:{single}")),
            multiple => {
                let tag_parts: Vec<String> = multiple.iter().map(|t| format!("tag:{t}")).collect();
                parts.push(format!("({})", tag_parts.join(" AND ")));
            }
        }
    }

    // Country filter
    if let Some(ref country) = query.country {
        let trimmed = country.trim();
        if !trimmed.is_empty() {
            parts.push(format!("country:{trimmed}"));
        }
    }

    // Email marketing subscription filter
    if let Some(ref accepts) = query.accepts_marketing {
        let trimmed = accepts.trim().to_lowercase();
        if trimmed == "true" || trimmed == "false" {
            parts.push(format!("accepts_marketing:{trimmed}"));
        }
    }

    // Has orders filter
    if let Some(ref has_orders) = query.has_orders {
        let trimmed = has_orders.trim().to_lowercase();
        if trimmed == "true" {
            parts.push("orders_count:>0".to_string());
        } else if trimmed == "false" {
            parts.push("orders_count:0".to_string());
        }
    }

    // Created date range filters
    if let Some(ref from) = query.created_from {
        let trimmed = from.trim();
        if !trimmed.is_empty() {
            parts.push(format!("created_at:>={trimmed}"));
        }
    }
    if let Some(ref to) = query.created_to {
        let trimmed = to.trim();
        if !trimmed.is_empty() {
            parts.push(format!("created_at:<={trimmed}"));
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" AND "))
    }
}

/// Build URL query string to preserve current filter/search state for links.
///
/// This is used in pagination and sort links to maintain the current filters.
fn build_preserve_params(query: &CustomersQuery) -> String {
    let mut params = Vec::new();

    if let Some(ref q) = query.query
        && !q.is_empty()
    {
        params.push(format!("query={}", urlencoding::encode(q)));
    }
    if let Some(ref v) = query.state
        && !v.is_empty()
    {
        params.push(format!("state={}", urlencoding::encode(v)));
    }
    if let Some(ref v) = query.tags
        && !v.is_empty()
    {
        params.push(format!("tags={}", urlencoding::encode(v)));
    }
    if let Some(ref v) = query.country
        && !v.is_empty()
    {
        params.push(format!("country={}", urlencoding::encode(v)));
    }
    if let Some(ref v) = query.accepts_marketing
        && !v.is_empty()
    {
        params.push(format!("accepts_marketing={v}"));
    }
    if let Some(ref v) = query.has_orders
        && !v.is_empty()
    {
        params.push(format!("has_orders={v}"));
    }
    if let Some(ref v) = query.created_from
        && !v.is_empty()
    {
        params.push(format!("created_from={}", urlencoding::encode(v)));
    }
    if let Some(ref v) = query.created_to
        && !v.is_empty()
    {
        params.push(format!("created_to={}", urlencoding::encode(v)));
    }

    if params.is_empty() {
        String::new()
    } else {
        format!("&{}", params.join("&"))
    }
}

// =============================================================================
// Marketing Consent Handlers
// =============================================================================

/// Form for updating marketing consent.
#[derive(Debug, Deserialize)]
pub struct UpdateMarketingForm {
    #[serde(rename = "type")]
    pub marketing_type: String, // "email" or "sms"
    pub subscribed: String, // "true" or "false"
}

/// Update customer marketing consent.
#[instrument(skip(state))]
pub async fn update_marketing(
    RequireAdminAuth(_): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<UpdateMarketingForm>,
) -> impl IntoResponse {
    let gid = format!("gid://shopify/Customer/{id}");
    let marketing_state = if form.subscribed == "true" {
        "SUBSCRIBED"
    } else {
        "UNSUBSCRIBED"
    };

    let result = match form.marketing_type.as_str() {
        "email" => {
            state
                .shopify()
                .update_customer_email_marketing(&gid, marketing_state)
                .await
        }
        "sms" => {
            state
                .shopify()
                .update_customer_sms_marketing(&gid, marketing_state)
                .await
        }
        _ => {
            return (StatusCode::BAD_REQUEST, "Invalid marketing type").into_response();
        }
    };

    match result {
        Ok(()) => Html(format!(
            r##"<div class="mb-3 p-2 bg-green-100 dark:bg-green-900/30 text-green-800 dark:text-green-400 text-sm rounded">
                Marketing preferences updated
            </div>
            <div class="space-y-4">
                <div class="flex items-center justify-between">
                    <div>
                        <p class="text-sm font-medium text-foreground">Email Marketing</p>
                        <p class="text-xs text-muted-foreground">Receive promotional emails</p>
                    </div>
                    <button type="button"
                            hx-post="/customers/{id}/marketing"
                            hx-vals='{{"type": "email", "subscribed": "{subscribed}"}}'
                            hx-target="#marketing-container"
                            hx-swap="innerHTML"
                            class="relative inline-flex h-6 w-11 items-center rounded-full transition-colors bg-primary">
                        <span class="inline-block h-4 w-4 transform rounded-full bg-white transition-transform translate-x-6"></span>
                    </button>
                </div>
            </div>"##,
            id = id,
            subscribed = if marketing_state == "SUBSCRIBED" { "false" } else { "true" }
        ))
        .into_response(),
        Err(e) => {
            tracing::error!("Failed to update marketing consent: {e}");
            (StatusCode::BAD_REQUEST, format!("Failed: {e}")).into_response()
        }
    }
}

// =============================================================================
// Address Handlers
// =============================================================================

/// Form for creating/updating addresses.
#[derive(Debug, Deserialize)]
pub struct AddressForm {
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub company: Option<String>,
    pub address1: Option<String>,
    pub address2: Option<String>,
    pub city: Option<String>,
    pub province: Option<String>,
    pub zip: Option<String>,
    pub country: Option<String>,
    pub phone: Option<String>,
    pub is_default: Option<String>,
}

/// Create a new address for a customer.
#[instrument(skip(state, form))]
pub async fn address_create(
    RequireAdminAuth(_): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<AddressForm>,
) -> impl IntoResponse {
    let gid = format!("gid://shopify/Customer/{id}");

    let address_input = crate::shopify::types::AddressInput {
        first_name: form.first_name,
        last_name: form.last_name,
        company: form.company,
        address1: form.address1,
        address2: form.address2,
        city: form.city,
        province_code: form.province,
        zip: form.zip,
        country_code: form.country,
        phone: form.phone,
    };

    match state
        .shopify()
        .create_customer_address(&gid, address_input)
        .await
    {
        Ok(_address) => {
            // Redirect back to customer page to show updated addresses
            Redirect::to(&format!("/customers/{id}")).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to create address: {e}");
            (StatusCode::BAD_REQUEST, format!("Failed: {e}")).into_response()
        }
    }
}

/// Update an existing customer address.
#[instrument(skip(state, form))]
pub async fn address_update(
    RequireAdminAuth(_): RequireAdminAuth,
    State(state): State<AppState>,
    Path((customer_id, address_id)): Path<(String, String)>,
    Form(form): Form<AddressForm>,
) -> impl IntoResponse {
    let shopify_customer_gid = format!("gid://shopify/Customer/{customer_id}");
    let mailing_address_gid = format!("gid://shopify/MailingAddress/{address_id}");

    let address_input = crate::shopify::types::AddressInput {
        first_name: form.first_name,
        last_name: form.last_name,
        company: form.company,
        address1: form.address1,
        address2: form.address2,
        city: form.city,
        province_code: form.province,
        zip: form.zip,
        country_code: form.country,
        phone: form.phone,
    };

    match state
        .shopify()
        .update_customer_address(&shopify_customer_gid, &mailing_address_gid, address_input)
        .await
    {
        Ok(_address) => Redirect::to(&format!("/customers/{customer_id}")).into_response(),
        Err(e) => {
            tracing::error!("Failed to update address: {e}");
            (StatusCode::BAD_REQUEST, format!("Failed: {e}")).into_response()
        }
    }
}

/// Delete a customer address.
#[instrument(skip(state))]
pub async fn address_delete(
    RequireAdminAuth(_): RequireAdminAuth,
    State(state): State<AppState>,
    Path((customer_id, address_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let shopify_customer_gid = format!("gid://shopify/Customer/{customer_id}");
    let mailing_address_gid = format!("gid://shopify/MailingAddress/{address_id}");

    match state
        .shopify()
        .delete_customer_address(&shopify_customer_gid, &mailing_address_gid)
        .await
    {
        Ok(_) => Redirect::to(&format!("/customers/{customer_id}")).into_response(),
        Err(e) => {
            tracing::error!("Failed to delete address: {e}");
            (StatusCode::BAD_REQUEST, format!("Failed: {e}")).into_response()
        }
    }
}

/// Set a customer's default address.
#[instrument(skip(state))]
pub async fn set_default_address(
    RequireAdminAuth(_): RequireAdminAuth,
    State(state): State<AppState>,
    Path((customer_id, address_id)): Path<(String, String)>,
) -> impl IntoResponse {
    let shopify_customer_gid = format!("gid://shopify/Customer/{customer_id}");
    let mailing_address_gid = format!("gid://shopify/MailingAddress/{address_id}");

    match state
        .shopify()
        .set_customer_default_address(&shopify_customer_gid, &mailing_address_gid)
        .await
    {
        Ok(()) => Redirect::to(&format!("/customers/{customer_id}")).into_response(),
        Err(e) => {
            tracing::error!("Failed to set default address: {e}");
            (StatusCode::BAD_REQUEST, format!("Failed: {e}")).into_response()
        }
    }
}

// =============================================================================
// Customer Merge Handler
// =============================================================================

/// Form for merging customers.
#[derive(Debug, Deserialize)]
pub struct MergeForm {
    pub merge_customer_id: String,
    pub override_first_name: Option<String>,
    pub override_last_name: Option<String>,
    pub override_email: Option<String>,
    pub override_phone: Option<String>,
    pub override_default_address: Option<String>,
}

/// Merge two customers (`super_admin` only).
#[instrument(skip(state, form))]
pub async fn merge(
    RequireSuperAdmin(_): RequireSuperAdmin,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<MergeForm>,
) -> impl IntoResponse {
    let customer_one_gid = format!("gid://shopify/Customer/{id}");

    // TODO: If merge_customer_id contains '@', look up customer by email first
    let customer_two_gid = format!("gid://shopify/Customer/{}", form.merge_customer_id);

    let overrides = crate::shopify::types::CustomerMergeOverrides {
        first_name: form.override_first_name.as_deref() == Some("true"),
        last_name: form.override_last_name.as_deref() == Some("true"),
        email: form.override_email.as_deref() == Some("true"),
        phone: form.override_phone.as_deref() == Some("true"),
        default_address: form.override_default_address.as_deref() == Some("true"),
    };

    match state
        .shopify()
        .merge_customers(&customer_one_gid, &customer_two_gid, overrides)
        .await
    {
        Ok(resulting_id) => {
            // Extract short ID from resulting GID
            let short_id = resulting_id.rsplit('/').next().unwrap_or(&id);
            Redirect::to(&format!("/customers/{short_id}")).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to merge customers: {e}");
            (StatusCode::BAD_REQUEST, format!("Failed: {e}")).into_response()
        }
    }
}

// =============================================================================
// Bulk Action Handlers
// =============================================================================

/// Form for bulk tag operations.
#[derive(Debug, Deserialize)]
pub struct BulkTagsForm {
    pub ids: String,    // Comma-separated customer IDs
    pub tags: String,   // Comma-separated tags
    pub action: String, // "add" or "remove"
}

/// Bulk add/remove tags from customers.
#[instrument(skip(state, form))]
pub async fn bulk_tags(
    RequireAdminAuth(_): RequireAdminAuth,
    State(state): State<AppState>,
    Form(form): Form<BulkTagsForm>,
) -> impl IntoResponse {
    let ids: Vec<&str> = form.ids.split(',').map(str::trim).collect();
    let tags: Vec<String> = form
        .tags
        .split(',')
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();

    if ids.is_empty() || tags.is_empty() {
        return (StatusCode::BAD_REQUEST, "No customers or tags specified").into_response();
    }

    let mut errors = Vec::new();

    for id in ids {
        let gid = format!("gid://shopify/Customer/{id}");
        let result = match form.action.as_str() {
            "add" => state.shopify().add_customer_tags(&gid, tags.clone()).await,
            "remove" => {
                state
                    .shopify()
                    .remove_customer_tags(&gid, tags.clone())
                    .await
            }
            _ => {
                return (StatusCode::BAD_REQUEST, "Invalid action").into_response();
            }
        };

        if let Err(e) = result {
            errors.push(format!("{id}: {e}"));
        }
    }

    if errors.is_empty() {
        Redirect::to("/customers").into_response()
    } else {
        (
            StatusCode::PARTIAL_CONTENT,
            format!("Some operations failed: {}", errors.join("; ")),
        )
            .into_response()
    }
}

/// Form for bulk marketing operations.
#[derive(Debug, Deserialize)]
pub struct BulkMarketingForm {
    pub ids: String,        // Comma-separated customer IDs
    pub subscribed: String, // "true" or "false"
}

/// Bulk update marketing consent.
#[instrument(skip(state, form))]
pub async fn bulk_marketing(
    RequireAdminAuth(_): RequireAdminAuth,
    State(state): State<AppState>,
    Form(form): Form<BulkMarketingForm>,
) -> impl IntoResponse {
    let ids: Vec<&str> = form.ids.split(',').map(str::trim).collect();
    let marketing_state = if form.subscribed == "true" {
        "SUBSCRIBED"
    } else {
        "UNSUBSCRIBED"
    };

    if ids.is_empty() {
        return (StatusCode::BAD_REQUEST, "No customers specified").into_response();
    }

    let mut errors = Vec::new();

    for id in ids {
        let gid = format!("gid://shopify/Customer/{id}");
        if let Err(e) = state
            .shopify()
            .update_customer_email_marketing(&gid, marketing_state)
            .await
        {
            errors.push(format!("{id}: {e}"));
        }
    }

    if errors.is_empty() {
        Redirect::to("/customers").into_response()
    } else {
        (
            StatusCode::PARTIAL_CONTENT,
            format!("Some operations failed: {}", errors.join("; ")),
        )
            .into_response()
    }
}
