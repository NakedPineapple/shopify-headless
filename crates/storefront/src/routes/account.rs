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
use tracing::{debug, error, info, instrument, warn};

use crate::config::{AnalyticsConfig, AnalyticsUserInfo};
use crate::filters;
use crate::middleware::{OptionalAuth, RequireShopifyCustomer};
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
    pub analytics_user_info: AnalyticsUserInfo,
    pub site: crate::middleware::SiteContext,
    pub nonce: String,
}

/// Order history page template.
#[derive(Template, WebTemplate)]
#[template(path = "account/orders.html")]
pub struct OrdersTemplate {
    pub orders: Vec<Order>,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub site: crate::middleware::SiteContext,
    pub nonce: String,
}

/// Addresses list page template.
#[derive(Template, WebTemplate)]
#[template(path = "account/addresses.html")]
pub struct AddressesTemplate {
    pub addresses: Vec<Address>,
    pub default_address_id: Option<String>,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub site: crate::middleware::SiteContext,
    pub nonce: String,
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
    pub analytics_user_info: AnalyticsUserInfo,
    pub site: crate::middleware::SiteContext,
    pub nonce: String,
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
#[instrument(skip(state, token, current_customer, nonce))]
pub async fn index(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    OptionalAuth(current_customer): OptionalAuth,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> impl IntoResponse {
    let analytics_user_info = AnalyticsUserInfo::from_customer(current_customer.as_ref());

    debug!("Rendering account overview page");

    // Fetch customer data from Shopify
    let customer = match state.customer().get_customer(&token.access_token).await {
        Ok(customer) => {
            debug!(email = ?customer.email, "Fetched customer data from Shopify");
            customer
        }
        Err(e) => {
            error!("Failed to fetch customer: {}", e);
            return Redirect::to("/auth/shopify/login").into_response();
        }
    };

    // Fetch recent orders
    let recent_orders = match state.customer().get_orders(&token.access_token, 3).await {
        Ok(orders) => {
            debug!(order_count = orders.len(), "Fetched recent orders");
            orders
                .into_iter()
                .map(|o| OrderView {
                    number: o.name.clone(),
                    total: format_money(&o.total_price),
                })
                .collect()
        }
        Err(e) => {
            warn!("Failed to fetch orders: {}", e);
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

    info!("Successfully rendered account overview page");

    AccountIndexTemplate {
        user,
        recent_orders,
        passkey_count: 0, // TODO: Fetch from database
        default_address,
        subscription_count: 0, // TODO: Implement subscriptions
        analytics: state.config().analytics.clone(),
        analytics_user_info,
        nonce,
        site,
    }
    .into_response()
}

/// Display order history page.
///
/// # Route
///
/// `GET /account/orders`
#[instrument(skip(state, token, customer, nonce))]
pub async fn orders(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    OptionalAuth(customer): OptionalAuth,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> impl IntoResponse {
    debug!("Fetching order history page");

    let orders = match state.customer().get_orders(&token.access_token, 50).await {
        Ok(orders) => {
            debug!(order_count = orders.len(), "Fetched orders from Shopify");
            orders
        }
        Err(e) => {
            error!("Failed to fetch orders: {}", e);
            Vec::new()
        }
    };

    info!(
        order_count = orders.len(),
        "Successfully rendered order history page"
    );

    OrdersTemplate {
        orders,
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::from_customer(customer.as_ref()),
        nonce,
        site,
    }
}

/// Display addresses list page.
///
/// # Route
///
/// `GET /account/addresses`
#[instrument(skip(state, token, customer, nonce))]
pub async fn addresses(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    OptionalAuth(customer): OptionalAuth,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> impl IntoResponse {
    debug!("Fetching addresses list page");

    // Fetch addresses
    let addresses = match state
        .customer()
        .get_addresses(&token.access_token, 50)
        .await
    {
        Ok(addresses) => {
            debug!(
                address_count = addresses.len(),
                "Fetched addresses from Shopify"
            );
            addresses
        }
        Err(e) => {
            error!("Failed to fetch addresses: {}", e);
            Vec::new()
        }
    };

    // Fetch customer to get default address ID
    let default_address_id = match state.customer().get_customer(&token.access_token).await {
        Ok(customer) => {
            debug!(
                has_default = customer.default_address.is_some(),
                "Fetched default address info"
            );
            customer.default_address.map(|a| a.id)
        }
        Err(e) => {
            warn!("Failed to fetch customer for default address: {}", e);
            None
        }
    };

    info!(
        address_count = addresses.len(),
        "Successfully rendered addresses list page"
    );

    AddressesTemplate {
        addresses,
        default_address_id,
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::from_customer(customer.as_ref()),
        nonce,
        site,
    }
}

/// Display new address form.
///
/// # Route
///
/// `GET /account/addresses/new`
#[instrument(skip(state, _token, customer, nonce))]
pub async fn new_address(
    State(state): State<AppState>,
    RequireShopifyCustomer(_token): RequireShopifyCustomer,
    OptionalAuth(customer): OptionalAuth,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> impl IntoResponse {
    debug!("Rendering new address form");

    info!("Successfully rendered new address form");

    AddressFormTemplate {
        is_edit: false,
        address_id: None,
        address: None,
        error: None,
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::from_customer(customer.as_ref()),
        nonce,
        site,
    }
}

/// Create a new address.
///
/// # Route
///
/// `POST /account/addresses`
#[instrument(skip(state, token, customer, nonce, form))]
pub async fn create_address(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    OptionalAuth(customer): OptionalAuth,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
    Form(form): Form<AddressForm>,
) -> Response {
    debug!("Creating new address");

    let input: AddressInput = form.into();

    match state
        .customer()
        .create_address(&token.access_token, input)
        .await
    {
        Ok(_) => {
            info!("Successfully created new address");
            Redirect::to("/account/addresses").into_response()
        }
        Err(e) => {
            error!("Failed to create address: {}", e);
            AddressFormTemplate {
                is_edit: false,
                address_id: None,
                address: None,
                error: Some(e.to_string()),
                analytics: state.config().analytics.clone(),
                analytics_user_info: AnalyticsUserInfo::from_customer(customer.as_ref()),
                nonce,
                site,
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
#[instrument(skip(state, token, customer, nonce), fields(address_id = %address_id))]
pub async fn edit_address(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    OptionalAuth(customer): OptionalAuth,
    Path(address_id): Path<String>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> Response {
    debug!("Rendering edit address form");

    // Fetch addresses and find the one we want
    let addresses = match state
        .customer()
        .get_addresses(&token.access_token, 50)
        .await
    {
        Ok(addresses) => {
            debug!(
                address_count = addresses.len(),
                "Fetched addresses from Shopify"
            );
            addresses
        }
        Err(e) => {
            error!("Failed to fetch addresses: {}", e);
            return Redirect::to("/account/addresses").into_response();
        }
    };

    let Some(addr) = addresses.into_iter().find(|a| a.id == address_id) else {
        warn!(address_id = %address_id, "Address not found");
        return Redirect::to("/account/addresses").into_response();
    };

    info!("Successfully rendered edit address form");

    AddressFormTemplate {
        is_edit: true,
        address_id: Some(addr.id.clone()),
        address: Some(addr),
        error: None,
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::from_customer(customer.as_ref()),
        nonce,
        site,
    }
    .into_response()
}

/// Update an existing address.
///
/// # Route
///
/// `POST /account/addresses/:id`
#[instrument(skip(state, token, customer, nonce, form), fields(address_id = %address_id))]
pub async fn update_address(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    OptionalAuth(customer): OptionalAuth,
    Path(address_id): Path<String>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
    Form(form): Form<AddressForm>,
) -> Response {
    debug!("Updating existing address");

    let input: AddressInput = form.into();

    match state
        .customer()
        .update_address(&token.access_token, &address_id, input)
        .await
    {
        Ok(_) => {
            info!("Successfully updated address");
            Redirect::to("/account/addresses").into_response()
        }
        Err(e) => {
            error!("Failed to update address: {}", e);
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
                analytics_user_info: AnalyticsUserInfo::from_customer(customer.as_ref()),
                nonce,
                site,
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
#[instrument(skip(state, token), fields(address_id = %address_id))]
pub async fn delete_address(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    Path(address_id): Path<String>,
) -> Response {
    debug!("Deleting address");

    match state
        .customer()
        .delete_address(&token.access_token, &address_id)
        .await
    {
        Ok(()) => {
            info!("Successfully deleted address");
            // Return empty response for HTMX (removes the element)
            StatusCode::OK.into_response()
        }
        Err(e) => {
            error!("Failed to delete address: {}", e);
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
