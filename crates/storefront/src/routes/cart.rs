//! Cart route handlers.
//!
//! Cart operations use HTMX for dynamic updates without full page reloads.

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    Form,
    extract::State,
    response::{Html, IntoResponse},
};
use serde::Deserialize;

use crate::filters;
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
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            subtotal: "$0.00".to_string(),
            item_count: 0,
        }
    }
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
pub async fn show(State(_state): State<AppState>) -> impl IntoResponse {
    // TODO: Fetch cart from session/Shopify
    let cart = CartView::empty();

    CartShowTemplate { cart }
}

/// Add item to cart (HTMX).
pub async fn add(
    State(_state): State<AppState>,
    Form(_form): Form<AddToCartForm>,
) -> impl IntoResponse {
    // TODO: Add item to cart via Shopify Storefront API

    // Return empty response - cart count will be updated via HTMX trigger
    Html("")
}

/// Update cart item quantity (HTMX).
pub async fn update(
    State(_state): State<AppState>,
    Form(_form): Form<UpdateCartForm>,
) -> impl IntoResponse {
    // TODO: Update cart via Shopify Storefront API
    let cart = CartView::empty();

    CartItemsTemplate { cart }
}

/// Remove item from cart (HTMX).
pub async fn remove(
    State(_state): State<AppState>,
    Form(_form): Form<RemoveFromCartForm>,
) -> impl IntoResponse {
    // TODO: Remove item from cart via Shopify Storefront API
    let cart = CartView::empty();

    CartItemsTemplate { cart }
}

/// Get cart count badge (HTMX).
pub async fn count(State(_state): State<AppState>) -> impl IntoResponse {
    // TODO: Get cart count from session/Shopify
    let count = 0;

    CartCountTemplate { count }
}
