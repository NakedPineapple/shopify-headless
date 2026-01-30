//! Shopify tool definitions organized by domain.
//!
//! This module provides two tiers of tools:
//!
//! **High-level analytics tools (12 total):**
//! Summarized, aggregate data for answering business questions.
//! These return compact responses ideal for questions like "what's our revenue?"
//!
//! **Low-level Shopify API tools (111 total):**
//! - 38 read operations (execute immediately)
//! - 73 write operations (require confirmation via Slack)
//!
//! These return detailed data for specific lookups and modifications.

mod analytics;
mod collections;
mod customers;
mod discounts;
mod finance;
mod fulfillment;
mod gift_cards;
mod inventory;
mod order_editing;
mod orders;
mod products;

pub use analytics::analytics_tools;
pub use collections::collection_tools;
pub use customers::customer_tools;
pub use discounts::discount_tools;
pub use finance::finance_tools;
pub use fulfillment::fulfillment_tools;
pub use gift_cards::gift_card_tools;
pub use inventory::inventory_tools;
pub use order_editing::order_editing_tools;
pub use orders::order_tools;
pub use products::product_tools;

use serde_json::json;
use tracing::instrument;

use crate::shopify::AdminClient;

use super::error::ClaudeError;
use super::types::Tool;

/// Get all tools (126 total: 15 high-level analytics + 111 low-level Shopify).
///
/// High-level analytics tools are listed first as they should be preferred
/// for answering common business questions.
#[must_use]
pub fn all_shopify_tools() -> Vec<Tool> {
    let mut tools = Vec::with_capacity(126);
    // High-level analytics tools (preferred for business questions)
    tools.extend(analytics_tools());
    // Low-level Shopify API tools (for specific lookups and modifications)
    tools.extend(order_tools());
    tools.extend(customer_tools());
    tools.extend(product_tools());
    tools.extend(inventory_tools());
    tools.extend(collection_tools());
    tools.extend(discount_tools());
    tools.extend(gift_card_tools());
    tools.extend(fulfillment_tools());
    tools.extend(finance_tools());
    tools.extend(order_editing_tools());
    tools
}

/// Get a tool by name.
#[must_use]
pub fn get_tool_by_name(name: &str) -> Option<Tool> {
    all_shopify_tools().into_iter().find(|t| t.name == name)
}

/// Get all tools for a specific domain.
#[must_use]
pub fn get_tools_by_domain(domain: &str) -> Vec<Tool> {
    all_shopify_tools()
        .into_iter()
        .filter(|t| t.domain.as_deref() == Some(domain))
        .collect()
}

/// Get tool names from a list of tools.
#[must_use]
pub fn get_tool_names(tools: &[Tool]) -> Vec<&str> {
    tools.iter().map(|t| t.name.as_str()).collect()
}

/// Check if a tool requires confirmation (write operation).
#[must_use]
pub fn requires_confirmation(tool_name: &str) -> bool {
    get_tool_by_name(tool_name).is_some_and(|t| t.requires_confirmation)
}

/// Get the domain for a tool.
#[must_use]
pub fn get_tool_domain(tool_name: &str) -> Option<String> {
    get_tool_by_name(tool_name).and_then(|t| t.domain)
}

/// Filter tools by names.
#[must_use]
pub fn filter_tools_by_names(names: &[String]) -> Vec<Tool> {
    let all = all_shopify_tools();
    names
        .iter()
        .filter_map(|name| all.iter().find(|t| &t.name == name).cloned())
        .collect()
}

/// Executor for Shopify tools.
///
/// Handles tool execution by mapping tool names to Shopify Admin API calls.
/// Write operations return a pending status for confirmation flow.
pub struct ToolExecutor<'a> {
    shopify: &'a AdminClient,
}

impl<'a> ToolExecutor<'a> {
    /// Create a new tool executor.
    #[must_use]
    pub const fn new(shopify: &'a AdminClient) -> Self {
        Self { shopify }
    }

    /// Execute a tool and return the result as a string.
    ///
    /// # Arguments
    ///
    /// * `name` - Tool name
    /// * `input` - Tool input parameters
    ///
    /// # Returns
    ///
    /// For read operations: the tool result as JSON.
    /// For write operations that need confirmation: a pending status message.
    ///
    /// # Errors
    ///
    /// Returns an error if the tool execution fails.
    #[instrument(skip(self, input), fields(tool_name = %name))]
    pub async fn execute(
        &self,
        name: &str,
        input: &serde_json::Value,
    ) -> Result<ToolResult, ClaudeError> {
        // Check if this tool requires confirmation
        if requires_confirmation(name) {
            return Ok(ToolResult::RequiresConfirmation {
                tool_name: name.to_string(),
                input: input.clone(),
                domain: get_tool_domain(name).unwrap_or_default(),
            });
        }

        // Execute read operation immediately
        let result = self.execute_read(name, input).await?;
        Ok(ToolResult::Success(result))
    }

    /// Execute a write operation after confirmation.
    ///
    /// This should only be called after Slack approval.
    ///
    /// # Errors
    ///
    /// Returns an error if the tool execution fails or the tool is not found.
    #[instrument(skip(self, input), fields(tool_name = %name))]
    pub async fn execute_confirmed(
        &self,
        name: &str,
        input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        self.execute_write(name, input).await
    }

    /// Execute a read-only operation.
    async fn execute_read(
        &self,
        name: &str,
        input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        match name {
            // High-level analytics tools (15 total)
            "get_sales_summary" => self.get_sales_summary(input).await,
            "get_sales_by_channel" => self.get_sales_by_channel(input).await,
            "get_sales_by_product" => self.get_sales_by_product(input).await,
            "get_sales_by_location" => self.get_sales_by_location(input).await,
            "get_sales_by_discount" => self.get_sales_by_discount(input).await,
            "get_order_summary" => self.get_order_summary(input).await,
            "get_customer_summary" => self.get_customer_summary(input).await,
            "get_top_customers" => self.get_top_customers(input).await,
            "get_customers_by_location" => self.get_customers_by_location(input).await,
            "get_product_catalog" => self.get_product_catalog(input).await,
            "get_inventory_summary" => self.get_inventory_summary(input).await,
            "get_profit_summary" => self.get_profit_summary(input).await,
            "get_payout_summary" => self.get_payout_summary(input).await,
            "get_gift_card_summary" => self.get_gift_card_summary(input).await,
            "get_fulfillment_summary" => self.get_fulfillment_summary(input).await,

            // Low-level Shopify API tools
            // Orders (read)
            "get_order" => self.get_order(input).await,
            "get_orders" => self.get_orders(input).await,
            "get_order_detail" => self.get_order_detail(input).await,
            "get_orders_list" => self.get_orders_list(input).await,

            // Customers (read)
            "get_customer" => self.get_customer(input).await,
            "get_customers" => self.get_customers(input).await,
            "generate_customer_activation_url" => {
                self.generate_customer_activation_url(input).await
            }
            "get_customer_segments" => self.get_customer_segments(input).await,

            // Products (read)
            "get_product" => self.get_product(input).await,
            "get_products" => self.get_products(input).await,

            // Inventory (read)
            "get_locations" => self.get_locations().await,
            "get_inventory_levels" => self.get_inventory_levels(input).await,
            "get_inventory_items" => self.get_inventory_items(input).await,
            "get_inventory_item" => self.get_inventory_item(input).await,

            // Collections (read)
            "get_collection" => self.get_collection(input).await,
            "get_collections" => self.get_collections(input).await,
            "get_collection_with_products" => self.get_collection_with_products(input).await,
            "get_publications" => self.get_publications().await,

            // Discounts (read)
            "get_discounts" => self.get_discounts(input).await,
            "get_discount" => self.get_discount(input).await,
            "get_discounts_for_list" => self.get_discounts_for_list(input).await,

            // Gift Cards (read)
            "get_gift_cards" => self.get_gift_cards(input).await,
            "get_gift_cards_count" => self.get_gift_cards_count(input).await,
            "get_gift_card_detail" => self.get_gift_card_detail(input).await,
            "get_gift_card_configuration" => self.get_gift_card_configuration().await,

            // Fulfillment (read)
            "get_fulfillment_orders" => self.get_fulfillment_orders(input).await,
            "get_suggested_refund" => self.get_suggested_refund(input).await,

            // Finance (read)
            "get_payouts" => self.get_payouts(input).await,
            "get_payout" => self.get_payout(input).await,
            "get_payout_detail" => self.get_payout_detail(input).await,
            "get_payout_transactions" => self.get_payout_transactions(input).await,
            "get_payout_schedule" => self.get_payout_schedule().await,
            "get_bank_accounts" => self.get_bank_accounts().await,
            "get_disputes" => self.get_disputes(input).await,
            "get_dispute" => self.get_dispute(input).await,

            // Order Editing (read - begin only)
            "order_edit_begin" => self.order_edit_begin(input).await,

            _ => Err(ClaudeError::ToolExecution(format!("Unknown tool: {name}"))),
        }
    }

    /// Execute a write operation.
    async fn execute_write(
        &self,
        name: &str,
        input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        match name {
            // Orders (write)
            "update_order_note" => self.update_order_note(input).await,
            "update_order_tags" => self.update_order_tags(input).await,
            "mark_order_as_paid" => self.mark_order_as_paid(input).await,
            "cancel_order" => self.cancel_order(input).await,
            "archive_order" => self.archive_order(input).await,
            "unarchive_order" => self.unarchive_order(input).await,
            "capture_order_payment" => self.capture_order_payment(input).await,
            "add_tags_to_order" => self.add_tags_to_order(input).await,
            "remove_tags_from_order" => self.remove_tags_from_order(input).await,

            // Customers (write)
            "create_customer" => self.create_customer(input).await,
            "update_customer" => self.update_customer(input).await,
            "delete_customer" => self.delete_customer(input).await,
            "add_customer_tags" => self.add_customer_tags(input).await,
            "remove_customer_tags" => self.remove_customer_tags(input).await,
            "send_customer_invite" => self.send_customer_invite(input).await,
            "create_customer_address" => self.create_customer_address(input).await,
            "update_customer_address" => self.update_customer_address(input).await,
            "delete_customer_address" => self.delete_customer_address(input).await,
            "set_customer_default_address" => self.set_customer_default_address(input).await,
            "update_customer_email_marketing" => self.update_customer_email_marketing(input).await,
            "update_customer_sms_marketing" => self.update_customer_sms_marketing(input).await,
            "merge_customers" => self.merge_customers(input).await,

            // Products (write)
            "create_product" => self.create_product(input).await,
            "update_product" => self.update_product(input).await,
            "delete_product" => self.delete_product(input).await,
            "update_variant" => self.update_variant(input).await,

            // Inventory (write)
            "adjust_inventory" => self.adjust_inventory(input).await,
            "set_inventory" => self.set_inventory(input).await,
            "update_inventory_item" => self.update_inventory_item(input).await,
            "move_inventory" => self.move_inventory(input).await,
            "activate_inventory" => self.activate_inventory(input).await,
            "deactivate_inventory" => self.deactivate_inventory(input).await,

            // Collections (write)
            "create_collection" => self.create_collection(input).await,
            "update_collection" => self.update_collection(input).await,
            "update_collection_sort_order" => self.update_collection_sort_order(input).await,
            "delete_collection" => self.delete_collection(input).await,
            "update_collection_image" => self.update_collection_image(input).await,
            "delete_collection_image" => self.delete_collection_image(input).await,
            "add_products_to_collection" => self.add_products_to_collection(input).await,
            "remove_products_from_collection" => self.remove_products_from_collection(input).await,
            "reorder_collection_products" => self.reorder_collection_products(input).await,
            "publish_collection" => self.publish_collection(input).await,
            "unpublish_collection" => self.unpublish_collection(input).await,

            // Discounts (write)
            "create_discount" => self.create_discount(input).await,
            "update_discount" => self.update_discount(input).await,
            "deactivate_discount" => self.deactivate_discount(input).await,
            "activate_discount" => self.activate_discount(input).await,
            "deactivate_automatic_discount" => self.deactivate_automatic_discount(input).await,
            "delete_discount" => self.delete_discount(input).await,
            "bulk_activate_code_discounts" => self.bulk_activate_code_discounts(input).await,
            "bulk_deactivate_code_discounts" => self.bulk_deactivate_code_discounts(input).await,
            "bulk_delete_code_discounts" => self.bulk_delete_code_discounts(input).await,

            // Gift Cards (write)
            "create_gift_card" => self.create_gift_card(input).await,
            "deactivate_gift_card" => self.deactivate_gift_card(input).await,
            "update_gift_card" => self.update_gift_card(input).await,
            "credit_gift_card" => self.credit_gift_card(input).await,
            "debit_gift_card" => self.debit_gift_card(input).await,
            "send_gift_card_notification_to_customer" => {
                self.send_gift_card_notification_to_customer(input).await
            }
            "send_gift_card_notification_to_recipient" => {
                self.send_gift_card_notification_to_recipient(input).await
            }

            // Fulfillment (write)
            "create_fulfillment" => self.create_fulfillment(input).await,
            "update_fulfillment_tracking" => self.update_fulfillment_tracking(input).await,
            "hold_fulfillment_order" => self.hold_fulfillment_order(input).await,
            "release_fulfillment_order_hold" => self.release_fulfillment_order_hold(input).await,
            "create_refund" => self.create_refund(input).await,
            "create_return" => self.create_return(input).await,

            // Order Editing (write)
            "order_edit_add_variant" => self.order_edit_add_variant(input).await,
            "order_edit_add_custom_item" => self.order_edit_add_custom_item(input).await,
            "order_edit_set_quantity" => self.order_edit_set_quantity(input).await,
            "order_edit_add_line_item_discount" => {
                self.order_edit_add_line_item_discount(input).await
            }
            "order_edit_update_discount" => self.order_edit_update_discount(input).await,
            "order_edit_remove_discount" => self.order_edit_remove_discount(input).await,
            "order_edit_add_shipping_line" => self.order_edit_add_shipping_line(input).await,
            "order_edit_update_shipping_line" => self.order_edit_update_shipping_line(input).await,
            "order_edit_remove_shipping_line" => self.order_edit_remove_shipping_line(input).await,
            "order_edit_commit" => self.order_edit_commit(input).await,

            _ => Err(ClaudeError::ToolExecution(format!(
                "Unknown write tool: {name}"
            ))),
        }
    }
}

/// Result of tool execution.
#[derive(Debug, Clone)]
pub enum ToolResult {
    /// Tool executed successfully.
    Success(String),
    /// Tool requires confirmation before execution.
    RequiresConfirmation {
        /// Name of the tool.
        tool_name: String,
        /// Input parameters.
        input: serde_json::Value,
        /// Domain of the tool.
        domain: String,
    },
}

impl ToolResult {
    /// Check if this result requires confirmation.
    #[must_use]
    pub const fn requires_confirmation(&self) -> bool {
        matches!(self, Self::RequiresConfirmation { .. })
    }

    /// Get the result string if successful.
    #[must_use]
    pub fn success_result(&self) -> Option<&str> {
        match self {
            Self::Success(s) => Some(s),
            Self::RequiresConfirmation { .. } => None,
        }
    }
}

// Include the executor implementations
mod analytics_executor;
mod executor;
