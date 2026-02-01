//! Cart route handlers.
//!
//! Cart operations use HTMX for dynamic updates without full page reloads.
//! Cart IDs are stored in the session and mapped to Shopify carts.

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    Form,
    extract::State,
    http::StatusCode,
    response::{AppendHeaders, Html, IntoResponse, Redirect, Response},
};
use serde::Deserialize;
use tower_sessions::Session;
use tracing::{debug, info, instrument, warn};

use crate::config::AnalyticsConfig;
use crate::filters;
use crate::models::session_keys;
use crate::shopify::types::{Cart as ShopifyCart, CartLineInput, CartLineUpdateInput, Money};
use crate::state::AppState;

/// Cart item display data for templates.
#[derive(Clone)]
pub struct CartItemView {
    pub id: String,
    pub handle: String,
    pub title: String,
    pub variant_title: Option<String>,
    pub quantity: u32,
    pub price: String,
    pub line_price: String,
    pub image: Option<ImageView>,
}

/// Image display data for templates.
#[derive(Clone)]
pub struct ImageView {
    pub url: String,
}

/// Cart display data for templates.
#[derive(Clone)]
pub struct CartView {
    pub items: Vec<CartItemView>,
    pub subtotal: String,
    pub item_count: u32,
}

impl CartView {
    /// Create an empty cart.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            subtotal: "$0.00".to_string(),
            item_count: 0,
        }
    }
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

impl From<&ShopifyCart> for CartView {
    fn from(cart: &ShopifyCart) -> Self {
        Self {
            items: cart.lines.iter().map(CartItemView::from).collect(),
            subtotal: format_price(&cart.cost.subtotal),
            item_count: u32::try_from(cart.total_quantity).unwrap_or(0),
        }
    }
}

impl From<&crate::shopify::types::CartLine> for CartItemView {
    fn from(line: &crate::shopify::types::CartLine) -> Self {
        Self {
            id: line.id.clone(),
            handle: line.merchandise.product.handle.clone(),
            title: line.merchandise.product.title.clone(),
            variant_title: if line.merchandise.title == "Default Title" {
                None
            } else {
                Some(line.merchandise.title.clone())
            },
            quantity: u32::try_from(line.quantity).unwrap_or(1),
            price: format_price(&line.cost.amount_per_quantity),
            line_price: format_price(&line.cost.total_amount),
            image: line.merchandise.image.as_ref().map(|img| ImageView {
                url: img.url.clone(),
            }),
        }
    }
}

// =============================================================================
// Session Helpers
// =============================================================================

/// Get the cart ID from the session.
async fn get_cart_id(session: &Session) -> Option<String> {
    session
        .get::<String>(session_keys::CART_ID)
        .await
        .ok()
        .flatten()
}

/// Set the cart ID in the session.
async fn set_cart_id(
    session: &Session,
    cart_id: &str,
) -> Result<(), tower_sessions::session::Error> {
    session.insert(session_keys::CART_ID, cart_id).await
}

/// Add to cart form data.
#[derive(Debug, Deserialize)]
pub struct AddToCartForm {
    pub variant_id: String,
    pub quantity: Option<u32>,
}

/// Update cart form data.
#[derive(Debug, Deserialize)]
pub struct UpdateCartForm {
    pub line_id: String,
    pub quantity: u32,
}

/// Remove from cart form data.
#[derive(Debug, Deserialize)]
pub struct RemoveFromCartForm {
    pub line_id: String,
}

/// Cart page template.
#[derive(Template, WebTemplate)]
#[template(path = "cart/show.html")]
pub struct CartShowTemplate {
    pub cart: CartView,
    pub analytics: AnalyticsConfig,
    pub nonce: String,
}

/// Cart items fragment template (for HTMX).
#[derive(Template, WebTemplate)]
#[template(path = "partials/cart_items.html")]
pub struct CartItemsTemplate {
    pub cart: CartView,
}

/// Cart count badge fragment template (for HTMX).
#[derive(Template, WebTemplate)]
#[template(path = "partials/cart_count.html")]
pub struct CartCountTemplate {
    pub count: u32,
}

/// Display cart page.
#[instrument(skip(state, session, nonce))]
pub async fn show(
    State(state): State<AppState>,
    session: Session,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> Response {
    debug!("Displaying cart page");

    // Get cart ID from session
    let cart = if let Some(cart_id) = get_cart_id(&session).await {
        debug!(cart_id = %cart_id, "Found existing cart in session");
        // Fetch cart from Shopify
        match state.storefront().get_cart(&cart_id).await {
            Ok(shopify_cart) => {
                let cart_view = CartView::from(&shopify_cart);
                info!(
                    cart_id = %cart_id,
                    item_count = cart_view.item_count,
                    "Successfully loaded cart"
                );
                cart_view
            }
            Err(e) => {
                warn!(cart_id = %cart_id, error = %e, "Failed to fetch cart from Shopify");
                CartView::empty()
            }
        }
    } else {
        debug!("No cart found in session, displaying empty cart");
        CartView::empty()
    };

    let template = CartShowTemplate {
        cart,
        analytics: state.config().analytics.clone(),
        nonce,
    };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to render cart/show.html template");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Template rendering failed",
            )
                .into_response()
        }
    }
}

/// Add item to cart (HTMX).
///
/// Creates a new cart if one doesn't exist, or adds to existing cart.
/// Returns an HTMX trigger to update the cart count badge.
#[instrument(skip(state, session), fields(variant_id = %form.variant_id, quantity = form.quantity))]
pub async fn add(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<AddToCartForm>,
) -> Response {
    debug!("Adding item to cart");

    let quantity = i64::from(form.quantity.unwrap_or(1));
    let variant_id = form.variant_id.clone();
    let line = CartLineInput {
        merchandise_id: form.variant_id,
        quantity,
        attributes: None,
        selling_plan_id: None,
    };

    let result = if let Some(cart_id) = get_cart_id(&session).await {
        debug!(cart_id = %cart_id, "Adding to existing cart");
        // Add to existing cart
        state.storefront().add_to_cart(&cart_id, vec![line]).await
    } else {
        debug!("Creating new cart with item");
        // Create new cart with this item
        state.storefront().create_cart(Some(vec![line]), None).await
    };

    match result {
        Ok(cart) => {
            // Save cart ID to session
            if let Err(e) = set_cart_id(&session, &cart.id).await {
                tracing::error!(cart_id = %cart.id, error = %e, "Failed to save cart ID to session");
            }

            let count = u32::try_from(cart.total_quantity).unwrap_or(0);
            info!(
                cart_id = %cart.id,
                variant_id = %variant_id,
                quantity = quantity,
                total_items = count,
                "Successfully added item to cart"
            );

            // Return cart count with HTMX trigger to update other elements
            (
                AppendHeaders([("HX-Trigger", "cartUpdated")]),
                CartCountTemplate { count },
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(variant_id = %variant_id, error = %e, "Failed to add item to cart");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html("<span class=\"text-red-500\">Error adding to cart</span>"),
            )
                .into_response()
        }
    }
}

/// Update cart item quantity (HTMX).
#[instrument(skip(state, session), fields(line_id = %form.line_id, quantity = form.quantity))]
pub async fn update(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<UpdateCartForm>,
) -> Response {
    debug!("Updating cart item quantity");

    let Some(cart_id) = get_cart_id(&session).await else {
        warn!("Attempted to update cart but no cart found in session");
        return CartItemsTemplate {
            cart: CartView::empty(),
        }
        .into_response();
    };

    let line_id = form.line_id.clone();
    let new_quantity = form.quantity;
    let line_update = CartLineUpdateInput {
        id: form.line_id,
        quantity: Some(i64::from(form.quantity)),
        merchandise_id: None,
        attributes: None,
        selling_plan_id: None,
    };

    match state
        .storefront()
        .update_cart(&cart_id, vec![line_update])
        .await
    {
        Ok(shopify_cart) => {
            let cart = CartView::from(&shopify_cart);
            info!(
                cart_id = %cart_id,
                line_id = %line_id,
                new_quantity = new_quantity,
                total_items = cart.item_count,
                "Successfully updated cart item quantity"
            );
            (
                AppendHeaders([("HX-Trigger", "cartUpdated")]),
                CartItemsTemplate { cart },
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(cart_id = %cart_id, line_id = %line_id, error = %e, "Failed to update cart");
            CartItemsTemplate {
                cart: CartView::empty(),
            }
            .into_response()
        }
    }
}

/// Remove item from cart (HTMX).
#[instrument(skip(state, session), fields(line_id = %form.line_id))]
pub async fn remove(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<RemoveFromCartForm>,
) -> Response {
    debug!("Removing item from cart");

    let Some(cart_id) = get_cart_id(&session).await else {
        warn!("Attempted to remove from cart but no cart found in session");
        return CartItemsTemplate {
            cart: CartView::empty(),
        }
        .into_response();
    };

    let line_id = form.line_id.clone();

    match state
        .storefront()
        .remove_from_cart(&cart_id, vec![form.line_id])
        .await
    {
        Ok(shopify_cart) => {
            let cart = CartView::from(&shopify_cart);
            info!(
                cart_id = %cart_id,
                line_id = %line_id,
                remaining_items = cart.item_count,
                "Successfully removed item from cart"
            );
            (
                AppendHeaders([("HX-Trigger", "cartUpdated")]),
                CartItemsTemplate { cart },
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(cart_id = %cart_id, line_id = %line_id, error = %e, "Failed to remove from cart");
            CartItemsTemplate {
                cart: CartView::empty(),
            }
            .into_response()
        }
    }
}

/// Get cart count badge (HTMX).
#[instrument(skip(state, session))]
pub async fn count(State(state): State<AppState>, session: Session) -> Response {
    debug!("Fetching cart count for badge");

    let count = if let Some(cart_id) = get_cart_id(&session).await {
        debug!(cart_id = %cart_id, "Fetching count for existing cart");
        match state.storefront().get_cart(&cart_id).await {
            Ok(cart) => {
                let count = u32::try_from(cart.total_quantity).unwrap_or(0);
                debug!(cart_id = %cart_id, count = count, "Retrieved cart count");
                count
            }
            Err(e) => {
                warn!(cart_id = %cart_id, error = %e, "Failed to fetch cart for count");
                0
            }
        }
    } else {
        debug!("No cart in session, returning count of 0");
        0
    };

    let template = CartCountTemplate { count };
    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to render partials/cart_count.html template");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Template rendering failed",
            )
                .into_response()
        }
    }
}

/// Redirect to Shopify checkout.
#[instrument(skip(state, session))]
pub async fn checkout(State(state): State<AppState>, session: Session) -> Response {
    debug!("Initiating checkout redirect");

    let Some(cart_id) = get_cart_id(&session).await else {
        warn!("Attempted checkout but no cart found in session");
        // No cart, redirect to cart page
        return Redirect::to("/cart").into_response();
    };

    match state.storefront().get_cart(&cart_id).await {
        Ok(cart) => {
            info!(cart_id = %cart_id, "Redirecting to Shopify checkout");
            Redirect::to(&cart.checkout_url).into_response()
        }
        Err(e) => {
            tracing::error!(cart_id = %cart_id, error = %e, "Failed to get cart for checkout");
            Redirect::to("/cart").into_response()
        }
    }
}
