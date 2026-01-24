//! Newtype IDs for type-safe entity references.
//!
//! # Future Implementation
//!
//! Use the `define_id!` macro to create type-safe ID wrappers:
//!
//! ```rust,ignore
//! macro_rules! define_id {
//!     ($name:ident) => {
//!         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
//!         #[serde(transparent)]
//!         pub struct $name(i64);
//!
//!         impl $name {
//!             pub fn new(id: i64) -> Self { Self(id) }
//!             pub fn as_i64(&self) -> i64 { self.0 }
//!         }
//!
//!         // SQLx Decode/Type implementations for PostgreSQL
//!         impl<'r> sqlx::Decode<'r, sqlx::Postgres> for $name {
//!             fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
//!                 let id = <i64 as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
//!                 Ok(Self(id))
//!             }
//!         }
//!
//!         impl sqlx::Type<sqlx::Postgres> for $name {
//!             fn type_info() -> sqlx::postgres::PgTypeInfo {
//!                 <i64 as sqlx::Type<sqlx::Postgres>>::type_info()
//!             }
//!         }
//!     };
//! }
//!
//! define_id!(UserId);
//! define_id!(ProductId);
//! define_id!(VariantId);
//! define_id!(OrderId);
//! define_id!(CartId);
//! define_id!(AddressId);
//! define_id!(AdminUserId);
//! define_id!(ChatSessionId);
//! define_id!(ChatMessageId);
//! ```

use serde::{Deserialize, Serialize};

/// A placeholder ID type until the macro is implemented.
///
/// TODO: Replace with `define_id!` macro for type-safe entity IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityId(i64);

impl EntityId {
    /// Create a new entity ID.
    #[must_use]
    pub const fn new(id: i64) -> Self {
        Self(id)
    }

    /// Get the underlying i64 value.
    #[must_use]
    pub const fn as_i64(&self) -> i64 {
        self.0
    }
}

impl From<i64> for EntityId {
    fn from(id: i64) -> Self {
        Self(id)
    }
}

impl From<EntityId> for i64 {
    fn from(id: EntityId) -> Self {
        id.0
    }
}
