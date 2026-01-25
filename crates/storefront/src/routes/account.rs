//! Account route handlers.
//!
//! These routes require authentication.

use askama::Template;
use askama_web::WebTemplate;
use axum::{extract::State, response::IntoResponse};

use crate::filters;
use crate::middleware::auth::RequireAuth;
use crate::state::AppState;

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

/// Account overview page template.
#[derive(Template, WebTemplate)]
#[template(path = "account/index.html")]
pub struct AccountIndexTemplate {
    pub user: UserView,
    pub recent_orders: Vec<OrderView>,
    pub passkey_count: u32,
    pub default_address: Option<AddressView>,
    pub subscription_count: u32,
}

/// Display account overview page.
///
/// Note: This handler expects the auth middleware to have verified the user.
/// The `RequireAuth` extractor ensures the user is logged in.
pub async fn index(
    State(_state): State<AppState>,
    RequireAuth(current_user): RequireAuth,
) -> impl IntoResponse {
    // TODO: Fetch account data from Shopify/database
    let user = UserView {
        email: current_user.email.to_string(),
        name: None,
    };

    AccountIndexTemplate {
        user,
        recent_orders: Vec::new(),
        passkey_count: 0,
        default_address: None,
        subscription_count: 0,
    }
}
