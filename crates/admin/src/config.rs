//! Admin configuration loaded from environment variables.
//!
//! # Environment Variables
//!
//! ## Required
//! - `ADMIN_DATABASE_URL` - `PostgreSQL` connection string
//! - `ADMIN_BASE_URL` - Public URL for the admin panel
//! - `ADMIN_SESSION_SECRET` - Session signing secret (min 32 chars, high entropy)
//! - `SHOPIFY_STORE` - Shopify store domain (e.g., your-store.myshopify.com)
//! - `SHOPIFY_ADMIN_CLIENT_ID` - Shopify Admin API OAuth client ID (HIGH PRIVILEGE)
//! - `SHOPIFY_ADMIN_CLIENT_SECRET` - Shopify Admin API OAuth client secret (HIGH PRIVILEGE)
//! - `CLAUDE_API_KEY` - Anthropic Claude API key
//! - `SMTP_HOST` - SMTP server hostname
//! - `SMTP_USERNAME` - SMTP authentication username
//! - `SMTP_PASSWORD` - SMTP authentication password
//! - `SMTP_FROM` - Email sender address
//!
//! ## Optional
//! - `ADMIN_HOST` - Bind address (default: 127.0.0.1)
//! - `ADMIN_PORT` - Listen port (default: 3001)
//! - `SHOPIFY_API_VERSION` - API version (default: 2026-01)
//! - `CLAUDE_MODEL` - Claude model ID (default: claude-sonnet-4-20250514)
//! - `OPENAI_API_KEY` - `OpenAI` API key (for embeddings, required for tool selection)
//! - `SMTP_PORT` - SMTP port (default: 587)
//! - `SENTRY_DSN` - Sentry error tracking DSN
//! - `KLAVIYO_API_KEY` - Klaviyo private API key (for newsletter campaigns)
//! - `KLAVIYO_LIST_ID` - Klaviyo newsletter list ID
//!
//! ## Optional (Slack - enables write operation confirmations)
//! - `SLACK_BOT_TOKEN` - Slack bot token (xoxb-...)
//! - `SLACK_SIGNING_SECRET` - Slack app signing secret
//! - `SLACK_CHANNEL_ID` - Default channel for confirmation messages
//!
//! ## Optional (TLS)
//! - `ADMIN_TLS_CERT` - PEM-encoded certificate chain
//! - `ADMIN_TLS_KEY` - PEM-encoded private key

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};

use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

const MIN_SESSION_SECRET_LENGTH: usize = 32;
const MIN_ENTROPY_BITS_PER_CHAR: f64 = 3.3;
const DEFAULT_CLAUDE_MODEL: &str = "claude-sonnet-4-20250514";

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

/// Admin application configuration.
#[derive(Debug, Clone)]
pub struct AdminConfig {
    /// `PostgreSQL` database connection URL (contains password)
    pub database_url: SecretString,
    /// IP address to bind the server to
    pub host: IpAddr,
    /// Port to listen on
    pub port: u16,
    /// Public base URL for the admin panel
    pub base_url: String,
    /// Session signing secret
    pub session_secret: SecretString,
    /// Shopify Admin API configuration
    pub shopify: ShopifyAdminConfig,
    /// Claude AI configuration
    pub claude: ClaudeConfig,
    /// `OpenAI` configuration for embeddings (optional, enables tool selection)
    pub openai: Option<OpenAIConfig>,
    /// Slack configuration for write operation confirmations (optional)
    pub slack: Option<SlackConfig>,
    /// Email configuration
    pub email: EmailConfig,
    /// Klaviyo configuration (optional - for newsletter campaigns)
    pub klaviyo: Option<KlaviyoConfig>,
    /// Sentry DSN for error tracking
    pub sentry_dsn: Option<String>,
    /// Sentry environment (e.g., "development", "staging", "production")
    pub sentry_environment: Option<String>,
    /// Sentry error sample rate (0.0 to 1.0)
    pub sentry_sample_rate: f32,
    /// Sentry traces sample rate for performance monitoring (0.0 to 1.0)
    pub sentry_traces_sample_rate: f32,
    /// TLS configuration for HTTPS (optional)
    pub tls: Option<TlsConfig>,
}

/// Shopify Admin API configuration.
///
/// Implements `Debug` manually to redact the HIGH PRIVILEGE credentials.
/// Uses OAuth for authentication - requires user to complete OAuth flow.
#[derive(Clone)]
pub struct ShopifyAdminConfig {
    /// Shopify store domain (e.g., your-store.myshopify.com)
    pub store: String,
    /// Shopify API version (e.g., 2026-01)
    pub api_version: String,
    /// OAuth client ID (HIGH PRIVILEGE - full store access)
    pub client_id: String,
    /// OAuth client secret (HIGH PRIVILEGE - full store access)
    pub client_secret: SecretString,
}

impl std::fmt::Debug for ShopifyAdminConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShopifyAdminConfig")
            .field("store", &self.store)
            .field("api_version", &self.api_version)
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .finish()
    }
}

/// Claude AI API configuration.
///
/// Implements `Debug` manually to redact the API key.
#[derive(Clone)]
pub struct ClaudeConfig {
    /// Anthropic API key
    pub api_key: SecretString,
    /// Model ID (e.g., claude-sonnet-4-20250514)
    pub model: String,
}

impl std::fmt::Debug for ClaudeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClaudeConfig")
            .field("api_key", &"[REDACTED]")
            .field("model", &self.model)
            .finish()
    }
}

/// `OpenAI` API configuration for embeddings.
///
/// Used for tool selection via semantic similarity search.
/// Implements `Debug` manually to redact the API key.
#[derive(Clone)]
pub struct OpenAIConfig {
    /// `OpenAI` API key
    pub api_key: SecretString,
}

impl std::fmt::Debug for OpenAIConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAIConfig")
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

/// Slack configuration for write operation confirmations.
///
/// When configured, write operations require Slack approval before execution.
/// Implements `Debug` manually to redact secrets.
#[derive(Clone)]
pub struct SlackConfig {
    /// Slack bot token (xoxb-...).
    pub bot_token: SecretString,
    /// Slack app signing secret for webhook verification.
    pub signing_secret: SecretString,
    /// Default channel ID for confirmation messages.
    pub channel_id: String,
}

impl std::fmt::Debug for SlackConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlackConfig")
            .field("bot_token", &"[REDACTED]")
            .field("signing_secret", &"[REDACTED]")
            .field("channel_id", &self.channel_id)
            .finish()
    }
}

/// Email (SMTP) configuration.
///
/// Implements `Debug` manually to redact the password.
#[derive(Clone)]
pub struct EmailConfig {
    /// SMTP server hostname
    pub smtp_host: String,
    /// SMTP server port
    pub smtp_port: u16,
    /// SMTP authentication username
    pub smtp_username: String,
    /// SMTP authentication password
    pub smtp_password: SecretString,
    /// Email sender address (From header)
    pub from_address: String,
}

impl std::fmt::Debug for EmailConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmailConfig")
            .field("smtp_host", &self.smtp_host)
            .field("smtp_port", &self.smtp_port)
            .field("smtp_username", &self.smtp_username)
            .field("smtp_password", &"[REDACTED]")
            .field("from_address", &self.from_address)
            .finish()
    }
}

/// Klaviyo API configuration for newsletter campaigns.
///
/// Implements `Debug` manually to redact the API key.
#[derive(Clone)]
pub struct KlaviyoConfig {
    /// Klaviyo private API key
    pub api_key: SecretString,
    /// Newsletter list ID
    pub list_id: String,
}

impl std::fmt::Debug for KlaviyoConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KlaviyoConfig")
            .field("api_key", &"[REDACTED]")
            .field("list_id", &self.list_id)
            .finish()
    }
}

impl KlaviyoConfig {
    fn from_env() -> Result<Option<Self>, ConfigError> {
        let api_key = get_optional_env("KLAVIYO_API_KEY");
        let list_id = get_optional_env("KLAVIYO_LIST_ID");

        match (api_key, list_id) {
            (Some(key), Some(id)) => {
                // Validate API key has sufficient entropy
                validate_secret_strength(&key, "KLAVIYO_API_KEY")?;
                Ok(Some(Self {
                    api_key: SecretString::from(key),
                    list_id: id,
                }))
            }
            (None, None) => Ok(None),
            _ => Err(ConfigError::InvalidEnvVar(
                "KLAVIYO_*".to_string(),
                "Both KLAVIYO_API_KEY and KLAVIYO_LIST_ID must be set together".to_string(),
            )),
        }
    }
}

/// TLS configuration for HTTPS.
#[derive(Clone)]
pub struct TlsConfig {
    /// PEM-encoded certificate chain
    pub cert_pem: String,
    /// PEM-encoded private key
    pub key_pem: SecretString,
}

impl std::fmt::Debug for TlsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TlsConfig")
            .field("cert_pem", &"[CERTIFICATE]")
            .field("key_pem", &"[REDACTED]")
            .finish()
    }
}

impl TlsConfig {
    fn from_env() -> Result<Option<Self>, ConfigError> {
        let cert_pem = get_optional_env("ADMIN_TLS_CERT");
        let key_pem = get_optional_env("ADMIN_TLS_KEY");

        match (cert_pem, key_pem) {
            (Some(cert), Some(key)) => Ok(Some(Self {
                cert_pem: cert,
                key_pem: SecretString::from(key),
            })),
            (None, None) => Ok(None),
            _ => Err(ConfigError::InvalidEnvVar(
                "ADMIN_TLS_*".to_string(),
                "Both ADMIN_TLS_CERT and ADMIN_TLS_KEY must be set together".to_string(),
            )),
        }
    }
}

impl AdminConfig {
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

        let database_url = get_database_url("ADMIN_DATABASE_URL")?;
        let host = get_env_or_default("ADMIN_HOST", "127.0.0.1")
            .parse::<IpAddr>()
            .map_err(|e| ConfigError::InvalidEnvVar("ADMIN_HOST".to_string(), e.to_string()))?;
        let port = get_env_or_default("ADMIN_PORT", "3001")
            .parse::<u16>()
            .map_err(|e| ConfigError::InvalidEnvVar("ADMIN_PORT".to_string(), e.to_string()))?;
        let base_url = get_required_env("ADMIN_BASE_URL")?;
        let session_secret = get_validated_secret("ADMIN_SESSION_SECRET")?;
        validate_session_secret(&session_secret, "ADMIN_SESSION_SECRET")?;

        let shopify = ShopifyAdminConfig::from_env()?;
        let claude = ClaudeConfig::from_env()?;
        let openai = OpenAIConfig::from_env();
        let slack = SlackConfig::from_env();
        let email = EmailConfig::from_env()?;
        let klaviyo = KlaviyoConfig::from_env()?;
        let sentry_dsn = get_optional_env("SENTRY_DSN");
        let sentry_environment = get_optional_env("SENTRY_ENVIRONMENT");
        let sentry_sample_rate = get_optional_env("SENTRY_SAMPLE_RATE")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0);
        let sentry_traces_sample_rate = get_optional_env("SENTRY_TRACES_SAMPLE_RATE")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0);
        let tls = TlsConfig::from_env()?;

        Ok(Self {
            database_url,
            host,
            port,
            base_url,
            session_secret,
            shopify,
            claude,
            openai,
            slack,
            email,
            klaviyo,
            sentry_dsn,
            sentry_environment,
            sentry_sample_rate,
            sentry_traces_sample_rate,
            tls,
        })
    }

    /// Returns the socket address for binding the server.
    #[must_use]
    pub const fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }

    /// Returns a reference to the Claude configuration.
    #[must_use]
    pub const fn claude(&self) -> &ClaudeConfig {
        &self.claude
    }

    /// Returns a reference to the Klaviyo configuration (if configured).
    #[must_use]
    pub const fn klaviyo(&self) -> Option<&KlaviyoConfig> {
        self.klaviyo.as_ref()
    }

    /// Returns a reference to the `OpenAI` configuration, if available.
    ///
    /// Returns `None` if `OPENAI_API_KEY` was not set, which disables
    /// embedding-based tool selection.
    #[must_use]
    pub const fn openai(&self) -> Option<&OpenAIConfig> {
        self.openai.as_ref()
    }

    /// Returns a reference to the Slack configuration, if available.
    ///
    /// Returns `None` if Slack variables are not set, which disables
    /// write operation confirmations via Slack.
    #[must_use]
    pub const fn slack(&self) -> Option<&SlackConfig> {
        self.slack.as_ref()
    }
}

impl ShopifyAdminConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            store: get_required_env("SHOPIFY_STORE")?,
            api_version: get_env_or_default("SHOPIFY_API_VERSION", "2026-01"),
            client_id: get_required_env("SHOPIFY_ADMIN_CLIENT_ID")?,
            client_secret: get_validated_secret("SHOPIFY_ADMIN_CLIENT_SECRET")?,
        })
    }
}

impl ClaudeConfig {
    fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            api_key: get_validated_secret("CLAUDE_API_KEY")?,
            model: get_env_or_default("CLAUDE_MODEL", DEFAULT_CLAUDE_MODEL),
        })
    }
}

impl OpenAIConfig {
    /// Load `OpenAI` configuration from environment.
    ///
    /// Returns `None` if `OPENAI_API_KEY` is not set (tool selection disabled).
    fn from_env() -> Option<Self> {
        get_optional_env("OPENAI_API_KEY").map(|key| {
            // Validate the key if present
            if let Err(e) = validate_secret_strength(&key, "OPENAI_API_KEY") {
                tracing::warn!("OPENAI_API_KEY validation warning: {e}");
            }
            Self {
                api_key: SecretString::from(key),
            }
        })
    }
}

impl SlackConfig {
    /// Load Slack configuration from environment.
    ///
    /// Returns `None` if Slack variables are not set (confirmations disabled).
    /// All three variables must be set together.
    fn from_env() -> Option<Self> {
        let bot_token = get_optional_env("SLACK_BOT_TOKEN")?;
        let signing_secret = get_optional_env("SLACK_SIGNING_SECRET")?;
        let channel_id = get_optional_env("SLACK_CHANNEL_ID")?;

        // Validate secrets if present
        if let Err(e) = validate_secret_strength(&bot_token, "SLACK_BOT_TOKEN") {
            tracing::warn!("SLACK_BOT_TOKEN validation warning: {e}");
        }
        if let Err(e) = validate_secret_strength(&signing_secret, "SLACK_SIGNING_SECRET") {
            tracing::warn!("SLACK_SIGNING_SECRET validation warning: {e}");
        }

        Some(Self {
            bot_token: SecretString::from(bot_token),
            signing_secret: SecretString::from(signing_secret),
            channel_id,
        })
    }
}

impl EmailConfig {
    fn from_env() -> Result<Self, ConfigError> {
        let smtp_port = get_env_or_default("SMTP_PORT", "587")
            .parse::<u16>()
            .map_err(|e| ConfigError::InvalidEnvVar("SMTP_PORT".to_string(), e.to_string()))?;

        Ok(Self {
            smtp_host: get_required_env("SMTP_HOST")?,
            smtp_port,
            smtp_username: get_required_env("SMTP_USERNAME")?,
            smtp_password: get_validated_secret("SMTP_PASSWORD")?,
            from_address: get_required_env("SMTP_FROM")?,
        })
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
    // Try primary key first (e.g., ADMIN_DATABASE_URL)
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
        let config = AdminConfig {
            database_url: SecretString::from("postgres://localhost/test"),
            host: "127.0.0.1".parse().unwrap(),
            port: 3001,
            base_url: "http://localhost:3001".to_string(),
            session_secret: SecretString::from("x".repeat(32)),
            shopify: ShopifyAdminConfig {
                store: "test.myshopify.com".to_string(),
                api_version: "2026-01".to_string(),
                client_id: "test_client_id".to_string(),
                client_secret: SecretString::from("test_client_secret"),
            },
            claude: ClaudeConfig {
                api_key: SecretString::from("sk-ant-test"),
                model: DEFAULT_CLAUDE_MODEL.to_string(),
            },
            openai: None,
            slack: None,
            email: EmailConfig {
                smtp_host: "smtp.example.com".to_string(),
                smtp_port: 587,
                smtp_username: "user".to_string(),
                smtp_password: SecretString::from("pass"),
                from_address: "admin@example.com".to_string(),
            },
            klaviyo: None,
            sentry_dsn: None,
            sentry_environment: None,
            sentry_sample_rate: 1.0,
            sentry_traces_sample_rate: 1.0,
            tls: None,
        };

        let addr = config.socket_addr();
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
        assert_eq!(addr.port(), 3001);
    }

    #[test]
    fn test_default_claude_model() {
        assert_eq!(DEFAULT_CLAUDE_MODEL, "claude-sonnet-4-20250514");
    }

    #[test]
    fn test_shopify_admin_config_debug_redacts_secrets() {
        let config = ShopifyAdminConfig {
            store: "test.myshopify.com".to_string(),
            api_version: "2026-01".to_string(),
            client_id: "test_client_id".to_string(),
            client_secret: SecretString::from("super_secret_client_secret"),
        };

        let debug_output = format!("{config:?}");

        // Public fields should be visible
        assert!(debug_output.contains("test.myshopify.com"));
        assert!(debug_output.contains("2026-01"));
        assert!(debug_output.contains("test_client_id"));

        // Secret fields should be redacted
        assert!(debug_output.contains("[REDACTED]"));
        assert!(!debug_output.contains("super_secret_client_secret"));
    }

    #[test]
    fn test_claude_config_debug_redacts_secrets() {
        let config = ClaudeConfig {
            api_key: SecretString::from("sk-ant-super-secret-key"),
            model: "claude-sonnet-4-20250514".to_string(),
        };

        let debug_output = format!("{config:?}");

        // Public fields should be visible
        assert!(debug_output.contains("claude-sonnet-4-20250514"));

        // Secret fields should be redacted
        assert!(debug_output.contains("[REDACTED]"));
        assert!(!debug_output.contains("sk-ant-super-secret-key"));
    }

    #[test]
    fn test_email_config_debug_redacts_secrets() {
        let config = EmailConfig {
            smtp_host: "smtp.example.com".to_string(),
            smtp_port: 587,
            smtp_username: "admin@example.com".to_string(),
            smtp_password: SecretString::from("super_secret_smtp_password"),
            from_address: "noreply@example.com".to_string(),
        };

        let debug_output = format!("{config:?}");

        // Public fields should be visible
        assert!(debug_output.contains("smtp.example.com"));
        assert!(debug_output.contains("587"));
        assert!(debug_output.contains("admin@example.com"));
        assert!(debug_output.contains("noreply@example.com"));

        // Secret fields should be redacted
        assert!(debug_output.contains("[REDACTED]"));
        assert!(!debug_output.contains("super_secret_smtp_password"));
    }
}
