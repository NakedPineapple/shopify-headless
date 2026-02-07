//! GraphQL query definitions for Shopify Customer Account API.

use graphql_client::GraphQLQuery;

// Scalar types for Shopify Customer Account API GraphQL schema.
// Must be defined in the same module where GraphQLQuery derive is used.
// These MUST match the GraphQL schema scalar names exactly (uppercase).
#[allow(clippy::upper_case_acronyms)]
type DateTime = String;
#[allow(clippy::upper_case_acronyms)]
type Decimal = String;
#[allow(clippy::upper_case_acronyms)]
type URL = String;
#[allow(clippy::upper_case_acronyms)]
type HTML = String;
#[allow(clippy::upper_case_acronyms)]
type JSON = serde_json::Value;
type UnsignedInt64 = String;

// ─────────────────────────────────────────────────────────────────────────────
// Customer Queries
// ─────────────────────────────────────────────────────────────────────────────

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/customer/schema.json",
    query_path = "graphql/customer/queries/customer.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetCustomer;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/customer/schema.json",
    query_path = "graphql/customer/queries/customer.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerUpdate;

// ─────────────────────────────────────────────────────────────────────────────
// Address Queries
// ─────────────────────────────────────────────────────────────────────────────

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/customer/schema.json",
    query_path = "graphql/customer/queries/addresses.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetAddresses;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/customer/schema.json",
    query_path = "graphql/customer/queries/addresses.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerAddressCreate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/customer/schema.json",
    query_path = "graphql/customer/queries/addresses.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerAddressUpdate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/customer/schema.json",
    query_path = "graphql/customer/queries/addresses.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerAddressDelete;

// ─────────────────────────────────────────────────────────────────────────────
// Order Queries
// ─────────────────────────────────────────────────────────────────────────────

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/customer/schema.json",
    query_path = "graphql/customer/queries/orders.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetOrders;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/customer/schema.json",
    query_path = "graphql/customer/queries/orders.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetOrder;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/customer/schema.json",
    query_path = "graphql/customer/queries/orders.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetOrderForReturn;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/customer/schema.json",
    query_path = "graphql/customer/queries/orders.graphql",
    response_derives = "Debug, Clone"
)]
pub struct OrderRequestReturn;

// ─────────────────────────────────────────────────────────────────────────────
// Subscription Queries
// ─────────────────────────────────────────────────────────────────────────────

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/customer/schema.json",
    query_path = "graphql/customer/queries/subscriptions.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetSubscriptions;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/customer/schema.json",
    query_path = "graphql/customer/queries/subscriptions.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetSubscription;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/customer/schema.json",
    query_path = "graphql/customer/queries/subscriptions.graphql",
    response_derives = "Debug, Clone"
)]
pub struct PauseSubscription;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/customer/schema.json",
    query_path = "graphql/customer/queries/subscriptions.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CancelSubscription;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/customer/schema.json",
    query_path = "graphql/customer/queries/subscriptions.graphql",
    response_derives = "Debug, Clone"
)]
pub struct ActivateSubscription;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/customer/schema.json",
    query_path = "graphql/customer/queries/subscriptions.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetUpcomingBillingCycles;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/customer/schema.json",
    query_path = "graphql/customer/queries/subscriptions.graphql",
    response_derives = "Debug, Clone"
)]
pub struct SkipBillingCycle;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/customer/schema.json",
    query_path = "graphql/customer/queries/subscriptions.graphql",
    response_derives = "Debug, Clone"
)]
pub struct UnskipBillingCycle;

// ─────────────────────────────────────────────────────────────────────────────
// Store Credit Queries
// ─────────────────────────────────────────────────────────────────────────────

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/customer/schema.json",
    query_path = "graphql/customer/queries/store_credit.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetStoreCredit;
