//! Cart route handlers.
//!
//! Cart operations use HTMX for dynamic updates without full page reloads.
//! Cart IDs are stored in the session and mapped to Shopify carts.

use std::collections::{HashMap, HashSet};

use askama::Template;
use askama_web::WebTemplate;
use axum::{
    Form,
    extract::State,
    http::StatusCode,
    response::{AppendHeaders, Html, IntoResponse, Redirect, Response},
};
use futures::future;
use serde::Deserialize;
use tower_sessions::Session;
use tracing::{debug, info, instrument, warn};

use crate::config::{AnalyticsConfig, AnalyticsUserInfo};
use crate::error::add_breadcrumb;
use crate::filters;
use crate::middleware::OptionalAuth;
use crate::models::session_keys;
use crate::routes::products::ProductView;
use crate::services::discount_matcher::{self, DiscountMatchResult, DiscountSuggestion, GwpAction};
use crate::shopify::types::{
    ActivePromotions, AttributeInput, Cart as ShopifyCart, CartLineInput, CartLineUpdateInput,
    Money, ProductRecommendationIntent, PromotionBanner,
};
use crate::state::AppState;

/// Cart item display data for templates.
#[derive(Clone)]
pub struct CartItemView {
    pub id: String,
    pub product_id: String,
    pub handle: String,
    pub title: String,
    pub variant_title: Option<String>,
    pub quantity: u32,
    pub price: String,
    pub line_price: String,
    pub image: Option<ImageView>,
    /// Whether this item is a gift-with-purchase (free item from BXGY discount).
    pub is_gwp: bool,
    /// The rule ID that granted this GWP (for tracking/removal).
    pub gwp_rule_id: Option<String>,
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

/// The attribute key used to mark GWP items.
const GWP_RULE_ID_ATTR: &str = "_gwp_rule_id";

/// Maximum quantity per cart line to prevent abuse.
const MAX_QUANTITY_PER_LINE: u32 = 999;

impl From<&crate::shopify::types::CartLine> for CartItemView {
    fn from(line: &crate::shopify::types::CartLine) -> Self {
        // Check if this line is a GWP by looking for the special attribute
        let gwp_rule_id = line
            .attributes
            .iter()
            .find(|attr| attr.key == GWP_RULE_ID_ATTR)
            .and_then(|attr| attr.value.clone());

        let is_gwp = gwp_rule_id.is_some();

        // For non-GWP items, use subtotal_amount (before automatic discounts) to show
        // the actual product price. Shopify's BXGY discounts can get applied to the
        // wrong line item from our perspective, so we display undiscounted prices
        // for regular items. GWP items show "FREE" in the template regardless.
        let line_price = if is_gwp {
            format_price(&line.cost.total_amount)
        } else {
            format_price(&line.cost.subtotal_amount)
        };

        Self {
            id: line.id.clone(),
            product_id: line.merchandise.product.id.clone(),
            handle: line.merchandise.product.handle.clone(),
            title: line.merchandise.product.title.clone(),
            variant_title: if line.merchandise.title == "Default Title" {
                None
            } else {
                Some(line.merchandise.title.clone())
            },
            quantity: u32::try_from(line.quantity).unwrap_or(1),
            price: format_price(&line.cost.amount_per_quantity),
            line_price,
            image: line.merchandise.image.as_ref().map(|img| ImageView {
                url: img.url.clone(),
            }),
            is_gwp,
            gwp_rule_id,
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

/// Claim GWP (gift-with-purchase) form data.
#[derive(Debug, Deserialize)]
pub struct ClaimGwpForm {
    /// The variant ID to add as the free gift.
    pub variant_id: String,
    /// The rule ID that grants this GWP.
    pub rule_id: String,
}

/// Cart page template.
#[derive(Template, WebTemplate)]
#[template(path = "cart/show.html")]
pub struct CartShowTemplate {
    pub cart: CartView,
    pub recommended_products: Vec<ProductView>,
    pub promotion_banners: Vec<PromotionBanner>,
    pub discount_suggestions: Vec<DiscountSuggestion>,
    pub qualifies_for_free_shipping: bool,
    /// Whether there's a pending GWP selection that blocks checkout.
    pub has_pending_gwp_selection: bool,
    pub analytics: AnalyticsConfig,
    pub analytics_user_info: AnalyticsUserInfo,
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

/// Order summary fragment template (for HTMX).
#[derive(Template, WebTemplate)]
#[template(path = "partials/order_summary.html")]
pub struct OrderSummaryTemplate {
    pub cart: CartView,
    pub promotion_banners: Vec<PromotionBanner>,
    pub discount_suggestions: Vec<DiscountSuggestion>,
    pub qualifies_for_free_shipping: bool,
    /// Whether there's a pending GWP selection that blocks checkout.
    pub has_pending_gwp_selection: bool,
}

/// Display cart page.
#[instrument(skip(state, session, nonce, customer))]
pub async fn show(
    State(state): State<AppState>,
    session: Session,
    OptionalAuth(customer): OptionalAuth,
    crate::middleware::CspNonce(nonce): crate::middleware::CspNonce,
) -> Response {
    debug!("Displaying cart page");

    let analytics_user_info = AnalyticsUserInfo::from_customer(customer.as_ref());

    // Get cart ID from session
    let Some(cart_id) = get_cart_id(&session).await else {
        debug!("No cart found in session, displaying empty cart");
        return render_cart_page(
            &state,
            CartView::empty(),
            nonce,
            analytics_user_info.clone(),
        )
        .await;
    };

    debug!(cart_id = %cart_id, "Found existing cart in session");

    // Fetch cart from Shopify
    let shopify_cart = match state.storefront().get_cart(&cart_id).await {
        Ok(cart) => cart,
        Err(e) => {
            warn!(cart_id = %cart_id, error = %e, "Failed to fetch cart from Shopify");
            return render_cart_page(&state, CartView::empty(), nonce, analytics_user_info).await;
        }
    };

    let cart = CartView::from(&shopify_cart);
    info!(cart_id = %cart_id, item_count = cart.item_count, "Successfully loaded cart");

    // Fetch promotions to check for auto-add GWPs
    let promotions = state
        .storefront()
        .get_active_promotions()
        .await
        .unwrap_or_else(|e| {
            warn!(error = %e, "Failed to fetch active promotions, using defaults");
            ActivePromotions::default()
        });

    // Match cart against qualifying rules
    let match_result = discount_matcher::match_qualifying_rules(
        &cart.items,
        &cart.subtotal,
        &promotions.progress_tracking,
        &promotions.qualifying_rules,
    );

    // Auto-add any unclaimed GWP items
    let cart =
        auto_add_gwp_items(&state, &session, &cart_id, cart, &match_result.suggestions).await;

    // Re-match after potential auto-adds (cart may have changed)
    let DiscountMatchResult {
        suggestions: discount_suggestions,
        qualifies_for_free_shipping,
    } = discount_matcher::match_qualifying_rules(
        &cart.items,
        &cart.subtotal,
        &promotions.progress_tracking,
        &promotions.qualifying_rules,
    );

    // Enrich AutoAdd suggestions with product titles
    let discount_suggestions =
        enrich_gwp_product_titles(state.storefront(), discount_suggestions).await;

    // Fetch recommendations
    let recommended_products = fetch_cart_recommendations(&state, &cart).await;

    // Collect IDs of claimed GWPs (to hide their banners)
    let claimed_gwp_rule_ids: std::collections::HashSet<_> = cart
        .items
        .iter()
        .filter_map(|item| item.gwp_rule_id.as_deref())
        .collect();

    // Sort banners by priority, filter out claimed GWPs, and take top 3
    let mut promotion_banners = promotions.banners;
    promotion_banners.retain(|b| !claimed_gwp_rule_ids.contains(b.id.as_str()));
    promotion_banners.sort_by_key(|b| b.priority);
    promotion_banners.truncate(3);

    // Check if there's a pending GWP selection
    let has_pending_gwp_selection = has_unclaimed_gwp_selection(&cart, &discount_suggestions);

    debug!(
        suggestions = discount_suggestions.len(),
        qualifies_for_free_shipping = qualifies_for_free_shipping,
        has_pending_gwp_selection = has_pending_gwp_selection,
        "Processed cart promotions"
    );

    let template = CartShowTemplate {
        cart,
        recommended_products,
        promotion_banners,
        discount_suggestions,
        qualifies_for_free_shipping,
        has_pending_gwp_selection,
        analytics: state.config().analytics.clone(),
        analytics_user_info,
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

/// Render the cart page with an empty or minimal cart (helper for early returns).
async fn render_cart_page(
    state: &AppState,
    cart: CartView,
    nonce: String,
    analytics_user_info: AnalyticsUserInfo,
) -> Response {
    let promotions = state
        .storefront()
        .get_active_promotions()
        .await
        .unwrap_or_default();

    let mut promotion_banners = promotions.banners;
    promotion_banners.sort_by_key(|b| b.priority);
    promotion_banners.truncate(3);

    let DiscountMatchResult {
        suggestions: discount_suggestions,
        qualifies_for_free_shipping,
    } = discount_matcher::match_qualifying_rules(
        &cart.items,
        &cart.subtotal,
        &promotions.progress_tracking,
        &promotions.qualifying_rules,
    );

    let template = CartShowTemplate {
        cart,
        recommended_products: Vec::new(),
        promotion_banners,
        discount_suggestions,
        qualifies_for_free_shipping,
        has_pending_gwp_selection: false,
        analytics: state.config().analytics.clone(),
        analytics_user_info,
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

/// Auto-add any unclaimed GWP items to the cart.
///
/// Checks for `AutoAdd` GWP suggestions where the customer is qualified but hasn't
/// claimed the gift yet, and automatically adds them to the cart.
async fn auto_add_gwp_items(
    state: &AppState,
    _session: &Session,
    cart_id: &str,
    mut cart: CartView,
    suggestions: &[DiscountSuggestion],
) -> CartView {
    for suggestion in suggestions {
        // Only process qualified AutoAdd suggestions
        if !suggestion.is_qualified || suggestion.gwp_claimed {
            continue;
        }

        let Some(GwpAction::AutoAdd {
            variant_id,
            product_id: _,
            product_title: _,
        }) = &suggestion.gwp_action
        else {
            continue;
        };

        // Check if already claimed (belt and suspenders with gwp_claimed flag)
        let already_in_cart = cart
            .items
            .iter()
            .any(|item| item.gwp_rule_id.as_deref() == Some(&suggestion.rule_id));

        if already_in_cart {
            continue;
        }

        debug!(
            rule_id = %suggestion.rule_id,
            variant_id = %variant_id,
            "Auto-adding GWP item to cart"
        );

        // Add the GWP to the cart
        let line = CartLineInput {
            merchandise_id: variant_id.clone(),
            quantity: 1,
            attributes: Some(vec![AttributeInput {
                key: GWP_RULE_ID_ATTR.to_string(),
                value: suggestion.rule_id.clone(),
            }]),
            selling_plan_id: None,
        };

        match state.storefront().add_to_cart(cart_id, vec![line]).await {
            Ok(updated_cart) => {
                info!(
                    cart_id = %cart_id,
                    rule_id = %suggestion.rule_id,
                    variant_id = %variant_id,
                    "Successfully auto-added GWP item"
                );
                cart = CartView::from(&updated_cart);
            }
            Err(e) => {
                warn!(
                    cart_id = %cart_id,
                    rule_id = %suggestion.rule_id,
                    variant_id = %variant_id,
                    error = %e,
                    "Failed to auto-add GWP item"
                );
            }
        }
    }

    cart
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
    add_breadcrumb(
        "cart",
        "Added item to cart",
        Some(&[
            ("variant_id", &form.variant_id),
            ("quantity", &form.quantity.unwrap_or(1).to_string()),
        ]),
    );
    debug!("Adding item to cart");

    let raw_quantity = form.quantity.unwrap_or(1);
    if raw_quantity == 0 || raw_quantity > MAX_QUANTITY_PER_LINE {
        warn!(
            quantity = raw_quantity,
            "Invalid quantity in add-to-cart request"
        );
        return (
            StatusCode::BAD_REQUEST,
            Html("<span class=\"text-red-500\">Quantity must be between 1 and 999</span>"),
        )
            .into_response();
    }
    let quantity = i64::from(raw_quantity);
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
    add_breadcrumb(
        "cart",
        "Updated cart quantity",
        Some(&[
            ("line_id", &form.line_id),
            ("quantity", &form.quantity.to_string()),
        ]),
    );
    debug!("Updating cart item quantity");

    let Some(cart_id) = get_cart_id(&session).await else {
        warn!("Attempted to update cart but no cart found in session");
        return CartItemsTemplate {
            cart: CartView::empty(),
        }
        .into_response();
    };

    if form.quantity > MAX_QUANTITY_PER_LINE {
        warn!(
            quantity = form.quantity,
            "Invalid quantity in update-cart request"
        );
        return (
            StatusCode::BAD_REQUEST,
            Html("<span class=\"text-red-500\">Quantity must be 999 or less</span>"),
        )
            .into_response();
    }

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
    add_breadcrumb(
        "cart",
        "Removed item from cart",
        Some(&[("line_id", &form.line_id)]),
    );
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

/// Get order summary fragment (HTMX).
///
/// Returns the order summary including subtotal, shipping status, and discount progress.
#[instrument(skip(state, session))]
pub async fn summary(State(state): State<AppState>, session: Session) -> Response {
    debug!("Fetching order summary");

    // Get cart from session
    let cart = if let Some(cart_id) = get_cart_id(&session).await {
        match state.storefront().get_cart(&cart_id).await {
            Ok(shopify_cart) => CartView::from(&shopify_cart),
            Err(e) => {
                warn!(cart_id = %cart_id, error = %e, "Failed to fetch cart for summary");
                CartView::empty()
            }
        }
    } else {
        CartView::empty()
    };

    // Fetch promotions
    let promotions = state
        .storefront()
        .get_active_promotions()
        .await
        .unwrap_or_default();

    // Match cart against qualifying rules (joins progress_tracking display config with rules)
    let DiscountMatchResult {
        suggestions: discount_suggestions,
        qualifies_for_free_shipping,
    } = discount_matcher::match_qualifying_rules(
        &cart.items,
        &cart.subtotal,
        &promotions.progress_tracking,
        &promotions.qualifying_rules,
    );

    // Enrich AutoAdd suggestions with product titles (fetched from Storefront API)
    let discount_suggestions =
        enrich_gwp_product_titles(state.storefront(), discount_suggestions).await;

    // Collect IDs of claimed GWPs (to hide their banners)
    let claimed_gwp_rule_ids: std::collections::HashSet<_> = cart
        .items
        .iter()
        .filter_map(|item| item.gwp_rule_id.as_deref())
        .collect();

    // Sort banners by priority, filter out claimed GWPs, and take top 3
    let mut promotion_banners = promotions.banners;
    promotion_banners.retain(|b| !claimed_gwp_rule_ids.contains(b.id.as_str()));
    promotion_banners.sort_by_key(|b| b.priority);
    promotion_banners.truncate(3);

    // Check if there's a pending GWP selection
    let has_pending_gwp_selection = has_unclaimed_gwp_selection(&cart, &discount_suggestions);

    let template = OrderSummaryTemplate {
        cart,
        promotion_banners,
        discount_suggestions,
        qualifies_for_free_shipping,
        has_pending_gwp_selection,
    };

    match template.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "Failed to render partials/order_summary.html template");
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
    add_breadcrumb("checkout", "Initiated checkout", None);
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

/// Validate that a GWP claim is legitimate.
///
/// Checks that:
/// 1. The rule ID format is valid (Shopify GID format)
/// 2. The customer qualifies for the rule
/// 3. The variant being claimed is valid for the GWP action
fn validate_gwp_claim(suggestions: &[DiscountSuggestion], rule_id: &str, variant_id: &str) -> bool {
    // Find the suggestion for this rule
    suggestions.iter().any(|s| {
        s.rule_id == rule_id
            && s.is_qualified
            && s.gwp_action.as_ref().is_some_and(|action| {
                // Check if the variant is valid for this GWP
                match action {
                    GwpAction::AutoAdd {
                        variant_id: gwp_variant,
                        ..
                    } => gwp_variant == variant_id,
                    GwpAction::PromptSelection { product_ids } => {
                        // For prompt selection, the variant should belong to one of the products
                        // We can't verify this precisely without fetching variant data, but we can
                        // at least check that the variant looks like a Shopify GID
                        variant_id.starts_with("gid://shopify/ProductVariant/")
                            && !product_ids.is_empty()
                    }
                    GwpAction::BrowseCollection { .. } => {
                        // For browse collection, we allow any variant that looks valid
                        // The actual validation happens when Shopify applies the discount
                        variant_id.starts_with("gid://shopify/ProductVariant/")
                    }
                }
            })
    })
}

/// Validate a GWP claim and return an error response if validation fails.
async fn validate_gwp_claim_request(
    state: &AppState,
    cart_id: &str,
    rule_id: &str,
    variant_id: &str,
) -> Result<(), Response> {
    // Validate rule_id format (must not be empty)
    if rule_id.is_empty() {
        warn!(rule_id = %rule_id, "Invalid GWP rule ID format");
        return Err((
            StatusCode::BAD_REQUEST,
            Html("<p class=\"text-sm text-destructive\">Invalid promotion</p>"),
        )
            .into_response());
    }

    // Fetch current cart
    let shopify_cart = state.storefront().get_cart(cart_id).await.map_err(|e| {
        tracing::error!(error = %e, "Failed to fetch cart for GWP validation");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Html("<p class=\"text-sm text-destructive\">Failed to validate promotion</p>"),
        )
            .into_response()
    })?;

    let cart = CartView::from(&shopify_cart);

    // Get active promotions
    let promotions = state
        .storefront()
        .get_active_promotions()
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to fetch promotions for GWP validation");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html("<p class=\"text-sm text-destructive\">Failed to validate promotion</p>"),
            )
                .into_response()
        })?;

    // Evaluate which rules the cart qualifies for
    let match_result = discount_matcher::match_qualifying_rules(
        &cart.items,
        &cart.subtotal,
        &promotions.progress_tracking,
        &promotions.qualifying_rules,
    );

    // Validate this specific claim
    if !validate_gwp_claim(&match_result.suggestions, rule_id, variant_id) {
        warn!(
            rule_id = %rule_id,
            variant_id = %variant_id,
            "GWP claim failed validation - customer may not qualify"
        );
        return Err((
            StatusCode::BAD_REQUEST,
            Html("<p class=\"text-sm text-destructive\">You don't qualify for this promotion</p>"),
        )
            .into_response());
    }

    Ok(())
}

/// Claim a gift-with-purchase item (HTMX).
///
/// Adds the free item to the cart with a special attribute marking it as a GWP.
/// Returns an HTML fragment confirming the GWP was added.
#[instrument(skip(state, session), fields(variant_id = %form.variant_id, rule_id = %form.rule_id))]
pub async fn claim_gwp(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<ClaimGwpForm>,
) -> Response {
    add_breadcrumb(
        "cart",
        "Claimed gift with purchase",
        Some(&[("variant_id", &form.variant_id)]),
    );
    debug!("Claiming GWP item");

    let Some(cart_id) = get_cart_id(&session).await else {
        warn!("Attempted to claim GWP but no cart found in session");
        return (
            StatusCode::BAD_REQUEST,
            Html("<p class=\"text-sm text-destructive\">No cart found</p>"),
        )
            .into_response();
    };

    // Validate the GWP claim
    if let Err(err_response) =
        validate_gwp_claim_request(&state, &cart_id, &form.rule_id, &form.variant_id).await
    {
        return err_response;
    }

    // Add the validated GWP item with a special attribute to track it
    let line = CartLineInput {
        merchandise_id: form.variant_id.clone(),
        quantity: 1,
        attributes: Some(vec![AttributeInput {
            key: GWP_RULE_ID_ATTR.to_string(),
            value: form.rule_id.clone(),
        }]),
        selling_plan_id: None,
    };

    match state.storefront().add_to_cart(&cart_id, vec![line]).await {
        Ok(_cart) => {
            info!(
                cart_id = %cart_id,
                variant_id = %form.variant_id,
                rule_id = %form.rule_id,
                "Successfully claimed GWP item"
            );

            // Return a confirmation message that replaces the picker
            // Also trigger cartUpdated to refresh the cart display
            (
                AppendHeaders([("HX-Trigger", "cartUpdated")]),
                Html(
                    r#"<div class="p-3 bg-leaf/10 border border-leaf/30 rounded-lg">
                        <div class="flex items-center gap-2">
                            <i class="ph ph-check-circle text-leaf text-lg"></i>
                            <p class="text-sm font-medium text-foreground">Free gift added to your cart!</p>
                        </div>
                    </div>"#
                        .to_string(),
                ),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(
                cart_id = %cart_id,
                variant_id = %form.variant_id,
                error = %e,
                "Failed to add GWP to cart"
            );
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html("<p class=\"text-sm text-destructive\">Failed to add free gift</p>"),
            )
                .into_response()
        }
    }
}

// =============================================================================
// GWP Helpers
// =============================================================================

/// Check if there's an unclaimed GWP that requires user selection.
///
/// Returns true if there's a qualified BXGY discount with a `PromptSelection` or
/// `BrowseCollection` action, and the customer hasn't yet claimed that GWP.
fn has_unclaimed_gwp_selection(cart: &CartView, suggestions: &[DiscountSuggestion]) -> bool {
    for suggestion in suggestions {
        if !suggestion.is_qualified {
            continue;
        }

        match &suggestion.gwp_action {
            Some(GwpAction::PromptSelection { .. } | GwpAction::BrowseCollection { .. }) => {
                // Check if this GWP has already been claimed
                let already_claimed = cart
                    .items
                    .iter()
                    .any(|item| item.gwp_rule_id.as_deref() == Some(&suggestion.rule_id));

                if !already_claimed {
                    return true;
                }
            }
            Some(GwpAction::AutoAdd { .. }) | None => {
                // Auto-add items don't block checkout
            }
        }
    }

    false
}

// =============================================================================
// Recommendation Helpers
// =============================================================================

/// Fetch and aggregate product recommendations for all cart items.
///
/// First checks the `custom.cart_recommendations` metafield for manually configured
/// recommendations. Falls back to Shopify's ML recommendations for products without
/// manual config. Products recommended by multiple cart items rank higher.
/// Filters out products already in the cart.
async fn fetch_cart_recommendations(state: &AppState, cart: &CartView) -> Vec<ProductView> {
    if cart.items.is_empty() {
        return Vec::new();
    }

    let cart_product_ids: HashSet<_> = cart.items.iter().map(|i| i.product_id.as_str()).collect();
    let manual_recommendations = state
        .storefront()
        .get_cart_recommendations()
        .await
        .unwrap_or_default();

    // Build lookup and collect manual/ML products
    let manual_lookup: HashMap<&str, &[crate::shopify::types::RelatedProduct]> =
        manual_recommendations
            .product_relations
            .iter()
            .map(|r| (r.product_id.as_str(), r.related_products.as_slice()))
            .collect();

    let (manual_product_ids, products_needing_ml) =
        collect_recommendation_sources(&cart.items, &manual_lookup);

    // Fetch ML recommendations for products without manual config
    let ml_results = fetch_ml_recommendations(state, &products_needing_ml).await;

    // Score and sort recommendations
    let recommendation_scores =
        score_recommendations(&cart_product_ids, &manual_product_ids, ml_results);

    let mut sorted: Vec<_> = recommendation_scores.into_iter().collect();
    sorted.sort_by(|a, b| b.1.1.cmp(&a.1.1));

    // Get the single best recommendation
    let Some((product_id, _)) = sorted.into_iter().next() else {
        return Vec::new();
    };

    // Always fetch full product details to get metafields (rating, benefits, etc.)
    state
        .storefront()
        .get_product_by_id(&product_id)
        .await
        .map_or_else(|_| Vec::new(), |product| vec![ProductView::from(&product)])
}

/// Collect product IDs from manual config and identify products needing ML fallback.
fn collect_recommendation_sources(
    items: &[CartItemView],
    manual_lookup: &HashMap<&str, &[crate::shopify::types::RelatedProduct]>,
) -> (Vec<String>, Vec<String>) {
    let mut manual_product_ids = Vec::new();
    let mut products_needing_ml = Vec::new();

    for item in items {
        if let Some(related) = manual_lookup.get(item.product_id.as_str()) {
            manual_product_ids.extend(related.iter().map(|rp| rp.product_id.clone()));
        } else {
            products_needing_ml.push(item.product_id.clone());
        }
    }

    (manual_product_ids, products_needing_ml)
}

/// Fetch ML recommendations for products without manual config.
async fn fetch_ml_recommendations(
    state: &AppState,
    product_ids: &[String],
) -> Vec<Vec<crate::shopify::types::Product>> {
    let ml_futures: Vec<_> = product_ids
        .iter()
        .map(|product_id| {
            let product_id = product_id.clone();
            let storefront = state.storefront().clone();
            async move {
                storefront
                    .get_product_recommendations(
                        &product_id,
                        Some(ProductRecommendationIntent::Complementary),
                    )
                    .await
                    .unwrap_or_default()
            }
        })
        .collect();

    future::join_all(ml_futures).await
}

/// Score recommendations: manual config gets higher score than ML.
fn score_recommendations(
    cart_product_ids: &HashSet<&str>,
    manual_product_ids: &[String],
    ml_results: Vec<Vec<crate::shopify::types::Product>>,
) -> HashMap<String, (Option<crate::shopify::types::Product>, u32)> {
    let mut scores: HashMap<String, (Option<crate::shopify::types::Product>, u32)> = HashMap::new();

    // Manual recommendations get bonus score (10 points)
    for product_id in manual_product_ids {
        if !cart_product_ids.contains(product_id.as_str()) {
            scores
                .entry(product_id.clone())
                .and_modify(|(_, score)| *score += 10)
                .or_insert((None, 10));
        }
    }

    // ML recommendations get 1 point each
    for products in ml_results {
        for product in products {
            if !cart_product_ids.contains(product.id.as_str()) {
                scores
                    .entry(product.id.clone())
                    .and_modify(|(existing, score)| {
                        *score += 1;
                        if existing.is_none() {
                            *existing = Some(product.clone());
                        }
                    })
                    .or_insert((Some(product), 1));
            }
        }
    }

    scores
}

// =============================================================================
// GWP Product Title Enrichment
// =============================================================================

/// Enrich GWP `AutoAdd` suggestions with product titles from the Storefront API.
///
/// The discount matcher returns `AutoAdd` suggestions with empty product titles
/// (to avoid storing titles in the metafield). This function fetches the actual
/// titles from Shopify for display in the UI.
async fn enrich_gwp_product_titles(
    storefront: &crate::shopify::StorefrontClient,
    mut suggestions: Vec<DiscountSuggestion>,
) -> Vec<DiscountSuggestion> {
    use futures::future;

    // Collect product IDs that need title fetching
    let product_ids_to_fetch: Vec<(usize, String)> = suggestions
        .iter()
        .enumerate()
        .filter_map(|(idx, s)| {
            if let Some(ref variant_id) = s.gwp_auto_add_variant_id {
                // Only fetch if title is empty and we have a product ID
                if s.gwp_auto_add_product_title
                    .as_ref()
                    .is_none_or(String::is_empty)
                {
                    // Extract product ID from variant ID or use the product_id field
                    // gwp_auto_add_product_id should be set alongside variant_id
                    if let Some(ref product_id) = s.gwp_auto_add_product_id {
                        return Some((idx, product_id.clone()));
                    }
                }
                // If we have a variant but no product ID, we can't fetch
                let _ = variant_id; // suppress unused warning
            }
            None
        })
        .collect();

    if product_ids_to_fetch.is_empty() {
        return suggestions;
    }

    // Fetch titles concurrently
    let futures: Vec<_> = product_ids_to_fetch
        .iter()
        .map(|(_, product_id)| {
            let storefront = storefront.clone();
            let product_id = product_id.clone();
            async move { storefront.get_product_title_by_id(&product_id).await.ok() }
        })
        .collect();

    let titles = future::join_all(futures).await;

    // Update suggestions with fetched titles
    for ((idx, _), title) in product_ids_to_fetch.into_iter().zip(titles) {
        if let (Some(suggestion), Some(title)) = (suggestions.get_mut(idx), title) {
            suggestion.gwp_auto_add_product_title = Some(title);
        }
    }

    suggestions
}
