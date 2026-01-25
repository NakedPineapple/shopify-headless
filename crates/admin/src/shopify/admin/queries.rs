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
