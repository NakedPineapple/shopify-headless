//! Integration tests for admin AI chat Slack message building.
//!
//! These tests verify that Slack Block Kit messages are built correctly
//! for various action states.

use serde_json::json;
use uuid::Uuid;

use naked_pineapple_admin::slack::{
    Block, build_approved_message, build_confirmation_message, build_error_message,
    build_rejected_message, build_timeout_message,
};

// =============================================================================
// Confirmation Message Tests
// =============================================================================

#[test]
fn test_confirmation_message_structure() {
    let action_id = Uuid::new_v4();
    let blocks = build_confirmation_message(
        action_id,
        "cancel_order",
        &json!({"order_id": "gid://shopify/Order/123"}),
        "Test Admin",
        "orders",
    );

    // Should have multiple blocks
    assert!(blocks.len() >= 4, "Should have at least 4 blocks");

    // First block should be header
    let first = blocks.first().expect("blocks not empty");
    assert!(matches!(first, Block::Header { .. }));
}

#[test]
fn test_confirmation_message_has_approve_reject_buttons() {
    let action_id = Uuid::new_v4();
    let blocks = build_confirmation_message(
        action_id,
        "delete_customer",
        &json!({"customer_id": "gid://shopify/Customer/456"}),
        "Admin User",
        "customers",
    );

    // Find the Actions block
    let actions_block = blocks.iter().find(|b| matches!(b, Block::Actions { .. }));
    assert!(actions_block.is_some(), "Should have an Actions block");

    if let Some(Block::Actions { elements }) = actions_block {
        assert_eq!(elements.len(), 2, "Should have approve and reject buttons");
    }
}

#[test]
fn test_confirmation_message_contains_tool_name() {
    let action_id = Uuid::new_v4();
    let blocks = build_confirmation_message(
        action_id,
        "create_discount",
        &json!({"code": "SUMMER20"}),
        "Marketing Admin",
        "discounts",
    );

    // Serialize to check content
    let json_str = serde_json::to_string(&blocks).expect("Should serialize");
    assert!(
        json_str.contains("create_discount"),
        "Message should contain tool name"
    );
}

#[test]
fn test_confirmation_message_contains_admin_name() {
    let action_id = Uuid::new_v4();
    let blocks = build_confirmation_message(
        action_id,
        "update_product",
        &json!({"product_id": "123"}),
        "Product Manager",
        "products",
    );

    let json_str = serde_json::to_string(&blocks).expect("Should serialize");
    assert!(
        json_str.contains("Product Manager"),
        "Message should contain admin name"
    );
}

// =============================================================================
// Approved Message Tests
// =============================================================================

#[test]
fn test_approved_message_structure() {
    let blocks = build_approved_message("cancel_order", "Approver Name", Some("Order cancelled"));

    assert!(!blocks.is_empty(), "Should have blocks");

    // First block should be header
    let first = blocks.first().expect("blocks not empty");
    assert!(matches!(first, Block::Header { .. }));
}

#[test]
fn test_approved_message_contains_approver() {
    let blocks = build_approved_message("adjust_inventory", "Warehouse Manager", None);

    let json_str = serde_json::to_string(&blocks).expect("Should serialize");
    assert!(
        json_str.contains("Warehouse Manager"),
        "Should contain approver name"
    );
}

#[test]
fn test_approved_message_with_result() {
    let blocks = build_approved_message(
        "create_fulfillment",
        "Shipping Admin",
        Some("Fulfillment created: FUL-123"),
    );

    let json_str = serde_json::to_string(&blocks).expect("Should serialize");
    assert!(
        json_str.contains("FUL-123"),
        "Should contain result when provided"
    );
}

#[test]
fn test_approved_message_without_result() {
    let blocks = build_approved_message("archive_order", "Support Agent", None);

    // Should still be valid even without result
    assert!(!blocks.is_empty());
}

// =============================================================================
// Rejected Message Tests
// =============================================================================

#[test]
fn test_rejected_message_structure() {
    let blocks = build_rejected_message("delete_product", "Store Owner");

    assert!(!blocks.is_empty(), "Should have blocks");

    // First block should be header
    let first = blocks.first().expect("blocks not empty");
    assert!(matches!(first, Block::Header { .. }));
}

#[test]
fn test_rejected_message_contains_rejecter() {
    let blocks = build_rejected_message("merge_customers", "Admin");

    let json_str = serde_json::to_string(&blocks).expect("Should serialize");
    assert!(json_str.contains("Admin"), "Should contain rejecter name");
}

#[test]
fn test_rejected_message_contains_tool_name() {
    let blocks = build_rejected_message("bulk_delete_code_discounts", "Marketing Lead");

    let json_str = serde_json::to_string(&blocks).expect("Should serialize");
    assert!(
        json_str.contains("bulk_delete_code_discounts"),
        "Should contain tool name"
    );
}

// =============================================================================
// Timeout Message Tests
// =============================================================================

#[test]
fn test_timeout_message_structure() {
    let blocks = build_timeout_message("create_refund");

    assert!(!blocks.is_empty(), "Should have blocks");

    // First block should be header
    let first = blocks.first().expect("blocks not empty");
    assert!(matches!(first, Block::Header { .. }));
}

#[test]
fn test_timeout_message_contains_tool_name() {
    let blocks = build_timeout_message("hold_fulfillment_order");

    let json_str = serde_json::to_string(&blocks).expect("Should serialize");
    assert!(
        json_str.contains("hold_fulfillment_order"),
        "Should contain tool name"
    );
}

#[test]
fn test_timeout_message_indicates_expiry() {
    let blocks = build_timeout_message("deactivate_gift_card");

    let json_str = serde_json::to_string(&blocks).expect("Should serialize");
    // Should mention expiry or timeout
    assert!(
        json_str.contains("expired") || json_str.contains("Expired"),
        "Should indicate expiration"
    );
}

// =============================================================================
// Error Message Tests
// =============================================================================

#[test]
fn test_error_message_structure() {
    let blocks = build_error_message("update_variant", "Invalid variant ID");

    assert!(!blocks.is_empty(), "Should have blocks");

    // First block should be header
    let first = blocks.first().expect("blocks not empty");
    assert!(matches!(first, Block::Header { .. }));
}

#[test]
fn test_error_message_contains_error() {
    let error_text = "GraphQL error: Product not found";
    let blocks = build_error_message("update_product", error_text);

    let json_str = serde_json::to_string(&blocks).expect("Should serialize");
    assert!(
        json_str.contains("Product not found"),
        "Should contain error message"
    );
}

#[test]
fn test_error_message_contains_tool_name() {
    let blocks = build_error_message("create_collection", "Duplicate handle");

    let json_str = serde_json::to_string(&blocks).expect("Should serialize");
    assert!(
        json_str.contains("create_collection"),
        "Should contain tool name"
    );
}

// =============================================================================
// Block Serialization Tests
// =============================================================================

#[test]
fn test_all_message_types_serialize_to_valid_json() {
    let action_id = Uuid::new_v4();

    let messages = vec![
        (
            "confirmation",
            build_confirmation_message(action_id, "test_tool", &json!({}), "Test", "orders"),
        ),
        (
            "approved",
            build_approved_message("test_tool", "Approver", None),
        ),
        ("rejected", build_rejected_message("test_tool", "Rejecter")),
        ("timeout", build_timeout_message("test_tool")),
        ("error", build_error_message("test_tool", "Error text")),
    ];

    for (name, blocks) in messages {
        let result = serde_json::to_string(&blocks);
        assert!(
            result.is_ok(),
            "{name} message should serialize to valid JSON"
        );

        // Also verify it can be parsed back
        let json_str = result.expect("serialization succeeded");
        let parsed: serde_json::Value = serde_json::from_str(&json_str)
            .unwrap_or_else(|_| panic!("{name} message JSON should be parseable"));

        assert!(
            parsed.is_array(),
            "{name} message should serialize to array"
        );
    }
}
