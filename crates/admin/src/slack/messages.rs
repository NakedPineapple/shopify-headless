//! Slack message builders for the AI chat confirmation flow.
//!
//! Provides factory functions for building Block Kit messages for:
//! - Tool execution confirmation requests
//! - Approval/rejection confirmations
//! - Timeout notifications

use uuid::Uuid;

use super::types::{ActionElement, Block, ButtonStyle, ContextElement, PlainText, Text};

/// Build a confirmation request message for a pending tool execution.
///
/// The message includes:
/// - Header with tool icon
/// - Tool name and description
/// - Input parameters (formatted)
/// - Requested by context
/// - Approve/Reject buttons
#[must_use]
pub fn build_confirmation_message(
    action_id: Uuid,
    tool_name: &str,
    tool_input: &serde_json::Value,
    admin_name: &str,
    domain: &str,
) -> Vec<Block> {
    let emoji = domain_emoji(domain);
    let formatted_input = format_tool_input(tool_input);

    vec![
        // Header
        Block::Header {
            text: PlainText::new(format!("{emoji} AI Action Request")),
        },
        // Tool name
        Block::Section {
            text: Text::mrkdwn(format!("*Tool:* `{tool_name}`")),
            accessory: None,
        },
        // Input parameters
        Block::Section {
            text: Text::mrkdwn(format!("*Parameters:*\n```\n{formatted_input}\n```")),
            accessory: None,
        },
        // Context: who requested
        Block::Context {
            elements: vec![ContextElement::Mrkdwn {
                text: format!("Requested by *{admin_name}* • Just now"),
            }],
        },
        // Divider
        Block::Divider,
        // Action buttons
        Block::Actions {
            elements: vec![
                ActionElement::Button {
                    text: PlainText::new("Approve"),
                    action_id: format!("approve_{action_id}"),
                    value: Some(action_id.to_string()),
                    style: Some(ButtonStyle::Primary),
                },
                ActionElement::Button {
                    text: PlainText::new("Reject"),
                    action_id: format!("reject_{action_id}"),
                    value: Some(action_id.to_string()),
                    style: Some(ButtonStyle::Danger),
                },
            ],
        },
    ]
}

/// Build an approval confirmation message (replaces the original).
#[must_use]
pub fn build_approved_message(
    tool_name: &str,
    approved_by: &str,
    result_summary: Option<&str>,
) -> Vec<Block> {
    let mut blocks = vec![
        Block::Header {
            text: PlainText::new("✅ Action Approved"),
        },
        Block::Section {
            text: Text::mrkdwn(format!("*Tool:* `{tool_name}`")),
            accessory: None,
        },
        Block::Context {
            elements: vec![ContextElement::Mrkdwn {
                text: format!("Approved by *{approved_by}*"),
            }],
        },
    ];

    if let Some(summary) = result_summary {
        blocks.push(Block::Section {
            text: Text::mrkdwn(format!("*Result:*\n```\n{summary}\n```")),
            accessory: None,
        });
    }

    blocks
}

/// Build a rejection confirmation message (replaces the original).
#[must_use]
pub fn build_rejected_message(tool_name: &str, rejected_by: &str) -> Vec<Block> {
    vec![
        Block::Header {
            text: PlainText::new("❌ Action Rejected"),
        },
        Block::Section {
            text: Text::mrkdwn(format!("*Tool:* `{tool_name}`")),
            accessory: None,
        },
        Block::Context {
            elements: vec![ContextElement::Mrkdwn {
                text: format!("Rejected by *{rejected_by}*"),
            }],
        },
    ]
}

/// Build a timeout message (replaces the original).
#[must_use]
pub fn build_timeout_message(tool_name: &str) -> Vec<Block> {
    vec![
        Block::Header {
            text: PlainText::new("⏰ Action Expired"),
        },
        Block::Section {
            text: Text::mrkdwn(format!("*Tool:* `{tool_name}`")),
            accessory: None,
        },
        Block::Context {
            elements: vec![ContextElement::Mrkdwn {
                text: "This action request has expired and was not executed.".to_string(),
            }],
        },
    ]
}

/// Build an error message for failed execution.
#[must_use]
pub fn build_error_message(tool_name: &str, error: &str) -> Vec<Block> {
    vec![
        Block::Header {
            text: PlainText::new("⚠️ Action Failed"),
        },
        Block::Section {
            text: Text::mrkdwn(format!("*Tool:* `{tool_name}`")),
            accessory: None,
        },
        Block::Section {
            text: Text::mrkdwn(format!("*Error:*\n```\n{error}\n```")),
            accessory: None,
        },
    ]
}

/// Get an emoji for a tool domain.
fn domain_emoji(domain: &str) -> &'static str {
    match domain {
        "orders" => "📦",
        "customers" => "👤",
        "products" => "🏷️",
        "inventory" => "📊",
        "collections" => "📁",
        "discounts" => "🎟️",
        "gift_cards" => "🎁",
        "fulfillment" => "🚚",
        "finance" => "💰",
        "order_editing" => "✏️",
        _ => "🔧",
    }
}

/// Format tool input as a readable string.
fn format_tool_input(input: &serde_json::Value) -> String {
    // Pretty-print JSON, but limit length
    let formatted = serde_json::to_string_pretty(input).unwrap_or_else(|_| input.to_string());

    // Truncate if too long (Slack has limits)
    if formatted.len() > 2000 {
        format!("{}...\n(truncated)", &formatted[..2000])
    } else {
        formatted
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_confirmation_message_has_buttons() {
        let action_id = Uuid::new_v4();
        let blocks = build_confirmation_message(
            action_id,
            "cancel_order",
            &json!({"id": "order_123"}),
            "Adam",
            "orders",
        );

        // Should have header, two sections, context, divider, and actions
        assert_eq!(blocks.len(), 6);

        // Last block should be actions
        let last_block = blocks.get(5).expect("Expected 6 blocks");
        match last_block {
            Block::Actions { elements } => {
                assert_eq!(elements.len(), 2);
            }
            _ => panic!("Expected Actions block"),
        }
    }

    #[test]
    fn test_domain_emoji() {
        assert_eq!(domain_emoji("orders"), "📦");
        assert_eq!(domain_emoji("customers"), "👤");
        assert_eq!(domain_emoji("unknown"), "🔧");
    }
}
