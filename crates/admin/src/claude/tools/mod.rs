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
mod collections_low_level_shopify;
mod customers_low_level_shopify;
mod discounts_low_level_shopify;
mod finance_low_level_shopify;
mod fulfillment_low_level_shopify;
mod gift_cards_low_level_shopify;
mod inventory_low_level_shopify;
mod order_editing_low_level_shopify;
mod orders_low_level_shopify;
mod products_low_level_shopify;

pub use analytics::analytics_tools;
pub use collections_low_level_shopify::collection_tools;
pub use customers_low_level_shopify::customer_tools;
pub use discounts_low_level_shopify::discount_tools;
pub use finance_low_level_shopify::finance_tools;
pub use fulfillment_low_level_shopify::fulfillment_tools;
pub use gift_cards_low_level_shopify::gift_card_tools;
pub use inventory_low_level_shopify::inventory_tools;
pub use order_editing_low_level_shopify::order_editing_tools;
pub use orders_low_level_shopify::order_tools;
pub use products_low_level_shopify::product_tools;

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

/// Get all tools for a specific domain, with low-level Shopify tools sorted last.
#[must_use]
pub fn get_tools_by_domain(domain: &str) -> Vec<Tool> {
    let mut tools: Vec<Tool> = all_shopify_tools()
        .into_iter()
        .filter(|t| t.domain.as_deref() == Some(domain))
        .collect();
    sort_tools_high_level_first(&mut tools);
    tools
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

/// Filter tools by names, with low-level Shopify tools sorted last.
///
/// This ensures high-level analytics tools appear before low-level API tools
/// when tools are presented to the LLM.
#[must_use]
pub fn filter_tools_by_names(names: &[String]) -> Vec<Tool> {
    let all = all_shopify_tools();
    let mut tools: Vec<Tool> = names
        .iter()
        .filter_map(|name| all.iter().find(|t| &t.name == name).cloned())
        .collect();
    sort_tools_high_level_first(&mut tools);
    tools
}

/// Sort tools so that high-level analytics tools come first and
/// low-level Shopify API tools (ending in `_low_level_shopify`) come last.
fn sort_tools_high_level_first(tools: &mut [Tool]) {
    tools.sort_by(|a, b| {
        let a_is_low_level = a.name.ends_with("_low_level_shopify");
        let b_is_low_level = b.name.ends_with("_low_level_shopify");
        a_is_low_level.cmp(&b_is_low_level)
    });
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
            "get_order_low_level_shopify" => self.get_order(input).await,
            "get_orders_low_level_shopify" => self.get_orders(input).await,
            "get_order_detail_low_level_shopify" => self.get_order_detail(input).await,
            "get_orders_list_low_level_shopify" => self.get_orders_list(input).await,

            // Customers (read)
            "get_customer_low_level_shopify" => self.get_customer(input).await,
            "get_customers_low_level_shopify" => self.get_customers(input).await,
            "generate_customer_activation_url_low_level_shopify" => {
                self.generate_customer_activation_url(input).await
            }
            "get_customer_segments_low_level_shopify" => self.get_customer_segments(input).await,

            // Products (read)
            "get_product_low_level_shopify" => self.get_product(input).await,
            "get_products_low_level_shopify" => self.get_products(input).await,

            // Inventory (read)
            "get_locations_low_level_shopify" => self.get_locations().await,
            "get_inventory_levels_low_level_shopify" => self.get_inventory_levels(input).await,
            "get_inventory_items_low_level_shopify" => self.get_inventory_items(input).await,
            "get_inventory_item_low_level_shopify" => self.get_inventory_item(input).await,

            // Collections (read)
            "get_collection_low_level_shopify" => self.get_collection(input).await,
            "get_collections_low_level_shopify" => self.get_collections(input).await,
            "get_collection_with_products_low_level_shopify" => {
                self.get_collection_with_products(input).await
            }
            "get_publications_low_level_shopify" => self.get_publications().await,

            // Discounts (read)
            "get_discounts_low_level_shopify" => self.get_discounts(input).await,
            "get_discount_low_level_shopify" => self.get_discount(input).await,
            "get_discounts_for_list_low_level_shopify" => self.get_discounts_for_list(input).await,

            // Gift Cards (read)
            "get_gift_cards_low_level_shopify" => self.get_gift_cards(input).await,
            "get_gift_cards_count_low_level_shopify" => self.get_gift_cards_count(input).await,
            "get_gift_card_detail_low_level_shopify" => self.get_gift_card_detail(input).await,
            "get_gift_card_configuration_low_level_shopify" => {
                self.get_gift_card_configuration().await
            }

            // Fulfillment (read)
            "get_fulfillment_orders_low_level_shopify" => self.get_fulfillment_orders(input).await,
            "get_suggested_refund_low_level_shopify" => self.get_suggested_refund(input).await,

            // Finance (read)
            "get_payouts_low_level_shopify" => self.get_payouts(input).await,
            "get_payout_low_level_shopify" => self.get_payout(input).await,
            "get_payout_detail_low_level_shopify" => self.get_payout_detail(input).await,
            "get_payout_transactions_low_level_shopify" => {
                self.get_payout_transactions(input).await
            }
            "get_payout_schedule_low_level_shopify" => self.get_payout_schedule().await,
            "get_bank_accounts_low_level_shopify" => self.get_bank_accounts().await,
            "get_disputes_low_level_shopify" => self.get_disputes(input).await,
            "get_dispute_low_level_shopify" => self.get_dispute(input).await,

            // Order Editing (read - begin only)
            "order_edit_begin_low_level_shopify" => self.order_edit_begin(input).await,

            _ => Err(ClaudeError::ToolExecution(format!("Unknown tool: {name}"))),
        }
    }

    /// Execute a write operation.
    async fn execute_write(
        &self,
        name: &str,
        input: &serde_json::Value,
    ) -> Result<String, ClaudeError> {
        // Try each domain's write operations
        if let Some(result) = self.execute_write_orders(name, input).await {
            return result;
        }
        if let Some(result) = self.execute_write_customers(name, input).await {
            return result;
        }
        if let Some(result) = self.execute_write_products(name, input).await {
            return result;
        }
        if let Some(result) = self.execute_write_inventory(name, input).await {
            return result;
        }
        if let Some(result) = self.execute_write_collections(name, input).await {
            return result;
        }
        if let Some(result) = self.execute_write_discounts(name, input).await {
            return result;
        }
        if let Some(result) = self.execute_write_gift_cards(name, input).await {
            return result;
        }
        if let Some(result) = self.execute_write_fulfillment(name, input).await {
            return result;
        }
        if let Some(result) = self.execute_write_order_editing(name, input).await {
            return result;
        }

        Err(ClaudeError::ToolExecution(format!(
            "Unknown write tool: {name}"
        )))
    }

    /// Execute order write operations.
    async fn execute_write_orders(
        &self,
        name: &str,
        input: &serde_json::Value,
    ) -> Option<Result<String, ClaudeError>> {
        let result = match name {
            "update_order_note_low_level_shopify" => self.update_order_note(input).await,
            "update_order_tags_low_level_shopify" => self.update_order_tags(input).await,
            "mark_order_as_paid_low_level_shopify" => self.mark_order_as_paid(input).await,
            "cancel_order_low_level_shopify" => self.cancel_order(input).await,
            "archive_order_low_level_shopify" => self.archive_order(input).await,
            "unarchive_order_low_level_shopify" => self.unarchive_order(input).await,
            "capture_order_payment_low_level_shopify" => self.capture_order_payment(input).await,
            "add_tags_to_order_low_level_shopify" => self.add_tags_to_order(input).await,
            "remove_tags_from_order_low_level_shopify" => self.remove_tags_from_order(input).await,
            _ => return None,
        };
        Some(result)
    }

    /// Execute customer write operations.
    async fn execute_write_customers(
        &self,
        name: &str,
        input: &serde_json::Value,
    ) -> Option<Result<String, ClaudeError>> {
        let result = match name {
            "create_customer_low_level_shopify" => self.create_customer(input).await,
            "update_customer_low_level_shopify" => self.update_customer(input).await,
            "delete_customer_low_level_shopify" => self.delete_customer(input).await,
            "add_customer_tags_low_level_shopify" => self.add_customer_tags(input).await,
            "remove_customer_tags_low_level_shopify" => self.remove_customer_tags(input).await,
            "send_customer_invite_low_level_shopify" => self.send_customer_invite(input).await,
            "create_customer_address_low_level_shopify" => {
                self.create_customer_address(input).await
            }
            "update_customer_address_low_level_shopify" => {
                self.update_customer_address(input).await
            }
            "delete_customer_address_low_level_shopify" => {
                self.delete_customer_address(input).await
            }
            "set_customer_default_address_low_level_shopify" => {
                self.set_customer_default_address(input).await
            }
            "update_customer_email_marketing_low_level_shopify" => {
                self.update_customer_email_marketing(input).await
            }
            "update_customer_sms_marketing_low_level_shopify" => {
                self.update_customer_sms_marketing(input).await
            }
            "merge_customers_low_level_shopify" => self.merge_customers(input).await,
            _ => return None,
        };
        Some(result)
    }

    /// Execute product write operations.
    async fn execute_write_products(
        &self,
        name: &str,
        input: &serde_json::Value,
    ) -> Option<Result<String, ClaudeError>> {
        let result = match name {
            "create_product_low_level_shopify" => self.create_product(input).await,
            "update_product_low_level_shopify" => self.update_product(input).await,
            "delete_product_low_level_shopify" => self.delete_product(input).await,
            "update_variant_low_level_shopify" => self.update_variant(input).await,
            _ => return None,
        };
        Some(result)
    }

    /// Execute inventory write operations.
    async fn execute_write_inventory(
        &self,
        name: &str,
        input: &serde_json::Value,
    ) -> Option<Result<String, ClaudeError>> {
        let result = match name {
            "adjust_inventory_low_level_shopify" => self.adjust_inventory(input).await,
            "set_inventory_low_level_shopify" => self.set_inventory(input).await,
            "update_inventory_item_low_level_shopify" => self.update_inventory_item(input).await,
            "move_inventory_low_level_shopify" => self.move_inventory(input).await,
            "activate_inventory_low_level_shopify" => self.activate_inventory(input).await,
            "deactivate_inventory_low_level_shopify" => self.deactivate_inventory(input).await,
            _ => return None,
        };
        Some(result)
    }

    /// Execute collection write operations.
    async fn execute_write_collections(
        &self,
        name: &str,
        input: &serde_json::Value,
    ) -> Option<Result<String, ClaudeError>> {
        let result = match name {
            "create_collection_low_level_shopify" => self.create_collection(input).await,
            "update_collection_low_level_shopify" => self.update_collection(input).await,
            "update_collection_sort_order_low_level_shopify" => {
                self.update_collection_sort_order(input).await
            }
            "delete_collection_low_level_shopify" => self.delete_collection(input).await,
            "update_collection_image_low_level_shopify" => {
                self.update_collection_image(input).await
            }
            "delete_collection_image_low_level_shopify" => {
                self.delete_collection_image(input).await
            }
            "add_products_to_collection_low_level_shopify" => {
                self.add_products_to_collection(input).await
            }
            "remove_products_from_collection_low_level_shopify" => {
                self.remove_products_from_collection(input).await
            }
            "reorder_collection_products_low_level_shopify" => {
                self.reorder_collection_products(input).await
            }
            "publish_collection_low_level_shopify" => self.publish_collection(input).await,
            "unpublish_collection_low_level_shopify" => self.unpublish_collection(input).await,
            _ => return None,
        };
        Some(result)
    }

    /// Execute discount write operations.
    async fn execute_write_discounts(
        &self,
        name: &str,
        input: &serde_json::Value,
    ) -> Option<Result<String, ClaudeError>> {
        let result = match name {
            "create_discount_low_level_shopify" => self.create_discount(input).await,
            "update_discount_low_level_shopify" => self.update_discount(input).await,
            "deactivate_discount_low_level_shopify" => self.deactivate_discount(input).await,
            "activate_discount_low_level_shopify" => self.activate_discount(input).await,
            "deactivate_automatic_discount_low_level_shopify" => {
                self.deactivate_automatic_discount(input).await
            }
            "delete_discount_low_level_shopify" => self.delete_discount(input).await,
            "bulk_activate_code_discounts_low_level_shopify" => {
                self.bulk_activate_code_discounts(input).await
            }
            "bulk_deactivate_code_discounts_low_level_shopify" => {
                self.bulk_deactivate_code_discounts(input).await
            }
            "bulk_delete_code_discounts_low_level_shopify" => {
                self.bulk_delete_code_discounts(input).await
            }
            _ => return None,
        };
        Some(result)
    }

    /// Execute gift card write operations.
    async fn execute_write_gift_cards(
        &self,
        name: &str,
        input: &serde_json::Value,
    ) -> Option<Result<String, ClaudeError>> {
        let result = match name {
            "create_gift_card_low_level_shopify" => self.create_gift_card(input).await,
            "deactivate_gift_card_low_level_shopify" => self.deactivate_gift_card(input).await,
            "update_gift_card_low_level_shopify" => self.update_gift_card(input).await,
            "credit_gift_card_low_level_shopify" => self.credit_gift_card(input).await,
            "debit_gift_card_low_level_shopify" => self.debit_gift_card(input).await,
            "send_gift_card_notification_to_customer_low_level_shopify" => {
                self.send_gift_card_notification_to_customer(input).await
            }
            "send_gift_card_notification_to_recipient_low_level_shopify" => {
                self.send_gift_card_notification_to_recipient(input).await
            }
            _ => return None,
        };
        Some(result)
    }

    /// Execute fulfillment write operations.
    async fn execute_write_fulfillment(
        &self,
        name: &str,
        input: &serde_json::Value,
    ) -> Option<Result<String, ClaudeError>> {
        let result = match name {
            "create_fulfillment_low_level_shopify" => self.create_fulfillment(input).await,
            "update_fulfillment_tracking_low_level_shopify" => {
                self.update_fulfillment_tracking(input).await
            }
            "hold_fulfillment_order_low_level_shopify" => self.hold_fulfillment_order(input).await,
            "release_fulfillment_order_hold_low_level_shopify" => {
                self.release_fulfillment_order_hold(input).await
            }
            "create_refund_low_level_shopify" => self.create_refund(input).await,
            "create_return_low_level_shopify" => self.create_return(input).await,
            _ => return None,
        };
        Some(result)
    }

    /// Execute order editing write operations.
    async fn execute_write_order_editing(
        &self,
        name: &str,
        input: &serde_json::Value,
    ) -> Option<Result<String, ClaudeError>> {
        let result = match name {
            "order_edit_add_variant_low_level_shopify" => self.order_edit_add_variant(input).await,
            "order_edit_add_custom_item_low_level_shopify" => {
                self.order_edit_add_custom_item(input).await
            }
            "order_edit_set_quantity_low_level_shopify" => {
                self.order_edit_set_quantity(input).await
            }
            "order_edit_add_line_item_discount_low_level_shopify" => {
                self.order_edit_add_line_item_discount(input).await
            }
            "order_edit_update_discount_low_level_shopify" => {
                self.order_edit_update_discount(input).await
            }
            "order_edit_remove_discount_low_level_shopify" => {
                self.order_edit_remove_discount(input).await
            }
            "order_edit_add_shipping_line_low_level_shopify" => {
                self.order_edit_add_shipping_line(input).await
            }
            "order_edit_update_shipping_line_low_level_shopify" => {
                self.order_edit_update_shipping_line(input).await
            }
            "order_edit_remove_shipping_line_low_level_shopify" => {
                self.order_edit_remove_shipping_line(input).await
            }
            "order_edit_commit_low_level_shopify" => self.order_edit_commit(input).await,
            _ => return None,
        };
        Some(result)
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
mod executor_low_level_shopify;
