//! Slack Block Kit types for building rich messages.
//!
//! These types represent a subset of the Slack Block Kit specification
//! needed for building interactive confirmation messages.
//!
//! See: <https://api.slack.com/block-kit>

use serde::{Deserialize, Serialize};

/// A Slack message with blocks.
#[derive(Debug, Clone, Serialize)]
pub struct SlackMessage {
    /// Channel ID to post to.
    pub channel: String,
    /// Message blocks.
    pub blocks: Vec<Block>,
    /// Optional plain text fallback.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

/// Block Kit block types.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Block {
    /// Header block with large text.
    Header { text: PlainText },
    /// Section block with text and optional accessory.
    Section {
        text: Text,
        #[serde(skip_serializing_if = "Option::is_none")]
        accessory: Option<Accessory>,
    },
    /// Context block with small muted text/images.
    Context { elements: Vec<ContextElement> },
    /// Actions block with interactive elements.
    Actions { elements: Vec<ActionElement> },
    /// Divider block (horizontal line).
    Divider,
}

/// Text object types.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Text {
    /// Plain text (no formatting).
    PlainText { text: String, emoji: bool },
    /// Markdown text (supports formatting).
    Mrkdwn { text: String },
}

impl Text {
    /// Create a plain text object.
    #[must_use]
    pub fn plain(text: impl Into<String>) -> Self {
        Self::PlainText {
            text: text.into(),
            emoji: true,
        }
    }

    /// Create a markdown text object.
    #[must_use]
    pub fn mrkdwn(text: impl Into<String>) -> Self {
        Self::Mrkdwn { text: text.into() }
    }
}

/// Plain text object (for headers).
#[derive(Debug, Clone, Serialize)]
pub struct PlainText {
    #[serde(rename = "type")]
    pub text_type: &'static str,
    pub text: String,
    pub emoji: bool,
}

impl PlainText {
    /// Create a new plain text object.
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text_type: "plain_text",
            text: text.into(),
            emoji: true,
        }
    }
}

/// Context block elements.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContextElement {
    /// Markdown text in context.
    Mrkdwn { text: String },
    /// Plain text in context.
    PlainText { text: String, emoji: bool },
}

/// Accessory elements for section blocks.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Accessory {
    /// Button accessory.
    Button {
        text: PlainText,
        action_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ButtonStyle>,
    },
}

/// Action block elements.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActionElement {
    /// Interactive button.
    Button {
        text: PlainText,
        action_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        style: Option<ButtonStyle>,
    },
}

/// Button style (affects color).
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonStyle {
    /// Green primary button.
    Primary,
    /// Red danger button.
    Danger,
}

// =============================================================================
// Response Types
// =============================================================================

/// Response from posting a message.
#[derive(Debug, Clone, Deserialize)]
pub struct PostMessageResponse {
    /// Whether the request was successful.
    pub ok: bool,
    /// Channel ID where message was posted.
    #[serde(default)]
    pub channel: Option<String>,
    /// Message timestamp (unique ID).
    #[serde(default)]
    pub ts: Option<String>,
    /// Error message if not ok.
    #[serde(default)]
    pub error: Option<String>,
}

/// Response from updating a message.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateMessageResponse {
    /// Whether the request was successful.
    pub ok: bool,
    /// Channel ID.
    #[serde(default)]
    pub channel: Option<String>,
    /// Updated message timestamp.
    #[serde(default)]
    pub ts: Option<String>,
    /// Error message if not ok.
    #[serde(default)]
    pub error: Option<String>,
}

/// Slack interaction payload from button clicks.
#[derive(Debug, Clone, Deserialize)]
pub struct InteractionPayload {
    /// Type of interaction.
    #[serde(rename = "type")]
    pub interaction_type: String,
    /// User who triggered the interaction.
    pub user: InteractionUser,
    /// Container information.
    pub container: InteractionContainer,
    /// Channel where interaction occurred.
    #[serde(default)]
    pub channel: Option<InteractionChannel>,
    /// Actions that were triggered.
    pub actions: Vec<InteractionAction>,
    /// Response URL for updating the message.
    #[serde(default)]
    pub response_url: Option<String>,
    /// Trigger ID for opening modals.
    #[serde(default)]
    pub trigger_id: Option<String>,
}

/// User who triggered an interaction.
#[derive(Debug, Clone, Deserialize)]
pub struct InteractionUser {
    /// Slack user ID.
    pub id: String,
    /// Username.
    #[serde(default)]
    pub username: Option<String>,
    /// Display name.
    #[serde(default)]
    pub name: Option<String>,
}

/// Container for the interaction.
#[derive(Debug, Clone, Deserialize)]
pub struct InteractionContainer {
    /// Container type (e.g., "message").
    #[serde(rename = "type")]
    pub container_type: String,
    /// Message timestamp.
    #[serde(default)]
    pub message_ts: Option<String>,
    /// Channel ID.
    #[serde(default)]
    pub channel_id: Option<String>,
}

/// Channel where interaction occurred.
#[derive(Debug, Clone, Deserialize)]
pub struct InteractionChannel {
    /// Channel ID.
    pub id: String,
    /// Channel name.
    #[serde(default)]
    pub name: Option<String>,
}

/// Action that was triggered.
#[derive(Debug, Clone, Deserialize)]
pub struct InteractionAction {
    /// Action ID (set when creating the button).
    pub action_id: String,
    /// Block ID containing this action.
    #[serde(default)]
    pub block_id: Option<String>,
    /// Value attached to the action.
    #[serde(default)]
    pub value: Option<String>,
    /// Action type.
    #[serde(rename = "type")]
    pub action_type: String,
}
