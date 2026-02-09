//! Email automation configuration loaded from environment variables.
//!
//! Reuses shared config types from `naked-pineapple-services` and adds
//! Microsoft 365 and scheduler-specific configuration.
//!
//! # Environment Variables
//!
//! ## Required
//! - `ADMIN_DATABASE_URL` - `PostgreSQL` connection string (`np_admin` database)
//! - `M365_TENANT_ID` - Azure AD tenant ID
//! - `M365_CLIENT_ID` - Azure AD application (client) ID
//! - `M365_CLIENT_SECRET` - Azure AD application client secret
//! - `M365_SHARED_MAILBOXES` - Comma-separated shared mailbox addresses
//! - `CLAUDE_API_KEY` - Anthropic Claude API key
//! - `SMTP_HOST` / `SMTP_USERNAME` / `SMTP_PASSWORD` / `SMTP_FROM` - SMTP config
//!
//! ## Optional
//! - `AUTOMATION_EMAIL_POLL_INTERVAL_SECS` - Email poll interval (default: 120)
//! - `AUTOMATION_CART_CHECK_INTERVAL_SECS` - Cart check interval (default: 900)
//! - `AUTOMATION_STOCK_CHECK_INTERVAL_SECS` - Stock check interval (default: 3600)
//! - `AUTOMATION_SEGMENT_SYNC_INTERVAL_SECS` - Segment sync interval (default: 86400)
//! - `AUTOMATION_LOW_STOCK_EMAIL_RECIPIENTS` - Comma-separated email addresses for alerts
//! - `AUTOMATION_HEALTH_PORT` - Health check port (default: 9092)
//! - `SENTRY_DSN` - Sentry error tracking DSN

use secrecy::SecretString;

pub use naked_pineapple_services::config::{
    ClaudeConfig, ConfigError, EmailConfig, KlaviyoConfig, SlackConfig, get_database_url,
    get_env_or_default, get_optional_env, get_required_env, get_validated_secret,
};

/// Email automation service configuration.
#[derive(Debug, Clone)]
pub struct AutomationConfig {
    /// `PostgreSQL` database connection URL (`np_admin` database).
    pub database_url: SecretString,
    /// Microsoft 365 configuration.
    pub m365: M365Config,
    /// Claude AI configuration.
    pub claude: ClaudeConfig,
    /// Slack configuration (optional).
    pub slack: Option<SlackConfig>,
    /// Klaviyo configuration (optional).
    pub klaviyo: Option<KlaviyoConfig>,
    /// Shopify Admin API configuration (optional, enables order/product lookups).
    pub shopify: Option<ShopifyConfig>,
    /// SMTP email configuration (optional — for internal alerts).
    pub email: Option<EmailConfig>,
    /// Scheduler timing configuration.
    pub scheduler: SchedulerConfig,
    /// Health check port.
    pub health_port: u16,
    /// Sentry DSN for error tracking.
    pub sentry_dsn: Option<String>,
    /// Sentry environment.
    pub sentry_environment: Option<String>,
    /// Sentry error sample rate (0.0 to 1.0).
    pub sentry_sample_rate: f32,
}

/// Shopify Admin API configuration for order/product lookups.
#[derive(Debug, Clone)]
pub struct ShopifyConfig {
    /// Shopify store domain (e.g., `your-store.myshopify.com`).
    pub store: String,
    /// Shopify API version (e.g., "2026-01").
    pub api_version: String,
}

/// Microsoft 365 Graph API configuration.
#[derive(Clone)]
pub struct M365Config {
    /// Azure AD tenant ID.
    pub tenant_id: String,
    /// Azure AD application (client) ID.
    pub client_id: String,
    /// Azure AD application client secret.
    pub client_secret: SecretString,
    /// Shared mailbox addresses to poll.
    pub shared_mailboxes: Vec<String>,
}

impl std::fmt::Debug for M365Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("M365Config")
            .field("tenant_id", &self.tenant_id)
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("shared_mailboxes", &self.shared_mailboxes)
            .finish()
    }
}

impl M365Config {
    fn from_env() -> Result<Self, ConfigError> {
        let mailboxes_raw = get_required_env("M365_SHARED_MAILBOXES")?;
        let shared_mailboxes: Vec<String> = mailboxes_raw
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        if shared_mailboxes.is_empty() {
            return Err(ConfigError::InvalidEnvVar(
                "M365_SHARED_MAILBOXES".to_string(),
                "must contain at least one mailbox address".to_string(),
            ));
        }

        Ok(Self {
            tenant_id: get_required_env("M365_TENANT_ID")?,
            client_id: get_required_env("M365_CLIENT_ID")?,
            client_secret: get_validated_secret("M365_CLIENT_SECRET")?,
            shared_mailboxes,
        })
    }
}

/// Scheduler timing configuration.
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Email poll interval in seconds.
    pub email_poll_interval_secs: u64,
    /// Abandoned cart check interval in seconds.
    pub cart_check_interval_secs: u64,
    /// Low stock check interval in seconds.
    pub stock_check_interval_secs: u64,
    /// Customer segment sync interval in seconds.
    pub segment_sync_interval_secs: u64,
    /// Shopify order/fulfillment poll interval in seconds.
    pub order_poll_interval_secs: u64,
    /// Subscription lifecycle check interval in seconds.
    pub subscription_check_interval_secs: u64,
    /// Minutes of inactivity before a checkout is considered abandoned.
    pub cart_abandon_delay_minutes: u64,
    /// Inventory units below which a low stock alert is triggered.
    pub low_stock_threshold: i32,
    /// Email recipients for low stock alerts (empty = no email alerts).
    pub low_stock_email_recipients: Vec<String>,
    /// Days before subscription renewal to send a reminder.
    pub subscription_renewal_reminder_days: u64,
    /// Days after subscription cancellation to send a win-back email.
    pub subscription_winback_delay_days: u64,
}

impl SchedulerConfig {
    fn from_env() -> Self {
        Self {
            email_poll_interval_secs: parse_env_u64("AUTOMATION_EMAIL_POLL_INTERVAL_SECS", 120),
            cart_check_interval_secs: parse_env_u64("AUTOMATION_CART_CHECK_INTERVAL_SECS", 900),
            stock_check_interval_secs: parse_env_u64("AUTOMATION_STOCK_CHECK_INTERVAL_SECS", 3600),
            segment_sync_interval_secs: parse_env_u64(
                "AUTOMATION_SEGMENT_SYNC_INTERVAL_SECS",
                86400,
            ),
            order_poll_interval_secs: parse_env_u64("AUTOMATION_ORDER_POLL_INTERVAL_SECS", 300),
            subscription_check_interval_secs: parse_env_u64(
                "AUTOMATION_SUBSCRIPTION_CHECK_INTERVAL_SECS",
                86400,
            ),
            cart_abandon_delay_minutes: parse_env_u64("AUTOMATION_CART_ABANDON_DELAY_MINUTES", 60),
            low_stock_threshold: get_env_or_default("AUTOMATION_LOW_STOCK_THRESHOLD", "10")
                .parse()
                .unwrap_or(10),
            low_stock_email_recipients: get_optional_env("AUTOMATION_LOW_STOCK_EMAIL_RECIPIENTS")
                .map(|s| {
                    s.split(',')
                        .map(|e| e.trim().to_string())
                        .filter(|e| !e.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            subscription_renewal_reminder_days: parse_env_u64(
                "AUTOMATION_SUBSCRIPTION_RENEWAL_REMINDER_DAYS",
                3,
            ),
            subscription_winback_delay_days: parse_env_u64(
                "AUTOMATION_SUBSCRIPTION_WINBACK_DELAY_DAYS",
                14,
            ),
        }
    }
}

impl ShopifyConfig {
    fn from_env() -> Option<Self> {
        let store = get_optional_env("SHOPIFY_STORE")?;
        let api_version = get_env_or_default("SHOPIFY_API_VERSION", "2026-01");
        Some(Self { store, api_version })
    }
}

fn parse_env_u64(key: &str, default: u64) -> u64 {
    get_env_or_default(key, &default.to_string())
        .parse()
        .unwrap_or(default)
}

impl AutomationConfig {
    /// Load configuration from environment variables.
    ///
    /// # Errors
    ///
    /// Returns `ConfigError` if required variables are missing or invalid.
    pub fn from_env() -> Result<Self, ConfigError> {
        let _ = dotenvy::dotenv();

        let database_url = get_database_url("ADMIN_DATABASE_URL")?;
        let m365 = M365Config::from_env()?;
        let claude = ClaudeConfig::from_env()?;
        let slack = SlackConfig::from_env();
        let klaviyo = KlaviyoConfig::from_env()?;
        let shopify = ShopifyConfig::from_env();
        let email = EmailConfig::from_env().ok();
        let scheduler = SchedulerConfig::from_env();

        let health_port = get_env_or_default("AUTOMATION_HEALTH_PORT", "9092")
            .parse::<u16>()
            .map_err(|e| {
                ConfigError::InvalidEnvVar("AUTOMATION_HEALTH_PORT".to_string(), e.to_string())
            })?;

        let sentry_dsn = get_optional_env("SENTRY_DSN");
        let sentry_environment = get_optional_env("SENTRY_ENVIRONMENT");
        let sentry_sample_rate = get_optional_env("SENTRY_SAMPLE_RATE")
            .and_then(|s| s.parse().ok())
            .unwrap_or(1.0);

        Ok(Self {
            database_url,
            m365,
            claude,
            slack,
            klaviyo,
            shopify,
            email,
            scheduler,
            health_port,
            sentry_dsn,
            sentry_environment,
            sentry_sample_rate,
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_m365_config_debug_redacts_secrets() {
        let config = M365Config {
            tenant_id: "tenant-123".to_string(),
            client_id: "client-456".to_string(),
            client_secret: SecretString::from("super-secret-value"),
            shared_mailboxes: vec!["info@example.com".to_string()],
        };

        let debug_output = format!("{config:?}");
        assert!(debug_output.contains("tenant-123"));
        assert!(debug_output.contains("client-456"));
        assert!(debug_output.contains("[REDACTED]"));
        assert!(!debug_output.contains("super-secret-value"));
    }

    #[test]
    fn test_scheduler_config_defaults() {
        let config = SchedulerConfig::from_env();
        assert_eq!(config.email_poll_interval_secs, 120);
        assert_eq!(config.cart_check_interval_secs, 900);
        assert_eq!(config.stock_check_interval_secs, 3600);
        assert_eq!(config.segment_sync_interval_secs, 86400);
        assert_eq!(config.order_poll_interval_secs, 300);
        assert_eq!(config.cart_abandon_delay_minutes, 60);
    }
}
