//! JSON Schema validation for shop metafields.
//!
//! Validates metafield JSON against schemas:
//! - `custom.active_promotions` - `schemas/active_promotions.json`
//! - `custom.cart_recommendations` - `schemas/cart_recommendations.json`

use std::sync::LazyLock;

use jsonschema::Validator;
use serde_json::Value;
use thiserror::Error;

/// The JSON schema for `active_promotions`, embedded at compile time.
const ACTIVE_PROMOTIONS_SCHEMA: &str = include_str!("../../../schemas/active_promotions.json");

/// The JSON schema for `cart_recommendations`, embedded at compile time.
const CART_RECOMMENDATIONS_SCHEMA: &str =
    include_str!("../../../schemas/cart_recommendations.json");

/// Lazily compiled schema validator for active promotions.
static ACTIVE_PROMOTIONS_VALIDATOR: LazyLock<Validator> = LazyLock::new(|| {
    let schema: Value = serde_json::from_str(ACTIVE_PROMOTIONS_SCHEMA)
        .expect("active_promotions.json schema is valid JSON");
    Validator::new(&schema).expect("active_promotions.json is a valid JSON Schema")
});

/// Lazily compiled schema validator for cart recommendations.
static CART_RECOMMENDATIONS_VALIDATOR: LazyLock<Validator> = LazyLock::new(|| {
    let schema: Value = serde_json::from_str(CART_RECOMMENDATIONS_SCHEMA)
        .expect("cart_recommendations.json schema is valid JSON");
    Validator::new(&schema).expect("cart_recommendations.json is a valid JSON Schema")
});

/// Validation errors for metafield schemas.
#[derive(Debug, Error)]
pub enum ValidationError {
    /// JSON parsing failed.
    #[error("Invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),

    /// Schema validation failed.
    #[error("Schema validation failed: {errors}")]
    SchemaValidation { errors: String },
}

// =============================================================================
// Active Promotions Validation
// =============================================================================

/// Validate a JSON value against the `active_promotions` schema.
///
/// # Errors
///
/// Returns `ValidationError::SchemaValidation` if the JSON does not conform to the schema.
pub fn validate(value: &Value) -> Result<(), ValidationError> {
    validate_active_promotions(value)
}

/// Validate a JSON value against the `active_promotions` schema.
///
/// # Errors
///
/// Returns `ValidationError::SchemaValidation` if the JSON does not conform to the schema.
pub fn validate_active_promotions(value: &Value) -> Result<(), ValidationError> {
    let errors: Vec<String> = ACTIVE_PROMOTIONS_VALIDATOR
        .iter_errors(value)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect();

    if !errors.is_empty() {
        return Err(ValidationError::SchemaValidation {
            errors: errors.join("; "),
        });
    }

    Ok(())
}

/// Validate a JSON string against the `active_promotions` schema.
///
/// # Errors
///
/// Returns `ValidationError::InvalidJson` if the string is not valid JSON.
/// Returns `ValidationError::SchemaValidation` if the JSON does not conform to the schema.
pub fn validate_str(json: &str) -> Result<(), ValidationError> {
    validate_active_promotions_str(json)
}

/// Validate a JSON string against the `active_promotions` schema.
///
/// # Errors
///
/// Returns `ValidationError::InvalidJson` if the string is not valid JSON.
/// Returns `ValidationError::SchemaValidation` if the JSON does not conform to the schema.
pub fn validate_active_promotions_str(json: &str) -> Result<(), ValidationError> {
    let value: Value = serde_json::from_str(json)?;
    validate_active_promotions(&value)
}

// =============================================================================
// Cart Recommendations Validation
// =============================================================================

/// Validate a JSON value against the `cart_recommendations` schema.
///
/// # Errors
///
/// Returns `ValidationError::SchemaValidation` if the JSON does not conform to the schema.
pub fn validate_cart_recommendations(value: &Value) -> Result<(), ValidationError> {
    let errors: Vec<String> = CART_RECOMMENDATIONS_VALIDATOR
        .iter_errors(value)
        .map(|e| format!("{} at {}", e, e.instance_path()))
        .collect();

    if !errors.is_empty() {
        return Err(ValidationError::SchemaValidation {
            errors: errors.join("; "),
        });
    }

    Ok(())
}

/// Validate a JSON string against the `cart_recommendations` schema.
///
/// # Errors
///
/// Returns `ValidationError::InvalidJson` if the string is not valid JSON.
/// Returns `ValidationError::SchemaValidation` if the JSON does not conform to the schema.
pub fn validate_cart_recommendations_str(json: &str) -> Result<(), ValidationError> {
    let value: Value = serde_json::from_str(json)?;
    validate_cart_recommendations(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // Active Promotions Tests
    // =========================================================================

    #[test]
    fn test_valid_empty_promotions() {
        let json = r"{}";
        assert!(validate_str(json).is_ok());
    }

    #[test]
    fn test_valid_promotions_with_banner() {
        let json = r#"{
            "banners": [{
                "discount_id": "gid://shopify/DiscountAutomaticNode/123",
                "title": "Free Gift"
            }]
        }"#;
        assert!(validate_str(json).is_ok());
    }

    #[test]
    fn test_invalid_banner_missing_title() {
        let json = r#"{
            "banners": [{
                "discount_id": "gid://shopify/DiscountAutomaticNode/123"
            }]
        }"#;
        let result = validate_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_qualifying_rule() {
        let json = r#"{
            "qualifying_rules": [{
                "discount_id": "gid://shopify/DiscountAutomaticNode/123",
                "type": "FREE_SHIPPING",
                "free_shipping": {
                    "minimum_requirement": {
                        "type": "AMOUNT",
                        "amount": "50.00"
                    }
                }
            }]
        }"#;
        assert!(validate_str(json).is_ok());
    }

    #[test]
    fn test_invalid_qualifying_rule_type() {
        let json = r#"{
            "qualifying_rules": [{
                "discount_id": "gid://shopify/DiscountAutomaticNode/123",
                "type": "INVALID_TYPE"
            }]
        }"#;
        let result = validate_str(json);
        assert!(result.is_err());
    }

    // =========================================================================
    // Cart Recommendations Tests
    // =========================================================================

    #[test]
    fn test_valid_empty_cart_recommendations() {
        let json = r"{}";
        assert!(validate_cart_recommendations_str(json).is_ok());
    }

    #[test]
    fn test_valid_cart_recommendations_with_relations() {
        let json = r#"{
            "product_relations": [{
                "product_id": "gid://shopify/Product/123",
                "related_products": [{
                    "product_id": "gid://shopify/Product/456",
                    "variant_id": "gid://shopify/ProductVariant/789"
                }]
            }]
        }"#;
        assert!(validate_cart_recommendations_str(json).is_ok());
    }

    #[test]
    fn test_valid_cart_recommendations_multiple_relations() {
        let json = r#"{
            "product_relations": [
                {
                    "product_id": "gid://shopify/Product/123",
                    "related_products": [
                        {
                            "product_id": "gid://shopify/Product/456",
                            "variant_id": "gid://shopify/ProductVariant/789"
                        },
                        {
                            "product_id": "gid://shopify/Product/111",
                            "variant_id": "gid://shopify/ProductVariant/222"
                        }
                    ]
                },
                {
                    "product_id": "gid://shopify/Product/333",
                    "related_products": [{
                        "product_id": "gid://shopify/Product/444",
                        "variant_id": "gid://shopify/ProductVariant/555"
                    }]
                }
            ]
        }"#;
        assert!(validate_cart_recommendations_str(json).is_ok());
    }

    #[test]
    fn test_invalid_cart_recommendations_missing_product_id() {
        let json = r#"{
            "product_relations": [{
                "related_products": [{
                    "product_id": "gid://shopify/Product/456",
                    "variant_id": "gid://shopify/ProductVariant/789"
                }]
            }]
        }"#;
        let result = validate_cart_recommendations_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_cart_recommendations_missing_variant_id() {
        let json = r#"{
            "product_relations": [{
                "product_id": "gid://shopify/Product/123",
                "related_products": [{
                    "product_id": "gid://shopify/Product/456"
                }]
            }]
        }"#;
        let result = validate_cart_recommendations_str(json);
        assert!(result.is_err());
    }
}
