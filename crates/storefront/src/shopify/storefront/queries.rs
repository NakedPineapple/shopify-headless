//! GraphQL query definitions for Shopify Storefront API.

use graphql_client::GraphQLQuery;

// Scalar types for Shopify GraphQL schema
// Must be defined in the same module where GraphQLQuery derive is used
// Note: These MUST match the GraphQL schema scalar names exactly (uppercase)
#[allow(clippy::upper_case_acronyms)]
type DateTime = String;
#[allow(clippy::upper_case_acronyms)]
type Decimal = String;
#[allow(clippy::upper_case_acronyms)]
type URL = String;
#[allow(clippy::upper_case_acronyms)]
type HTML = String;
#[allow(dead_code, clippy::upper_case_acronyms)]
type Color = String;
#[allow(dead_code, clippy::upper_case_acronyms)]
type JSON = serde_json::Value;
#[allow(dead_code)]
type UnsignedInt64 = String;

// Product queries
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/products.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetProductByHandle;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/products.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetProducts;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/products.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetProductRecommendations;

// Collection queries
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/collections.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetCollectionByHandle;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/collections.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetCollections;

// Cart mutations and queries
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/cart.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CreateCart;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/cart.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetCart;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/cart.graphql",
    response_derives = "Debug, Clone"
)]
pub struct AddToCart;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/cart.graphql",
    response_derives = "Debug, Clone"
)]
pub struct UpdateCartLines;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/cart.graphql",
    response_derives = "Debug, Clone"
)]
pub struct RemoveFromCart;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/cart.graphql",
    response_derives = "Debug, Clone"
)]
pub struct UpdateCartDiscountCodes;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/cart.graphql",
    response_derives = "Debug, Clone"
)]
pub struct UpdateCartNote;

// Customer mutations (Storefront API authentication)
#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/customer.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerCreate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/customer.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerAccessTokenCreate;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/customer.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerActivateByUrl;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/customer.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerRecover;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/customer.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerResetByUrl;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/customer.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerAccessTokenRenew;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/customer.graphql",
    response_derives = "Debug, Clone"
)]
pub struct CustomerAccessTokenDelete;

#[derive(GraphQLQuery)]
#[graphql(
    schema_path = "graphql/storefront/schema.json",
    query_path = "graphql/storefront/queries/customer.graphql",
    response_derives = "Debug, Clone"
)]
pub struct GetCustomerByToken;
