//! Account route handlers.
//!
//! These routes require authentication via Shopify Customer OAuth.
//! The change password route uses Storefront API auth (`RequireAuth`) instead.
//!
//! # Routes
//!
//! - `GET /account` - Account overview
//! - `GET /account/profile` - Profile editing page
//! - `POST /account/profile` - Update profile
//! - `GET /account/orders` - Order history
//! - `GET /account/orders/:id` - Order detail
//! - `GET /account/orders/:id/return` - Return request form
//! - `POST /account/orders/:id/return` - Submit return request
//! - `GET /account/change-password` - Change password page
//! - `POST /account/change-password` - Change password action
//! - `GET /account/addresses` - Address list
//! - `GET /account/addresses/new` - New address form
//! - `POST /account/addresses` - Create address
//! - `GET /account/addresses/:id/edit` - Edit address form
//! - `POST /account/addresses/:id` - Update address
//! - `DELETE /account/addresses/:id` - Delete address
//! - `GET /account/subscriptions` - Subscription list
//! - `GET /account/subscriptions/:id` - Subscription detail
//! - `POST /account/subscriptions/:id/pause` - Pause subscription
//! - `POST /account/subscriptions/:id/cancel` - Cancel subscription
//! - `POST /account/subscriptions/:id/activate` - Activate subscription
//! - `POST /account/subscriptions/:id/skip/:cycle_index` - Skip billing cycle
//! - `POST /account/subscriptions/:id/unskip/:cycle_index` - Unskip billing cycle
//! - `GET /account/passkeys` - Passkey management
//! - `DELETE /account/passkeys/:id` - Delete passkey

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    Form,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Redirect, Response},
};
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use tower_sessions::Session;
use tracing::{debug, error, info, instrument, warn};

use crate::config::{AnalyticsConfig, AnalyticsUserInfo};
use crate::filters;
use crate::middleware::{OptionalAuth, RequireAuth, RequireShopifyCustomer, set_current_customer};
use crate::models::CurrentCustomer;
use crate::services::auth::AuthService;
use crate::shopify::Money;
use crate::shopify::customer::{
    Address, AddressInput, Order, ReturnRequestLineItemInput, ReturnStatus, SubscriptionContract,
    SubscriptionContractStatus,
};
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
    pub store_credit_balance: Option<String>,
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

/// Profile display data for templates.
#[derive(Clone)]
pub struct ProfileView {
    pub email: String,
    pub first_name: String,
    pub last_name: String,
    pub phone: String,
    pub accepts_marketing: bool,
}

/// Passkey display data for templates.
#[derive(Clone)]
pub struct PasskeyView {
    pub id: i32,
    pub name: String,
    pub created_at: String,
}

/// Order detail display data for templates.
#[derive(Clone)]
pub struct OrderDetailView {
    pub id: String,
    pub name: String,
    pub processed_at: String,
    pub financial_status_label: String,
    pub fulfillment_status_label: String,
    pub total: String,
    pub subtotal: String,
    pub shipping: String,
    pub tax: String,
    pub line_items: Vec<OrderLineItemView>,
    pub shipping_address: Option<String>,
    pub returns: Vec<ReturnSummaryView>,
    pub can_request_return: bool,
}

/// Order line item display data for templates.
#[derive(Clone)]
pub struct OrderLineItemView {
    pub title: String,
    pub variant_title: Option<String>,
    pub quantity: i64,
    pub unit_price: String,
    pub total: String,
    pub image_url: Option<String>,
}

/// Return summary display data for templates.
#[derive(Clone)]
pub struct ReturnSummaryView {
    pub name: String,
    pub status_label: String,
}

/// Return form line item display data for templates.
#[derive(Clone)]
pub struct ReturnFormLineItem {
    pub id: String,
    pub title: String,
    pub variant_title: Option<String>,
    pub quantity: i64,
    pub image_url: Option<String>,
    pub reasons: Vec<ReturnReasonView>,
}

/// Return reason display data for templates.
#[derive(Clone)]
pub struct ReturnReasonView {
    pub id: String,
    pub name: String,
}

/// Billing cycle display data for templates.
#[derive(Clone)]
pub struct BillingCycleView {
    pub cycle_index: i64,
    pub expected_date: String,
    pub skipped: bool,
}

/// Subscription display data for templates.
#[derive(Clone)]
pub struct SubscriptionView {
    pub id: String,
    pub status: SubscriptionContractStatus,
    pub status_label: String,
    pub next_billing_date: Option<String>,
    pub interval_label: String,
    pub line_items: Vec<SubscriptionLineView>,
    pub can_pause: bool,
    pub can_cancel: bool,
    pub can_activate: bool,
}

/// Subscription line item display data for templates.
#[derive(Clone)]
pub struct SubscriptionLineView {
    pub name: String,
    pub quantity: i64,
    pub price: String,
    pub image_url: Option<String>,
}

/// Subscription list page template.
#[derive(Template, WebTemplate)]
#[template(path = "account/subscriptions.html")]
pub struct SubscriptionsTemplate {
    pub subscriptions: Vec<SubscriptionView>,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub site: crate::middleware::SiteContext,
    pub nonce: String,
}

/// Subscription detail page template.
#[derive(Template, WebTemplate)]
#[template(path = "account/subscription_detail.html")]
pub struct SubscriptionDetailTemplate {
    pub subscription: SubscriptionView,
    pub delivery_price: String,
    pub created_at: String,
    pub upcoming_cycles: Vec<BillingCycleView>,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub site: crate::middleware::SiteContext,
    pub nonce: String,
}

/// Profile editing page template.
#[derive(Template, WebTemplate)]
#[template(path = "account/profile.html")]
pub struct ProfileTemplate {
    pub profile: ProfileView,
    pub error: Option<String>,
    pub success: Option<String>,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub site: crate::middleware::SiteContext,
    pub nonce: String,
}

/// Passkeys management page template.
#[derive(Template, WebTemplate)]
#[template(path = "account/passkeys.html")]
pub struct PasskeysTemplate {
    pub passkeys: Vec<PasskeyView>,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub site: crate::middleware::SiteContext,
    pub nonce: String,
}

/// Order detail page template.
#[derive(Template, WebTemplate)]
#[template(path = "account/order_detail.html")]
pub struct OrderDetailTemplate {
    pub order: OrderDetailView,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub site: crate::middleware::SiteContext,
    pub nonce: String,
}

/// Return request form template.
#[derive(Template, WebTemplate)]
#[template(path = "account/return_form.html")]
pub struct ReturnFormTemplate {
    pub order_id: String,
    pub order_name: String,
    pub line_items: Vec<ReturnFormLineItem>,
    pub error: Option<String>,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub site: crate::middleware::SiteContext,
    pub nonce: String,
}

/// Change password page template.
#[derive(Template, WebTemplate)]
#[template(path = "account/change_password.html")]
pub struct ChangePasswordTemplate {
    pub error: Option<String>,
    pub success: Option<String>,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
    pub site: crate::middleware::SiteContext,
    pub nonce: String,
}

// =============================================================================
// Form Data
// =============================================================================

/// Change password form data.
#[derive(Debug, Deserialize)]
pub struct ChangePasswordForm {
    pub current_password: String,
    pub new_password: String,
    pub new_password_confirm: String,
}

/// Query parameters for the change password page.
#[derive(Debug, Deserialize)]
pub struct ChangePasswordQuery {
    pub error: Option<String>,
    pub success: Option<String>,
}

/// Profile form data.
#[derive(Debug, Deserialize)]
pub struct ProfileForm {
    pub first_name: String,
    pub last_name: String,
    pub phone: Option<String>,
    pub accepts_marketing: Option<String>,
}

/// Query parameters for the profile page.
#[derive(Debug, Deserialize)]
pub struct ProfileQuery {
    pub error: Option<String>,
    pub success: Option<String>,
}

/// Return request form item data.
#[derive(Debug, Deserialize)]
pub struct ReturnFormItem {
    pub selected: Option<String>,
    pub line_item_id: String,
    pub quantity: i32,
    pub reason_id: Option<String>,
    pub note: Option<String>,
}

/// Return request form data.
#[derive(Debug, Deserialize)]
pub struct ReturnForm {
    pub items: Vec<ReturnFormItem>,
}

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

    // Fetch orders, subscriptions, passkeys, and store credit concurrently
    let shopify_customer_id = current_customer
        .as_ref()
        .map(|c| c.shopify_customer_id.clone());

    let (orders_result, subscriptions_result, passkey_result, store_credit_result) = tokio::join!(
        state.customer().get_orders(&token.access_token, 3),
        state.customer().get_subscriptions(&token.access_token, 50),
        async {
            if let Some(id) = &shopify_customer_id {
                let auth = AuthService::new(state.pool(), state.webauthn_for_host(&site.host));
                auth.get_credentials_by_shopify_customer_id(id).await.ok()
            } else {
                None
            }
        },
        state.customer().get_store_credit(&token.access_token),
    );

    let recent_orders = match orders_result {
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

    let subscription_count: u32 = match subscriptions_result {
        Ok(subs) => {
            let count = subs
                .iter()
                .filter(|s| s.status == SubscriptionContractStatus::Active)
                .count();
            debug!(active_count = count, "Fetched subscription count");
            u32::try_from(count).unwrap_or(0)
        }
        Err(e) => {
            warn!("Failed to fetch subscriptions: {}", e);
            0
        }
    };

    let passkey_count = passkey_result.map_or(0, |creds| u32::try_from(creds.len()).unwrap_or(0));

    let store_credit_balance = match store_credit_result {
        Ok(Some(balance)) => Some(format_money(&balance)),
        _ => None,
    };

    let user = build_user_view(&customer);
    let default_address = customer.default_address.as_ref().map(build_address_view);

    info!("Successfully rendered account overview page");

    AccountIndexTemplate {
        user,
        recent_orders,
        passkey_count,
        default_address,
        subscription_count,
        store_credit_balance,
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
// Subscriptions
// =============================================================================

/// Convert a `SubscriptionContract` to a `SubscriptionView`.
fn subscription_to_view(contract: &SubscriptionContract) -> SubscriptionView {
    let line_items = contract
        .line_items()
        .into_iter()
        .map(|line| SubscriptionLineView {
            name: line.name.clone(),
            quantity: line.quantity,
            price: format_money(&line.current_price),
            image_url: line.image.as_ref().map(|img| img.url.clone()),
        })
        .collect();

    SubscriptionView {
        id: contract.id.clone(),
        status_label: contract.status.label().to_string(),
        next_billing_date: contract.next_billing_date.clone(),
        interval_label: contract.billing_policy.frequency_label(),
        line_items,
        can_pause: contract.status.can_pause(),
        can_cancel: contract.status.can_cancel(),
        can_activate: contract.status.can_activate(),
        status: contract.status.clone(),
    }
}

/// Display subscriptions list page.
///
/// # Route
///
/// `GET /account/subscriptions`
#[instrument(skip(state, token, customer, nonce, site))]
pub async fn subscriptions(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    OptionalAuth(customer): OptionalAuth,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> impl IntoResponse {
    debug!("Fetching subscriptions list page");

    let contracts = match state
        .customer()
        .get_subscriptions(&token.access_token, 50)
        .await
    {
        Ok(subs) => {
            debug!(count = subs.len(), "Fetched subscriptions from Shopify");
            subs
        }
        Err(e) => {
            error!("Failed to fetch subscriptions: {}", e);
            Vec::new()
        }
    };

    let subscriptions: Vec<SubscriptionView> = contracts.iter().map(subscription_to_view).collect();

    info!(
        count = subscriptions.len(),
        "Successfully rendered subscriptions list page"
    );

    SubscriptionsTemplate {
        subscriptions,
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::from_customer(customer.as_ref()),
        nonce,
        site,
    }
}

/// Display subscription detail page.
///
/// # Route
///
/// `GET /account/subscriptions/:id`
#[instrument(skip(state, token, customer, nonce, site), fields(subscription_id = %id))]
pub async fn subscription_detail(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    OptionalAuth(customer): OptionalAuth,
    Path(id): Path<String>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> Response {
    debug!("Fetching subscription detail page");

    let contract = match state
        .customer()
        .get_subscription(&token.access_token, &id)
        .await
    {
        Ok(Some(contract)) => contract,
        Ok(None) => {
            warn!(subscription_id = %id, "Subscription not found");
            return Redirect::to("/account/subscriptions").into_response();
        }
        Err(e) => {
            error!("Failed to fetch subscription: {}", e);
            return Redirect::to("/account/subscriptions").into_response();
        }
    };

    let delivery_price = format_money(&contract.delivery_price);
    let created_at = contract.created_at.clone();
    let subscription = subscription_to_view(&contract);

    // Fetch upcoming billing cycles for active subscriptions
    let upcoming_cycles = if contract.status == SubscriptionContractStatus::Active {
        match state
            .customer()
            .get_upcoming_billing_cycles(&token.access_token, &id, 5)
            .await
        {
            Ok(cycles) => cycles
                .into_iter()
                .map(|c| BillingCycleView {
                    cycle_index: c.cycle_index,
                    expected_date: c.billing_attempt_expected_date,
                    skipped: c.skipped,
                })
                .collect(),
            Err(e) => {
                warn!("Failed to fetch billing cycles: {}", e);
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    info!("Successfully rendered subscription detail page");

    SubscriptionDetailTemplate {
        subscription,
        delivery_price,
        created_at,
        upcoming_cycles,
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::from_customer(customer.as_ref()),
        nonce,
        site,
    }
    .into_response()
}

/// Pause a subscription.
///
/// # Route
///
/// `POST /account/subscriptions/:id/pause`
#[instrument(skip(state, token), fields(subscription_id = %id))]
pub async fn pause_subscription(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    Path(id): Path<String>,
) -> Response {
    debug!("Pausing subscription");

    match state
        .customer()
        .pause_subscription(&token.access_token, &id)
        .await
    {
        Ok(()) => {
            info!("Successfully paused subscription");
        }
        Err(e) => {
            error!("Failed to pause subscription: {}", e);
        }
    }

    Redirect::to(&format!("/account/subscriptions/{id}")).into_response()
}

/// Cancel a subscription.
///
/// # Route
///
/// `POST /account/subscriptions/:id/cancel`
#[instrument(skip(state, token), fields(subscription_id = %id))]
pub async fn cancel_subscription(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    Path(id): Path<String>,
) -> Response {
    debug!("Cancelling subscription");

    match state
        .customer()
        .cancel_subscription(&token.access_token, &id)
        .await
    {
        Ok(()) => {
            info!("Successfully cancelled subscription");
        }
        Err(e) => {
            error!("Failed to cancel subscription: {}", e);
        }
    }

    Redirect::to(&format!("/account/subscriptions/{id}")).into_response()
}

/// Activate (resume) a paused subscription.
///
/// # Route
///
/// `POST /account/subscriptions/:id/activate`
#[instrument(skip(state, token), fields(subscription_id = %id))]
pub async fn activate_subscription(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    Path(id): Path<String>,
) -> Response {
    debug!("Activating subscription");

    match state
        .customer()
        .activate_subscription(&token.access_token, &id)
        .await
    {
        Ok(()) => {
            info!("Successfully activated subscription");
        }
        Err(e) => {
            error!("Failed to activate subscription: {}", e);
        }
    }

    Redirect::to(&format!("/account/subscriptions/{id}")).into_response()
}

// =============================================================================
// Profile
// =============================================================================

/// Display profile editing page.
///
/// # Route
///
/// `GET /account/profile`
#[instrument(skip(state, token, current_customer, nonce, site))]
pub async fn profile_page(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    OptionalAuth(current_customer): OptionalAuth,
    Query(query): Query<ProfileQuery>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> Response {
    debug!("Rendering profile page");

    let customer = match state.customer().get_customer(&token.access_token).await {
        Ok(customer) => customer,
        Err(e) => {
            error!("Failed to fetch customer: {}", e);
            return Redirect::to("/account").into_response();
        }
    };

    let profile = ProfileView {
        email: customer.email.clone().unwrap_or_default(),
        first_name: customer.first_name.clone().unwrap_or_default(),
        last_name: customer.last_name.clone().unwrap_or_default(),
        phone: customer.phone.clone().unwrap_or_default(),
        accepts_marketing: customer.accepts_marketing,
    };

    info!("Successfully rendered profile page");

    ProfileTemplate {
        profile,
        error: query.error,
        success: query.success,
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::from_customer(current_customer.as_ref()),
        nonce,
        site,
    }
    .into_response()
}

/// Handle profile update form submission.
///
/// # Route
///
/// `POST /account/profile`
#[instrument(skip(state, token, form))]
pub async fn update_profile(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    Form(form): Form<ProfileForm>,
) -> Response {
    debug!("Processing profile update");

    let input = crate::shopify::customer::CustomerUpdateInput {
        first_name: Some(form.first_name),
        last_name: Some(form.last_name),
        phone: form.phone,
        accepts_marketing: Some(form.accepts_marketing.is_some()),
    };

    match state
        .customer()
        .update_customer(&token.access_token, input)
        .await
    {
        Ok(_) => {
            info!("Successfully updated profile");
            Redirect::to("/account/profile?success=profile_updated").into_response()
        }
        Err(e) => {
            error!("Failed to update profile: {}", e);
            Redirect::to("/account/profile?error=update_failed").into_response()
        }
    }
}

// =============================================================================
// Passkeys
// =============================================================================

/// Display passkeys management page.
///
/// # Route
///
/// `GET /account/passkeys`
#[instrument(skip(state, _token, current_customer, nonce, site))]
pub async fn passkeys(
    State(state): State<AppState>,
    RequireShopifyCustomer(_token): RequireShopifyCustomer,
    OptionalAuth(current_customer): OptionalAuth,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> Response {
    debug!("Rendering passkeys page");

    let shopify_customer_id = current_customer
        .as_ref()
        .map(|c| c.shopify_customer_id.clone());

    let passkeys = if let Some(id) = &shopify_customer_id {
        let auth = AuthService::new(state.pool(), state.webauthn_for_host(&site.host));
        match auth.get_credentials_by_shopify_customer_id(id).await {
            Ok(creds) => creds
                .into_iter()
                .map(|c| PasskeyView {
                    id: c.id.as_i32(),
                    name: c.name,
                    created_at: c.created_at.format("%b %d, %Y").to_string(),
                })
                .collect(),
            Err(e) => {
                warn!("Failed to fetch passkeys: {}", e);
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    info!(count = passkeys.len(), "Rendered passkeys page");

    PasskeysTemplate {
        passkeys,
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::from_customer(current_customer.as_ref()),
        nonce,
        site,
    }
    .into_response()
}

/// Delete a passkey.
///
/// # Route
///
/// `DELETE /account/passkeys/:id`
#[instrument(skip(state, _token, current_customer, site), fields(passkey_id = %id))]
pub async fn delete_passkey(
    State(state): State<AppState>,
    RequireShopifyCustomer(_token): RequireShopifyCustomer,
    OptionalAuth(current_customer): OptionalAuth,
    Path(id): Path<i32>,
    site: crate::middleware::SiteContext,
) -> Response {
    debug!("Deleting passkey");

    let Some(customer) = current_customer else {
        return StatusCode::FORBIDDEN.into_response();
    };

    let auth = AuthService::new(state.pool(), state.webauthn_for_host(&site.host));
    let credential_id = naked_pineapple_core::CredentialId::new(id);

    match auth
        .delete_credential_for_shopify_customer(&customer.shopify_customer_id, credential_id)
        .await
    {
        Ok(true) => {
            info!("Successfully deleted passkey");
            StatusCode::OK.into_response()
        }
        Ok(false) => {
            warn!("Passkey not found for deletion");
            StatusCode::NOT_FOUND.into_response()
        }
        Err(e) => {
            error!("Failed to delete passkey: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

// =============================================================================
// Order Detail & Returns
// =============================================================================

/// Display order detail page.
///
/// # Route
///
/// `GET /account/orders/:id`
#[instrument(skip(state, token, customer, nonce, site), fields(order_id = %id))]
pub async fn order_detail(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    OptionalAuth(customer): OptionalAuth,
    Path(id): Path<String>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> Response {
    debug!("Fetching order detail page");

    let detail = match state.customer().get_order(&token.access_token, &id).await {
        Ok(Some(detail)) => detail,
        Ok(None) => {
            warn!(order_id = %id, "Order not found");
            return Redirect::to("/account/orders").into_response();
        }
        Err(e) => {
            error!("Failed to fetch order: {}", e);
            return Redirect::to("/account/orders").into_response();
        }
    };

    let order = build_order_detail_view(&detail);

    info!("Successfully rendered order detail page");

    OrderDetailTemplate {
        order,
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::from_customer(customer.as_ref()),
        nonce,
        site,
    }
    .into_response()
}

/// Display the return request form for an order.
///
/// # Route
///
/// `GET /account/orders/:id/return`
#[instrument(skip(state, token, customer, nonce, site), fields(order_id = %id))]
pub async fn return_form(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    OptionalAuth(customer): OptionalAuth,
    Path(id): Path<String>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> Response {
    debug!("Rendering return form");

    let order = match state
        .customer()
        .get_order_for_return(&token.access_token, &id)
        .await
    {
        Ok(Some(order)) => order,
        Ok(None) => {
            warn!(order_id = %id, "Order not found for return");
            return Redirect::to("/account/orders").into_response();
        }
        Err(e) => {
            error!("Failed to fetch order for return: {}", e);
            return Redirect::to(&format!("/account/orders/{id}")).into_response();
        }
    };

    let line_items = build_return_form_items(&order);

    info!("Successfully rendered return form");

    ReturnFormTemplate {
        order_id: order.id,
        order_name: order.name,
        line_items,
        error: None,
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::from_customer(customer.as_ref()),
        nonce,
        site,
    }
    .into_response()
}

/// Handle return request form submission.
///
/// # Route
///
/// `POST /account/orders/:id/return`
#[instrument(skip(state, token, form), fields(order_id = %id))]
pub async fn request_return(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    Path(id): Path<String>,
    Form(form): Form<ReturnForm>,
) -> Response {
    debug!("Processing return request");

    let items: Vec<ReturnRequestLineItemInput> = form
        .items
        .into_iter()
        .filter(|item| item.selected.is_some())
        .map(|item| ReturnRequestLineItemInput {
            line_item_id: item.line_item_id,
            quantity: item.quantity,
            reason_id: item.reason_id.filter(|s| !s.is_empty()),
            note: item.note.filter(|s| !s.is_empty()),
        })
        .collect();

    if items.is_empty() {
        return Redirect::to(&format!(
            "/account/orders/{id}/return?error=no_items_selected"
        ))
        .into_response();
    }

    match state
        .customer()
        .request_return(&token.access_token, &id, &items)
        .await
    {
        Ok(()) => {
            info!("Successfully submitted return request");
            Redirect::to(&format!("/account/orders/{id}")).into_response()
        }
        Err(e) => {
            error!("Failed to submit return request: {}", e);
            Redirect::to(&format!("/account/orders/{id}")).into_response()
        }
    }
}

// =============================================================================
// Billing Cycle Skip/Unskip
// =============================================================================

/// Skip a subscription billing cycle.
///
/// # Route
///
/// `POST /account/subscriptions/:id/skip/:cycle_index`
#[instrument(skip(state, token), fields(subscription_id = %id, cycle_index = %cycle_index))]
pub async fn skip_billing_cycle(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    Path((id, cycle_index)): Path<(String, i64)>,
) -> Response {
    debug!("Skipping billing cycle");

    match state
        .customer()
        .skip_billing_cycle(&token.access_token, &id, cycle_index)
        .await
    {
        Ok(()) => {
            info!("Successfully skipped billing cycle");
        }
        Err(e) => {
            error!("Failed to skip billing cycle: {}", e);
        }
    }

    Redirect::to(&format!("/account/subscriptions/{id}")).into_response()
}

/// Unskip a subscription billing cycle.
///
/// # Route
///
/// `POST /account/subscriptions/:id/unskip/:cycle_index`
#[instrument(skip(state, token), fields(subscription_id = %id, cycle_index = %cycle_index))]
pub async fn unskip_billing_cycle(
    State(state): State<AppState>,
    RequireShopifyCustomer(token): RequireShopifyCustomer,
    Path((id, cycle_index)): Path<(String, i64)>,
) -> Response {
    debug!("Unskipping billing cycle");

    match state
        .customer()
        .unskip_billing_cycle(&token.access_token, &id, cycle_index)
        .await
    {
        Ok(()) => {
            info!("Successfully unskipped billing cycle");
        }
        Err(e) => {
            error!("Failed to unskip billing cycle: {}", e);
        }
    }

    Redirect::to(&format!("/account/subscriptions/{id}")).into_response()
}

// =============================================================================
// Change Password
// =============================================================================

/// Display change password page.
///
/// # Route
///
/// `GET /account/change-password`
#[instrument(skip(state, _customer, nonce, site))]
pub async fn change_password_page(
    State(state): State<AppState>,
    RequireAuth(_customer): RequireAuth,
    Query(query): Query<ChangePasswordQuery>,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
    site: crate::middleware::SiteContext,
) -> impl IntoResponse {
    debug!("Rendering change password page");

    ChangePasswordTemplate {
        error: query.error,
        success: query.success,
        analytics: state.config().analytics.clone(),
        analytics_user_info: AnalyticsUserInfo::default(),
        site,
        nonce,
    }
}

/// Handle change password form submission.
///
/// Verifies current password, then updates via Shopify `customerUpdate` mutation.
/// Updates the session with the new access token since password change
/// invalidates all existing tokens.
///
/// # Route
///
/// `POST /account/change-password`
#[instrument(skip(state, session, customer, form))]
pub async fn change_password(
    State(state): State<AppState>,
    session: Session,
    RequireAuth(customer): RequireAuth,
    Form(form): Form<ChangePasswordForm>,
) -> Response {
    debug!("Processing change password request");

    // Validate new passwords match
    if form.new_password != form.new_password_confirm {
        return Redirect::to("/account/change-password?error=password_mismatch").into_response();
    }

    // Validate new password complexity
    if let Err(code) = crate::routes::auth::validate_password_complexity(&form.new_password) {
        return Redirect::to(&format!(
            "/account/change-password?error={}",
            urlencoding::encode(code)
        ))
        .into_response();
    }

    // Verify current password by creating a fresh access token.
    // This also gives us a known-good token for the customerUpdate call,
    // which handles passkey-authenticated users who may have empty session tokens.
    let fresh_token = match state
        .storefront()
        .create_access_token(&customer.email, &form.current_password)
        .await
    {
        Ok(token) => token,
        Err(e) => {
            warn!(
                "Change password: current password verification failed: {}",
                e
            );
            return Redirect::to("/account/change-password?error=invalid_current_password")
                .into_response();
        }
    };

    // Update password via Shopify customerUpdate mutation
    match state
        .storefront()
        .update_customer_password(&fresh_token.access_token, &form.new_password)
        .await
    {
        Ok((updated_customer, new_token)) => {
            // Update session with new access token (old ones are invalidated)
            let updated = CurrentCustomer::new(
                updated_customer.id,
                updated_customer
                    .email
                    .unwrap_or_else(|| customer.email.clone()),
                updated_customer
                    .first_name
                    .or_else(|| customer.first_name.clone()),
                updated_customer
                    .last_name
                    .or_else(|| customer.last_name.clone()),
                SecretString::from(new_token.access_token),
                new_token.expires_at,
            );

            if let Err(e) = set_current_customer(&session, &updated).await {
                error!("Failed to update session after password change: {}", e);
                return Redirect::to("/account/change-password?error=session").into_response();
            }

            info!("Customer password changed successfully");
            Redirect::to("/account/change-password?success=password_changed").into_response()
        }
        Err(e) => {
            warn!("Password change failed: {}", e);
            Redirect::to("/account/change-password?error=update_failed").into_response()
        }
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Build a `UserView` from a Shopify `Customer`.
fn build_user_view(customer: &crate::shopify::customer::Customer) -> UserView {
    UserView {
        email: customer.email.clone().unwrap_or_default(),
        name: match (&customer.first_name, &customer.last_name) {
            (Some(first), Some(last)) => Some(format!("{first} {last}")),
            (Some(first), None) => Some(first.clone()),
            (None, Some(last)) => Some(last.clone()),
            (None, None) => None,
        },
    }
}

/// Build an `AddressView` from a Shopify `Address`.
fn build_address_view(addr: &Address) -> AddressView {
    AddressView {
        name: format!(
            "{} {}",
            addr.first_name.as_deref().unwrap_or_default(),
            addr.last_name.as_deref().unwrap_or_default()
        )
        .trim()
        .to_string(),
        address1: addr.address1.clone().unwrap_or_default(),
        city: addr.city.clone().unwrap_or_default(),
        province: addr.province_code.clone().unwrap_or_default(),
        zip: addr.zip.clone().unwrap_or_default(),
    }
}

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

/// Build an `OrderDetailView` from an `OrderDetail` API response.
fn build_order_detail_view(detail: &crate::shopify::customer::OrderDetail) -> OrderDetailView {
    let line_items = detail
        .line_items
        .edges
        .iter()
        .map(|edge| {
            let item = &edge.node;
            OrderLineItemView {
                title: item.title.clone(),
                variant_title: item.variant_title.clone(),
                quantity: item.quantity,
                unit_price: format_money(&item.unit_price),
                total: format_money(&item.total_price),
                image_url: item.image.as_ref().map(|img| img.url.clone()),
            }
        })
        .collect();

    let returns: Vec<ReturnSummaryView> = detail
        .returns
        .edges
        .iter()
        .map(|edge| ReturnSummaryView {
            name: edge.node.name.clone(),
            status_label: edge.node.status.label().to_string(),
        })
        .collect();

    let has_pending_return = detail.returns.edges.iter().any(|edge| {
        matches!(
            edge.node.status,
            ReturnStatus::Requested | ReturnStatus::Open
        )
    });

    let is_fulfilled = detail
        .fulfillment_status
        .as_deref()
        .is_some_and(|s| s == "FULFILLED");

    let can_request_return = is_fulfilled && !has_pending_return;

    let shipping_address = detail.shipping_address.as_ref().map(format_address);

    OrderDetailView {
        id: detail.id.clone(),
        name: detail.name.clone(),
        processed_at: detail.processed_at.clone(),
        financial_status_label: format_status_label(detail.financial_status.as_deref()),
        fulfillment_status_label: format_status_label(detail.fulfillment_status.as_deref()),
        total: format_money(&detail.total_price),
        subtotal: format_money(&detail.subtotal),
        shipping: format_money(&detail.total_shipping),
        tax: format_money(&detail.total_tax),
        line_items,
        shipping_address,
        returns,
        can_request_return,
    }
}

/// Build return form line items from an order-for-return API response.
fn build_return_form_items(
    order: &crate::shopify::customer::OrderForReturn,
) -> Vec<ReturnFormLineItem> {
    order
        .line_items
        .edges
        .iter()
        .map(|edge| {
            let item = &edge.node;
            let reasons = item
                .suggested_reasons
                .edges
                .iter()
                .map(|r| ReturnReasonView {
                    id: r.node.id.clone(),
                    name: r.node.name.clone(),
                })
                .collect();

            ReturnFormLineItem {
                id: item.id.clone(),
                title: item.title.clone(),
                variant_title: item.variant_title.clone(),
                quantity: item.quantity,
                image_url: item.image.as_ref().map(|img| img.url.clone()),
                reasons,
            }
        })
        .collect()
}

/// Format an `Address` as a multi-line string.
fn format_address(addr: &Address) -> String {
    let mut parts = Vec::new();
    let name = format!(
        "{} {}",
        addr.first_name.as_deref().unwrap_or_default(),
        addr.last_name.as_deref().unwrap_or_default()
    );
    let name = name.trim();
    if !name.is_empty() {
        parts.push(name.to_string());
    }
    if let Some(a1) = &addr.address1
        && !a1.is_empty()
    {
        parts.push(a1.clone());
    }
    if let Some(a2) = &addr.address2
        && !a2.is_empty()
    {
        parts.push(a2.clone());
    }
    let city_state_zip = format!(
        "{}, {} {}",
        addr.city.as_deref().unwrap_or_default(),
        addr.province_code.as_deref().unwrap_or_default(),
        addr.zip.as_deref().unwrap_or_default()
    );
    parts.push(city_state_zip);
    parts.join("\n")
}

/// Format a Shopify status string (e.g., `PARTIALLY_FULFILLED`) for display.
fn format_status_label(status: Option<&str>) -> String {
    status.map_or_else(
        || "Unknown".to_string(),
        |s| {
            s.split('_')
                .map(|word| {
                    let mut chars = word.chars();
                    chars.next().map_or_else(String::new, |c| {
                        c.to_uppercase().to_string() + &chars.as_str().to_lowercase()
                    })
                })
                .collect::<Vec<_>>()
                .join(" ")
        },
    )
}
