//! Type conversion functions for Shopify Storefront API responses.

pub mod cart;
pub mod collections;
pub mod products;

pub use cart::{
    CartData, convert_add_user_error, convert_cart, convert_discount_user_error,
    convert_note_user_error, convert_remove_user_error, convert_update_user_error,
    convert_user_error,
};
pub use collections::{convert_collection, convert_collection_connection};
pub use products::{convert_product, convert_product_connection, convert_product_recommendation};
