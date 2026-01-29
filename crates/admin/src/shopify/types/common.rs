//! Common domain types shared across Shopify Admin API.

use serde::{Deserialize, Serialize};

// =============================================================================
// Money Types
// =============================================================================

/// Monetary amount with currency code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    /// Decimal amount as string (preserves precision).
    pub amount: String,
    /// ISO 4217 currency code.
    pub currency_code: String,
}

// =============================================================================
// Image Types
// =============================================================================

/// Product or media image.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Image {
    /// Shopify image ID.
    pub id: Option<String>,
    /// Image URL.
    pub url: String,
    /// Alt text for accessibility.
    pub alt_text: Option<String>,
    /// Image width in pixels.
    pub width: Option<i64>,
    /// Image height in pixels.
    pub height: Option<i64>,
}

// =============================================================================
// Address Types
// =============================================================================

/// Mailing address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Address {
    /// Address ID (for mutations).
    pub id: Option<String>,
    /// First line of the address.
    pub address1: Option<String>,
    /// Second line of the address.
    pub address2: Option<String>,
    /// City.
    pub city: Option<String>,
    /// Province or state code.
    pub province_code: Option<String>,
    /// Country code (ISO 3166-1 alpha-2).
    pub country_code: Option<String>,
    /// Postal/ZIP code.
    pub zip: Option<String>,
    /// First name.
    pub first_name: Option<String>,
    /// Last name.
    pub last_name: Option<String>,
    /// Company name.
    pub company: Option<String>,
    /// Phone number.
    pub phone: Option<String>,
}

/// Input for creating/updating a mailing address.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AddressInput {
    /// First line of the address.
    pub address1: Option<String>,
    /// Second line of the address.
    pub address2: Option<String>,
    /// City.
    pub city: Option<String>,
    /// Province or state code.
    pub province_code: Option<String>,
    /// Country code (ISO 3166-1 alpha-2).
    pub country_code: Option<String>,
    /// Postal/ZIP code.
    pub zip: Option<String>,
    /// First name.
    pub first_name: Option<String>,
    /// Last name.
    pub last_name: Option<String>,
    /// Company name.
    pub company: Option<String>,
    /// Phone number.
    pub phone: Option<String>,
}

// =============================================================================
// Metafield Types
// =============================================================================

/// A metafield for storing custom data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metafield {
    /// Metafield ID.
    pub id: Option<String>,
    /// Namespace for grouping metafields.
    pub namespace: String,
    /// Key within the namespace.
    pub key: String,
    /// The metafield value.
    pub value: String,
}

/// Input for creating/updating metafields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetafieldInput {
    /// Namespace for the metafield.
    pub namespace: String,
    /// Key within the namespace.
    pub key: String,
    /// The value to store.
    pub value: String,
    /// The metafield type (e.g., `single_line_text_field`, `number_integer`).
    pub type_: String,
}

/// Parameters for updating a customer.
#[derive(Debug, Clone, Default)]
pub struct CustomerUpdateParams {
    /// Email address.
    pub email: Option<String>,
    /// First name.
    pub first_name: Option<String>,
    /// Last name.
    pub last_name: Option<String>,
    /// Phone number.
    pub phone: Option<String>,
    /// Note about the customer.
    pub note: Option<String>,
    /// Tags to set on the customer.
    pub tags: Option<Vec<String>>,
}

/// Override settings for customer merge operation.
///
/// Each field indicates whether to take the value from the source customer
/// (being merged) instead of the target customer (that remains).
// Allow: Each boolean represents an independent merge override choice from
// the Shopify API with no logical grouping into enums or state machines.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Default)]
pub struct CustomerMergeOverrides {
    /// Take first name from the source customer.
    pub first_name: bool,
    /// Take last name from the source customer.
    pub last_name: bool,
    /// Take email from the source customer.
    pub email: bool,
    /// Take phone from the source customer.
    pub phone: bool,
    /// Take default address from the source customer.
    pub default_address: bool,
}

/// Identifier for a metafield (used in delete operations).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetafieldIdentifier {
    /// The owner resource ID.
    pub owner_id: String,
    /// Namespace of the metafield.
    pub namespace: String,
    /// Key of the metafield.
    pub key: String,
}

// =============================================================================
// Pagination Types
// =============================================================================

/// Pagination information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageInfo {
    /// Whether there are more items after this page.
    pub has_next_page: bool,
    /// Whether there are items before this page.
    pub has_previous_page: bool,
    /// Cursor for the first item.
    pub start_cursor: Option<String>,
    /// Cursor for the last item.
    pub end_cursor: Option<String>,
}
