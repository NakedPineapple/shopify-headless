//! Integration tests for admin AI chat tool selection.
//!
//! These tests verify the tool selection system works correctly
//! without requiring actual API calls.

use naked_pineapple_admin::claude::tools::{
    all_shopify_tools, filter_tools_by_names, get_tool_by_name, get_tool_domain,
    get_tools_by_domain, requires_confirmation,
};
use naked_pineapple_admin::tool_selection::{
    DOMAINS, ToolExampleConfig, ToolExamplesConfig, validate_config,
};

// =============================================================================
// Tool Registry Tests
// =============================================================================

#[test]
fn test_all_tools_count() {
    let tools = all_shopify_tools();
    // 38 read + 73 write = 111 total
    assert_eq!(tools.len(), 111, "Should have 111 tools total");
}

#[test]
fn test_read_tools_dont_require_confirmation() {
    // Sample of read tools that should not require confirmation
    let read_tools = [
        "get_order",
        "get_orders",
        "get_customer",
        "get_products",
        "get_inventory_levels",
        "get_collections",
        "get_discounts",
        "get_payouts",
    ];

    for tool_name in read_tools {
        assert!(
            !requires_confirmation(tool_name),
            "{tool_name} should not require confirmation"
        );
    }
}

#[test]
fn test_write_tools_require_confirmation() {
    // Sample of write tools that should require confirmation
    let write_tools = [
        "cancel_order",
        "create_customer",
        "delete_product",
        "adjust_inventory",
        "create_collection",
        "create_discount",
        "create_gift_card",
        "create_fulfillment",
    ];

    for tool_name in write_tools {
        assert!(
            requires_confirmation(tool_name),
            "{tool_name} should require confirmation"
        );
    }
}

#[test]
fn test_get_tool_by_name_found() {
    let tool = get_tool_by_name("get_orders");
    assert!(tool.is_some());
    let tool = tool.expect("tool should exist");
    assert_eq!(tool.name, "get_orders");
}

#[test]
fn test_get_tool_by_name_not_found() {
    let tool = get_tool_by_name("nonexistent_tool");
    assert!(tool.is_none());
}

#[test]
fn test_get_tool_domain() {
    assert_eq!(get_tool_domain("get_orders"), Some("orders".to_string()));
    assert_eq!(
        get_tool_domain("get_customers"),
        Some("customers".to_string())
    );
    assert_eq!(
        get_tool_domain("get_products"),
        Some("products".to_string())
    );
    assert_eq!(get_tool_domain("nonexistent"), None);
}

#[test]
fn test_get_tools_by_domain_orders() {
    let tools = get_tools_by_domain("orders");
    // Orders domain should have tools
    assert!(!tools.is_empty());

    // All returned tools should be in orders domain
    for tool in &tools {
        assert_eq!(tool.domain.as_deref(), Some("orders"));
    }
}

#[test]
fn test_get_tools_by_domain_all_domains() {
    for domain in DOMAINS {
        let tools = get_tools_by_domain(domain);
        assert!(
            !tools.is_empty(),
            "Domain {domain} should have at least one tool"
        );
    }
}

#[test]
fn test_filter_tools_by_names() {
    let names = vec![
        "get_orders".to_string(),
        "get_customers".to_string(),
        "cancel_order".to_string(),
    ];

    let tools = filter_tools_by_names(&names);
    assert_eq!(tools.len(), 3);

    let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
    assert!(tool_names.contains(&"get_orders"));
    assert!(tool_names.contains(&"get_customers"));
    assert!(tool_names.contains(&"cancel_order"));
}

#[test]
fn test_filter_tools_by_names_ignores_unknown() {
    let names = vec![
        "get_orders".to_string(),
        "unknown_tool".to_string(),
        "get_customers".to_string(),
    ];

    let tools = filter_tools_by_names(&names);
    assert_eq!(tools.len(), 2);
}

// =============================================================================
// Configuration Validation Tests
// =============================================================================

#[test]
fn test_validate_config_valid() {
    let mut config = ToolExamplesConfig::new();
    config.insert(
        "get_orders".to_string(),
        ToolExampleConfig {
            domain: "orders".to_string(),
            examples: vec![
                "Show me recent orders".to_string(),
                "What orders came in today".to_string(),
            ],
        },
    );

    let errors = validate_config(&config);
    assert!(
        errors.is_empty(),
        "Valid config should have no errors: {errors:?}"
    );
}

#[test]
fn test_validate_config_unknown_tool() {
    let mut config = ToolExamplesConfig::new();
    config.insert(
        "unknown_tool".to_string(),
        ToolExampleConfig {
            domain: "orders".to_string(),
            examples: vec!["Some query".to_string()],
        },
    );

    let errors = validate_config(&config);
    assert!(!errors.is_empty());
    assert!(errors.iter().any(|e| e.contains("Unknown tool")));
}

#[test]
fn test_validate_config_invalid_domain() {
    let mut config = ToolExamplesConfig::new();
    config.insert(
        "get_orders".to_string(),
        ToolExampleConfig {
            domain: "invalid_domain".to_string(),
            examples: vec!["Some query".to_string()],
        },
    );

    let errors = validate_config(&config);
    assert!(!errors.is_empty());
    assert!(errors.iter().any(|e| e.contains("Invalid domain")));
}

#[test]
fn test_validate_config_empty_examples() {
    let mut config = ToolExamplesConfig::new();
    config.insert(
        "get_orders".to_string(),
        ToolExampleConfig {
            domain: "orders".to_string(),
            examples: vec![],
        },
    );

    let errors = validate_config(&config);
    assert!(!errors.is_empty());
    assert!(errors.iter().any(|e| e.contains("No examples")));
}

#[test]
fn test_validate_config_empty_example_string() {
    let mut config = ToolExamplesConfig::new();
    config.insert(
        "get_orders".to_string(),
        ToolExampleConfig {
            domain: "orders".to_string(),
            examples: vec!["Valid query".to_string(), "   ".to_string()],
        },
    );

    let errors = validate_config(&config);
    assert!(!errors.is_empty());
    assert!(errors.iter().any(|e| e.contains("Empty example")));
}

// =============================================================================
// YAML Parsing Tests
// =============================================================================

#[test]
fn test_parse_yaml_config() {
    let yaml = r#"
get_orders:
  domain: orders
  examples:
    - "Show me recent orders"
    - "What orders came in today?"

cancel_order:
  domain: orders
  examples:
    - "Cancel order #1001"
"#;

    let config: ToolExamplesConfig = serde_yaml::from_str(yaml).expect("Should parse YAML");
    assert_eq!(config.len(), 2);

    let get_orders = config.get("get_orders").expect("Should have get_orders");
    assert_eq!(get_orders.domain, "orders");
    assert_eq!(get_orders.examples.len(), 2);

    let cancel_order = config
        .get("cancel_order")
        .expect("Should have cancel_order");
    assert_eq!(cancel_order.examples.len(), 1);
}

// =============================================================================
// Domain Constants Tests
// =============================================================================

#[test]
fn test_all_domains_exist() {
    let expected_domains = [
        "orders",
        "customers",
        "products",
        "inventory",
        "collections",
        "discounts",
        "gift_cards",
        "fulfillment",
        "finance",
        "order_editing",
    ];

    assert_eq!(DOMAINS.len(), expected_domains.len());

    for domain in expected_domains {
        assert!(
            DOMAINS.contains(&domain),
            "Domain {domain} should exist in DOMAINS"
        );
    }
}

// =============================================================================
// Tool Structure Tests
// =============================================================================

#[test]
fn test_tool_has_required_fields() {
    let tools = all_shopify_tools();

    for tool in &tools {
        // Every tool should have a non-empty name
        assert!(!tool.name.is_empty(), "Tool name should not be empty");

        // Every tool should have a non-empty description
        assert!(
            !tool.description.is_empty(),
            "Tool {} should have description",
            tool.name
        );

        // Every tool should have a domain
        assert!(
            tool.domain.is_some(),
            "Tool {} should have domain",
            tool.name
        );

        // Input schema should be a valid JSON object
        assert!(
            tool.input_schema.is_object(),
            "Tool {} should have object input_schema",
            tool.name
        );
    }
}

#[test]
fn test_tool_domains_are_valid() {
    let tools = all_shopify_tools();

    for tool in &tools {
        if let Some(domain) = &tool.domain {
            assert!(
                DOMAINS.contains(&domain.as_str()),
                "Tool {} has invalid domain: {}",
                tool.name,
                domain
            );
        }
    }
}
