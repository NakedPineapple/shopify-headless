//! Customer and collection domain types for Shopify Admin API.

use serde::{Deserialize, Serialize};

use super::common::{Address, Image, Money, PageInfo};

// =============================================================================
// Customer Types
// =============================================================================

/// Customer account state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CustomerState {
    /// Customer has not yet accepted the invite.
    Disabled,
    /// Customer has accepted the invite.
    Enabled,
    /// Customer was invited but hasn't accepted.
    Invited,
    /// Customer account was declined.
    Declined,
}

/// Sort key for customer lists.
///
/// Some keys are supported natively by Shopify API, others require
/// client-side sorting in Rust after fetching data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CustomerSortKey {
    // === Shopify API supported ===
    /// Sort by creation date (Shopify native).
    CreatedAt,
    /// Sort by ID (Shopify native).
    Id,
    /// Sort by location (Shopify native).
    Location,
    /// Sort by name (Shopify native).
    #[default]
    Name,
    /// Sort by relevance for search queries (Shopify native).
    Relevance,
    /// Sort by last update date (Shopify native).
    UpdatedAt,

    // === Client-side sorting (Rust) ===
    /// Sort by total amount spent.
    AmountSpent,
    /// Sort by total orders count.
    OrdersCount,
}

impl CustomerSortKey {
    /// Parse a sort key from a URL parameter string.
    #[must_use]
    pub fn from_str_param(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "created_at" | "created" => Some(Self::CreatedAt),
            "id" => Some(Self::Id),
            "location" => Some(Self::Location),
            "name" => Some(Self::Name),
            "relevance" => Some(Self::Relevance),
            "updated_at" | "updated" => Some(Self::UpdatedAt),
            "amount_spent" | "spent" => Some(Self::AmountSpent),
            "orders_count" | "orders" => Some(Self::OrdersCount),
            _ => None,
        }
    }

    /// Get the URL parameter string for this sort key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CreatedAt => "created_at",
            Self::Id => "id",
            Self::Location => "location",
            Self::Name => "name",
            Self::Relevance => "relevance",
            Self::UpdatedAt => "updated_at",
            Self::AmountSpent => "amount_spent",
            Self::OrdersCount => "orders_count",
        }
    }

    /// Whether this sort key is supported natively by Shopify API.
    #[must_use]
    pub const fn is_shopify_native(self) -> bool {
        matches!(
            self,
            Self::CreatedAt
                | Self::Id
                | Self::Location
                | Self::Name
                | Self::Relevance
                | Self::UpdatedAt
        )
    }
}

/// Parameters for listing customers.
#[derive(Debug, Clone, Default)]
pub struct CustomerListParams {
    /// Maximum number of customers to return.
    pub first: Option<i64>,
    /// Cursor for pagination.
    pub after: Option<String>,
    /// Search/filter query string (Shopify query syntax).
    pub query: Option<String>,
    /// Sort key.
    pub sort_key: Option<CustomerSortKey>,
    /// Whether to reverse sort order.
    pub reverse: bool,
}

/// Marketing consent state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MarketingState {
    /// Not subscribed to marketing.
    NotSubscribed,
    /// Pending confirmation.
    Pending,
    /// Subscribed to marketing.
    Subscribed,
    /// Unsubscribed from marketing.
    Unsubscribed,
    /// Data has been redacted.
    Redacted,
    /// Invalid state.
    Invalid,
}

/// Marketing consent information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketingConsent {
    /// Current marketing state.
    pub state: MarketingState,
    /// Opt-in level (e.g., `SINGLE_OPT_IN`, `CONFIRMED_OPT_IN`).
    pub opt_in_level: Option<String>,
    /// When consent was last updated.
    pub consent_updated_at: Option<String>,
}

/// A customer's recent order (for detail view).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerOrder {
    /// Order ID.
    pub id: String,
    /// Order name (e.g., "#1001").
    pub name: String,
    /// Creation timestamp.
    pub created_at: String,
    /// Financial status display string.
    pub financial_status: Option<String>,
    /// Fulfillment status display string.
    pub fulfillment_status: Option<String>,
    /// Total price.
    pub total_price: Money,
}

/// A customer in the admin.
// Allow: Shopify API Customer object has independent boolean properties
// (accepts_marketing, tax_exempt, can_delete, is_mergeable) that cannot be grouped.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Customer {
    /// Customer ID.
    pub id: String,
    /// Email address.
    pub email: Option<String>,
    /// First name.
    pub first_name: Option<String>,
    /// Last name.
    pub last_name: Option<String>,
    /// Display name.
    pub display_name: String,
    /// Phone number.
    pub phone: Option<String>,
    /// Account state.
    pub state: CustomerState,
    /// Customer locale (language preference).
    pub locale: Option<String>,
    /// Whether email marketing is accepted (legacy field).
    pub accepts_marketing: bool,
    /// Marketing opt-in timestamp (legacy field).
    pub accepts_marketing_updated_at: Option<String>,
    /// Email marketing consent details.
    pub email_marketing_consent: Option<MarketingConsent>,
    /// SMS marketing consent details.
    pub sms_marketing_consent: Option<MarketingConsent>,
    /// Total orders count.
    pub orders_count: i64,
    /// Total amount spent.
    pub total_spent: Money,
    /// Human-readable lifetime duration (e.g., "2 years").
    pub lifetime_duration: Option<String>,
    /// Whether customer is tax exempt.
    pub tax_exempt: bool,
    /// List of tax exemption codes.
    pub tax_exemptions: Vec<String>,
    /// Customer note.
    pub note: Option<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Whether this customer can be deleted (no orders).
    pub can_delete: bool,
    /// Whether this customer can be merged with another.
    pub is_mergeable: bool,
    /// Default address.
    pub default_address: Option<Address>,
    /// All addresses.
    pub addresses: Vec<Address>,
    /// Recent orders (populated on detail view).
    pub recent_orders: Vec<CustomerOrder>,
    /// Creation timestamp.
    pub created_at: String,
    /// Last update timestamp.
    pub updated_at: String,
}

/// Paginated list of customers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerConnection {
    /// Customers in this page.
    pub customers: Vec<Customer>,
    /// Pagination info.
    pub page_info: PageInfo,
}

// =============================================================================
// Collection Types
// =============================================================================

/// A rule that defines membership in a smart collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionRule {
    /// The attribute column to check (TAG, TITLE, VENDOR, `PRODUCT_TYPE`, etc.).
    pub column: String,
    /// The relation operator (EQUALS, `NOT_EQUALS`, CONTAINS, etc.).
    pub relation: String,
    /// The value to match against.
    pub condition: String,
}

/// A set of rules that define a smart collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionRuleSet {
    /// If true, products matching ANY rule are included (OR logic).
    /// If false, products must match ALL rules (AND logic).
    pub applied_disjunctively: bool,
    /// The individual rules in this set.
    pub rules: Vec<CollectionRule>,
}

/// SEO metadata for a collection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CollectionSeo {
    /// SEO title (shown in search results).
    pub title: Option<String>,
    /// SEO meta description.
    pub description: Option<String>,
}

/// A sales channel/publication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Publication {
    /// Publication ID.
    pub id: String,
    /// Publication name (e.g., "Online Store", "TikTok").
    pub name: String,
}

/// Publication status for a resource.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePublication {
    /// The publication/sales channel.
    pub publication: Publication,
    /// Whether the resource is published on this channel.
    pub is_published: bool,
}

/// A product collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    /// Collection ID.
    pub id: String,
    /// Collection title.
    pub title: String,
    /// URL handle.
    pub handle: String,
    /// Plain text description.
    pub description: String,
    /// HTML description.
    pub description_html: Option<String>,
    /// Number of products in the collection.
    pub products_count: i64,
    /// Collection image.
    pub image: Option<Image>,
    /// Last update timestamp.
    pub updated_at: Option<String>,
    /// Rule set for smart collections (None for manual collections).
    pub rule_set: Option<CollectionRuleSet>,
    /// Sort order for products in the collection.
    pub sort_order: Option<String>,
    /// SEO metadata.
    pub seo: Option<CollectionSeo>,
    /// Publication status on each sales channel.
    pub publications: Vec<ResourcePublication>,
}

/// A product within a collection (simplified view).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionProduct {
    /// Product ID.
    pub id: String,
    /// Product title.
    pub title: String,
    /// URL handle.
    pub handle: String,
    /// Product status (ACTIVE, DRAFT, ARCHIVED).
    pub status: String,
    /// Featured image URL.
    pub image_url: Option<String>,
    /// Total inventory quantity.
    pub total_inventory: i64,
    /// Minimum variant price.
    pub price: String,
    /// Currency code.
    pub currency_code: String,
}

/// A collection with its products.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionWithProducts {
    /// The collection.
    pub collection: Collection,
    /// Products in this collection.
    pub products: Vec<CollectionProduct>,
    /// Whether there are more products to load.
    pub has_next_page: bool,
    /// Cursor for loading more products.
    pub end_cursor: Option<String>,
}

/// Paginated list of collections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollectionConnection {
    /// Collections in this page.
    pub collections: Vec<Collection>,
    /// Pagination info.
    pub page_info: PageInfo,
}
