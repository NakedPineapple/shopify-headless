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
use tracing::instrument;

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
) -> impl IntoResponse {
    // Get cart ID from session
    let cart = match get_cart_id(&session).await {
        Some(cart_id) => {
            // Fetch cart from Shopify
            match state.storefront().get_cart(&cart_id).await {
                Ok(shopify_cart) => CartView::from(&shopify_cart),
                Err(e) => {
                    tracing::warn!("Failed to fetch cart {cart_id}: {e}");
                    CartView::empty()
                }
            }
        }
        None => CartView::empty(),
    };

    CartShowTemplate {
        cart,
        analytics: state.config().analytics.clone(),
        nonce,
    }
}

/// Add item to cart (HTMX).
///
/// Creates a new cart if one doesn't exist, or adds to existing cart.
/// Returns an HTMX trigger to update the cart count badge.
#[instrument(skip(state, session))]
pub async fn add(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<AddToCartForm>,
) -> Response {
    let quantity = i64::from(form.quantity.unwrap_or(1));
    let line = CartLineInput {
        merchandise_id: form.variant_id,
        quantity,
        attributes: None,
        selling_plan_id: None,
    };

    let result = match get_cart_id(&session).await {
        Some(cart_id) => {
            // Add to existing cart
            state.storefront().add_to_cart(&cart_id, vec![line]).await
        }
        None => {
            // Create new cart with this item
            state.storefront().create_cart(Some(vec![line]), None).await
        }
    };

    match result {
        Ok(cart) => {
            // Save cart ID to session
            if let Err(e) = set_cart_id(&session, &cart.id).await {
                tracing::error!("Failed to save cart ID to session: {e}");
            }

            let count = u32::try_from(cart.total_quantity).unwrap_or(0);

            // Return cart count with HTMX trigger to update other elements
            (
                AppendHeaders([("HX-Trigger", "cart-updated")]),
                CartCountTemplate { count },
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to add item to cart: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html("<span class=\"text-red-500\">Error adding to cart</span>"),
            )
                .into_response()
        }
    }
}

/// Update cart item quantity (HTMX).
#[instrument(skip(state, session))]
pub async fn update(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<UpdateCartForm>,
) -> Response {
    let Some(cart_id) = get_cart_id(&session).await else {
        return CartItemsTemplate {
            cart: CartView::empty(),
        }
        .into_response();
    };

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
            (
                AppendHeaders([("HX-Trigger", "cart-updated")]),
                CartItemsTemplate { cart },
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to update cart: {e}");
            CartItemsTemplate {
                cart: CartView::empty(),
            }
            .into_response()
        }
    }
}

/// Remove item from cart (HTMX).
#[instrument(skip(state, session))]
pub async fn remove(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<RemoveFromCartForm>,
) -> Response {
    let Some(cart_id) = get_cart_id(&session).await else {
        return CartItemsTemplate {
            cart: CartView::empty(),
        }
        .into_response();
    };

    match state
        .storefront()
        .remove_from_cart(&cart_id, vec![form.line_id])
        .await
    {
        Ok(shopify_cart) => {
            let cart = CartView::from(&shopify_cart);
            (
                AppendHeaders([("HX-Trigger", "cart-updated")]),
                CartItemsTemplate { cart },
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to remove from cart: {e}");
            CartItemsTemplate {
                cart: CartView::empty(),
            }
            .into_response()
        }
    }
}

/// Get cart count badge (HTMX).
#[instrument(skip(state, session))]
pub async fn count(State(state): State<AppState>, session: Session) -> impl IntoResponse {
    let count = match get_cart_id(&session).await {
        Some(cart_id) => state
            .storefront()
            .get_cart(&cart_id)
            .await
            .map(|cart| u32::try_from(cart.total_quantity).unwrap_or(0))
            .unwrap_or(0),
        None => 0,
    };

    CartCountTemplate { count }
}

/// Redirect to Shopify checkout.
#[instrument(skip(state, session))]
pub async fn checkout(State(state): State<AppState>, session: Session) -> Response {
    let Some(cart_id) = get_cart_id(&session).await else {
        // No cart, redirect to cart page
        return Redirect::to("/cart").into_response();
    };

    match state.storefront().get_cart(&cart_id).await {
        Ok(cart) => Redirect::to(&cart.checkout_url).into_response(),
        Err(e) => {
            tracing::error!("Failed to get cart for checkout: {e}");
            Redirect::to("/cart").into_response()
        }
    }
}
