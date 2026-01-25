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
    #[allow(dead_code)]
    pub token_type: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Customer Types
// ─────────────────────────────────────────────────────────────────────────────

/// A Shopify customer.
#[derive(Debug, Clone, Deserialize)]
pub struct Customer {
    /// The customer's unique ID.
    pub id: String,
    /// The customer's email address.
    pub email: Option<String>,
    /// The customer's first name.
    #[serde(rename = "firstName")]
    pub first_name: Option<String>,
    /// The customer's last name.
    #[serde(rename = "lastName")]
    pub last_name: Option<String>,
    /// The customer's phone number.
    pub phone: Option<String>,
    /// Whether the customer accepts marketing.
    #[serde(rename = "acceptsMarketing")]
    pub accepts_marketing: bool,
    /// The customer's default address.
    #[serde(rename = "defaultAddress")]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    /// The address ID.
    pub id: String,
    /// First name.
    #[serde(rename = "firstName")]
    pub first_name: Option<String>,
    /// Last name.
    #[serde(rename = "lastName")]
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
    /// Province/state code.
    #[serde(rename = "provinceCode")]
    pub province_code: Option<String>,
    /// Country.
    pub country: Option<String>,
    /// Country code.
    #[serde(rename = "countryCode")]
    pub country_code: Option<String>,
    /// Postal/ZIP code.
    pub zip: Option<String>,
    /// Phone number.
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
#[derive(Debug, Clone, Deserialize)]
pub struct Order {
    /// The order ID.
    pub id: String,
    /// The order name (e.g., "#1001").
    pub name: String,
    /// The order number.
    #[serde(rename = "orderNumber")]
    pub number: i64,
    /// When the order was processed.
    #[serde(rename = "processedAt")]
    pub processed_at: String,
    /// The financial status.
    #[serde(rename = "financialStatus")]
    pub financial_status: Option<String>,
    /// The fulfillment status.
    #[serde(rename = "fulfillmentStatus")]
    pub fulfillment_status: Option<String>,
    /// The total price.
    #[serde(rename = "totalPrice")]
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
// Input Types
// ─────────────────────────────────────────────────────────────────────────────

/// Input for creating or updating an address.
#[derive(Debug, Default, Serialize)]
pub struct AddressInput {
    /// First name.
    #[serde(rename = "firstName", skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    /// Last name.
    #[serde(rename = "lastName", skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    /// Company name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company: Option<String>,
    /// Address line 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address1: Option<String>,
    /// Address line 2.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address2: Option<String>,
    /// City.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    /// Province/state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub province: Option<String>,
    /// Country.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    /// Postal/ZIP code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zip: Option<String>,
    /// Phone number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
}

/// Input for updating customer information.
#[derive(Debug, Default, Serialize)]
pub struct CustomerUpdateInput {
    /// First name.
    #[serde(rename = "firstName", skip_serializing_if = "Option::is_none")]
    pub first_name: Option<String>,
    /// Last name.
    #[serde(rename = "lastName", skip_serializing_if = "Option::is_none")]
    pub last_name: Option<String>,
    /// Phone number.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
    /// Whether the customer accepts marketing.
    #[serde(rename = "acceptsMarketing", skip_serializing_if = "Option::is_none")]
    pub accepts_marketing: Option<bool>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal Response Types
// ─────────────────────────────────────────────────────────────────────────────

/// User error from a mutation.
#[derive(Debug, Deserialize)]
pub(super) struct CustomerUserError {
    #[allow(dead_code)]
    pub field: Option<Vec<String>>,
    pub message: String,
    #[allow(dead_code)]
    pub code: Option<String>,
}
