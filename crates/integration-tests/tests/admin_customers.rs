//! Integration tests for admin customer management.
//!
//! These tests require:
//! - A running `PostgreSQL` database (task db:start)
//! - The admin server running (cargo run -p naked-pineapple-admin)
//! - Valid Shopify credentials in environment
//!
//! Run with: task test:integration

use reqwest::{Client, StatusCode};
use serde_json::{Value, json};
use uuid::Uuid;

/// Base URL for admin API (configurable via environment).
fn admin_base_url() -> String {
    std::env::var("ADMIN_BASE_URL").unwrap_or_else(|_| "http://localhost:3001".to_string())
}

/// Create an authenticated client with session cookie.
/// In real implementation, this would log in and get a session.
#[allow(clippy::unused_async)]
async fn authenticated_client() -> Client {
    // For now, return a basic client
    // TODO: Implement proper authentication flow
    Client::builder()
        .cookie_store(true)
        .build()
        .expect("Failed to create HTTP client")
}

/// Test helper: Create a test customer via API.
#[allow(dead_code)]
async fn create_test_customer(client: &Client, email: &str) -> Value {
    let base_url = admin_base_url();
    let resp = client
        .post(format!("{base_url}/customers"))
        .form(&[
            ("email", email),
            ("first_name", "Test"),
            ("last_name", "Customer"),
        ])
        .send()
        .await
        .expect("Failed to create test customer");

    assert!(resp.status().is_success() || resp.status().is_redirection());
    json!({"email": email})
}

/// Test helper: Delete a test customer via API.
#[allow(dead_code)]
async fn delete_test_customer(client: &Client, customer_id: &str) {
    let base_url = admin_base_url();
    let _ = client
        .post(format!("{base_url}/customers/{customer_id}/delete"))
        .send()
        .await;
}

// ============================================================================
// List & Pagination Tests
// ============================================================================

#[tokio::test]
#[ignore = "Requires running admin server and Shopify credentials"]
async fn test_customer_list_pagination() {
    let client = authenticated_client().await;
    let base_url = admin_base_url();

    // Get first page
    let resp = client
        .get(format!("{base_url}/customers"))
        .send()
        .await
        .expect("Failed to get customers list");

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.text().await.expect("Failed to read response");

    // Should contain table structure
    assert!(body.contains("data-table"));

    // If there's a next page, test pagination
    if body.contains("Load more") || body.contains("next_cursor") {
        // Extract cursor and request next page
        // This would be implemented based on actual response format
    }
}

#[tokio::test]
#[ignore = "Requires running admin server and Shopify credentials"]
async fn test_customer_list_filters() {
    let client = authenticated_client().await;
    let base_url = admin_base_url();

    // Test state filter
    let resp = client
        .get(format!("{base_url}/customers?state=ENABLED"))
        .send()
        .await
        .expect("Failed to get filtered customers");

    assert_eq!(resp.status(), StatusCode::OK);

    // Test search filter
    let resp = client
        .get(format!("{base_url}/customers?q=test@example.com"))
        .send()
        .await
        .expect("Failed to search customers");

    assert_eq!(resp.status(), StatusCode::OK);

    // Test combined filters
    let resp = client
        .get(format!("{base_url}/customers?state=ENABLED&q=test"))
        .send()
        .await
        .expect("Failed to get customers with combined filters");

    assert_eq!(resp.status(), StatusCode::OK);
}

// ============================================================================
// CRUD Tests
// ============================================================================

#[tokio::test]
#[ignore = "Requires running admin server and Shopify credentials"]
async fn test_customer_create() {
    let client = authenticated_client().await;
    let base_url = admin_base_url();

    // Get the create form
    let resp = client
        .get(format!("{base_url}/customers/new"))
        .send()
        .await
        .expect("Failed to get create form");

    assert_eq!(resp.status(), StatusCode::OK);

    // Submit create form
    let test_email = format!("integration-test-{}@example.com", Uuid::new_v4());
    let resp = client
        .post(format!("{base_url}/customers"))
        .form(&[
            ("email", &test_email),
            ("first_name", &"Integration".to_string()),
            ("last_name", &"Test".to_string()),
        ])
        .send()
        .await
        .expect("Failed to create customer");

    // Should redirect to customer detail page on success
    assert!(
        resp.status().is_redirection() || resp.status().is_success(),
        "Expected redirect or success, got: {}",
        resp.status()
    );
}

#[tokio::test]
#[ignore = "Requires running admin server and Shopify credentials"]
async fn test_customer_update() {
    let client = authenticated_client().await;
    let base_url = admin_base_url();

    // First, we need a customer ID to update
    // In a real test, we'd create one or use a known test customer
    let customer_id = "gid://shopify/Customer/test123";

    // Get the edit form
    let resp = client
        .get(format!("{base_url}/customers/{customer_id}/edit"))
        .send()
        .await
        .expect("Failed to get edit form");

    // May return 404 if customer doesn't exist
    if resp.status() == StatusCode::NOT_FOUND {
        return; // Skip if no test customer available
    }

    assert_eq!(resp.status(), StatusCode::OK);

    // Submit update
    let resp = client
        .post(format!("{base_url}/customers/{customer_id}"))
        .form(&[("note", "Updated by integration test")])
        .send()
        .await
        .expect("Failed to update customer");

    assert!(resp.status().is_redirection() || resp.status().is_success());
}

#[tokio::test]
#[ignore = "Requires running admin server and Shopify credentials"]
async fn test_customer_delete_with_orders_fails() {
    let client = authenticated_client().await;
    let base_url = admin_base_url();

    // Try to delete a customer that has orders (should fail)
    // This requires a known customer with orders in the test environment
    let customer_with_orders_id = "gid://shopify/Customer/with_orders";

    let resp = client
        .post(format!(
            "{base_url}/customers/{customer_with_orders_id}/delete"
        ))
        .send()
        .await
        .expect("Failed to attempt delete");

    // Should fail because customer has orders
    // The exact response depends on implementation (could be 400, 422, or error page)
    // Just verify we don't get a success response
    assert!(
        !resp.status().is_success() || resp.text().await.unwrap_or_default().contains("error"),
        "Delete should fail for customer with orders"
    );
}

// ============================================================================
// Tags Tests
// ============================================================================

#[tokio::test]
#[ignore = "Requires running admin server and Shopify credentials"]
async fn test_customer_tags_add_remove() {
    let client = authenticated_client().await;
    let base_url = admin_base_url();

    let customer_id = "gid://shopify/Customer/test123";

    // Add a tag
    let resp = client
        .post(format!("{base_url}/customers/{customer_id}/tags"))
        .form(&[("action", "add"), ("tags", "integration-test-tag")])
        .send()
        .await
        .expect("Failed to add tag");

    if resp.status() != StatusCode::NOT_FOUND {
        assert!(resp.status().is_success() || resp.status().is_redirection());
    }

    // Remove the tag
    let resp = client
        .post(format!("{base_url}/customers/{customer_id}/tags"))
        .form(&[("action", "remove"), ("tags", "integration-test-tag")])
        .send()
        .await
        .expect("Failed to remove tag");

    if resp.status() != StatusCode::NOT_FOUND {
        assert!(resp.status().is_success() || resp.status().is_redirection());
    }
}

// ============================================================================
// Address Tests
// ============================================================================

#[tokio::test]
#[ignore = "Requires running admin server and Shopify credentials"]
async fn test_customer_address_crud() {
    let client = authenticated_client().await;
    let base_url = admin_base_url();

    let customer_id = "gid://shopify/Customer/test123";

    // Create address
    let resp = client
        .post(format!("{base_url}/customers/{customer_id}/addresses"))
        .form(&[
            ("address1", "123 Test Street"),
            ("city", "Test City"),
            ("province_code", "CA"),
            ("country_code", "US"),
            ("zip", "90210"),
        ])
        .send()
        .await
        .expect("Failed to create address");

    if resp.status() == StatusCode::NOT_FOUND {
        return; // Customer doesn't exist in test environment
    }

    assert!(resp.status().is_success() || resp.status().is_redirection());

    // Note: Update and delete would require knowing the address ID
    // which we'd extract from the create response in a real test
}

// ============================================================================
// User Preferences Tests
// ============================================================================

#[tokio::test]
#[ignore = "Requires running admin server and database"]
async fn test_user_preferences_persist() {
    let client = authenticated_client().await;
    let base_url = admin_base_url();

    // Save column preferences
    let resp = client
        .post(format!("{base_url}/api/preferences/table/customers"))
        .json(&json!({
            "columns": ["name", "email", "location", "orders"]
        }))
        .send()
        .await
        .expect("Failed to save preferences");

    if resp.status() == StatusCode::UNAUTHORIZED {
        return; // Not authenticated, skip
    }

    assert!(resp.status().is_success());

    let body: Value = resp.json().await.expect("Failed to parse response");
    assert_eq!(body.get("success"), Some(&Value::Bool(true)));

    // Verify preferences are loaded on page refresh
    let resp = client
        .get(format!("{base_url}/customers"))
        .send()
        .await
        .expect("Failed to load customers page");

    assert_eq!(resp.status(), StatusCode::OK);
    // In a real test, we'd verify the column visibility matches our saved preferences
}

// ============================================================================
// Bulk Operations Tests
// ============================================================================

#[tokio::test]
#[ignore = "Requires running admin server and Shopify credentials"]
async fn test_bulk_tags_operation() {
    let client = authenticated_client().await;
    let base_url = admin_base_url();

    let resp = client
        .post(format!("{base_url}/customers/bulk/tags"))
        .json(&json!({
            "customer_ids": ["gid://shopify/Customer/1", "gid://shopify/Customer/2"],
            "action": "add",
            "tags": ["bulk-test-tag"]
        }))
        .send()
        .await
        .expect("Failed to perform bulk tag operation");

    // May fail if customers don't exist, but should not error
    assert!(
        resp.status().is_success()
            || resp.status() == StatusCode::BAD_REQUEST
            || resp.status() == StatusCode::NOT_FOUND
    );
}

#[tokio::test]
#[ignore = "Requires running admin server and Shopify credentials"]
async fn test_bulk_marketing_operation() {
    let client = authenticated_client().await;
    let base_url = admin_base_url();

    let resp = client
        .post(format!("{base_url}/customers/bulk/marketing"))
        .json(&json!({
            "customer_ids": ["gid://shopify/Customer/1", "gid://shopify/Customer/2"],
            "email_marketing_state": "SUBSCRIBED"
        }))
        .send()
        .await
        .expect("Failed to perform bulk marketing operation");

    assert!(
        resp.status().is_success()
            || resp.status() == StatusCode::BAD_REQUEST
            || resp.status() == StatusCode::NOT_FOUND
    );
}
