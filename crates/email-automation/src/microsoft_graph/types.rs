//! Types for Microsoft Graph API mail operations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// `OAuth2` access token with expiry tracking.
#[derive(Debug, Clone)]
pub struct AccessToken {
    /// The bearer token string.
    pub token: String,
    /// When this token expires.
    pub expires_at: DateTime<Utc>,
}

impl AccessToken {
    /// Returns true if the token has expired or will expire within the given buffer.
    pub fn is_expired_with_buffer(&self, buffer: chrono::Duration) -> bool {
        Utc::now() + buffer >= self.expires_at
    }
}

/// `OAuth2` token response from Azure AD.
#[derive(Debug, Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub expires_in: i64,
    pub token_type: String,
}

/// Graph API error response.
#[derive(Debug, Deserialize)]
pub struct GraphErrorResponse {
    pub error: GraphErrorDetail,
}

/// Error detail from a Graph API error response.
#[derive(Debug, Deserialize)]
pub struct GraphErrorDetail {
    pub code: Option<String>,
    pub message: String,
}

/// A single email message from the Graph API.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphMessage {
    pub id: String,
    pub conversation_id: Option<String>,
    pub subject: Option<String>,
    pub body_preview: Option<String>,
    pub body: Option<MessageBody>,
    pub from: Option<EmailAddressWrapper>,
    pub to_recipients: Option<Vec<EmailAddressWrapper>>,
    pub received_date_time: Option<DateTime<Utc>>,
}

/// Message body with content type and content.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageBody {
    pub content: Option<String>,
}

/// Wrapper for email address in Graph API responses.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailAddressWrapper {
    pub email_address: EmailAddress,
}

/// An email address with optional display name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmailAddress {
    pub name: Option<String>,
    pub address: Option<String>,
}

/// Paginated list of messages from the Graph API.
#[derive(Debug, Deserialize)]
pub struct MessageListResponse {
    pub value: Vec<GraphMessage>,
}

/// Body of an outbound message.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboundBody {
    pub content_type: String,
    pub content: String,
}

/// Recipient of an outbound message.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboundRecipient {
    pub email_address: OutboundEmailAddress,
}

/// Email address for outbound messages.
#[derive(Debug, Serialize)]
pub struct OutboundEmailAddress {
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// Request body for replying to a message.
#[derive(Debug, Serialize)]
pub struct ReplyRequest {
    pub message: ReplyBody,
}

/// Body for a reply message.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplyBody {
    pub body: OutboundBody,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to_recipients: Option<Vec<OutboundRecipient>>,
}

/// Request body for moving a message to a folder.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveRequest {
    pub destination_id: String,
}
