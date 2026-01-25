//! Newtype IDs for type-safe entity references.
//!
//! Use the `define_id!` macro to create type-safe ID wrappers that prevent
//! accidentally mixing IDs from different entity types.

use serde::{Deserialize, Serialize};

/// Macro to define a type-safe ID wrapper.
///
/// Creates a newtype wrapper around `i32` with:
/// - `Serialize`/`Deserialize` with `#[serde(transparent)]`
/// - `Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`, `Hash`
/// - Conversion methods: `new()`, `as_i32()`
/// - `From<i32>` and `Into<i32>` implementations
/// - `sqlx` `Type`, `Encode`, and `Decode` implementations (with `postgres` feature)
///
/// # Example
///
/// ```rust
/// # use naked_pineapple_core::define_id;
/// define_id!(UserId);
/// define_id!(OrderId);
///
/// let user_id = UserId::new(1);
/// let order_id = OrderId::new(1);
///
/// // These are different types, so this won't compile:
/// // let _: UserId = order_id;
/// ```
#[macro_export]
macro_rules! define_id {
    ($name:ident) => {
        #[derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            Hash,
            ::serde::Serialize,
            ::serde::Deserialize
        )]
        #[serde(transparent)]
        pub struct $name(i32);

        impl $name {
            /// Create a new ID from an i32 value.
            #[must_use]
            pub const fn new(id: i32) -> Self {
                Self(id)
            }

            /// Get the underlying i32 value.
            #[must_use]
            pub const fn as_i32(&self) -> i32 {
                self.0
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl From<i32> for $name {
            fn from(id: i32) -> Self {
                Self(id)
            }
        }

        impl From<$name> for i32 {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        #[cfg(feature = "postgres")]
        impl ::sqlx::Type<::sqlx::Postgres> for $name {
            fn type_info() -> ::sqlx::postgres::PgTypeInfo {
                <i32 as ::sqlx::Type<::sqlx::Postgres>>::type_info()
            }

            fn compatible(ty: &::sqlx::postgres::PgTypeInfo) -> bool {
                <i32 as ::sqlx::Type<::sqlx::Postgres>>::compatible(ty)
            }
        }

        #[cfg(feature = "postgres")]
        impl<'r> ::sqlx::Decode<'r, ::sqlx::Postgres> for $name {
            fn decode(
                value: ::sqlx::postgres::PgValueRef<'r>,
            ) -> ::core::result::Result<Self, ::sqlx::error::BoxDynError> {
                let id = <i32 as ::sqlx::Decode<::sqlx::Postgres>>::decode(value)?;
                Ok(Self(id))
            }
        }

        #[cfg(feature = "postgres")]
        impl ::sqlx::Encode<'_, ::sqlx::Postgres> for $name {
            fn encode_by_ref(
                &self,
                buf: &mut ::sqlx::postgres::PgArgumentBuffer,
            ) -> ::std::result::Result<::sqlx::encode::IsNull, ::sqlx::error::BoxDynError> {
                <i32 as ::sqlx::Encode<::sqlx::Postgres>>::encode_by_ref(&self.0, buf)
            }
        }
    };
}

// Define standard entity IDs
define_id!(UserId);
define_id!(CredentialId);
define_id!(ProductId);
define_id!(VariantId);
define_id!(OrderId);
define_id!(CartId);
define_id!(AddressId);
define_id!(AdminUserId);
define_id!(ChatSessionId);
define_id!(ChatMessageId);

/// A generic placeholder ID type for migration purposes.
///
/// Prefer using specific ID types like `UserId`, `OrderId`, etc.
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
