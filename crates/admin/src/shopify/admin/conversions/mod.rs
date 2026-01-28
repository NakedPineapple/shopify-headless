//! Type conversions from GraphQL response types to domain types.
//!
//! These functions convert the generated `graphql_client` types
//! into our clean domain types.

mod customers;
mod inventory;
mod orders;
mod products;

pub use customers::{convert_customer, convert_customer_connection};
pub use inventory::{convert_inventory_level_connection, convert_location_connection};
pub use orders::{
    convert_fulfillment_orders, convert_order, convert_order_connection,
    convert_order_list_connection,
};
pub use products::{convert_product, convert_product_connection};

/// Convert a currency code enum to string.
pub fn currency_code_to_string<T: std::fmt::Debug>(code: T) -> String {
    format!("{code:?}")
}
