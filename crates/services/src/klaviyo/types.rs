//! Klaviyo API types.
//!
//! These types follow Klaviyo's JSON:API format with `data`, `attributes`, etc.

use serde::{Deserialize, Serialize};

/// Wrapper for JSON:API response with a single resource.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiResponse<T> {
    pub data: T,
}

/// Wrapper for JSON:API response with multiple resources.
#[derive(Debug, Clone, Deserialize)]
pub struct ApiListResponse<T> {
    pub data: Vec<T>,
    #[serde(default)]
    pub links: Option<PaginationLinks>,
}

/// Pagination links in JSON:API responses.
#[derive(Debug, Clone, Deserialize)]
pub struct PaginationLinks {
    #[serde(rename = "self")]
    pub self_link: Option<String>,
    pub next: Option<String>,
    pub prev: Option<String>,
}

/// Campaign resource.
#[derive(Debug, Clone, Deserialize)]
pub struct Campaign {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub attributes: CampaignAttributes,
}

/// Campaign attributes.
#[derive(Debug, Clone, Deserialize)]
pub struct CampaignAttributes {
    pub name: String,
    pub status: CampaignStatus,
    #[serde(default)]
    pub archived: bool,
    pub audiences: CampaignAudiences,
    pub send_options: Option<CampaignSendOptions>,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub scheduled_at: Option<String>,
    #[serde(default)]
    pub send_time: Option<String>,
}

/// Campaign status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CampaignStatus {
    Draft,
    Scheduled,
    Sending,
    Sent,
    Cancelled,
}

/// Campaign channel (email or SMS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CampaignChannel {
    Email,
    Sms,
}

impl CampaignChannel {
    /// Get the Klaviyo API filter value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Email => "email",
            Self::Sms => "sms",
        }
    }

    /// Get a human-readable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Email => "Email",
            Self::Sms => "SMS",
        }
    }
}

impl CampaignStatus {
    /// Get a human-readable label for the status.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Draft => "Draft",
            Self::Scheduled => "Scheduled",
            Self::Sending => "Sending",
            Self::Sent => "Sent",
            Self::Cancelled => "Cancelled",
        }
    }

    /// Get a CSS class for styling the status badge.
    #[must_use]
    pub const fn badge_class(self) -> &'static str {
        match self {
            Self::Draft => "bg-lagoon-400/20 text-lagoon-400",
            Self::Scheduled => "bg-honey/20 text-honey",
            Self::Sending => "bg-coral/20 text-coral",
            Self::Sent => "bg-leaf/20 text-leaf",
            Self::Cancelled => "bg-lagoon-600/20 text-lagoon-400",
        }
    }
}

/// Campaign audiences configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignAudiences {
    #[serde(default)]
    pub included: Vec<String>,
    #[serde(default)]
    pub excluded: Vec<String>,
}

/// Campaign send options.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignSendOptions {
    #[serde(default)]
    pub use_smart_sending: bool,
}

/// Campaign message (email content).
#[derive(Debug, Clone, Deserialize)]
pub struct CampaignMessage {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub attributes: CampaignMessageAttributes,
}

/// Campaign message attributes.
#[derive(Debug, Clone, Deserialize)]
pub struct CampaignMessageAttributes {
    pub label: String,
    pub channel: String,
    pub content: CampaignMessageContent,
}

/// Campaign message content (email HTML/text).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignMessageContent {
    pub subject: String,
    #[serde(default)]
    pub preview_text: Option<String>,
    pub from_email: String,
    pub from_label: String,
    #[serde(default)]
    pub reply_to_email: Option<String>,
    #[serde(default)]
    pub cc_email: Option<String>,
    #[serde(default)]
    pub bcc_email: Option<String>,
}

/// SMS message content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmsMessageContent {
    /// SMS message body (160 chars standard, 70 with emoji).
    pub body: String,
}

/// SMS character limits.
pub const SMS_STANDARD_CHAR_LIMIT: usize = 160;
pub const SMS_EMOJI_CHAR_LIMIT: usize = 70;

/// Input for creating a new campaign.
#[derive(Debug, Clone, Serialize)]
pub struct CreateCampaignInput {
    pub data: CreateCampaignData,
}

/// Data for creating a campaign.
#[derive(Debug, Clone, Serialize)]
pub struct CreateCampaignData {
    #[serde(rename = "type")]
    pub resource_type: &'static str,
    pub attributes: CreateCampaignAttributes,
}

/// Attributes for creating a campaign.
#[derive(Debug, Clone, Serialize)]
pub struct CreateCampaignAttributes {
    pub name: String,
    pub audiences: CampaignAudiences,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_options: Option<CampaignSendOptions>,
    pub campaign_messages: CreateCampaignMessages,
}

/// Messages for creating a campaign.
#[derive(Debug, Clone, Serialize)]
pub struct CreateCampaignMessages {
    pub data: Vec<CreateCampaignMessageData>,
}

/// Message data for creating a campaign.
#[derive(Debug, Clone, Serialize)]
pub struct CreateCampaignMessageData {
    #[serde(rename = "type")]
    pub resource_type: &'static str,
    pub attributes: CreateCampaignMessageAttributes,
}

/// Message attributes for creating a campaign.
#[derive(Debug, Clone, Serialize)]
pub struct CreateCampaignMessageAttributes {
    pub channel: &'static str,
    pub label: String,
    pub content: CampaignMessageContent,
    pub render_options: Option<RenderOptions>,
}

/// Render options for campaign messages.
#[derive(Debug, Clone, Serialize)]
pub struct RenderOptions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shorten_links: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add_org_prefix: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add_info_link: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add_opt_out_language: Option<bool>,
}

/// Input for updating a campaign.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateCampaignInput {
    pub data: UpdateCampaignData,
}

/// Data for updating a campaign.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateCampaignData {
    #[serde(rename = "type")]
    pub resource_type: &'static str,
    pub id: String,
    pub attributes: UpdateCampaignAttributes,
}

/// Attributes for updating a campaign.
#[derive(Debug, Clone, Serialize)]
pub struct UpdateCampaignAttributes {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audiences: Option<CampaignAudiences>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub send_options: Option<CampaignSendOptions>,
}

/// Input for sending a campaign.
#[derive(Debug, Clone, Serialize)]
pub struct SendCampaignInput {
    pub data: SendCampaignData,
}

/// Data for sending a campaign.
#[derive(Debug, Clone, Serialize)]
pub struct SendCampaignData {
    #[serde(rename = "type")]
    pub resource_type: &'static str,
    pub id: String,
}

/// Campaign send job response.
#[derive(Debug, Clone, Deserialize)]
pub struct CampaignSendJob {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub attributes: CampaignSendJobAttributes,
}

/// Campaign send job attributes.
#[derive(Debug, Clone, Deserialize)]
pub struct CampaignSendJobAttributes {
    pub status: String,
}

/// List resource.
#[derive(Debug, Clone, Deserialize)]
pub struct List {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub attributes: ListAttributes,
}

/// List attributes.
#[derive(Debug, Clone, Deserialize)]
pub struct ListAttributes {
    pub name: String,
    pub created: Option<String>,
    pub updated: Option<String>,
    #[serde(default)]
    pub opt_in_process: Option<String>,
}

/// Profile (subscriber) resource.
#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    pub id: String,
    #[serde(rename = "type")]
    pub resource_type: String,
    pub attributes: ProfileAttributes,
}

/// Profile attributes.
#[derive(Debug, Clone, Deserialize)]
pub struct ProfileAttributes {
    pub email: Option<String>,
    pub phone_number: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    #[serde(default)]
    pub subscriptions: Option<ProfileSubscriptions>,
    pub created: Option<String>,
    pub updated: Option<String>,
}

/// Profile subscription status.
#[derive(Debug, Clone, Deserialize)]
pub struct ProfileSubscriptions {
    pub email: Option<ChannelSubscription>,
    pub sms: Option<ChannelSubscription>,
}

/// Channel subscription details (email or SMS).
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelSubscription {
    pub marketing: SubscriptionStatus,
}

/// Subscription status details.
#[derive(Debug, Clone, Deserialize)]
pub struct SubscriptionStatus {
    pub consent: String,
    #[serde(default)]
    pub timestamp: Option<String>,
}

/// Subscriber statistics for dashboard.
#[derive(Debug, Clone, Default)]
pub struct SubscriberStats {
    /// Total email subscribers.
    pub email_subscribers: u64,
    /// Total SMS subscribers.
    pub sms_subscribers: u64,
}

/// Campaign statistics.
#[derive(Debug, Clone, Default)]
pub struct CampaignStats {
    pub recipients: u64,
    pub opens: u64,
    pub unique_opens: u64,
    pub clicks: u64,
    pub unique_clicks: u64,
    pub bounces: u64,
    pub unsubscribes: u64,
    pub open_rate: f64,
    pub click_rate: f64,
}

impl CampaignStats {
    /// Calculate rates from raw counts.
    #[must_use]
    pub fn with_rates(mut self) -> Self {
        if self.recipients > 0 {
            #[allow(clippy::cast_precision_loss)]
            {
                self.open_rate = (self.unique_opens as f64 / self.recipients as f64) * 100.0;
                self.click_rate = (self.unique_clicks as f64 / self.recipients as f64) * 100.0;
            }
        }
        self
    }
}
