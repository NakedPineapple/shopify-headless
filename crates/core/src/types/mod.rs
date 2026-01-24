//! Core types for Naked Pineapple.
//!
//! This module provides type-safe wrappers for common domain concepts.

pub mod email;
pub mod id;
pub mod price;
pub mod status;

pub use email::Email;
pub use id::*;
pub use price::Price;
pub use status::*;
