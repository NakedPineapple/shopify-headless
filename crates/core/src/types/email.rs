//! Email address type.

use core::fmt;

use serde::{Deserialize, Serialize};

/// Errors that can occur when parsing an [`Email`].
#[derive(thiserror::Error, Debug, Clone)]
pub enum EmailError {
    /// The input string is empty.
    #[error("email cannot be empty")]
    Empty,
    /// The input string is too long.
    #[error("email must be at most {max} characters")]
    TooLong {
        /// Maximum allowed length.
        max: usize,
    },
    /// The input does not contain an @ symbol.
    #[error("email must contain an @ symbol")]
    MissingAtSymbol,
    /// The local part (before @) is empty.
    #[error("email local part cannot be empty")]
    EmptyLocalPart,
    /// The domain part (after @) is empty.
    #[error("email domain cannot be empty")]
    EmptyDomain,
}

/// An email address.
///
/// This type provides basic validation for email addresses, ensuring they
/// have a valid structure with a local part and domain separated by an @ symbol.
///
/// ## Constraints
///
/// - Length: 1-254 characters (RFC 5321 limit)
/// - Must contain exactly one @ symbol
/// - Local part (before @) must not be empty
/// - Domain part (after @) must not be empty
///
/// ## Examples
///
/// ```
/// use naked_pineapple_core::Email;
///
/// // Valid emails
/// assert!(Email::parse("user@example.com").is_ok());
/// assert!(Email::parse("user.name+tag@domain.co.uk").is_ok());
///
/// // Invalid emails
/// assert!(Email::parse("").is_err());           // empty
/// assert!(Email::parse("no-at-symbol").is_err()); // missing @
/// assert!(Email::parse("@domain.com").is_err());  // empty local part
/// assert!(Email::parse("user@").is_err());        // empty domain
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct Email(String);

impl Email {
    /// Maximum length of an email address (RFC 5321).
    pub const MAX_LENGTH: usize = 254;

    /// Parse an `Email` from a string.
    ///
    /// # Errors
    ///
    /// Returns an error if the input:
    /// - Is empty
    /// - Is longer than 254 characters
    /// - Does not contain an @ symbol
    /// - Has an empty local part or domain
    pub fn parse(s: &str) -> Result<Self, EmailError> {
        if s.is_empty() {
            return Err(EmailError::Empty);
        }

        if s.len() > Self::MAX_LENGTH {
            return Err(EmailError::TooLong {
                max: Self::MAX_LENGTH,
            });
        }

        let at_pos = s.find('@').ok_or(EmailError::MissingAtSymbol)?;

        if at_pos == 0 {
            return Err(EmailError::EmptyLocalPart);
        }

        if at_pos == s.len() - 1 {
            return Err(EmailError::EmptyDomain);
        }

        Ok(Self(s.to_owned()))
    }

    /// Returns the email address as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the `Email` and returns its inner string.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }

    /// Returns the email address as a byte slice.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Returns the local part of the email (before the @).
    #[must_use]
    pub fn local_part(&self) -> &str {
        self.0.split('@').next().unwrap_or("")
    }

    /// Returns the domain part of the email (after the @).
    #[must_use]
    pub fn domain(&self) -> &str {
        self.0.split('@').nth(1).unwrap_or("")
    }
}

impl fmt::Display for Email {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for Email {
    type Err = EmailError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl AsRef<str> for Email {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

// SQLx support (with postgres feature)
#[cfg(feature = "postgres")]
impl sqlx::Type<sqlx::Postgres> for Email {
    fn type_info() -> sqlx::postgres::PgTypeInfo {
        // TEXT or CITEXT - both work
        <String as sqlx::Type<sqlx::Postgres>>::type_info()
    }

    fn compatible(ty: &sqlx::postgres::PgTypeInfo) -> bool {
        <String as sqlx::Type<sqlx::Postgres>>::compatible(ty)
    }
}

#[cfg(feature = "postgres")]
impl<'r> sqlx::Decode<'r, sqlx::Postgres> for Email {
    fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s = <String as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
        // Database values are assumed valid
        Ok(Self(s))
    }
}

#[cfg(feature = "postgres")]
impl sqlx::Encode<'_, sqlx::Postgres> for Email {
    fn encode_by_ref(
        &self,
        buf: &mut sqlx::postgres::PgArgumentBuffer,
    ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
        <String as sqlx::Encode<sqlx::Postgres>>::encode_by_ref(&self.0, buf)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_emails() {
        assert!(Email::parse("user@example.com").is_ok());
        assert!(Email::parse("user.name@example.com").is_ok());
        assert!(Email::parse("user+tag@example.com").is_ok());
        assert!(Email::parse("user@subdomain.example.com").is_ok());
        assert!(Email::parse("user@example.co.uk").is_ok());
        assert!(Email::parse("a@b.c").is_ok());
    }

    #[test]
    fn test_parse_empty() {
        assert!(matches!(Email::parse(""), Err(EmailError::Empty)));
    }

    #[test]
    fn test_parse_too_long() {
        let long = format!("{}@example.com", "a".repeat(250));
        assert!(matches!(
            Email::parse(&long),
            Err(EmailError::TooLong { .. })
        ));
    }

    #[test]
    fn test_parse_missing_at() {
        assert!(matches!(
            Email::parse("no-at-symbol"),
            Err(EmailError::MissingAtSymbol)
        ));
    }

    #[test]
    fn test_parse_empty_local_part() {
        assert!(matches!(
            Email::parse("@domain.com"),
            Err(EmailError::EmptyLocalPart)
        ));
    }

    #[test]
    fn test_parse_empty_domain() {
        assert!(matches!(
            Email::parse("user@"),
            Err(EmailError::EmptyDomain)
        ));
    }

    #[test]
    fn test_local_part() {
        let email = Email::parse("user@example.com").unwrap();
        assert_eq!(email.local_part(), "user");
    }

    #[test]
    fn test_domain() {
        let email = Email::parse("user@example.com").unwrap();
        assert_eq!(email.domain(), "example.com");
    }

    #[test]
    fn test_display() {
        let email = Email::parse("user@example.com").unwrap();
        assert_eq!(format!("{email}"), "user@example.com");
    }

    #[test]
    fn test_serde_roundtrip() {
        let email = Email::parse("user@example.com").unwrap();
        let json = serde_json::to_string(&email).unwrap();
        assert_eq!(json, "\"user@example.com\"");

        let parsed: Email = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, email);
    }

    #[test]
    fn test_from_str() {
        let email: Email = "user@example.com".parse().unwrap();
        assert_eq!(email.as_str(), "user@example.com");
    }

    #[test]
    fn test_as_ref() {
        let email = Email::parse("user@example.com").unwrap();
        let s: &str = email.as_ref();
        assert_eq!(s, "user@example.com");
    }
}
