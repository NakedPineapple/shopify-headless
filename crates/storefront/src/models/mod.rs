//! Domain models for storefront.
//!
//! These types represent validated domain objects used throughout the application.

pub mod session;
pub mod user;

pub use session::{CurrentUser, keys as session_keys};
pub use user::{User, UserCredential};
