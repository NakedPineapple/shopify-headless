//! GraphQL query definitions for Shopify Admin API.
//!
//! Uses `graphql_client` to generate type-safe Rust code from GraphQL queries.

use graphql_client::GraphQLQuery;

// =============================================================================
// Custom scalar type aliases (used by graphql_client)
// =============================================================================

/// ISO 8601 date-time string.
type DateTime = String;

/// Decimal number as string (preserves precision).
type Decimal = String;

/// Money amount as decimal string.
type Money = String;

/// URL string.
#[allow(clippy::upper_case_acronyms)]
type URL = String;

/// HTML string.
#[allow(clippy::upper_case_acronyms)]
type HTML = String;

/// Unsigned 64-bit integer as string.
type UnsignedInt64 = String;

/// Date string (YYYY-MM-DD format).
type Date = String;

// =============================================================================
// Product queries
// =============================================================================

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/products.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetProduct;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/products.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetProducts;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/products.graphql",
    response_derives = "Debug, Clone"
)]
pub struct ProductCreate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/products.graphql",
    response_derives = "Debug, Clone"
)]
pub struct ProductUpdate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/products.graphql",
    response_derives = "Debug, Clone"
)]
pub struct ProductDelete;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/products.graphql",
    response_derives = "Debug, Clone"
)]
pub struct ProductVariantsBulkUpdate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/products.graphql",
    response_derives = "Debug, Clone"
)]
pub struct StagedUploadsCreate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/products.graphql",
    response_derives = "Debug, Clone"
)]
pub struct ProductCreateMedia;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/products.graphql",
    response_derives = "Debug, Clone"
)]
pub struct FileDelete;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/products.graphql",
    response_derives = "Debug, Clone"
)]
pub struct ProductReorderMedia;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/products.graphql",
    response_derives = "Debug, Clone"
)]
pub struct FileUpdate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/products.graphql",
    response_derives = "Debug, Clone"
)]
pub struct ProductSetMedia;

// =============================================================================
// Collection queries and mutations
// =============================================================================

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/collections.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetCollection;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/collections.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetCollections;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/collections.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CollectionCreate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/collections.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CollectionUpdate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/collections.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CollectionDelete;

// =============================================================================
// Order queries
// =============================================================================

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/orders.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetOrder;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/orders.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetOrders;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/orders.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderUpdate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/orders.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderMarkAsPaid;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/orders.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderCancel;

// =============================================================================
// Customer queries
// =============================================================================

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetCustomer;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetCustomers;

// =============================================================================
// Inventory queries and mutations
// =============================================================================

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/inventory.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetInventoryLevels;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/inventory.graphql",
    response_derives = "Debug, Clone"
)]
pub struct InventoryAdjustQuantities;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/inventory.graphql",
    response_derives = "Debug, Clone"
)]
pub struct InventorySetQuantities;

// =============================================================================
// Gift Card queries and mutations
// =============================================================================

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/gift_cards.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetGiftCards;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/gift_cards.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetGiftCard;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/gift_cards.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GiftCardCreate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/gift_cards.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GiftCardUpdate;

// =============================================================================
// Discount queries and mutations
// =============================================================================

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/discounts.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetDiscountCodes;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/discounts.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetDiscountCode;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/discounts.graphql",
    response_derives = "Debug, Clone"
)]
pub struct DiscountCodeBasicCreate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/discounts.graphql",
    response_derives = "Debug, Clone"
)]
pub struct DiscountCodeBasicUpdate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/discounts.graphql",
    response_derives = "Debug, Clone"
)]
pub struct DiscountCodeDeactivate;

// =============================================================================
// Fulfillment queries and mutations
// =============================================================================

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/fulfillments.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetFulfillmentOrders;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/fulfillments.graphql",
    response_derives = "Debug, Clone"
)]
pub struct FulfillmentCreateV2;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/fulfillments.graphql",
    response_derives = "Debug, Clone"
)]
pub struct FulfillmentTrackingInfoUpdateV2;

// =============================================================================
// Refund queries and mutations
// =============================================================================

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/refunds.graphql",
    response_derives = "Debug, Clone"
)]
pub struct SuggestedRefund;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/refunds.graphql",
    response_derives = "Debug, Clone"
)]
pub struct RefundCreate;

// =============================================================================
// Location and inventory queries
// =============================================================================

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/inventory.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetLocations;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/inventory.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetProductsWithInventory;

// =============================================================================
// Payout queries
// =============================================================================

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/payouts.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetPayout;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/payouts.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetPayouts;
