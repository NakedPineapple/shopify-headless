//! Finance tools for Claude.

use serde_json::json;

use crate::claude::types::Tool;

/// Get all finance-related tools.
#[must_use]
pub fn finance_tools() -> Vec<Tool> {
    vec![
        // All finance operations are read-only
        get_payouts_tool(),
        get_payout_tool(),
        get_payout_detail_tool(),
        get_payout_transactions_tool(),
        get_payout_schedule_tool(),
        get_bank_accounts_tool(),
        get_disputes_tool(),
        get_dispute_tool(),
    ]
}

fn get_payouts_tool() -> Tool {
    Tool {
        name: "get_payouts_low_level_shopify".to_string(),
        description: "Get payout history (bank deposits).".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Number of payouts to fetch (default 10)"
                },
                "status": {
                    "type": "string",
                    "enum": ["SCHEDULED", "IN_TRANSIT", "PAID", "FAILED", "CANCELED"],
                    "description": "Filter by payout status"
                }
            }
        }),
        domain: Some("finance".to_string()),
        requires_confirmation: false,
    }
}

fn get_payout_tool() -> Tool {
    Tool {
        name: "get_payout_low_level_shopify".to_string(),
        description: "Get a specific payout by ID.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The payout ID"
                }
            },
            "required": ["id"]
        }),
        domain: Some("finance".to_string()),
        requires_confirmation: false,
    }
}

fn get_payout_detail_tool() -> Tool {
    Tool {
        name: "get_payout_detail_low_level_shopify".to_string(),
        description: "Get detailed breakdown of a payout including fees and adjustments."
            .to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The payout ID"
                }
            },
            "required": ["id"]
        }),
        domain: Some("finance".to_string()),
        requires_confirmation: false,
    }
}

fn get_payout_transactions_tool() -> Tool {
    Tool {
        name: "get_payout_transactions_low_level_shopify".to_string(),
        description: "Get transactions included in a specific payout.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "payout_id": {
                    "type": "string",
                    "description": "The payout ID"
                },
                "limit": {
                    "type": "integer",
                    "description": "Number of transactions to fetch (default 20)"
                }
            },
            "required": ["payout_id"]
        }),
        domain: Some("finance".to_string()),
        requires_confirmation: false,
    }
}

fn get_payout_schedule_tool() -> Tool {
    Tool {
        name: "get_payout_schedule_low_level_shopify".to_string(),
        description: "Get the current payout schedule settings.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
        domain: Some("finance".to_string()),
        requires_confirmation: false,
    }
}

fn get_bank_accounts_tool() -> Tool {
    Tool {
        name: "get_bank_accounts_low_level_shopify".to_string(),
        description: "Get configured bank accounts for payouts.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {}
        }),
        domain: Some("finance".to_string()),
        requires_confirmation: false,
    }
}

fn get_disputes_tool() -> Tool {
    Tool {
        name: "get_disputes_low_level_shopify".to_string(),
        description: "Get chargebacks and disputes.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Number of disputes to fetch (default 10)"
                },
                "status": {
                    "type": "string",
                    "enum": ["NEEDS_RESPONSE", "UNDER_REVIEW", "CHARGE_REFUNDED", "ACCEPTED", "WON", "LOST"],
                    "description": "Filter by dispute status"
                }
            }
        }),
        domain: Some("finance".to_string()),
        requires_confirmation: false,
    }
}

fn get_dispute_tool() -> Tool {
    Tool {
        name: "get_dispute_low_level_shopify".to_string(),
        description: "Get details of a specific dispute/chargeback.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "id": {
                    "type": "string",
                    "description": "The dispute ID"
                }
            },
            "required": ["id"]
        }),
        domain: Some("finance".to_string()),
        requires_confirmation: false,
    }
}
