//! Slack Web API client.
//!
//! Provides methods for sending messages, updating messages, and verifying
//! webhook signatures.

use hmac::{Hmac, Mac};
use reqwest::Client;
use secrecy::{ExposeSecret, SecretString};
use sha2::Sha256;
use tracing::{debug, error, instrument, warn};

use super::error::SlackError;
use super::types::{Block, PostMessageResponse, SlackMessage, Text, UpdateMessageResponse};

/// Slack Web API base URL.
const SLACK_API_BASE: &str = "https://slack.com/api";

/// Slack API client for sending and updating messages.
#[derive(Clone)]
pub struct SlackClient {
    /// HTTP client.
    client: Client,
    /// Bot token for authentication.
    bot_token: SecretString,
    /// Signing secret for verifying webhooks.
    signing_secret: SecretString,
    /// Default channel ID for confirmation messages.
    default_channel: String,
}

impl std::fmt::Debug for SlackClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SlackClient")
            .field("bot_token", &"[REDACTED]")
            .field("signing_secret", &"[REDACTED]")
            .field("default_channel", &self.default_channel)
            .finish_non_exhaustive()
    }
}

impl SlackClient {
    /// Create a new Slack client.
    #[must_use]
    pub fn new(
        bot_token: SecretString,
        signing_secret: SecretString,
        default_channel: String,
    ) -> Self {
        Self {
            client: Client::new(),
            bot_token,
            signing_secret,
            default_channel,
        }
    }

    /// Get the default channel ID.
    #[must_use]
    pub fn default_channel(&self) -> &str {
        &self.default_channel
    }

    /// Post a message to a channel.
    ///
    /// # Errors
    ///
    /// Returns error if the API request fails or Slack returns an error.
    #[instrument(skip(self, blocks), fields(channel = %channel, block_count = blocks.len()))]
    pub async fn post_message(
        &self,
        channel: &str,
        blocks: Vec<Block>,
        fallback_text: Option<&str>,
    ) -> Result<PostMessageResponse, SlackError> {
        debug!(
            has_fallback_text = fallback_text.is_some(),
            "Sending message to Slack channel"
        );

        let message = SlackMessage {
            channel: channel.to_string(),
            blocks,
            text: fallback_text.map(String::from),
        };

        let start = std::time::Instant::now();
        let response = self
            .client
            .post(format!("{SLACK_API_BASE}/chat.postMessage"))
            .bearer_auth(self.bot_token.expose_secret())
            .json(&message)
            .send()
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to send request to Slack API");
                SlackError::Request(e.to_string())
            })?;

        debug!(
            status = %response.status(),
            duration_ms = %start.elapsed().as_millis(),
            "Slack postMessage API request completed"
        );

        let result: PostMessageResponse = response.json().await.map_err(|e| {
            error!(error = %e, "Failed to parse Slack API response");
            SlackError::Response(e.to_string())
        })?;

        if !result.ok {
            error!(
                error = ?result.error,
                "Slack API error posting message"
            );
            return Err(SlackError::Api(
                result.error.unwrap_or_else(|| "Unknown error".to_string()),
            ));
        }

        debug!(
            ts = ?result.ts,
            channel = ?result.channel,
            "Message posted to Slack"
        );

        Ok(result)
    }

    /// Update an existing message.
    ///
    /// # Errors
    ///
    /// Returns error if the API request fails or Slack returns an error.
    #[instrument(skip(self, blocks), fields(channel = %channel, ts = %ts, block_count = blocks.len()))]
    pub async fn update_message(
        &self,
        channel: &str,
        ts: &str,
        blocks: Vec<Block>,
        fallback_text: Option<&str>,
    ) -> Result<UpdateMessageResponse, SlackError> {
        debug!(
            has_fallback_text = fallback_text.is_some(),
            "Updating existing Slack message"
        );

        #[derive(serde::Serialize)]
        struct UpdateMessage {
            channel: String,
            ts: String,
            blocks: Vec<Block>,
            #[serde(skip_serializing_if = "Option::is_none")]
            text: Option<String>,
        }

        let message = UpdateMessage {
            channel: channel.to_string(),
            ts: ts.to_string(),
            blocks,
            text: fallback_text.map(String::from),
        };

        let start = std::time::Instant::now();
        let response = self
            .client
            .post(format!("{SLACK_API_BASE}/chat.update"))
            .bearer_auth(self.bot_token.expose_secret())
            .json(&message)
            .send()
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to send update request to Slack API");
                SlackError::Request(e.to_string())
            })?;

        debug!(
            status = %response.status(),
            duration_ms = %start.elapsed().as_millis(),
            "Slack update API request completed"
        );

        let result: UpdateMessageResponse = response.json().await.map_err(|e| {
            error!(error = %e, "Failed to parse Slack update response");
            SlackError::Response(e.to_string())
        })?;

        if !result.ok {
            error!(
                error = ?result.error,
                "Slack API error updating message"
            );
            return Err(SlackError::Api(
                result.error.unwrap_or_else(|| "Unknown error".to_string()),
            ));
        }

        debug!(ts = %ts, "Message updated in Slack");

        Ok(result)
    }

    /// Respond to a `response_url` (for interaction responses).
    ///
    /// # Errors
    ///
    /// Returns error if the request fails.
    #[instrument(skip(self, blocks), fields(block_count = blocks.len(), replace_original = %replace_original))]
    pub async fn respond_to_url(
        &self,
        response_url: &str,
        blocks: Vec<Block>,
        replace_original: bool,
    ) -> Result<(), SlackError> {
        debug!("Sending response to Slack response_url");

        #[derive(serde::Serialize)]
        struct ResponseMessage {
            blocks: Vec<Block>,
            replace_original: bool,
        }

        let message = ResponseMessage {
            blocks,
            replace_original,
        };

        let start = std::time::Instant::now();
        let response = self
            .client
            .post(response_url)
            .json(&message)
            .send()
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to send response to Slack response_url");
                SlackError::Request(e.to_string())
            })?;

        let status = response.status();
        debug!(
            status = %status,
            duration_ms = %start.elapsed().as_millis(),
            "Slack response_url request completed"
        );

        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            error!(
                status = %status,
                "Slack response_url request failed"
            );
            return Err(SlackError::Response(format!(
                "Response URL returned {status}: {body}"
            )));
        }

        debug!("Successfully responded to Slack response_url");

        Ok(())
    }

    /// Verify a Slack webhook signature.
    ///
    /// This implements Slack's signature verification:
    /// <https://api.slack.com/authentication/verifying-requests-from-slack>
    ///
    /// # Arguments
    ///
    /// * `timestamp` - The `X-Slack-Request-Timestamp` header value
    /// * `body` - The raw request body
    /// * `signature` - The `X-Slack-Signature` header value
    ///
    /// # Errors
    ///
    /// Returns error if signature verification fails.
    #[instrument(skip(self, body, signature), fields(timestamp = %timestamp))]
    pub fn verify_signature(
        &self,
        timestamp: &str,
        body: &str,
        signature: &str,
    ) -> Result<(), SlackError> {
        debug!(body_len = body.len(), "Verifying Slack webhook signature");

        // Check timestamp to prevent replay attacks (5 minutes)
        let ts: i64 = timestamp.parse().map_err(|_| {
            warn!("Slack signature verification failed: invalid timestamp format");
            SlackError::InvalidSignature("Invalid timestamp".to_string())
        })?;

        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| SlackError::InvalidSignature(e.to_string()))?
            .as_secs();

        let now = i64::try_from(now_secs)
            .map_err(|_| SlackError::InvalidSignature("System time overflow".to_string()))?;

        let age_secs = (now - ts).abs();
        if age_secs > 300 {
            warn!(
                age_secs = age_secs,
                "Slack signature verification failed: request timestamp too old (possible replay attack)"
            );
            return Err(SlackError::InvalidSignature(
                "Request timestamp too old".to_string(),
            ));
        }

        // Compute expected signature
        let sig_basestring = format!("v0:{timestamp}:{body}");

        let mut mac =
            Hmac::<Sha256>::new_from_slice(self.signing_secret.expose_secret().as_bytes())
                .map_err(|e| SlackError::InvalidSignature(e.to_string()))?;

        mac.update(sig_basestring.as_bytes());

        let expected = format!("v0={}", hex::encode(mac.finalize().into_bytes()));

        // Constant-time comparison
        if !constant_time_compare(&expected, signature) {
            warn!("Slack signature verification failed: signature mismatch");
            return Err(SlackError::InvalidSignature(
                "Signature mismatch".to_string(),
            ));
        }

        debug!("Slack webhook signature verified successfully");

        Ok(())
    }

    /// Post a simple text message (convenience method).
    ///
    /// # Errors
    ///
    /// Returns error if posting fails.
    #[instrument(skip(self, text), fields(channel = %channel, text_len = text.len()))]
    pub async fn post_text(
        &self,
        channel: &str,
        text: &str,
    ) -> Result<PostMessageResponse, SlackError> {
        debug!("Posting simple text message to Slack");

        let blocks = vec![Block::Section {
            text: Text::mrkdwn(text),
            accessory: None,
        }];

        self.post_message(channel, blocks, Some(text)).await
    }
}

/// Constant-time string comparison to prevent timing attacks.
fn constant_time_compare(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut result: u8 = 0;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }

    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constant_time_compare_equal() {
        assert!(constant_time_compare("hello", "hello"));
        assert!(constant_time_compare("", ""));
    }

    #[test]
    fn test_constant_time_compare_not_equal() {
        assert!(!constant_time_compare("hello", "world"));
        assert!(!constant_time_compare("hello", "hell"));
        assert!(!constant_time_compare("hello", "helloo"));
    }

    #[test]
    fn test_signature_verification_valid() {
        let client = SlackClient::new(
            SecretString::from("xoxb-test-token".to_string()),
            SecretString::from("test-signing-secret".to_string()),
            "C12345".to_string(),
        );

        // Generate a valid signature
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before epoch")
            .as_secs()
            .to_string();

        let body = "test=body";
        let sig_basestring = format!("v0:{timestamp}:{body}");

        let mut mac =
            Hmac::<Sha256>::new_from_slice(b"test-signing-secret").expect("valid key length");
        mac.update(sig_basestring.as_bytes());
        let signature = format!("v0={}", hex::encode(mac.finalize().into_bytes()));

        assert!(
            client
                .verify_signature(&timestamp, body, &signature)
                .is_ok()
        );
    }

    #[test]
    fn test_signature_verification_invalid_signature() {
        let client = SlackClient::new(
            SecretString::from("xoxb-test-token".to_string()),
            SecretString::from("test-signing-secret".to_string()),
            "C12345".to_string(),
        );

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before epoch")
            .as_secs()
            .to_string();

        let body = "test=body";
        let invalid_signature = "v0=invalid_signature_hash";

        let result = client.verify_signature(&timestamp, body, invalid_signature);
        assert!(result.is_err());
        assert!(matches!(result, Err(SlackError::InvalidSignature(_))));
    }

    #[test]
    fn test_signature_verification_invalid_timestamp() {
        let client = SlackClient::new(
            SecretString::from("xoxb-test-token".to_string()),
            SecretString::from("test-signing-secret".to_string()),
            "C12345".to_string(),
        );

        let result = client.verify_signature("not-a-number", "body", "v0=sig");
        assert!(result.is_err());
        assert!(matches!(result, Err(SlackError::InvalidSignature(_))));
    }

    #[test]
    fn test_signature_verification_old_timestamp() {
        let client = SlackClient::new(
            SecretString::from("xoxb-test-token".to_string()),
            SecretString::from("test-signing-secret".to_string()),
            "C12345".to_string(),
        );

        // Timestamp from 10 minutes ago
        let old_timestamp = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before epoch")
            .as_secs()
            - 600)
            .to_string();

        let body = "test=body";
        let sig_basestring = format!("v0:{old_timestamp}:{body}");

        let mut mac =
            Hmac::<Sha256>::new_from_slice(b"test-signing-secret").expect("valid key length");
        mac.update(sig_basestring.as_bytes());
        let signature = format!("v0={}", hex::encode(mac.finalize().into_bytes()));

        let result = client.verify_signature(&old_timestamp, body, &signature);
        assert!(result.is_err());
        // Should fail due to old timestamp, not signature mismatch
    }

    #[test]
    fn test_signature_verification_tampered_body() {
        let client = SlackClient::new(
            SecretString::from("xoxb-test-token".to_string()),
            SecretString::from("test-signing-secret".to_string()),
            "C12345".to_string(),
        );

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time before epoch")
            .as_secs()
            .to_string();

        let original_body = "original=body";
        let sig_basestring = format!("v0:{timestamp}:{original_body}");

        let mut mac =
            Hmac::<Sha256>::new_from_slice(b"test-signing-secret").expect("valid key length");
        mac.update(sig_basestring.as_bytes());
        let signature = format!("v0={}", hex::encode(mac.finalize().into_bytes()));

        // Use signature from original body but verify against tampered body
        let tampered_body = "tampered=body";
        let result = client.verify_signature(&timestamp, tampered_body, &signature);
        assert!(result.is_err());
    }

    #[test]
    fn test_default_channel() {
        let client = SlackClient::new(
            SecretString::from("xoxb-test-token".to_string()),
            SecretString::from("test-signing-secret".to_string()),
            "C12345".to_string(),
        );

        assert_eq!(client.default_channel(), "C12345");
    }
}
