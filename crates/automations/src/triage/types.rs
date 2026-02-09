//! Types for the email triage pipeline.
//!
//! Defines classification categories, triage results, and routing actions
//! for inbound emails processed by the AI classifier.

use serde::{Deserialize, Serialize};

/// Email classification categories assigned by the AI classifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmailClassification {
    /// Spam or unsolicited messages.
    Spam,
    /// Marketing newsletters and promotions.
    MarketingNewsletter,
    /// Customer asking about an order.
    OrderInquiry,
    /// Customer asking about a product.
    ProductQuestion,
    /// Customer requesting a return or exchange.
    ReturnRequest,
    /// Customer reporting a shipping or delivery issue.
    ShippingIssue,
    /// Customer asking about subscriptions.
    SubscriptionInquiry,
    /// Business/vendor inquiry (wholesale, partnership, etc.).
    BusinessVendor,
    /// Customer complaint.
    Complaint,
    /// Customer praise or positive feedback.
    Praise,
    /// Could not confidently classify.
    Unknown,
}

impl EmailClassification {
    /// Whether this classification should be archived without further action.
    #[must_use]
    pub const fn is_archivable(self) -> bool {
        matches!(self, Self::Spam | Self::MarketingNewsletter)
    }

    /// Whether this classification should trigger a draft response for Slack review.
    #[must_use]
    pub const fn needs_draft_response(self) -> bool {
        matches!(
            self,
            Self::OrderInquiry
                | Self::ProductQuestion
                | Self::ReturnRequest
                | Self::ShippingIssue
                | Self::SubscriptionInquiry
        )
    }

    /// Whether this classification should route to Klaviyo Helpdesk.
    #[must_use]
    pub const fn routes_to_helpdesk(self) -> bool {
        matches!(self, Self::Complaint | Self::Praise | Self::Unknown)
    }

    /// Human-readable label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Spam => "Spam",
            Self::MarketingNewsletter => "Marketing Newsletter",
            Self::OrderInquiry => "Order Inquiry",
            Self::ProductQuestion => "Product Question",
            Self::ReturnRequest => "Return Request",
            Self::ShippingIssue => "Shipping Issue",
            Self::SubscriptionInquiry => "Subscription Inquiry",
            Self::BusinessVendor => "Business/Vendor",
            Self::Complaint => "Complaint",
            Self::Praise => "Praise",
            Self::Unknown => "Unknown",
        }
    }
}

impl std::fmt::Display for EmailClassification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Result of the AI classification step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    /// The assigned classification category.
    pub classification: EmailClassification,
    /// Optional sub-category for more specific routing.
    pub sub_category: Option<String>,
    /// Confidence score (0.0 to 1.0).
    pub confidence: f64,
    /// AI reasoning for the classification.
    pub reasoning: String,
    /// Extracted entities (order numbers, product names, etc.).
    pub extracted_entities: ExtractedEntities,
}

/// Entities extracted from the email by the AI classifier.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExtractedEntities {
    /// Order numbers mentioned in the email.
    #[serde(default)]
    pub order_numbers: Vec<String>,
    /// Product names or SKUs mentioned.
    #[serde(default)]
    pub product_names: Vec<String>,
    /// Tracking numbers mentioned.
    #[serde(default)]
    pub tracking_numbers: Vec<String>,
    /// Customer name if identifiable.
    pub customer_name: Option<String>,
}

/// Status values for inbound emails in the database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmailStatus {
    /// Newly received, not yet processed.
    Pending,
    /// AI classification complete.
    Classified,
    /// Routed to destination (Klaviyo, archive, etc.).
    Routed,
    /// Draft response pending Slack review.
    PendingReview,
    /// Response approved and sent.
    Responded,
    /// Archived (spam/newsletter).
    Archived,
    /// Processing failed.
    Failed,
}

impl EmailStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Classified => "classified",
            Self::Routed => "routed",
            Self::PendingReview => "pending_review",
            Self::Responded => "responded",
            Self::Archived => "archived",
            Self::Failed => "failed",
        }
    }
}

impl std::fmt::Display for EmailStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classification_archivable() {
        assert!(EmailClassification::Spam.is_archivable());
        assert!(EmailClassification::MarketingNewsletter.is_archivable());
        assert!(!EmailClassification::OrderInquiry.is_archivable());
    }

    #[test]
    fn test_classification_needs_draft() {
        assert!(EmailClassification::OrderInquiry.needs_draft_response());
        assert!(EmailClassification::ProductQuestion.needs_draft_response());
        assert!(EmailClassification::ReturnRequest.needs_draft_response());
        assert!(!EmailClassification::Spam.needs_draft_response());
        assert!(!EmailClassification::Complaint.needs_draft_response());
    }

    #[test]
    fn test_classification_routes_to_helpdesk() {
        assert!(EmailClassification::Complaint.routes_to_helpdesk());
        assert!(EmailClassification::Praise.routes_to_helpdesk());
        assert!(EmailClassification::Unknown.routes_to_helpdesk());
        assert!(!EmailClassification::OrderInquiry.routes_to_helpdesk());
    }

    #[test]
    fn test_classification_serialization() {
        let json = serde_json::to_string(&EmailClassification::OrderInquiry).expect("serialize");
        assert_eq!(json, "\"order_inquiry\"");
    }

    #[test]
    fn test_classification_deserialization() {
        let c: EmailClassification =
            serde_json::from_str("\"shipping_issue\"").expect("deserialize");
        assert_eq!(c, EmailClassification::ShippingIssue);
    }

    #[test]
    fn test_email_status_as_str() {
        assert_eq!(EmailStatus::Pending.as_str(), "pending");
        assert_eq!(EmailStatus::PendingReview.as_str(), "pending_review");
        assert_eq!(EmailStatus::Responded.as_str(), "responded");
    }
}
