//! Customers list route handler.

use askama::Template;
use axum::{
    extract::{Query, State},
    response::Html,
};
use serde::Deserialize;
use tracing::instrument;

use crate::{
    filters,
    middleware::auth::RequireAdminAuth,
    models::CurrentAdmin,
    shopify::types::{Customer, Money},
    state::AppState,
};

use naked_pineapple_core::AdminRole;

use super::dashboard::AdminUserView;

/// Pagination query parameters.
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub cursor: Option<String>,
    pub query: Option<String>,
}

/// Customer view for templates.
#[derive(Debug, Clone)]
pub struct CustomerView {
    pub id: String,
    pub name: String,
    pub email: Option<String>,
    pub orders_count: i64,
    pub total_spent: String,
    pub created_at: String,
    pub location: Option<String>,
}

// =============================================================================
// Type Conversions
// =============================================================================

/// Format a Shopify Money type as a price string.
fn format_price(money: &Money) -> String {
    if let Ok(amount) = money.amount.parse::<f64>() {
        format!("${amount:.2}")
    } else {
        format!("${}", money.amount)
    }
}

impl From<&Customer> for CustomerView {
    fn from(customer: &Customer) -> Self {
        // Get location from default address
        let location = customer.default_address.as_ref().and_then(|addr| {
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
        });

        Self {
            id: customer.id.clone(),
            name: customer.display_name.clone(),
            email: customer.email.clone(),
            orders_count: customer.orders_count,
            total_spent: format_price(&customer.total_spent),
            created_at: customer.created_at.clone(),
            location,
        }
    }
}

/// Customers list page template.
#[derive(Template)]
#[template(path = "customers/index.html")]
pub struct CustomersIndexTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub customers: Vec<CustomerView>,
    pub has_next_page: bool,
    pub next_cursor: Option<String>,
    pub search_query: Option<String>,
}

/// Customers list page handler.
#[instrument(skip(admin, state))]
pub async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Html<String> {
    let result = state
        .shopify()
        .get_customers(25, query.cursor.clone(), query.query.clone())
        .await;

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

    let template = CustomersIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/customers".to_string(),
        customers,
        has_next_page,
        next_cursor,
        search_query: query.query,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}
