//! Automations configuration loaded from environment variables.
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
//! - `WEBHOOK_DATABASE_URL` - Restricted DB connection for webhook handlers
//! - `WEBHOOK_PORT` - Public webhook listener port (default: 8080)
//! - `WEBHOOK_BASE_URL` - Public URL for Shopify webhook subscription registration
//! - `SHOPIFY_WEBHOOK_SECRET` - HMAC secret for Shopify webhook verification
//! - `GITHUB_WEBHOOK_SECRET` - HMAC secret for GitHub webhook verification
//! - `SENTRY_WEBHOOK_SECRET` - Secret for Sentry webhook verification
//! - `FLY_WEBHOOK_TOKEN` - Bearer token for Fly.io webhook verification
//! - `BETTERSTACK_WEBHOOK_SECRET` - Secret for Better Stack webhook verification
//! - `SENTRY_DSN` - Sentry error tracking DSN

use secrecy::SecretString;

pub use naked_pineapple_services::config::{
    ClaudeConfig, ConfigError, EmailConfig, KlaviyoConfig, M365Config, OpenAIConfig, SlackConfig,
    get_database_url, get_env_or_default, get_optional_env,
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
    /// `OpenAI` API configuration (optional — for email embeddings).
    pub openai: Option<OpenAIConfig>,
    /// SMTP email configuration (optional — for internal alerts).
    pub email: Option<EmailConfig>,
    /// Scheduler timing configuration.
    pub scheduler: SchedulerConfig,
    /// Webhook listener configuration (optional — enables public webhook ingestion).
    pub webhook: Option<WebhookConfig>,
    /// Health check port.
    pub health_port: u16,
    /// Sentry DSN for error tracking.
    pub sentry_dsn: Option<String>,
    /// Sentry environment.
    pub sentry_environment: Option<String>,
    /// Sentry error sample rate (0.0 to 1.0).
    pub sentry_sample_rate: f32,
    /// Storefront database URL (optional — enables support conversation creation from email triage).
    pub storefront_database_url: Option<SecretString>,
    /// Storefront sync configuration (optional — enables search index refresh on product changes).
    pub storefront_sync: Option<StorefrontSyncConfig>,
}

/// Shopify Admin API configuration for order/product lookups.
#[derive(Debug, Clone)]
pub struct ShopifyConfig {
    /// Shopify store domain (e.g., `your-store.myshopify.com`).
    pub store: String,
    /// Shopify API version (e.g., "2026-01").
    pub api_version: String,
}

/// Storefront integration configuration.
#[derive(Debug, Clone)]
pub struct StorefrontSyncConfig {
    /// Internal URL for the storefront health listener (e.g., `http://storefront.internal:9091`).
    ///
    /// Used to send incremental search index refresh/delete requests.
    pub internal_url: String,
}

/// Scheduler timing configuration.
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// Email poll interval in seconds.
    pub email_poll_interval_secs: u64,
    /// Optional sync ceiling for email polling (ISO 8601 / RFC 3339).
    ///
    /// When set, the email sync will only fetch messages with
    /// `receivedDateTime < until`, allowing incremental testing.
    /// When `None`, syncs all the way to the present.
    pub email_sync_until: Option<chrono::DateTime<chrono::Utc>>,
    /// Abandoned cart check interval in seconds.
    pub cart_check_interval_secs: u64,
    /// Low stock check interval in seconds.
    pub stock_check_interval_secs: u64,
    /// Customer segment sync interval in seconds.
    pub segment_sync_interval_secs: u64,
    /// Shopify order/fulfillment poll interval in seconds.
    pub order_poll_interval_secs: u64,
    /// Amazon order poll interval in seconds.
    pub amazon_order_poll_interval_secs: u64,
    /// Meta Commerce order poll interval in seconds.
    pub meta_order_poll_interval_secs: u64,
    /// TikTok Shop order poll interval in seconds.
    pub tiktok_order_poll_interval_secs: u64,
    /// TikTok Shop settlement sync interval in seconds.
    pub tiktok_settlement_poll_interval_secs: u64,
    /// TikTok Shop return sync interval in seconds.
    pub tiktok_return_poll_interval_secs: u64,
    /// TikTok Shop performance metrics poll interval in seconds.
    pub tiktok_performance_poll_interval_secs: u64,
    /// Pinterest conversion sync interval in seconds.
    pub pinterest_conversion_poll_interval_secs: u64,
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
    /// Webhook event processing interval in seconds.
    pub webhook_event_interval_secs: u64,
    /// Summary check interval in seconds (wall-clock check frequency).
    pub summary_check_interval_secs: u64,
    /// Hour (0-23) to send the daily summary email.
    pub daily_summary_hour: u8,
    /// Minute (0-59) to send the daily summary email.
    pub daily_summary_minute: u8,
    /// Day of the week to send the weekly summary email (e.g., "monday").
    pub weekly_summary_day: String,
    /// Hour (0-23) to send the weekly summary email.
    pub weekly_summary_hour: u8,
    /// Minute (0-59) to send the weekly summary email.
    pub weekly_summary_minute: u8,
    /// Email recipients for business summary emails (empty = disabled).
    pub summary_email_recipients: Vec<String>,
}

impl SchedulerConfig {
    fn from_env() -> Self {
        Self {
            email_poll_interval_secs: parse_env_u64("AUTOMATION_EMAIL_POLL_INTERVAL_SECS", 120),
            email_sync_until: get_optional_env("AUTOMATION_EMAIL_SYNC_UNTIL")
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(&s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc)),
            cart_check_interval_secs: parse_env_u64("AUTOMATION_CART_CHECK_INTERVAL_SECS", 900),
            stock_check_interval_secs: parse_env_u64("AUTOMATION_STOCK_CHECK_INTERVAL_SECS", 3600),
            segment_sync_interval_secs: parse_env_u64(
                "AUTOMATION_SEGMENT_SYNC_INTERVAL_SECS",
                86400,
            ),
            order_poll_interval_secs: parse_env_u64("AUTOMATION_ORDER_POLL_INTERVAL_SECS", 300),
            amazon_order_poll_interval_secs: parse_env_u64(
                "AUTOMATION_AMAZON_ORDER_POLL_INTERVAL_SECS",
                900,
            ),
            meta_order_poll_interval_secs: parse_env_u64(
                "AUTOMATION_META_ORDER_POLL_INTERVAL_SECS",
                3600,
            ),
            tiktok_order_poll_interval_secs: parse_env_u64(
                "AUTOMATION_TIKTOK_ORDER_POLL_INTERVAL_SECS",
                3600,
            ),
            tiktok_settlement_poll_interval_secs: parse_env_u64(
                "AUTOMATION_TIKTOK_SETTLEMENT_POLL_INTERVAL_SECS",
                86400,
            ),
            tiktok_return_poll_interval_secs: parse_env_u64(
                "AUTOMATION_TIKTOK_RETURN_POLL_INTERVAL_SECS",
                3600,
            ),
            tiktok_performance_poll_interval_secs: parse_env_u64(
                "AUTOMATION_TIKTOK_PERFORMANCE_POLL_INTERVAL_SECS",
                86400,
            ),
            pinterest_conversion_poll_interval_secs: parse_env_u64(
                "AUTOMATION_PINTEREST_CONVERSION_POLL_INTERVAL_SECS",
                900,
            ),
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
            webhook_event_interval_secs: parse_env_u64(
                "AUTOMATION_WEBHOOK_EVENT_INTERVAL_SECS",
                15,
            ),
            summary_check_interval_secs: parse_env_u64(
                "AUTOMATION_SUMMARY_CHECK_INTERVAL_SECS",
                60,
            ),
            daily_summary_hour: parse_env_u8("AUTOMATION_DAILY_SUMMARY_HOUR", 7),
            daily_summary_minute: parse_env_u8("AUTOMATION_DAILY_SUMMARY_MINUTE", 0),
            weekly_summary_day: get_env_or_default("AUTOMATION_WEEKLY_SUMMARY_DAY", "monday")
                .to_lowercase(),
            weekly_summary_hour: parse_env_u8("AUTOMATION_WEEKLY_SUMMARY_HOUR", 7),
            weekly_summary_minute: parse_env_u8("AUTOMATION_WEEKLY_SUMMARY_MINUTE", 0),
            summary_email_recipients: get_optional_env("AUTOMATION_SUMMARY_EMAIL_RECIPIENTS")
                .map(|s| {
                    s.split(',')
                        .map(|e| e.trim().to_string())
                        .filter(|e| !e.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
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

/// Configuration for the public webhook listener.
///
/// Requires `WEBHOOK_DATABASE_URL` to be set (a restricted-privilege connection).
/// The public listener is only started if this config is present.
#[derive(Clone)]
pub struct WebhookConfig {
    /// Restricted-privilege database connection URL for webhook handlers.
    pub database_url: SecretString,
    /// Port for the public webhook listener.
    pub port: u16,
    /// Public base URL for registering Shopify webhook subscriptions.
    pub base_url: Option<String>,
    /// HMAC secret for Shopify webhook verification.
    pub shopify_secret: Option<SecretString>,
    /// HMAC secret for GitHub webhook verification.
    pub github_secret: Option<SecretString>,
    /// Secret for Sentry webhook verification.
    pub sentry_secret: Option<SecretString>,
    /// Bearer token for Fly.io webhook verification.
    pub fly_token: Option<SecretString>,
    /// Secret for Better Stack webhook verification.
    pub betterstack_secret: Option<SecretString>,
}

impl std::fmt::Debug for WebhookConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebhookConfig")
            .field("database_url", &"[REDACTED]")
            .field("port", &self.port)
            .field("base_url", &self.base_url)
            .field(
                "shopify_secret",
                &self.shopify_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "github_secret",
                &self.github_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field(
                "sentry_secret",
                &self.sentry_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("fly_token", &self.fly_token.as_ref().map(|_| "[REDACTED]"))
            .field(
                "betterstack_secret",
                &self.betterstack_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .finish()
    }
}

impl WebhookConfig {
    fn from_env() -> Option<Self> {
        let database_url = get_database_url("WEBHOOK_DATABASE_URL").ok()?;

        let port = get_env_or_default("WEBHOOK_PORT", "8080")
            .parse::<u16>()
            .unwrap_or(8080);

        Some(Self {
            database_url,
            port,
            base_url: get_optional_env("WEBHOOK_BASE_URL"),
            shopify_secret: get_optional_env("SHOPIFY_WEBHOOK_SECRET").map(SecretString::from),
            github_secret: get_optional_env("GITHUB_WEBHOOK_SECRET").map(SecretString::from),
            sentry_secret: get_optional_env("SENTRY_WEBHOOK_SECRET").map(SecretString::from),
            fly_token: get_optional_env("FLY_WEBHOOK_TOKEN").map(SecretString::from),
            betterstack_secret: get_optional_env("BETTERSTACK_WEBHOOK_SECRET")
                .map(SecretString::from),
        })
    }
}

fn parse_env_u64(key: &str, default: u64) -> u64 {
    get_env_or_default(key, &default.to_string())
        .parse()
        .unwrap_or(default)
}

fn parse_env_u8(key: &str, default: u8) -> u8 {
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
        let openai = OpenAIConfig::from_env();
        let email = EmailConfig::from_env().ok();
        let scheduler = SchedulerConfig::from_env();

        let webhook = WebhookConfig::from_env();

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

        let storefront_database_url =
            get_optional_env("STOREFRONT_DATABASE_URL").map(SecretString::from);

        let storefront_sync = get_optional_env("STOREFRONT_INTERNAL_URL")
            .map(|url| StorefrontSyncConfig { internal_url: url });

        Ok(Self {
            database_url,
            m365,
            claude,
            slack,
            klaviyo,
            shopify,
            openai,
            email,
            scheduler,
            webhook,
            health_port,
            sentry_dsn,
            sentry_environment,
            sentry_sample_rate,
            storefront_database_url,
            storefront_sync,
        })
    }
}

/// Log which optional environment variables are not set.
///
/// Called at startup after tracing is initialized so operators can see
/// which optional features and integrations are disabled.
pub fn log_unset_env_vars() {
    const OPTIONAL_VARS: &[&str] = &[
        "BETTERSTACK_WEBHOOK_SECRET",
        "FLY_WEBHOOK_TOKEN",
        "GITHUB_WEBHOOK_SECRET",
        "KLAVIYO_API_KEY",
        "KLAVIYO_LIST_ID",
        "OPENAI_API_KEY",
        "SENTRY_DSN",
        "SENTRY_ENVIRONMENT",
        "SENTRY_WEBHOOK_SECRET",
        "SHOPIFY_STORE",
        "SHOPIFY_WEBHOOK_SECRET",
        "SLACK_BOT_TOKEN",
        "SLACK_CHANNEL_ID",
        "SLACK_SIGNING_SECRET",
        "SMTP_FROM",
        "SMTP_HOST",
        "SMTP_PASSWORD",
        "SMTP_USERNAME",
        "STOREFRONT_DATABASE_URL",
        "STOREFRONT_INTERNAL_URL",
        "WEBHOOK_BASE_URL",
        "WEBHOOK_DATABASE_URL",
    ];

    let unset: Vec<&str> = OPTIONAL_VARS
        .iter()
        .copied()
        .filter(|var| std::env::var(var).is_err())
        .collect();

    if unset.is_empty() {
        tracing::info!("all optional environment variables are set");
    } else {
        tracing::info!(
            count = unset.len(),
            variables = ?unset,
            "optional environment variables not set"
        );
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
