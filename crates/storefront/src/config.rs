//! Storefront configuration loaded from environment variables.
//!
//! # Environment Variables
//!
//! ## Required
//! - `STOREFRONT_DATABASE_URL` - `PostgreSQL` connection string
//! - `STOREFRONT_BASE_URL` - Public URL for the storefront
//! - `STOREFRONT_SESSION_SECRET` - Session signing secret (min 32 chars, high entropy)
//! - `SHOPIFY_STORE` - Shopify store domain (e.g., your-store.myshopify.com)
//! - `SHOPIFY_STOREFRONT_PUBLIC_TOKEN` - Storefront API public access token
//! - `SHOPIFY_STOREFRONT_PRIVATE_TOKEN` - Storefront API private access token
//! - `SHOPIFY_CUSTOMER_CLIENT_ID` - Customer Account API OAuth client ID
//! - `SHOPIFY_CUSTOMER_CLIENT_SECRET` - Customer Account API OAuth client secret
//!
//! ## Optional
//! - `STOREFRONT_HOST` - Bind address (default: 127.0.0.1)
//! - `STOREFRONT_PORT` - Listen port (default: 3000)
//! - `SHOPIFY_API_VERSION` - API version (default: 2026-01)
//! - `GA4_MEASUREMENT_ID` - Google Analytics 4 measurement ID
//! - `META_PIXEL_ID` - Meta (Facebook) pixel ID
//! - `TIKTOK_PIXEL_ID` - TikTok pixel ID
//! - `PINTEREST_TAG_ID` - Pinterest tag ID
//! - `GOOGLE_ADS_ID` - Google Ads conversion ID
//! - `GOOGLE_ADS_CONVERSION_LABEL` - Google Ads conversion label
//! - `SENTRY_DSN` - Sentry error tracking DSN

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};

use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

const MIN_SESSION_SECRET_LENGTH: usize = 32;
const MIN_ENTROPY_BITS_PER_CHAR: f64 = 3.3;

/// Blocklist of common placeholder patterns (case-insensitive)
const PLACEHOLDER_PATTERNS: &[&str] = &[
    "your-",
    "changeme",
    "replace",
    "placeholder",
    "example",
    "secret",
    "password",
    "xxx",
    "todo",
    "fixme",
    "insert",
    "enter-",
    "put-your",
    "add-your",
];

/// Configuration errors that can occur during loading.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Missing environment variable: {0}")]
    MissingEnvVar(String),
    #[error("Invalid environment variable {0}: {1}")]
    InvalidEnvVar(String, String),
    #[error("Insecure secret in {0}: {1}")]
    InsecureSecret(String, String),
}

/// Storefront application configuration.
#[derive(Debug, Clone)]
pub struct StorefrontConfig {
    /// `PostgreSQL` database connection URL (contains password)
    pub database_url: SecretString,
    /// IP address to bind the server to
    pub host: IpAddr,
    /// Port to listen on
    pub port: u16,
    /// Public base URL for the storefront
    pub base_url: String,
    /// Session signing secret
    pub session_secret: SecretString,
    /// Shopify Storefront API configuration
    pub shopify: ShopifyStorefrontConfig,
    /// Analytics tracking configuration
    pub analytics: AnalyticsConfig,
    /// Sentry DSN for error tracking
    pub sentry_dsn: Option<String>,
}

/// Shopify Storefront API configuration.
///
/// Implements `Debug` manually to redact secret fields.
#[derive(Clone)]
pub struct ShopifyStorefrontConfig {
    /// Shopify store domain (e.g., your-store.myshopify.com)
    pub store: String,
    /// Shopify API version (e.g., 2026-01)
    pub api_version: String,
    /// Storefront API public access token (safe to expose in browser)
    pub storefront_public_token: String,
    /// Storefront API private access token (server-side only)
    pub storefront_private_token: SecretString,
    /// Customer Account API numeric shop ID (found in Shopify admin URL)
    pub customer_shop_id: String,
    /// Customer Account API OAuth client ID
    pub customer_client_id: String,
    /// Customer Account API OAuth client secret
    pub customer_client_secret: SecretString,
}

impl std::fmt::Debug for ShopifyStorefrontConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShopifyStorefrontConfig")
            .field("store", &self.store)
            .field("api_version", &self.api_version)
            .field("storefront_public_token", &self.storefront_public_token)
            .field("storefront_private_token", &"[REDACTED]")
            .field("customer_shop_id", &self.customer_shop_id)
            .field("customer_client_id", &self.customer_client_id)
            .field("customer_client_secret", &"[REDACTED]")
            .finish()
    }
}

/// Analytics and tracking pixel configuration.
#[derive(Debug, Clone, Default)]
pub struct AnalyticsConfig {
    /// Google Analytics 4 measurement ID
    pub ga4_measurement_id: Option<String>,
    /// Meta (Facebook) pixel ID
    pub meta_pixel_id: Option<String>,
    /// TikTok pixel ID
    pub tiktok_pixel_id: Option<String>,
    /// Pinterest tag ID
    pub pinterest_tag_id: Option<String>,
    /// Google Ads conversion ID
    pub google_ads_id: Option<String>,
    /// Google Ads conversion label
    pub google_ads_conversion_label: Option<String>,
}

impl StorefrontConfig {
    /// Load configuration from environment variables.
    ///
    /// Calls `dotenvy::dotenv()` to load from `.env` file if present.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if required variables are missing, invalid, or
    /// if secrets fail validation (placeholder detection, entropy check).
    pub fn from_env() -> Result<Self, ConfigError> {
        // Load .env file if present (ignore errors if not found)
        let _ = dotenvy::dotenv();

        let database_url = get_database_url("STOREFRONT_DATABASE_URL")?;
        let host = get_env_or_default("STOREFRONT_HOST", "127.0.0.1")
            .parse::<IpAddr>()
            .map_err(|e| {
                ConfigError::InvalidEnvVar("STOREFRONT_HOST".to_string(), e.to_string())
            })?;
        let port = get_env_or_default("STOREFRONT_PORT", "3000")
            .parse::<u16>()
            .map_err(|e| {
                ConfigError::InvalidEnvVar("STOREFRONT_PORT".to_string(), e.to_string())
            })?;
        let base_url = get_required_env("STOREFRONT_BASE_URL")?;
        let session_secret = get_validated_secret("STOREFRONT_SESSION_SECRET")?;
        validate_session_secret(&session_secret, "STOREFRONT_SESSION_SECRET")?;

        let shopify = ShopifyStorefrontConfig::from_env()?;
        let analytics = AnalyticsConfig::from_env();
        let sentry_dsn = get_optional_env("SENTRY_DSN");

        Ok(Self {
            database_url,
            host,
            port,
            base_url,
            session_secret,
            shopify,
            analytics,
            sentry_dsn,
        })
    }

    /// Returns the socket address for binding the server.
    #[must_use]
    pub const fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }
}

impl ShopifyStorefrontConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            store: get_required_env("SHOPIFY_STORE")?,
            api_version: get_env_or_default("SHOPIFY_API_VERSION", "2026-01"),
            storefront_public_token: get_required_env("SHOPIFY_STOREFRONT_PUBLIC_TOKEN")?,
            storefront_private_token: get_validated_secret("SHOPIFY_STOREFRONT_PRIVATE_TOKEN")?,
            customer_shop_id: get_required_env("SHOPIFY_CUSTOMER_SHOP_ID")?,
            customer_client_id: get_required_env("SHOPIFY_CUSTOMER_CLIENT_ID")?,
            customer_client_secret: get_validated_secret("SHOPIFY_CUSTOMER_CLIENT_SECRET")?,
        })
    }
}

impl AnalyticsConfig {
    fn from_env() -> Self {
        Self {
            ga4_measurement_id: get_optional_env("GA4_MEASUREMENT_ID"),
            meta_pixel_id: get_optional_env("META_PIXEL_ID"),
            tiktok_pixel_id: get_optional_env("TIKTOK_PIXEL_ID"),
            pinterest_tag_id: get_optional_env("PINTEREST_TAG_ID"),
            google_ads_id: get_optional_env("GOOGLE_ADS_ID"),
            google_ads_conversion_label: get_optional_env("GOOGLE_ADS_CONVERSION_LABEL"),
        }
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Get a required environment variable.
fn get_required_env(key: &str) -> Result<String, ConfigError> {
    std::env::var(key).map_err(|_| ConfigError::MissingEnvVar(key.to_string()))
}

/// Get a required environment variable as a secret.
fn get_required_secret(key: &str) -> Result<SecretString, ConfigError> {
    let value = get_required_env(key)?;
    Ok(SecretString::from(value))
}

/// Get database URL with fallback to generic `DATABASE_URL` (used by Fly.io postgres attach).
fn get_database_url(primary_key: &str) -> Result<SecretString, ConfigError> {
    // Try primary key first (e.g., STOREFRONT_DATABASE_URL)
    if let Ok(value) = std::env::var(primary_key) {
        return Ok(SecretString::from(value));
    }
    // Fallback to generic DATABASE_URL (set by Fly.io postgres attach)
    if let Ok(value) = std::env::var("DATABASE_URL") {
        return Ok(SecretString::from(value));
    }
    Err(ConfigError::MissingEnvVar(primary_key.to_string()))
}

/// Get an optional environment variable.
fn get_optional_env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

/// Get an environment variable with a default value.
fn get_env_or_default(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Validate that a session secret meets minimum length requirements.
fn validate_session_secret(secret: &SecretString, var_name: &str) -> Result<(), ConfigError> {
    let value = secret.expose_secret();
    if value.len() < MIN_SESSION_SECRET_LENGTH {
        return Err(ConfigError::InsecureSecret(
            var_name.to_string(),
            format!(
                "must be at least {} characters (got {})",
                MIN_SESSION_SECRET_LENGTH,
                value.len()
            ),
        ));
    }
    Ok(())
}

/// Calculate Shannon entropy in bits per character.
fn shannon_entropy(s: &str) -> f64 {
    if s.is_empty() {
        return 0.0;
    }

    let mut freq: HashMap<char, usize> = HashMap::new();
    for c in s.chars() {
        *freq.entry(c).or_insert(0) += 1;
    }

    #[allow(clippy::cast_precision_loss)] // String length will never exceed f64 precision
    let len = s.len() as f64;
    freq.values()
        .map(|&count| {
            #[allow(clippy::cast_precision_loss)] // Character count will never exceed f64 precision
            let p = count as f64 / len;
            -p * p.log2()
        })
        .sum()
}

/// Validate that a secret is not a placeholder and has sufficient entropy.
fn validate_secret_strength(secret: &str, var_name: &str) -> Result<(), ConfigError> {
    let lower = secret.to_lowercase();

    // Check blocklist
    for pattern in PLACEHOLDER_PATTERNS {
        if lower.contains(pattern) {
            return Err(ConfigError::InsecureSecret(
                var_name.to_string(),
                format!("appears to be a placeholder (contains '{pattern}')"),
            ));
        }
    }

    // Check entropy (real secrets like API keys have high entropy)
    let entropy = shannon_entropy(secret);
    if entropy < MIN_ENTROPY_BITS_PER_CHAR {
        return Err(ConfigError::InsecureSecret(
            var_name.to_string(),
            format!(
                "entropy too low ({entropy:.2} bits/char, need >= {MIN_ENTROPY_BITS_PER_CHAR:.1}). Use a randomly generated secret."
            ),
        ));
    }

    Ok(())
}

/// Load and validate a secret from environment.
fn get_validated_secret(key: &str) -> Result<SecretString, ConfigError> {
    let value = get_required_env(key)?;
    validate_secret_strength(&value, key)?;
    Ok(SecretString::from(value))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_shannon_entropy_empty() {
        assert!((shannon_entropy("") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_shannon_entropy_single_char() {
        // All same character = 0 entropy
        assert!((shannon_entropy("aaaaaaa") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_shannon_entropy_two_chars() {
        // "ab" has entropy of 1 bit per char (50% a, 50% b)
        let entropy = shannon_entropy("ab");
        assert!((entropy - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_shannon_entropy_high() {
        // Random-looking string should have high entropy
        let entropy = shannon_entropy("aB3$xY9!mK2@nL5#");
        assert!(entropy > 3.3);
    }

    #[test]
    fn test_validate_secret_strength_placeholder() {
        let result = validate_secret_strength("your-api-key-here", "TEST_VAR");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::InsecureSecret(_, _)));
    }

    #[test]
    fn test_validate_secret_strength_changeme() {
        let result = validate_secret_strength("changeme123", "TEST_VAR");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_secret_strength_low_entropy() {
        let result = validate_secret_strength("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "TEST_VAR");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ConfigError::InsecureSecret(_, _)));
    }

    #[test]
    fn test_validate_secret_strength_valid() {
        // High-entropy random string
        let result = validate_secret_strength("aB3$xY9!mK2@nL5#pQ7&rT0*uW4^zC6", "TEST_VAR");
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_session_secret_too_short() {
        let secret = SecretString::from("short");
        let result = validate_session_secret(&secret, "TEST_SESSION");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_session_secret_valid_length() {
        let secret = SecretString::from("a".repeat(32));
        let result = validate_session_secret(&secret, "TEST_SESSION");
        assert!(result.is_ok());
    }

    #[test]
    fn test_socket_addr() {
        let config = StorefrontConfig {
            database_url: SecretString::from("postgres://localhost/test"),
            host: "127.0.0.1".parse().unwrap(),
            port: 3000,
            base_url: "http://localhost:3000".to_string(),
            session_secret: SecretString::from("x".repeat(32)),
            shopify: ShopifyStorefrontConfig {
                store: "test.myshopify.com".to_string(),
                api_version: "2026-01".to_string(),
                storefront_public_token: "public".to_string(),
                storefront_private_token: SecretString::from("private"),
                customer_shop_id: "12345678901".to_string(),
                customer_client_id: "client_id".to_string(),
                customer_client_secret: SecretString::from("client_secret"),
            },
            analytics: AnalyticsConfig::default(),
            sentry_dsn: None,
        };

        let addr = config.socket_addr();
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_eq!(addr.port(), 3000);
    }

    #[test]
    fn test_shopify_config_debug_redacts_secrets() {
        let config = ShopifyStorefrontConfig {
            store: "test.myshopify.com".to_string(),
            api_version: "2026-01".to_string(),
            storefront_public_token: "public_token_value".to_string(),
            storefront_private_token: SecretString::from("super_secret_private_token"),
            customer_shop_id: "12345678901".to_string(),
            customer_client_id: "client_id_value".to_string(),
            customer_client_secret: SecretString::from("super_secret_client_secret"),
        };

        let debug_output = format!("{config:?}");

        // Public fields should be visible
        assert!(debug_output.contains("test.myshopify.com"));
        assert!(debug_output.contains("public_token_value"));
        assert!(debug_output.contains("client_id_value"));

        // Secret fields should be redacted
        assert!(debug_output.contains("[REDACTED]"));
        assert!(!debug_output.contains("super_secret_private_token"));
        assert!(!debug_output.contains("super_secret_client_secret"));
    }
}
