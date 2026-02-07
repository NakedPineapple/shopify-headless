//! Types for Shopify Customer Account API OAuth and responses.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::shopify::types::Money;

// ─────────────────────────────────────────────────────────────────────────────
// OAuth Types
// ─────────────────────────────────────────────────────────────────────────────

/// Customer access token obtained via OAuth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerAccessToken {
    /// The access token for API requests.
    pub access_token: String,
    /// The ID token (`OpenID` Connect).
    pub id_token: Option<String>,
    /// The refresh token for obtaining new access tokens.
    pub refresh_token: Option<String>,
    /// Token lifetime in seconds.
    pub expires_in: Option<i64>,
    /// Unix timestamp when the token was obtained.
    pub obtained_at: i64,
}

impl CustomerAccessToken {
    /// Check if the access token is expired (with 60s buffer).
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.expires_in.is_some_and(|expires_in| {
            let now = Utc::now().timestamp();
            let expires_at = self.obtained_at + expires_in;
            now >= (expires_at - 60)
        })
    }
}

/// Raw token response from Shopify OAuth endpoint.
#[derive(Debug, Deserialize)]
pub(super) struct TokenResponse {
    pub access_token: String,
    pub id_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_in: Option<i64>,
    #[allow(dead_code)] // Required by Shopify's response format but unused
    pub token_type: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Customer Types
// ─────────────────────────────────────────────────────────────────────────────

/// A Shopify customer.
///
/// Fields are populated by conversion functions from the Customer Account API's
/// nested response types (e.g., `emailAddress.emailAddress` → `email`).
#[derive(Debug, Clone)]
pub struct Customer {
    /// The customer's unique ID.
    pub id: String,
    /// The customer's email address (extracted from nested `emailAddress`).
    pub email: Option<String>,
    /// The customer's first name.
    pub first_name: Option<String>,
    /// The customer's last name.
    pub last_name: Option<String>,
    /// The customer's phone number (extracted from nested `phoneNumber`).
    pub phone: Option<String>,
    /// The customer's default address.
    pub default_address: Option<Address>,
}

impl Customer {
    /// Get the customer's full name.
    #[must_use]
    pub fn full_name(&self) -> String {
        match (&self.first_name, &self.last_name) {
            (Some(first), Some(last)) => format!("{first} {last}"),
            (Some(first), None) => first.clone(),
            (None, Some(last)) => last.clone(),
            (None, None) => String::new(),
        }
    }
}

/// A customer address.
///
/// Populated by conversion functions. `Serialize` is kept for template usage.
/// Field names use Rust conventions; conversions map from schema names
/// (`zoneCode` → `province_code`, `territoryCode` → `country_code`,
/// `phoneNumber` → `phone`).
#[derive(Debug, Clone, Serialize)]
pub struct Address {
    /// The address ID.
    pub id: String,
    /// First name.
    pub first_name: Option<String>,
    /// Last name.
    pub last_name: Option<String>,
    /// Company name.
    pub company: Option<String>,
    /// Address line 1.
    pub address1: Option<String>,
    /// Address line 2.
    pub address2: Option<String>,
    /// City.
    pub city: Option<String>,
    /// Province/state.
    pub province: Option<String>,
    /// Province/state code (from schema `zoneCode`).
    pub province_code: Option<String>,
    /// Country.
    pub country: Option<String>,
    /// Country code (from schema `territoryCode`).
    pub country_code: Option<String>,
    /// Postal/ZIP code.
    pub zip: Option<String>,
    /// Phone number (from schema `phoneNumber`).
    pub phone: Option<String>,
}

impl Address {
    /// Format the address as a single line.
    #[must_use]
    pub fn formatted_single_line(&self) -> String {
        let mut parts = Vec::new();

        if let Some(addr1) = &self.address1
            && !addr1.is_empty()
        {
            parts.push(addr1.clone());
        }
        if let Some(city) = &self.city
            && !city.is_empty()
        {
            parts.push(city.clone());
        }
        if let Some(province) = &self.province_code
            && !province.is_empty()
        {
            parts.push(province.clone());
        }
        if let Some(zip) = &self.zip
            && !zip.is_empty()
        {
            parts.push(zip.clone());
        }
        if let Some(country) = &self.country
            && !country.is_empty()
        {
            parts.push(country.clone());
        }

        parts.join(", ")
    }
}

/// A customer order.
#[derive(Debug, Clone)]
pub struct Order {
    /// The order ID.
    pub id: String,
    /// The order name (e.g., "#1001").
    pub name: String,
    /// The order number.
    pub number: i64,
    /// When the order was processed.
    pub processed_at: String,
    /// The financial status.
    pub financial_status: Option<String>,
    /// The fulfillment status.
    pub fulfillment_status: Option<String>,
    /// The total price.
    pub total_price: Money,
}

impl Order {
    /// Parse the `processed_at` timestamp.
    #[must_use]
    pub fn processed_at_datetime(&self) -> Option<DateTime<Utc>> {
        DateTime::parse_from_rfc3339(&self.processed_at)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Subscription Types
// ─────────────────────────────────────────────────────────────────────────────

/// Status of a subscription contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionContractStatus {
    /// The contract is active and continuing per its policies.
    Active,
    /// The contract is temporarily paused.
    Paused,
    /// The contract was cancelled by the customer.
    Cancelled,
    /// The contract has completed all billing cycles.
    Expired,
    /// The contract ended due to billing failures.
    Failed,
    /// The contract expired due to inactivity.
    Stale,
}

impl SubscriptionContractStatus {
    /// Whether the subscription can be paused.
    #[must_use]
    pub const fn can_pause(&self) -> bool {
        matches!(self, Self::Active)
    }

    /// Whether the subscription can be cancelled.
    #[must_use]
    pub const fn can_cancel(&self) -> bool {
        matches!(self, Self::Active | Self::Paused)
    }

    /// Whether the subscription can be activated/resumed.
    #[must_use]
    pub const fn can_activate(&self) -> bool {
        matches!(self, Self::Paused)
    }

    /// Human-readable label for the status.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Paused => "Paused",
            Self::Cancelled => "Cancelled",
            Self::Expired => "Expired",
            Self::Failed => "Failed",
            Self::Stale => "Stale",
        }
    }
}

/// Billing policy for a subscription.
#[derive(Debug, Clone)]
pub struct SubscriptionBillingPolicy {
    /// Billing interval type (e.g., WEEK, MONTH).
    pub interval: String,
    /// Number of intervals between billings.
    pub interval_count: Option<i64>,
}

impl SubscriptionBillingPolicy {
    /// Human-readable label for the billing frequency.
    #[must_use]
    pub fn frequency_label(&self) -> String {
        let count = self.interval_count.unwrap_or(1);
        let interval = self.interval.to_lowercase();
        if count == 1 {
            format!("Every {interval}")
        } else {
            format!("Every {count} {interval}s")
        }
    }
}

/// Image on a subscription line item.
#[derive(Debug, Clone)]
pub struct SubscriptionLineImage {
    /// Image URL.
    pub url: String,
    /// Alt text for accessibility.
    pub alt_text: Option<String>,
}

/// A line item in a subscription contract.
#[derive(Debug, Clone)]
pub struct SubscriptionLine {
    /// Line item ID.
    pub id: String,
    /// Product name.
    pub name: String,
    /// Quantity per delivery.
    pub quantity: i64,
    /// Current price per unit.
    pub current_price: Money,
    /// Product image.
    pub image: Option<SubscriptionLineImage>,
}

/// A subscription contract.
#[derive(Debug, Clone)]
pub struct SubscriptionContract {
    /// The contract ID.
    pub id: String,
    /// Current status.
    pub status: SubscriptionContractStatus,
    /// When the contract was created.
    pub created_at: String,
    /// Next billing date.
    pub next_billing_date: Option<String>,
    /// Billing policy (frequency).
    pub billing_policy: SubscriptionBillingPolicy,
    /// Delivery price per billing cycle.
    pub delivery_price: Money,
    /// Line items in the subscription.
    pub lines: Vec<SubscriptionLine>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Order Detail Types
// ─────────────────────────────────────────────────────────────────────────────

/// A detailed order with line items and shipping info.
#[derive(Debug, Clone)]
pub struct OrderDetail {
    /// The order ID.
    pub id: String,
    /// The order name (e.g., "#1001").
    pub name: String,
    /// The order number.
    pub number: i64,
    /// When the order was processed.
    pub processed_at: String,
    /// The financial status.
    pub financial_status: Option<String>,
    /// The fulfillment status.
    pub fulfillment_status: Option<String>,
    /// The total price.
    pub total_price: Money,
    /// The subtotal before shipping/taxes.
    pub subtotal: Option<Money>,
    /// The total shipping cost.
    pub total_shipping: Money,
    /// The total tax.
    pub total_tax: Option<Money>,
    /// The order's line items.
    pub line_items: Vec<OrderLineItem>,
    /// The shipping address.
    pub shipping_address: Option<Address>,
    /// Returns on this order.
    pub returns: Vec<Return>,
}

/// A line item in an order.
#[derive(Debug, Clone)]
pub struct OrderLineItem {
    /// The line item ID.
    pub id: String,
    /// The product title.
    pub title: String,
    /// The quantity ordered.
    pub quantity: i64,
    /// The unit price.
    pub unit_price: Option<Money>,
    /// The total price for this line.
    pub total_price: Option<Money>,
    /// The product image.
    pub image: Option<OrderLineItemImage>,
    /// The variant title (e.g., "Large / Blue").
    pub variant_title: Option<String>,
}

/// Image on an order line item.
#[derive(Debug, Clone)]
pub struct OrderLineItemImage {
    /// Image URL.
    pub url: String,
    /// Alt text for accessibility.
    pub alt_text: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Return Types
// ─────────────────────────────────────────────────────────────────────────────

/// Status of a return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReturnStatus {
    /// The return has been requested.
    Requested,
    /// The return is in progress.
    Open,
    /// The return has been completed.
    Closed,
    /// The return was canceled.
    Canceled,
    /// The return was declined.
    Declined,
}

impl ReturnStatus {
    /// Human-readable label for the status.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Requested => "Requested",
            Self::Open => "In Progress",
            Self::Closed => "Completed",
            Self::Canceled => "Canceled",
            Self::Declined => "Declined",
        }
    }
}

/// A return on an order.
#[derive(Debug, Clone)]
pub struct Return {
    /// The return ID.
    pub id: String,
    /// The return name (e.g., "#1001-R1").
    pub name: String,
    /// The return status.
    pub status: ReturnStatus,
}

/// A return reason definition from Shopify.
#[derive(Debug, Clone)]
pub struct ReturnReasonDefinition {
    /// The reason ID.
    pub id: String,
    /// The localized display name.
    pub name: String,
}

/// A line item with return reason suggestions (for the return form).
#[derive(Debug, Clone)]
pub struct OrderLineItemWithReasons {
    /// The line item ID.
    pub id: String,
    /// The product title.
    pub title: String,
    /// The quantity ordered.
    pub quantity: i64,
    /// The product image.
    pub image: Option<OrderLineItemImage>,
    /// The variant title.
    pub variant_title: Option<String>,
    /// Suggested return reasons for this line item.
    pub suggested_reasons: Vec<ReturnReasonDefinition>,
}

/// An order with line items and their suggested return reasons.
#[derive(Debug, Clone)]
pub struct OrderForReturn {
    /// The order ID.
    pub id: String,
    /// The order name.
    pub name: String,
    /// The order's line items with return reason suggestions.
    pub line_items: Vec<OrderLineItemWithReasons>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Store Credit Types
// ─────────────────────────────────────────────────────────────────────────────

/// A store credit account.
#[derive(Debug, Clone)]
pub struct StoreCreditAccount {
    /// The balance.
    pub balance: Money,
}

// ─────────────────────────────────────────────────────────────────────────────
// Billing Cycle Types
// ─────────────────────────────────────────────────────────────────────────────

/// A billing cycle in a subscription contract.
#[derive(Debug, Clone)]
pub struct SubscriptionBillingCycle {
    /// The expected billing date.
    pub billing_attempt_expected_date: String,
    /// The cycle index.
    pub cycle_index: i64,
    /// Whether this cycle was skipped.
    pub skipped: bool,
    /// The cycle status.
    pub status: BillingCycleStatus,
}

/// Status of a billing cycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BillingCycleStatus {
    /// The cycle has been billed.
    Billed,
    /// The cycle has not been billed.
    Unbilled,
}

// ─────────────────────────────────────────────────────────────────────────────
// Input Types
// ─────────────────────────────────────────────────────────────────────────────

/// Input for creating or updating an address.
///
/// Uses Rust-friendly field names; conversion to the generated
/// `CustomerAddressInput` type happens in the client methods.
#[derive(Debug, Default)]
pub struct AddressInput {
    /// First name.
    pub first_name: Option<String>,
    /// Last name.
    pub last_name: Option<String>,
    /// Company name.
    pub company: Option<String>,
    /// Address line 1.
    pub address1: Option<String>,
    /// Address line 2.
    pub address2: Option<String>,
    /// City.
    pub city: Option<String>,
    /// Province/state.
    pub province: Option<String>,
    /// Country.
    pub country: Option<String>,
    /// Postal/ZIP code.
    pub zip: Option<String>,
    /// Phone number.
    pub phone: Option<String>,
}

/// Input for updating customer information.
///
/// The Customer Account API's `CustomerUpdateInput` only supports
/// `firstName` and `lastName`.
#[derive(Debug, Default)]
pub struct CustomerUpdateInput {
    /// First name.
    pub first_name: Option<String>,
    /// Last name.
    pub last_name: Option<String>,
}

/// Input for requesting a return on a line item.
#[derive(Debug)]
pub struct ReturnRequestLineItemInput {
    /// The line item ID.
    pub line_item_id: String,
    /// The quantity to return.
    pub quantity: i32,
    /// The return reason definition ID.
    pub reason_id: Option<String>,
    /// Customer note (max 300 chars).
    pub note: Option<String>,
}
