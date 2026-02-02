//! Discount matching service for cart-based discount suggestions.
//!
//! Evaluates cart contents against qualifying rules from the shop metafield
//! to show customers their progress toward automatic discounts.
//!
//! ## Architecture
//!
//! The active promotions metafield separates concerns:
//! - `ProgressTracking` - Display configuration (icons, templates, colors)
//! - `QualifyingRule` - Matching logic from Shopify API (requirements, items)
//!
//! This service joins them by `discount_id` to create `DiscountSuggestion` entries.

// Template placeholders like "{needed}" are intentional, not format strings
#![allow(clippy::literal_string_with_formatting_args)]

use rust_decimal::Decimal;
use rust_decimal::prelude::*;

use crate::routes::cart::CartItemView;
use crate::shopify::types::{
    BuysRequirementType, Bxgy, CustomerBuys, CustomerGets, CustomerGetsItems,
    CustomerGetsValueType, DiscountItems, DiscountItemsType, FreeShipping, MinimumRequirement,
    MinimumRequirementType, ProgressTracking, QualifyingRule, QualifyingRuleType,
};

/// Core qualification result from evaluating a rule against cart contents.
/// Separate from display configuration - captures just the discount logic.
#[derive(Debug, Clone)]
struct QualificationResult {
    rule_id: String,
    rule_type: QualifyingRuleType,
    is_qualified: bool,
    progress_percent: u8,
    /// For quantity-based rules: how many more items needed.
    needed_quantity: Option<u32>,
    /// For amount-based rules: how much more spend needed.
    needed_amount: Option<Decimal>,
    /// GWP action when qualified (for BXGY rules).
    gwp_action: Option<GwpAction>,
    /// Whether this GWP has already been claimed.
    gwp_claimed: bool,
}

/// Gift-with-purchase action when a BXGY discount is qualified.
#[derive(Debug, Clone)]
pub enum GwpAction {
    /// Auto-add a specific item (single product in `customer_gets`).
    AutoAdd {
        /// Variant GID to add to cart.
        variant_id: String,
        /// Product GID for tracking.
        product_id: String,
        /// Product title for display.
        product_title: String,
    },
    /// Prompt user to choose from specific products.
    PromptSelection {
        /// Product IDs to choose from (need to fetch full product data).
        product_ids: Vec<String>,
    },
    /// Prompt user to browse a collection.
    BrowseCollection {
        /// Collection ID to browse.
        collection_id: String,
    },
}

/// Result of matching a cart against a qualifying rule.
#[derive(Debug, Clone)]
pub struct DiscountSuggestion {
    /// Rule ID for tracking (`discount_id`).
    pub rule_id: String,
    /// Display name derived from rule type.
    pub display_name: String,
    /// Icon name for UI.
    pub icon: String,
    /// Accent color for UI.
    pub accent_color: String,
    /// Progress percentage (0-100).
    pub progress_percent: u8,
    /// Whether the customer qualifies.
    pub is_qualified: bool,
    /// Suggestion or qualified text to display.
    pub message: String,
    /// Badge text to display above the message.
    pub badge_text: Option<String>,
    /// Whether to show a progress bar.
    pub show_progress_bar: bool,
    /// Optional CTA button text.
    pub cta_text: Option<String>,
    /// Optional CTA button URL.
    pub cta_url: Option<String>,
    /// GWP action when qualified (only for BXGY rules).
    pub gwp_action: Option<GwpAction>,
    /// Whether this GWP has already been claimed (item is in cart).
    pub gwp_claimed: bool,

    // Flattened GWP fields for template access (Askama can't match on enum variants)
    /// For `AutoAdd`: the variant ID to add.
    pub gwp_auto_add_variant_id: Option<String>,
    /// For `AutoAdd`: the product ID (for fetching title).
    pub gwp_auto_add_product_id: Option<String>,
    /// For `AutoAdd`: the product title.
    pub gwp_auto_add_product_title: Option<String>,
    /// For `PromptSelection`: the product IDs to choose from.
    pub gwp_selection_product_ids: Vec<String>,
    /// For `BrowseCollection`: the collection ID.
    pub gwp_browse_collection_id: Option<String>,
}

/// Result of matching cart against all qualifying rules.
#[derive(Debug, Clone)]
pub struct DiscountMatchResult {
    /// Suggestions to display (excludes hidden qualified rules).
    pub suggestions: Vec<DiscountSuggestion>,
    /// Whether the customer qualifies for free shipping.
    pub qualifies_for_free_shipping: bool,
}

/// Match cart items against discount qualifying rules.
///
/// Joins `progress_tracking` (display config) with `qualifying_rules` (matching logic)
/// by `discount_id` to create suggestions.
///
/// Returns suggestions sorted by priority (unqualified items first to encourage
/// completion) and a flag indicating if the customer qualifies for free shipping.
///
/// Free shipping rules are always hidden when qualified - instead, the cart
/// displays "Free" in the shipping line.
#[must_use]
pub fn match_qualifying_rules(
    cart_items: &[CartItemView],
    cart_subtotal: &str,
    progress_tracking: &[ProgressTracking],
    qualifying_rules: &[QualifyingRule],
) -> DiscountMatchResult {
    tracing::debug!(
        cart_item_count = cart_items.len(),
        cart_subtotal = %cart_subtotal,
        progress_tracking_count = progress_tracking.len(),
        qualifying_rules_count = qualifying_rules.len(),
        "match_qualifying_rules called"
    );

    let mut suggestions = Vec::new();
    let mut qualifies_for_free_shipping = false;

    // Evaluate ALL qualifying rules - progress_tracking is optional UI config
    for rule in qualifying_rules {
        // Step 1: Evaluate the rule (pure discount logic, no display concerns)
        let Some(qualification) = evaluate_rule_core(cart_items, cart_subtotal, rule) else {
            tracing::debug!(rule_id = %rule.id, "Rule evaluation returned None");
            continue;
        };

        tracing::debug!(
            rule_id = %rule.id,
            is_qualified = qualification.is_qualified,
            progress = qualification.progress_percent,
            gwp_action = ?qualification.gwp_action,
            "Rule evaluated successfully"
        );

        // Step 2: Handle free shipping qualification flag
        if rule.rule_type == QualifyingRuleType::FreeShipping && qualification.is_qualified {
            qualifies_for_free_shipping = true;
            // Don't add to suggestions - show "Free" in shipping line instead
            continue;
        }

        // Step 3: Find optional progress_tracking for display customization
        let tracking = progress_tracking.iter().find(|pt| pt.id == rule.id);

        // Step 4: Decide whether to include in suggestions
        let hide_when_qualified = tracking.is_some_and(|t| t.hide_when_qualified);
        let gwp_already_claimed = qualification.gwp_claimed;

        if qualification.is_qualified {
            // Hide qualified rules when:
            // - Configured to hide when qualified (and no unclaimed GWP), OR
            // - Has a GWP action that's already been claimed
            let has_unclaimed_gwp = qualification.gwp_action.is_some() && !gwp_already_claimed;

            if (hide_when_qualified && !has_unclaimed_gwp) || gwp_already_claimed {
                continue;
            }
        }

        // Step 5: Build display suggestion from qualification + optional tracking
        let suggestion = build_suggestion_from_qualification(qualification, tracking, cart_items);

        suggestions.push(suggestion);
    }

    // Sort by: unqualified first (to show progress), then by priority
    suggestions.sort_by(|a, b| match (a.is_qualified, b.is_qualified) {
        (false, true) => std::cmp::Ordering::Less,
        (true, false) => std::cmp::Ordering::Greater,
        _ => std::cmp::Ordering::Equal,
    });

    DiscountMatchResult {
        suggestions,
        qualifies_for_free_shipping,
    }
}

/// Evaluate a qualifying rule against cart contents.
/// Returns pure qualification data with no display logic.
fn evaluate_rule_core(
    cart_items: &[CartItemView],
    cart_subtotal: &str,
    rule: &QualifyingRule,
) -> Option<QualificationResult> {
    match rule.rule_type {
        QualifyingRuleType::FreeShipping => {
            let fs = rule.free_shipping.as_ref()?;
            evaluate_free_shipping_core(cart_items, cart_subtotal, rule, fs)
        }
        QualifyingRuleType::BuyXGetY => {
            let bxgy = rule.bxgy.as_ref()?;
            evaluate_bxgy_core(cart_items, cart_subtotal, rule, bxgy)
        }
        QualifyingRuleType::AmountOffProducts | QualifyingRuleType::AmountOffOrder => {
            let min_req = rule
                .amount_off_products
                .as_ref()
                .and_then(|a| a.minimum_requirement.as_ref())
                .or_else(|| {
                    rule.amount_off_order
                        .as_ref()
                        .and_then(|a| a.minimum_requirement.as_ref())
                });

            min_req.map_or_else(
                || {
                    // No minimum requirement = always qualified
                    Some(QualificationResult {
                        rule_id: rule.id.clone(),
                        rule_type: rule.rule_type,
                        is_qualified: true,
                        progress_percent: 100,
                        needed_quantity: None,
                        needed_amount: None,
                        gwp_action: None,
                        gwp_claimed: false,
                    })
                },
                |requirement| {
                    evaluate_minimum_requirement_core(cart_items, cart_subtotal, rule, requirement)
                },
            )
        }
    }
}

/// Build a display suggestion from a qualification result and optional tracking config.
fn build_suggestion_from_qualification(
    qual: QualificationResult,
    tracking: Option<&ProgressTracking>,
    cart_items: &[CartItemView],
) -> DiscountSuggestion {
    // Display name based on rule type
    let display_name = match qual.rule_type {
        QualifyingRuleType::FreeShipping => "Free Shipping".to_string(),
        QualifyingRuleType::BuyXGetY => "Gift with Purchase".to_string(),
        QualifyingRuleType::AmountOffProducts => "Product Discount".to_string(),
        QualifyingRuleType::AmountOffOrder => "Order Discount".to_string(),
    };

    // Use tracking config if available, otherwise use defaults
    let (icon, accent_color, show_progress_bar, cta_text, cta_url) = if let Some(t) = tracking {
        (
            t.icon.clone(),
            t.accent_color.clone(),
            t.show_progress_bar,
            t.cta_text.clone(),
            t.cta_url.clone(),
        )
    } else {
        let icon = match qual.rule_type {
            QualifyingRuleType::FreeShipping => "truck",
            QualifyingRuleType::BuyXGetY => "gift",
            _ => "percent",
        };
        (icon.to_string(), "honey".to_string(), false, None, None)
    };

    // Build message from tracking templates or use defaults
    let (message, badge_text) = if let Some(t) = tracking {
        let msg = if qual.is_qualified {
            t.qualified_template.clone()
        } else {
            let needed_str = qual
                .needed_quantity
                .map(|n| n.to_string())
                .or_else(|| qual.needed_amount.map(format_currency))
                .unwrap_or_default();
            t.suggestion_template.replace("{needed}", &needed_str)
        };
        let badge = if qual.is_qualified {
            t.qualified_badge_text.clone()
        } else {
            t.suggestion_badge_text.clone()
        };
        (msg, badge)
    } else {
        // Default messages when no tracking configured
        let msg = if qual.is_qualified {
            match qual.rule_type {
                QualifyingRuleType::BuyXGetY => "You qualify for a free gift!".to_string(),
                QualifyingRuleType::FreeShipping => "You qualify for free shipping!".to_string(),
                _ => "Discount applied!".to_string(),
            }
        } else {
            String::new() // No progress message without tracking config
        };
        (msg, None)
    };

    // Check if GWP has been claimed
    let gwp_claimed = cart_items
        .iter()
        .any(|item| item.gwp_rule_id.as_deref() == Some(&qual.rule_id));

    // Flatten GWP action for template access
    let (
        gwp_auto_add_variant_id,
        gwp_auto_add_product_id,
        gwp_auto_add_product_title,
        gwp_selection_product_ids,
        gwp_browse_collection_id,
    ) = match &qual.gwp_action {
        Some(GwpAction::AutoAdd {
            variant_id,
            product_id,
            product_title,
        }) => (
            Some(variant_id.clone()),
            Some(product_id.clone()),
            Some(product_title.clone()),
            Vec::new(),
            None,
        ),
        Some(GwpAction::PromptSelection { product_ids }) => {
            (None, None, None, product_ids.clone(), None)
        }
        Some(GwpAction::BrowseCollection { collection_id }) => {
            (None, None, None, Vec::new(), Some(collection_id.clone()))
        }
        None => (None, None, None, Vec::new(), None),
    };

    DiscountSuggestion {
        rule_id: qual.rule_id,
        display_name,
        icon,
        accent_color,
        progress_percent: qual.progress_percent,
        is_qualified: qual.is_qualified,
        message,
        badge_text,
        show_progress_bar,
        cta_text,
        cta_url,
        gwp_action: qual.gwp_action,
        gwp_claimed,
        gwp_auto_add_variant_id,
        gwp_auto_add_product_id,
        gwp_auto_add_product_title,
        gwp_selection_product_ids,
        gwp_browse_collection_id,
    }
}

// =============================================================================
// Core Evaluation Functions (no display logic)
// =============================================================================

/// Evaluate a BXGY rule - core logic without display concerns.
fn evaluate_bxgy_core(
    cart_items: &[CartItemView],
    cart_subtotal: &str,
    rule: &QualifyingRule,
    bxgy: &Bxgy,
) -> Option<QualificationResult> {
    tracing::debug!(rule_id = %rule.id, ?bxgy, "evaluate_bxgy_core called");

    let buys = &bxgy.customer_buys;

    // Filter cart items to only those that qualify
    let qualifying_items: Vec<_> = cart_items
        .iter()
        .filter(|item| matches_discount_items(&item.product_id, buys.items.as_ref()))
        .collect();

    tracing::debug!(
        rule_id = %rule.id,
        cart_item_count = cart_items.len(),
        qualifying_item_count = qualifying_items.len(),
        requirement_type = ?buys.requirement_type,
        "Filtered qualifying items"
    );

    let (is_qualified, progress, needed_quantity, needed_amount) = match buys.requirement_type {
        BuysRequirementType::Quantity => {
            let required = buys.quantity.unwrap_or(1);
            let current: u32 = qualifying_items.iter().map(|i| i.quantity).sum();
            let is_qualified = current >= required;
            let progress = calculate_quantity_progress(current, required);
            let needed = if is_qualified {
                None
            } else {
                Some(required.saturating_sub(current))
            };
            (is_qualified, progress, needed, None)
        }
        BuysRequirementType::Amount => {
            let required = parse_decimal(buys.amount.as_deref()?)?;
            let current: Decimal = if buys.items.is_none() {
                parse_price(cart_subtotal)?
            } else {
                qualifying_items
                    .iter()
                    .filter_map(|item| parse_price(&item.line_price))
                    .sum()
            };
            let is_qualified = current >= required;
            let progress = calculate_progress(current, required);
            let needed = if is_qualified {
                None
            } else {
                Some(required - current)
            };
            (is_qualified, progress, None, needed)
        }
    };

    tracing::debug!(
        rule_id = %rule.id,
        is_qualified,
        progress,
        ?needed_quantity,
        ?needed_amount,
        "BXGY evaluation result"
    );

    // Determine GWP action when qualified
    let gwp_action = if is_qualified {
        tracing::debug!(rule_id = %rule.id, "Customer qualified, determining GWP action");
        let action = determine_gwp_action(&bxgy.customer_gets);
        tracing::debug!(rule_id = %rule.id, ?action, "GWP action determined");
        action
    } else {
        None
    };

    // Check if GWP has been claimed
    let gwp_claimed = cart_items
        .iter()
        .any(|item| item.gwp_rule_id.as_deref() == Some(&rule.id));

    Some(QualificationResult {
        rule_id: rule.id.clone(),
        rule_type: rule.rule_type,
        is_qualified,
        progress_percent: progress,
        needed_quantity,
        needed_amount,
        gwp_action,
        gwp_claimed,
    })
}

/// Evaluate a free shipping rule - core logic without display concerns.
fn evaluate_free_shipping_core(
    cart_items: &[CartItemView],
    cart_subtotal: &str,
    rule: &QualifyingRule,
    fs: &FreeShipping,
) -> Option<QualificationResult> {
    let requirement = fs.minimum_requirement.as_ref()?;
    evaluate_minimum_requirement_core(cart_items, cart_subtotal, rule, requirement)
}

/// Evaluate a minimum requirement - core logic without display concerns.
fn evaluate_minimum_requirement_core(
    cart_items: &[CartItemView],
    cart_subtotal: &str,
    rule: &QualifyingRule,
    requirement: &MinimumRequirement,
) -> Option<QualificationResult> {
    match requirement.requirement_type {
        MinimumRequirementType::None => Some(QualificationResult {
            rule_id: rule.id.clone(),
            rule_type: rule.rule_type,
            is_qualified: true,
            progress_percent: 100,
            needed_quantity: None,
            needed_amount: None,
            gwp_action: None,
            gwp_claimed: false,
        }),
        MinimumRequirementType::Quantity => {
            let required = requirement.quantity.unwrap_or(1);
            let current: u32 = cart_items.iter().map(|i| i.quantity).sum();
            let is_qualified = current >= required;
            let progress = calculate_quantity_progress(current, required);
            let needed = if is_qualified {
                None
            } else {
                Some(required.saturating_sub(current))
            };
            Some(QualificationResult {
                rule_id: rule.id.clone(),
                rule_type: rule.rule_type,
                is_qualified,
                progress_percent: progress,
                needed_quantity: needed,
                needed_amount: None,
                gwp_action: None,
                gwp_claimed: false,
            })
        }
        MinimumRequirementType::Amount => {
            let required = parse_decimal(requirement.amount.as_deref()?)?;
            let current = parse_price(cart_subtotal)?;
            let is_qualified = current >= required;
            let progress = calculate_progress(current, required);
            let needed = if is_qualified {
                None
            } else {
                Some(required - current)
            };
            Some(QualificationResult {
                rule_id: rule.id.clone(),
                rule_type: rule.rule_type,
                is_qualified,
                progress_percent: progress,
                needed_quantity: None,
                needed_amount: needed,
                gwp_action: None,
                gwp_claimed: false,
            })
        }
    }
}

/// Determine the GWP action based on what the customer gets.
fn determine_gwp_action(customer_gets: &CustomerGets) -> Option<GwpAction> {
    tracing::debug!(?customer_gets, "determine_gwp_action called");

    // Only handle FREE items as GWP (100% discount or explicit Free type)
    let is_free = customer_gets
        .discount_value
        .as_ref()
        .is_some_and(|dv| dv.value_type == CustomerGetsValueType::Free);

    tracing::debug!(is_free, "Checked if discount is free");

    if !is_free {
        // Not a free gift, no GWP action needed
        tracing::debug!("Not a free gift, returning None");
        return None;
    }

    let Some(items) = customer_gets.items.as_ref() else {
        tracing::debug!("No items in customer_gets, returning None");
        return None;
    };

    tracing::debug!(?items, "Processing customer_gets items");

    match items.items_type {
        DiscountItemsType::SpecificProducts => {
            let Some(products) = items.products.as_ref() else {
                tracing::debug!("items.products is None, returning None");
                return None;
            };
            tracing::debug!(product_count = products.len(), ?products, "Found products");
            match products.as_slice() {
                [] => {
                    tracing::debug!("Empty products array, returning None");
                    None
                }
                [single_product] => {
                    // Single product with variant: auto-add
                    // product_title will be fetched by cart handler
                    tracing::debug!(
                        variant_id = %single_product.variant_id,
                        product_id = %single_product.product_id,
                        "Single product - returning AutoAdd"
                    );
                    Some(GwpAction::AutoAdd {
                        variant_id: single_product.variant_id.clone(),
                        product_id: single_product.product_id.clone(),
                        product_title: String::new(), // Populated by cart handler
                    })
                }
                _ => {
                    // Multiple products: prompt selection
                    let product_ids: Vec<_> =
                        products.iter().map(|p| p.product_id.clone()).collect();
                    tracing::debug!(
                        ?product_ids,
                        "Multiple products - returning PromptSelection"
                    );
                    Some(GwpAction::PromptSelection { product_ids })
                }
            }
        }
        DiscountItemsType::SpecificCollections => {
            let collection_ids = items.collection_ids.as_ref()?;
            tracing::debug!(?collection_ids, "Returning BrowseCollection");
            collection_ids
                .first()
                .map(|id| GwpAction::BrowseCollection {
                    collection_id: id.clone(),
                })
        }
    }
}

/// Check if a product ID matches the discount items specification.
fn matches_discount_items(product_id: &str, items: Option<&DiscountItems>) -> bool {
    let Some(items) = items else {
        return true; // No items restriction = all products match
    };

    match items.items_type {
        DiscountItemsType::SpecificProducts => items
            .product_ids
            .as_ref()
            .is_some_and(|ids| ids.iter().any(|id| id == product_id)),
        DiscountItemsType::SpecificCollections => {
            // TODO: Check if product is in any of the collections
            // For now, we can't verify this client-side without additional data
            true
        }
    }
}

/// Parse a price string (e.g., "$50.00" or "50.00") to Decimal.
fn parse_price(price: &str) -> Option<Decimal> {
    let cleaned = price.trim_start_matches('$').replace(',', "");
    Decimal::from_str(&cleaned).ok()
}

/// Parse a decimal string to Decimal.
fn parse_decimal(s: &str) -> Option<Decimal> {
    Decimal::from_str(s).ok()
}

/// Calculate progress percentage (capped at 100).
fn calculate_progress(current: Decimal, required: Decimal) -> u8 {
    if required.is_zero() {
        return 100;
    }
    let progress = (current / required) * Decimal::from(100);
    progress.to_u8().map_or(100, |p| std::cmp::min(p, 100))
}

/// Calculate quantity progress percentage (capped at 100).
fn calculate_quantity_progress(current: u32, required: u32) -> u8 {
    if required == 0 {
        return 100;
    }
    #[expect(clippy::cast_possible_truncation, reason = "result capped at 100")]
    let progress = ((u64::from(current) * 100) / u64::from(required)) as u8;
    std::cmp::min(progress, 100)
}

/// Format a Decimal as currency (e.g., "$50.00").
fn format_currency(amount: Decimal) -> String {
    format!("${amount:.2}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shopify::types::{CustomerGetsDiscountValue, CustomerGetsValueType};

    fn make_cart_item(product_id: &str, quantity: u32, line_price: &str) -> CartItemView {
        CartItemView {
            id: "line_1".to_string(),
            product_id: product_id.to_string(),
            handle: "test".to_string(),
            title: "Test Product".to_string(),
            variant_title: None,
            quantity,
            price: "$10.00".to_string(),
            line_price: line_price.to_string(),
            image: None,
            is_gwp: false,
            gwp_rule_id: None,
        }
    }

    fn make_tracking(id: &str) -> ProgressTracking {
        ProgressTracking {
            id: id.to_string(),
            icon: "gift".to_string(),
            accent_color: "honey".to_string(),
            cta_text: None,
            cta_url: None,
            suggestion_template: "Add {needed} more to qualify!".to_string(),
            suggestion_badge_text: None,
            qualified_template: "You qualify!".to_string(),
            qualified_badge_text: None,
            priority: 0,
            show_progress_bar: true,
            hide_when_qualified: false,
        }
    }

    #[test]
    fn test_free_shipping_threshold() {
        let tracking = ProgressTracking {
            suggestion_template: "Add {needed} more for free shipping!".to_string(),
            qualified_template: "You qualify for free shipping!".to_string(),
            ..make_tracking("free-shipping")
        };

        let rule = QualifyingRule {
            id: "free-shipping".to_string(),
            rule_type: QualifyingRuleType::FreeShipping,
            starts_at: None,
            ends_at: None,
            combines_with: None,
            amount_off_products: None,
            amount_off_order: None,
            bxgy: None,
            free_shipping: Some(FreeShipping {
                minimum_requirement: Some(MinimumRequirement {
                    requirement_type: MinimumRequirementType::Amount,
                    amount: Some("50.00".to_string()),
                    quantity: None,
                }),
            }),
        };

        // Test below threshold - shows progress suggestion
        let result = match_qualifying_rules(
            &[],
            "$30.00",
            std::slice::from_ref(&tracking),
            std::slice::from_ref(&rule),
        );
        assert!(!result.qualifies_for_free_shipping);
        assert_eq!(result.suggestions.len(), 1);
        let first = result
            .suggestions
            .first()
            .expect("should have one suggestion");
        assert!(!first.is_qualified);
        assert_eq!(first.progress_percent, 60);
        assert!(first.message.contains("$20.00"));

        // Test at threshold - hides suggestion, sets qualifies_for_free_shipping
        let result = match_qualifying_rules(
            &[],
            "$50.00",
            std::slice::from_ref(&tracking),
            std::slice::from_ref(&rule),
        );
        assert!(result.qualifies_for_free_shipping);
        assert!(
            result.suggestions.is_empty(),
            "free shipping should hide when qualified"
        );
    }

    #[test]
    fn test_bxgy_quantity() {
        let tracking = ProgressTracking {
            suggestion_template: "Add {needed} more item(s) to qualify!".to_string(),
            qualified_template: "You qualify for a free item!".to_string(),
            cta_text: Some("Shop Now".to_string()),
            cta_url: Some("/collections/skincare".to_string()),
            ..make_tracking("bogo")
        };

        let rule = QualifyingRule {
            id: "bogo".to_string(),
            rule_type: QualifyingRuleType::BuyXGetY,
            starts_at: None,
            ends_at: None,
            combines_with: None,
            amount_off_products: None,
            amount_off_order: None,
            bxgy: Some(Bxgy {
                customer_buys: CustomerBuys {
                    requirement_type: BuysRequirementType::Quantity,
                    quantity: Some(2),
                    amount: None,
                    items: None, // All products
                },
                customer_gets: CustomerGets {
                    quantity: 1,
                    items: None,
                    discount_value: Some(CustomerGetsDiscountValue {
                        value_type: CustomerGetsValueType::Free,
                        percentage: None,
                        amount: None,
                    }),
                },
            }),
            free_shipping: None,
        };

        let cart_items = vec![make_cart_item("product_1", 1, "$10.00")];
        let result = match_qualifying_rules(
            &cart_items,
            "$10.00",
            std::slice::from_ref(&tracking),
            std::slice::from_ref(&rule),
        );
        let first = result
            .suggestions
            .first()
            .expect("should have one suggestion");
        assert!(!first.is_qualified);
        assert_eq!(first.progress_percent, 50);
        assert!(first.message.contains('1'));

        let cart_items = vec![make_cart_item("product_1", 2, "$20.00")];
        let result = match_qualifying_rules(
            &cart_items,
            "$20.00",
            std::slice::from_ref(&tracking),
            std::slice::from_ref(&rule),
        );
        let first = result
            .suggestions
            .first()
            .expect("should have one suggestion");
        assert!(first.is_qualified);
    }

    #[test]
    fn test_product_specific_discount() {
        let tracking = ProgressTracking {
            suggestion_template: "Add {needed} more skincare item(s)!".to_string(),
            ..make_tracking("skincare-bogo")
        };

        let rule = QualifyingRule {
            id: "skincare-bogo".to_string(),
            rule_type: QualifyingRuleType::BuyXGetY,
            starts_at: None,
            ends_at: None,
            combines_with: None,
            amount_off_products: None,
            amount_off_order: None,
            bxgy: Some(Bxgy {
                customer_buys: CustomerBuys {
                    requirement_type: BuysRequirementType::Quantity,
                    quantity: Some(2),
                    amount: None,
                    items: Some(DiscountItems {
                        items_type: DiscountItemsType::SpecificProducts,
                        product_ids: Some(vec!["skincare_1".to_string(), "skincare_2".to_string()]),
                        collection_ids: None,
                    }),
                },
                customer_gets: CustomerGets {
                    quantity: 1,
                    items: None,
                    discount_value: Some(CustomerGetsDiscountValue {
                        value_type: CustomerGetsValueType::Free,
                        percentage: None,
                        amount: None,
                    }),
                },
            }),
            free_shipping: None,
        };

        // Non-qualifying product doesn't count
        let cart_items = vec![make_cart_item("other_product", 5, "$50.00")];
        let result = match_qualifying_rules(
            &cart_items,
            "$50.00",
            std::slice::from_ref(&tracking),
            std::slice::from_ref(&rule),
        );
        let first = result
            .suggestions
            .first()
            .expect("should have one suggestion");
        assert!(!first.is_qualified);
        assert_eq!(first.progress_percent, 0);

        // Qualifying product counts
        let cart_items = vec![
            make_cart_item("skincare_1", 1, "$10.00"),
            make_cart_item("skincare_2", 1, "$10.00"),
        ];
        let result = match_qualifying_rules(
            &cart_items,
            "$20.00",
            std::slice::from_ref(&tracking),
            std::slice::from_ref(&rule),
        );
        let first = result
            .suggestions
            .first()
            .expect("should have one suggestion");
        assert!(first.is_qualified);
    }
}
