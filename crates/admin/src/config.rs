//! Admin configuration loaded from environment variables.
//!
//! Shared config types (`ClaudeConfig`, `EmailConfig`, `KlaviyoConfig`, `SlackConfig`,
//! `OpenAIConfig`) are re-exported from `naked-pineapple-services`. Admin-specific
//! types (`AdminConfig`, `ShopifyAdminConfig`, `TlsConfig`) are defined here.
//!
//! # Environment Variables
//!
//! ## Required
//! - `ADMIN_DATABASE_URL` - `PostgreSQL` connection string
//! - `ADMIN_HOSTS` - Comma-separated hostnames (first is primary, used as `WebAuthn` RP ID)
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

use std::collections::HashSet;
use std::net::{IpAddr, SocketAddr};

use secrecy::SecretString;

// Re-export shared config types from services crate
pub use naked_pineapple_services::config::DEFAULT_CLAUDE_MODEL;
pub use naked_pineapple_services::config::{
    ClaudeConfig, ConfigError, EmailConfig, KlaviyoConfig, OpenAIConfig, SlackConfig,
    get_database_url, get_env_or_default, get_optional_env, get_required_env, get_validated_secret,
    validate_secret_strength, validate_session_secret,
};

/// Admin application configuration.
#[derive(Debug, Clone)]
pub struct AdminConfig {
    /// `PostgreSQL` database connection URL (contains password)
    pub database_url: SecretString,
    /// IP address to bind the server to
    pub host: IpAddr,
    /// Port to listen on
    pub port: u16,
    /// Port for plain HTTP health checks (Fly.io internal network only)
    pub health_port: u16,
    /// Primary hostname (used as `WebAuthn` RP ID, e.g., "admin.nakedpineapple.co")
    pub primary_host: String,
    /// All admin hostnames
    pub hosts: HashSet<String>,
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
        let _ = dotenvy::dotenv();

        let database_url = get_database_url("ADMIN_DATABASE_URL")?;
        let host = get_env_or_default("ADMIN_HOST", "127.0.0.1")
            .parse::<IpAddr>()
            .map_err(|e| ConfigError::InvalidEnvVar("ADMIN_HOST".to_string(), e.to_string()))?;
        let port = get_env_or_default("ADMIN_PORT", "3001")
            .parse::<u16>()
            .map_err(|e| ConfigError::InvalidEnvVar("ADMIN_PORT".to_string(), e.to_string()))?;
        let health_port = get_env_or_default("ADMIN_HEALTH_PORT", "9091")
            .parse::<u16>()
            .map_err(|e| {
                ConfigError::InvalidEnvVar("ADMIN_HEALTH_PORT".to_string(), e.to_string())
            })?;
        let (hosts, primary_host) = parse_admin_hosts()?;
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
            health_port,
            primary_host,
            hosts,
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
    #[must_use]
    pub const fn openai(&self) -> Option<&OpenAIConfig> {
        self.openai.as_ref()
    }

    /// Returns a reference to the Slack configuration, if available.
    #[must_use]
    pub const fn slack(&self) -> Option<&SlackConfig> {
        self.slack.as_ref()
    }

    /// Derive the full origin URL for a hostname.
    /// Localhost uses http with the configured port; everything else uses https.
    #[must_use]
    pub fn origin_for(&self, host: &str) -> String {
        if host == "localhost" {
            format!("http://localhost:{}", self.port)
        } else {
            format!("https://{host}")
        }
    }

    /// Returns the full origin URL for the primary host.
    #[must_use]
    pub fn primary_origin(&self) -> String {
        self.origin_for(&self.primary_host)
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

/// Parse `ADMIN_HOSTS` into a set of hostnames and the primary host.
///
/// The value is a comma-separated list of hostnames (no scheme).
/// The first entry becomes the primary host (used as `WebAuthn` RP ID).
fn parse_admin_hosts() -> Result<(HashSet<String>, String), ConfigError> {
    let raw = get_required_env("ADMIN_HOSTS")?;
    let mut hosts = HashSet::new();
    let mut primary = None;

    for host_str in raw.split(',') {
        let host_str = host_str.trim();
        if host_str.is_empty() {
            continue;
        }
        if primary.is_none() {
            primary = Some(host_str.to_owned());
        }
        hosts.insert(host_str.to_owned());
    }

    let primary = primary.ok_or_else(|| {
        ConfigError::InvalidEnvVar(
            "ADMIN_HOSTS".to_string(),
            "must contain at least one hostname".to_string(),
        )
    })?;

    Ok((hosts, primary))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_socket_addr() {
        let config = AdminConfig {
            database_url: SecretString::from("postgres://localhost/test"),
            host: "127.0.0.1".parse().unwrap(),
            port: 3001,
            health_port: 9091,
            primary_host: "localhost".to_string(),
            hosts: HashSet::from(["localhost".to_string()]),
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
        assert!(debug_output.contains("test.myshopify.com"));
        assert!(debug_output.contains("2026-01"));
        assert!(debug_output.contains("test_client_id"));
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
        assert!(debug_output.contains("claude-sonnet-4-20250514"));
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
        assert!(debug_output.contains("smtp.example.com"));
        assert!(debug_output.contains("587"));
        assert!(debug_output.contains("admin@example.com"));
        assert!(debug_output.contains("noreply@example.com"));
        assert!(debug_output.contains("[REDACTED]"));
        assert!(!debug_output.contains("super_secret_smtp_password"));
    }
}
