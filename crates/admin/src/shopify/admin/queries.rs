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

/// Formatted string (rich text/HTML).
type FormattedString = String;

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
    response_derives = "Debug, Clone",
    skip_none
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
    response_derives = "Debug, Clone",
    skip_none
)]
pub struct CollectionUpdate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/collections.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CollectionUpdateFields;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/collections.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CollectionUpdateSortOrder;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/collections.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CollectionDelete;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/collections.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetCollectionWithProducts;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/collections.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CollectionAddProductsV2;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/collections.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CollectionRemoveProducts;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/collections.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetPublications;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/collections.graphql",
    response_derives = "Debug, Clone"
)]
pub struct PublishablePublish;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/collections.graphql",
    response_derives = "Debug, Clone"
)]
pub struct PublishableUnpublish;

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
pub struct GetOrderDetail;

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

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/orders.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderClose;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/orders.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderOpen;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/orders.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderCapture;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/orders.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderTagsAdd;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/orders.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderTagsRemove;

// =============================================================================
// Order Edit queries
// =============================================================================

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/order_edit.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderEditBegin;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/order_edit.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderEditAddVariant;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/order_edit.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderEditAddCustomItem;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/order_edit.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderEditSetQuantity;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/order_edit.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderEditAddLineItemDiscount;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/order_edit.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderEditUpdateDiscount;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/order_edit.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderEditRemoveDiscount;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/order_edit.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderEditAddShippingLine;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/order_edit.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderEditUpdateShippingLine;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/order_edit.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderEditRemoveShippingLine;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/order_edit.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderEditCommit;

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

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerCreate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone",
    skip_none
)]
pub struct CustomerUpdate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerDelete;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone"
)]
pub struct TagsAdd;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone"
)]
pub struct TagsRemove;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerEmailMarketingConsentUpdate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerSmsMarketingConsentUpdate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerAddTaxExemptions;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerRemoveTaxExemptions;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerGenerateAccountActivationUrl;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerSendAccountInviteEmail;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerMerge;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone"
)]
pub struct MetafieldsSet;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone"
)]
pub struct MetafieldsDelete;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerAddressCreate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerAddressUpdate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerAddressDelete;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/customers.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerUpdateDefaultAddress;

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
pub struct FulfillmentCreate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/fulfillments.graphql",
    response_derives = "Debug, Clone"
)]
pub struct FulfillmentTrackingInfoUpdate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/fulfillments.graphql",
    response_derives = "Debug, Clone"
)]
pub struct FulfillmentOrderHold;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/fulfillments.graphql",
    response_derives = "Debug, Clone"
)]
pub struct FulfillmentOrderReleaseHold;

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
// Return queries and mutations
// =============================================================================

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/admin/schema.json",
    query_path = "graphql/admin/queries/returns.graphql",
    response_derives = "Debug, Clone"
)]
pub struct ReturnCreate;

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
