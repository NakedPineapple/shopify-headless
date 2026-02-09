//! Shared configuration types loaded from environment variables.
//!
//! These config structs are used by both admin and automations binaries.
//! Each binary has its own top-level config that composes these shared types.

use std::collections::HashMap;

use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

const MIN_SESSION_SECRET_LENGTH: usize = 32;
const MIN_ENTROPY_BITS_PER_CHAR: f64 = 3.3;

/// Default Claude model ID.
pub const DEFAULT_CLAUDE_MODEL: &str = "claude-sonnet-4-20250514";

/// Blocklist of common placeholder patterns (case-insensitive).
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

/// Claude AI API configuration.
#[derive(Clone)]
pub struct ClaudeConfig {
    /// Anthropic API key.
    pub api_key: SecretString,
    /// Model ID (e.g., claude-sonnet-4-20250514).
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

impl ClaudeConfig {
    /// Load from environment variables.
    ///
    /// # Errors
    ///
    /// Returns error if `CLAUDE_API_KEY` is missing or insecure.
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            api_key: get_validated_secret("CLAUDE_API_KEY")?,
            model: get_env_or_default("CLAUDE_MODEL", DEFAULT_CLAUDE_MODEL),
        })
    }
}

/// `OpenAI` API configuration for embeddings.
#[derive(Clone)]
pub struct OpenAIConfig {
    /// `OpenAI` API key.
    pub api_key: SecretString,
}

impl std::fmt::Debug for OpenAIConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAIConfig")
            .field("api_key", &"[REDACTED]")
            .finish()
    }
}

impl OpenAIConfig {
    /// Load from environment. Returns `None` if `OPENAI_API_KEY` is not set.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        get_optional_env("OPENAI_API_KEY").map(|key| {
            if let Err(e) = validate_secret_strength(&key, "OPENAI_API_KEY") {
                tracing::warn!("OPENAI_API_KEY validation warning: {e}");
            }
            Self {
                api_key: SecretString::from(key),
            }
        })
    }
}

/// Slack configuration for write operation confirmations.
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

impl SlackConfig {
    /// Load from environment. Returns `None` if Slack variables are not set.
    /// All three variables must be set together.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        let bot_token = get_optional_env("SLACK_BOT_TOKEN")?;
        let signing_secret = get_optional_env("SLACK_SIGNING_SECRET")?;
        let channel_id = get_optional_env("SLACK_CHANNEL_ID")?;

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

/// Email (SMTP) configuration.
#[derive(Clone)]
pub struct EmailConfig {
    /// SMTP server hostname.
    pub smtp_host: String,
    /// SMTP server port.
    pub smtp_port: u16,
    /// SMTP authentication username.
    pub smtp_username: String,
    /// SMTP authentication password.
    pub smtp_password: SecretString,
    /// Email sender address (From header).
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

impl EmailConfig {
    /// Load from environment variables.
    ///
    /// # Errors
    ///
    /// Returns error if required SMTP variables are missing.
    pub fn from_env() -> Result<Self, ConfigError> {
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

/// Klaviyo API configuration for newsletter campaigns.
#[derive(Clone)]
pub struct KlaviyoConfig {
    /// Klaviyo private API key.
    pub api_key: SecretString,
    /// Newsletter list ID.
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
    /// Load from environment. Returns `None` if both vars are unset.
    ///
    /// # Errors
    ///
    /// Returns error if only one of the pair is set, or if the API key is insecure.
    pub fn from_env() -> Result<Option<Self>, ConfigError> {
        let api_key = get_optional_env("KLAVIYO_API_KEY");
        let list_id = get_optional_env("KLAVIYO_LIST_ID");

        match (api_key, list_id) {
            (Some(key), Some(id)) => {
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

// =============================================================================
// Helper Functions
// =============================================================================

/// Get a required environment variable.
///
/// # Errors
///
/// Returns error if the variable is not set.
pub fn get_required_env(key: &str) -> Result<String, ConfigError> {
    std::env::var(key).map_err(|_| ConfigError::MissingEnvVar(key.to_string()))
}

/// Get an optional environment variable.
#[must_use]
pub fn get_optional_env(key: &str) -> Option<String> {
    std::env::var(key).ok()
}

/// Get an environment variable with a default value.
#[must_use]
pub fn get_env_or_default(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

/// Get database URL with fallback to generic `DATABASE_URL` (used by Fly.io postgres attach).
///
/// # Errors
///
/// Returns error if neither the primary key nor `DATABASE_URL` is set.
pub fn get_database_url(primary_key: &str) -> Result<SecretString, ConfigError> {
    if let Ok(value) = std::env::var(primary_key) {
        return Ok(SecretString::from(value));
    }
    if let Ok(value) = std::env::var("DATABASE_URL") {
        return Ok(SecretString::from(value));
    }
    Err(ConfigError::MissingEnvVar(primary_key.to_string()))
}

/// Validate that a session secret meets minimum length requirements.
///
/// # Errors
///
/// Returns error if the secret is too short.
pub fn validate_session_secret(secret: &SecretString, var_name: &str) -> Result<(), ConfigError> {
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

    #[allow(clippy::cast_precision_loss)]
    let len = s.len() as f64;
    freq.values()
        .map(|&count| {
            #[allow(clippy::cast_precision_loss)]
            let p = count as f64 / len;
            -p * p.log2()
        })
        .sum()
}

/// Validate that a secret is not a placeholder and has sufficient entropy.
///
/// # Errors
///
/// Returns error if the secret matches a placeholder pattern or has low entropy.
pub fn validate_secret_strength(secret: &str, var_name: &str) -> Result<(), ConfigError> {
    let lower = secret.to_lowercase();

    for pattern in PLACEHOLDER_PATTERNS {
        if lower.contains(pattern) {
            return Err(ConfigError::InsecureSecret(
                var_name.to_string(),
                format!("appears to be a placeholder (contains '{pattern}')"),
            ));
        }
    }

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
///
/// # Errors
///
/// Returns error if the variable is missing or the secret is insecure.
pub fn get_validated_secret(key: &str) -> Result<SecretString, ConfigError> {
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
        assert!((shannon_entropy("aaaaaaa") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_shannon_entropy_two_chars() {
        let entropy = shannon_entropy("ab");
        assert!((entropy - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_shannon_entropy_high() {
        let entropy = shannon_entropy("aB3$xY9!mK2@nL5#");
        assert!(entropy > 3.3);
    }

    #[test]
    fn test_validate_secret_strength_placeholder() {
        let result = validate_secret_strength("your-api-key-here", "TEST_VAR");
        assert!(result.is_err());
        assert!(matches!(result, Err(ConfigError::InsecureSecret(_, _))));
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
        assert!(matches!(result, Err(ConfigError::InsecureSecret(_, _))));
    }

    #[test]
    fn test_validate_secret_strength_valid() {
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
    fn test_default_claude_model() {
        assert_eq!(DEFAULT_CLAUDE_MODEL, "claude-sonnet-4-20250514");
    }
}
