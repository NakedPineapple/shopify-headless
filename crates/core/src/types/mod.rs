//! Core types for Naked Pineapple.
//!
//! This module provides type-safe wrappers for common domain concepts.

pub mod credential;
pub mod email;
pub mod id;
pub mod price;
pub mod status;

pub use credential::{StoredPasskey, WebAuthnCredentialId};
pub use email::{Email, EmailError};
pub use id::*;
pub use price::Price;
pub use status::*;
