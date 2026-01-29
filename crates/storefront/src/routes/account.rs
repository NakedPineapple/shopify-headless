//! Account route handlers.
//!
//! These routes require authentication via Shopify Customer OAuth.
//!
//! # Routes
//!
//! - `GET /account` - Account overview
//! - `GET /account/orders` - Order history
//! - `GET /account/addresses` - Address list
//! - `GET /account/addresses/new` - New address form
//! - `POST /account/addresses` - Create address
//! - `GET /account/addresses/:id/edit` - Edit address form
//! - `POST /account/addresses/:id` - Update address
//! - `DELETE /account/addresses/:id` - Delete address

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    Form,
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use serde::Deserialize;

use crate::config::AnalyticsConfig;
use crate::filters;
use crate::middleware::RequireShopifyCustomer;
use crate::shopify::Money;
use crate::shopify::customer::{Address, AddressInput, Order};
use crate::state::AppState;

// =============================================================================
// View Models
// =============================================================================

/// User display data for templates.
#[derive(Clone)]
pub struct UserView {
    pub email: String,
    pub name: Option<String>,
}

/// Order display data for templates.
#[derive(Clone)]
pub struct OrderView {
    pub number: String,
    pub total: String,
}

/// Address display data for templates.
#[derive(Clone)]
pub struct AddressView {
    pub name: String,
    pub address1: String,
    pub city: String,
    pub province: String,
    pub zip: String,
}

// =============================================================================
// Templates
// =============================================================================

/// Account overview page template.
#[derive(Template, WebTemplate)]
#[template(path = "account/index.html")]
pub struct AccountIndexTemplate {
    pub user: UserView,
    pub recent_orders: Vec<OrderView>,
    pub passkey_count: u32,
    pub default_address: Option<AddressView>,
    pub subscription_count: u32,
    pub analytics: AnalyticsConfig,
}

/// Order history page template.
#[derive(Template, WebTemplate)]
#[template(path = "account/orders.html")]
pub struct OrdersTemplate {
    pub orders: Vec<Order>,
    pub analytics: AnalyticsConfig,
}

/// Addresses list page template.
#[derive(Template, WebTemplate)]
#[template(path = "account/addresses.html")]
pub struct AddressesTemplate {
    pub addresses: Vec<Address>,
    pub default_address_id: Option<String>,
    pub analytics: AnalyticsConfig,
}

impl AddressesTemplate {
    /// Check if an address ID is the default address.
    #[must_use]
    pub fn is_default(&self, address_id: &str) -> bool {
        self.default_address_id
            .as_ref()
            .is_some_and(|id| id == address_id)
    }
}

/// Address form template (create/edit).
#[derive(Template, WebTemplate)]
#[template(path = "account/address_form.html")]
pub struct AddressFormTemplate {
    pub is_edit: bool,
    pub address_id: Option<String>,
    pub address: Option<Address>,
    pub error: Option<String>,
    pub analytics: AnalyticsConfig,
}

// =============================================================================
// Form Data
// =============================================================================

/// Address form data.
#[derive(Debug, Deserialize)]
pub struct AddressForm {
    pub first_name: String,
    pub last_name: String,
    pub company: Option<String>,
    pub address1: String,
    pub address2: Option<String>,
    pub city: String,
    pub province: String,
    pub zip: String,
    pub country: String,
    pub phone: Option<String>,
}

impl From<AddressForm> for AddressInput {
    fn from(form: AddressForm) -> Self {
        Self {
            first_name: Some(form.first_name),
            last_name: Some(form.last_name),
            company: form.company,
            address1: Some(form.address1),
            address2: form.address2,
            city: Some(form.city),
            province: Some(form.province),
            zip: Some(form.zip),
            country: Some(form.country),
            phone: form.phone,
        }
    }
}

// =============================================================================
// Route Handlers
// =============================================================================

/// Display account overview page.
///
/// # Route
///
/// `GET /account`
pub async fn index(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
) -> impl IntoResponse {
    // Fetch customer data from Shopify
    let customer = match state.customer().get_customer(&token.access_token).await {
        Ok(customer) => customer,
        Err(e) => {
            tracing::error!("Failed to fetch customer: {}", e);
            return Redirect::to("/auth/shopify/login").into_response();
        }
    };

    // Fetch recent orders
    let recent_orders = match state.customer().get_orders(&token.access_token, 3).await {
        Ok(orders) => orders
            .into_iter()
            .map(|o| OrderView {
                number: o.name.clone(),
                total: format_money(&o.total_price),
            })
            .collect(),
        Err(e) => {
            tracing::warn!("Failed to fetch orders: {}", e);
            Vec::new()
        }
    };

    // Build user view
    let user = UserView {
        email: customer.email.clone().unwrap_or_default(),
        name: match (&customer.first_name, &customer.last_name) {
            (Some(first), Some(last)) => Some(format!("{first} {last}")),
            (Some(first), None) => Some(first.clone()),
            (None, Some(last)) => Some(last.clone()),
            (None, None) => None,
        },
    };

    // Build default address view
    let default_address = customer.default_address.map(|addr| AddressView {
        name: format!(
            "{} {}",
            addr.first_name.as_deref().unwrap_or_default(),
            addr.last_name.as_deref().unwrap_or_default()
        )
        .trim()
        .to_string(),
        address1: addr.address1.unwrap_or_default(),
        city: addr.city.unwrap_or_default(),
        province: addr.province_code.unwrap_or_default(),
        zip: addr.zip.unwrap_or_default(),
    });

    AccountIndexTemplate {
        user,
        recent_orders,
        passkey_count: 0, // TODO: Fetch from database
        default_address,
        subscription_count: 0, // TODO: Implement subscriptions
        analytics: state.config().analytics.clone(),
    }
    .into_response()
}

/// Display order history page.
///
/// # Route
///
/// `GET /account/orders`
pub async fn orders(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
) -> impl IntoResponse {
    let orders = match state.customer().get_orders(&token.access_token, 50).await {
        Ok(orders) => orders,
        Err(e) => {
            tracing::error!("Failed to fetch orders: {}", e);
            Vec::new()
        }
    };

    OrdersTemplate {
        orders,
        analytics: state.config().analytics.clone(),
    }
}

/// Display addresses list page.
///
/// # Route
///
/// `GET /account/addresses`
pub async fn addresses(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
) -> impl IntoResponse {
    // Fetch addresses
    let addresses = match state
        .customer()
        .get_addresses(&token.access_token, 50)
        .await
    {
        Ok(addresses) => addresses,
        Err(e) => {
            tracing::error!("Failed to fetch addresses: {}", e);
            Vec::new()
        }
    };

    // Fetch customer to get default address ID
    let default_address_id = match state.customer().get_customer(&token.access_token).await {
        Ok(customer) => customer.default_address.map(|a| a.id),
        Err(_) => None,
    };

    AddressesTemplate {
        addresses,
        default_address_id,
        analytics: state.config().analytics.clone(),
    }
}

/// Display new address form.
///
/// # Route
///
/// `GET /account/addresses/new`
pub async fn new_address(
    State(state): State<AppState>,
    RequireShopifyCustomer(_token): RequireShopifyCustomer,
) -> impl IntoResponse {
    AddressFormTemplate {
        is_edit: false,
        address_id: None,
        address: None,
        error: None,
        analytics: state.config().analytics.clone(),
    }
}

/// Create a new address.
///
/// # Route
///
/// `POST /account/addresses`
pub async fn create_address(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    Form(form): Form<AddressForm>,
) -> Response {
    let input: AddressInput = form.into();

    match state
        .customer()
        .create_address(&token.access_token, input)
        .await
    {
        Ok(_) => Redirect::to("/account/addresses").into_response(),
        Err(e) => {
            tracing::error!("Failed to create address: {}", e);
            AddressFormTemplate {
                is_edit: false,
                address_id: None,
                address: None,
                error: Some(e.to_string()),
                analytics: state.config().analytics.clone(),
            }
            .into_response()
        }
    }
}

/// Display edit address form.
///
/// # Route
///
/// `GET /account/addresses/:id/edit`
pub async fn edit_address(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    Path(address_id): Path<String>,
) -> Response {
    // Fetch addresses and find the one we want
    let addresses = match state
        .customer()
        .get_addresses(&token.access_token, 50)
        .await
    {
        Ok(addresses) => addresses,
        Err(e) => {
            tracing::error!("Failed to fetch addresses: {}", e);
            return Redirect::to("/account/addresses").into_response();
        }
    };

    let Some(addr) = addresses.into_iter().find(|a| a.id == address_id) else {
        tracing::warn!("Address not found: {}", address_id);
        return Redirect::to("/account/addresses").into_response();
    };

    AddressFormTemplate {
        is_edit: true,
        address_id: Some(addr.id.clone()),
        address: Some(addr),
        error: None,
        analytics: state.config().analytics.clone(),
    }
    .into_response()
}

/// Update an existing address.
///
/// # Route
///
/// `POST /account/addresses/:id`
pub async fn update_address(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    Path(address_id): Path<String>,
    Form(form): Form<AddressForm>,
) -> Response {
    let input: AddressInput = form.into();

    match state
        .customer()
        .update_address(&token.access_token, &address_id, input)
        .await
    {
        Ok(_) => Redirect::to("/account/addresses").into_response(),
        Err(e) => {
            tracing::error!("Failed to update address: {}", e);
            // Fetch the address again to show the form with error
            let addresses = state
                .customer()
                .get_addresses(&token.access_token, 50)
                .await
                .unwrap_or_default();
            let address = addresses.into_iter().find(|a| a.id == address_id);

            AddressFormTemplate {
                is_edit: true,
                address_id: Some(address_id),
                address,
                error: Some(e.to_string()),
                analytics: state.config().analytics.clone(),
            }
            .into_response()
        }
    }
}

/// Delete an address.
///
/// # Route
///
/// `DELETE /account/addresses/:id`
pub async fn delete_address(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    Path(address_id): Path<String>,
) -> Response {
    match state
        .customer()
        .delete_address(&token.access_token, &address_id)
        .await
    {
        Ok(()) => {
            // Return empty response for HTMX (removes the element)
            StatusCode::OK.into_response()
        }
        Err(e) => {
            tracing::error!("Failed to delete address: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Format a Money value for display.
fn format_money(money: &Money) -> String {
    let amount: f64 = money.amount.parse().unwrap_or(0.0);
    let currency = &money.currency_code;

    match currency.as_str() {
        "USD" => format!("${amount:.2}"),
        "EUR" => format!("\u{20ac}{amount:.2}"),
        "GBP" => format!("\u{00a3}{amount:.2}"),
        _ => format!("{amount:.2} {currency}"),
    }
}
