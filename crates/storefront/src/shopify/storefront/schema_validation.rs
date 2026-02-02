//! JSON Schema validation for active promotions.
//!
//! Validates the `custom.active_promotions` shop metafield JSON against
//! the schema defined in `crates/admin/schemas/active_promotions.json`.

use std::sync::LazyLock;

use jsonschema::Validator;
use serde_json::Value;
use thiserror::Error;

/// The JSON schema for `active_promotions`, embedded at compile time.
/// The schema is shared with the admin crate.
const SCHEMA_JSON: &str = include_str!("../../../../admin/schemas/active_promotions.json");

/// Lazily compiled schema validator.
static VALIDATOR: LazyLock<Validator> = LazyLock::new(|| {
    let schema: Value =
        serde_json::from_str(SCHEMA_JSON).expect("active_promotions.json schema is valid JSON");
    Validator::new(&schema).expect("active_promotions.json is a valid JSON Schema")
});

/// Validation errors for active promotions.
#[derive(Debug, Error)]
pub enum ValidationError {
    /// JSON parsing failed.
    #[error("Invalid JSON: {0}")]
    InvalidJson(#[from] serde_json::Error),

    /// Schema validation failed.
    #[error("Schema validation failed: {errors}")]
    SchemaValidation { errors: String },
}

/// Validate a JSON value against the `active_promotions` schema.
///
/// # Errors
///
/// Returns `ValidationError::SchemaValidation` if the JSON does not conform to the schema.
pub fn validate(value: &Value) -> Result<(), ValidationError> {
    let errors: Vec<String> = VALIDATOR
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
    let value: Value = serde_json::from_str(json)?;
    validate(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
