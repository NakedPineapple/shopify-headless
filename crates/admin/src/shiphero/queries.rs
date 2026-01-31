//! GraphQL query definitions for `ShipHero` API.
//!
//! Uses `graphql_client` to generate type-safe Rust code from GraphQL queries.

use graphql_client::GraphQLQuery;

// =============================================================================
// Custom scalar type aliases (used by graphql_client)
// =============================================================================

/// ISO 8601 date-time string.
type ISODateTime = String;

/// Date string (YYYY-MM-DD format).
type Date = String;

/// Generic JSON value.
#[allow(clippy::upper_case_acronyms)]
type JSON = serde_json::Value;

/// URL string.
#[allow(clippy::upper_case_acronyms)]
type URL = String;

/// Generic ID type.
#[allow(clippy::upper_case_acronyms)]
type ID = String;

// =============================================================================
// Order queries
// =============================================================================

/// Get orders awaiting fulfillment.
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/shiphero/schema.json",
    query_path = "graphql/shiphero/queries/orders.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetPendingOrders;

/// Get a single order by ID.
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/shiphero/schema.json",
    query_path = "graphql/shiphero/queries/orders.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetOrder;

/// Get order history/timeline.
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/shiphero/schema.json",
    query_path = "graphql/shiphero/queries/orders.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetOrderHistory;

// =============================================================================
// Shipment queries
// =============================================================================

/// Get recent shipments.
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/shiphero/schema.json",
    query_path = "graphql/shiphero/queries/shipments.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetShipments;

/// Get a single shipment by ID.
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/shiphero/schema.json",
    query_path = "graphql/shiphero/queries/shipments.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetShipment;

/// Get packs/shipments per day for dashboard metrics.
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/shiphero/schema.json",
    query_path = "graphql/shiphero/queries/shipments.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetPacksPerDay;

// =============================================================================
// Inventory queries
// =============================================================================

/// Get products with inventory levels.
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/shiphero/schema.json",
    query_path = "graphql/shiphero/queries/inventory.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetProducts;

/// Get a single product by SKU.
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/shiphero/schema.json",
    query_path = "graphql/shiphero/queries/inventory.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetProductBySku;

/// Get warehouse products with detailed inventory.
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/shiphero/schema.json",
    query_path = "graphql/shiphero/queries/inventory.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetWarehouseProducts;

/// Get locations (bins) in warehouse.
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/shiphero/schema.json",
    query_path = "graphql/shiphero/queries/inventory.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetLocations;

/// Get inventory changes/history.
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/shiphero/schema.json",
    query_path = "graphql/shiphero/queries/inventory.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetInventoryChanges;

/// Get lot/batch information for products with expiration.
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/shiphero/schema.json",
    query_path = "graphql/shiphero/queries/inventory.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetExpirationLots;

/// Get inventory snapshot for a point-in-time view.
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/shiphero/schema.json",
    query_path = "graphql/shiphero/queries/inventory.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetInventorySnapshot;
