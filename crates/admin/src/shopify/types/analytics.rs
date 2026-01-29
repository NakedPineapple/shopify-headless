//! Domain types for analytics and channel data.

use serde::{Deserialize, Serialize};

use super::Money;

// =============================================================================
// `ShopifyQL` Types
// =============================================================================

/// Column metadata from a `ShopifyQL` query result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopifyqlColumn {
    /// The column name (machine-readable).
    pub name: String,
    /// The display name (human-readable).
    pub display_name: String,
    /// The data type of the column (e.g., "STRING", "NUMBER", "MONEY").
    pub data_type: String,
}

/// Result of a `ShopifyQL` query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShopifyqlResult {
    /// Column definitions.
    pub columns: Vec<ShopifyqlColumn>,
    /// Row data as JSON values.
    pub rows: Vec<serde_json::Value>,
    /// Parse errors if the query was invalid.
    pub parse_errors: Vec<String>,
}

impl ShopifyqlResult {
    /// Check if the query had parse errors.
    #[must_use]
    pub const fn has_errors(&self) -> bool {
        !self.parse_errors.is_empty()
    }

    /// Get the index of a column by name.
    #[must_use]
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c.name == name)
    }
}

// =============================================================================
// Sales Channel Types
// =============================================================================

/// A sales channel (publication) where products can be sold.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesChannel {
    /// The Shopify GID for this publication.
    pub id: String,
    /// The name of the channel.
    pub name: String,
    /// The app that provides this channel.
    pub app: Option<SalesChannelApp>,
    /// Whether products are automatically published to this channel.
    pub auto_publish: bool,
    /// Whether this channel supports future publishing.
    pub supports_future_publishing: bool,
}

/// App information for a sales channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalesChannelApp {
    /// The app's Shopify GID.
    pub id: String,
    /// The app's display title.
    pub title: String,
    /// The app's handle (url-safe name).
    pub handle: Option<String>,
}

// =============================================================================
// Channel Metrics Types
// =============================================================================

/// Performance metrics for a sales channel.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelMetrics {
    /// The channel name or identifier.
    pub channel_name: String,
    /// Total sales amount.
    pub total_sales: f64,
    /// Net sales (after returns/refunds).
    pub net_sales: f64,
    /// Total number of orders.
    pub orders: i64,
    /// Total units sold.
    pub units_sold: i64,
    /// Average order value.
    pub average_order_value: f64,
}

impl ChannelMetrics {
    /// Calculate average order value from total sales and orders.
    // Order counts in e-commerce will never exceed i64's f64-safe range (2^52)
    #[allow(clippy::cast_precision_loss)]
    pub fn calculate_aov(&mut self) {
        if self.orders > 0 {
            self.average_order_value = self.total_sales / self.orders as f64;
        }
    }
}

/// Summary of analytics data across all channels.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnalyticsSummary {
    /// Total sales across all channels.
    pub total_sales: f64,
    /// Total net sales across all channels.
    pub total_net_sales: f64,
    /// Total orders across all channels.
    pub total_orders: i64,
    /// Total units sold across all channels.
    pub total_units: i64,
    /// Average order value across all channels.
    pub average_order_value: f64,
    /// Per-channel breakdown.
    pub channels: Vec<ChannelMetrics>,
}

impl AnalyticsSummary {
    /// Calculate totals from channel data.
    // Order counts in e-commerce will never exceed i64's f64-safe range (2^52)
    #[allow(clippy::cast_precision_loss)]
    pub fn calculate_totals(&mut self) {
        self.total_sales = self.channels.iter().map(|c| c.total_sales).sum();
        self.total_net_sales = self.channels.iter().map(|c| c.net_sales).sum();
        self.total_orders = self.channels.iter().map(|c| c.orders).sum();
        self.total_units = self.channels.iter().map(|c| c.units_sold).sum();

        if self.total_orders > 0 {
            self.average_order_value = self.total_sales / self.total_orders as f64;
        }
    }
}

/// Daily metrics for time series data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyMetrics {
    /// The date (YYYY-MM-DD format).
    pub date: String,
    /// Total sales for this day.
    pub total_sales: f64,
    /// Number of orders for this day.
    pub orders: i64,
    /// Optional channel name if grouped by channel.
    pub channel_name: Option<String>,
}

// =============================================================================
// Date Range Types
// =============================================================================

/// Date range for analytics queries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DateRange {
    /// Start date in `ShopifyQL` format (e.g., "-30d", "2024-01-01").
    pub start: String,
    /// End date in `ShopifyQL` format (e.g., "today", "2024-01-31").
    pub end: String,
}

impl Default for DateRange {
    fn default() -> Self {
        Self {
            start: "-30d".to_string(),
            end: "today".to_string(),
        }
    }
}

impl DateRange {
    /// Create a date range for the last N days.
    #[must_use]
    pub fn last_days(days: u32) -> Self {
        Self {
            start: format!("-{days}d"),
            end: "today".to_string(),
        }
    }

    /// Create a date range for a specific period.
    #[must_use]
    pub fn new(start: impl Into<String>, end: impl Into<String>) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
        }
    }
}

// =============================================================================
// Marketing Activity Types
// =============================================================================

/// Status of a marketing activity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MarketingActivityStatus {
    /// Activity is currently running.
    Active,
    /// Activity has been paused.
    Paused,
    /// Activity has finished.
    Inactive,
    /// Activity is scheduled but not started.
    Scheduled,
    /// Activity has been deleted.
    Deleted,
    /// Activity failed to start.
    Failed,
    /// Activity is being created.
    Pending,
    /// Unknown status.
    #[serde(other)]
    Unknown,
}

impl MarketingActivityStatus {
    /// Get a display-friendly label for the status.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Active => "Active",
            Self::Paused => "Paused",
            Self::Inactive => "Ended",
            Self::Scheduled => "Scheduled",
            Self::Deleted => "Deleted",
            Self::Failed => "Failed",
            Self::Pending => "Pending",
            Self::Unknown => "Unknown",
        }
    }

    /// Get a CSS class for styling the status badge.
    #[must_use]
    pub const fn badge_class(self) -> &'static str {
        match self {
            Self::Active => "badge-success",
            Self::Paused => "badge-warning",
            Self::Scheduled => "badge-info",
            Self::Failed => "badge-error",
            Self::Inactive | Self::Deleted | Self::Pending | Self::Unknown => "badge-secondary",
        }
    }
}

/// A marketing activity (campaign).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketingActivity {
    /// The Shopify GID.
    pub id: String,
    /// The activity title.
    pub title: String,
    /// Current status.
    pub status: MarketingActivityStatus,
    /// The marketing channel type (e.g., "SOCIAL", "SEARCH", "EMAIL").
    pub channel_type: String,
    /// The marketing tactic (e.g., "AD", "POST", "MESSAGE").
    pub tactic: Option<String>,
    /// Source and medium string (e.g., "facebook / cpc").
    pub source_and_medium: Option<String>,
    /// Total budget amount.
    pub budget: Option<Money>,
    /// Amount spent on ads.
    pub ad_spend: Option<Money>,
    /// UTM campaign parameter.
    pub utm_campaign: Option<String>,
    /// UTM medium parameter.
    pub utm_medium: Option<String>,
    /// UTM source parameter.
    pub utm_source: Option<String>,
    /// When the activity was created.
    pub created_at: String,
    /// When the activity started.
    pub started_at: Option<String>,
    /// When the activity ended.
    pub ended_at: Option<String>,
    /// When the activity is scheduled to end.
    pub scheduled_to_end_at: Option<String>,
}

/// Summary of marketing activities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MarketingActivitySummary {
    /// Total number of activities.
    pub total_activities: i64,
    /// Number of active activities.
    pub active_count: i64,
    /// Total budget across all activities.
    pub total_budget: f64,
    /// Total ad spend across all activities.
    pub total_ad_spend: f64,
    /// Currency code for budget/spend values.
    pub currency_code: Option<String>,
}
