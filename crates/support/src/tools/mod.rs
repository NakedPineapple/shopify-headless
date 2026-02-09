//! Customer support tool definitions for Claude.
//!
//! Tools available to the support chat AI:
//! - `lookup_faq` - Targeted knowledge base retrieval
//! - `lookup_product` - Product info via Shopify Storefront API
//! - `lookup_order_status` - Order status (authenticated only)
//! - `lookup_subscription` - Subscription details (authenticated only)
//! - `request_human_help` - Escalate to human agent

pub mod context;
pub mod faq;
pub mod handoff;
pub mod order;
pub mod product;
pub mod subscription;

pub use context::ToolContext;
use naked_pineapple_services::claude::types::Tool;

/// Build the list of tools available to the support chat AI.
///
/// When `is_authenticated` is true, auth-only tools (order lookup, subscription,
/// human handoff) are included. Anonymous users only get FAQ and product tools.
#[must_use]
pub fn support_tools(is_authenticated: bool) -> Vec<Tool> {
    let mut tools = vec![faq::tool_definition(), product::tool_definition()];

    if is_authenticated {
        tools.push(order::tool_definition());
        tools.push(subscription::tool_definition());
    }

    // Always include handoff - the tool itself handles the auth check
    tools.push(handoff::tool_definition());

    tools
}
