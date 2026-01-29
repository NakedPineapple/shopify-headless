//! Payment, payout, and dispute domain types for Shopify Admin API.

use serde::{Deserialize, Serialize};

use super::common::{Money, PageInfo};

// =============================================================================
// Payout Types
// =============================================================================

/// Payout status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PayoutStatus {
    /// Payout is scheduled.
    Scheduled,
    /// Payout is in transit.
    InTransit,
    /// Payout has been paid.
    Paid,
    /// Payout failed.
    Failed,
    /// Payout was canceled.
    Canceled,
}

impl std::fmt::Display for PayoutStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Scheduled => write!(f, "Scheduled"),
            Self::InTransit => write!(f, "In Transit"),
            Self::Paid => write!(f, "Paid"),
            Self::Failed => write!(f, "Failed"),
            Self::Canceled => write!(f, "Canceled"),
        }
    }
}

/// A Shopify Payments payout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payout {
    /// Payout ID.
    pub id: String,
    /// Legacy resource ID.
    pub legacy_resource_id: Option<String>,
    /// Payout status.
    pub status: PayoutStatus,
    /// Net amount (the payout amount).
    pub net: Money,
    /// When the payout was issued.
    pub issued_at: Option<String>,
}

/// Connection type for paginated payouts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutConnection {
    /// List of payouts.
    pub payouts: Vec<Payout>,
    /// Pagination info.
    pub page_info: PageInfo,
    /// Current account balance.
    pub balance: Option<Money>,
}

/// Payout transaction type (deposit or withdrawal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PayoutTransactionType {
    /// Regular deposit payout.
    Deposit,
    /// Withdrawal payout.
    Withdrawal,
}

impl std::fmt::Display for PayoutTransactionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Deposit => write!(f, "Deposit"),
            Self::Withdrawal => write!(f, "Withdrawal"),
        }
    }
}

/// Sort key for payout list queries.
///
/// Some keys are supported natively by Shopify's API, while others require
/// client-side sorting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PayoutSortKey {
    // === Shopify API supported ===
    /// Sort by issued date (Shopify native).
    #[default]
    IssuedAt,
    /// Sort by status (Shopify native).
    Status,
    /// Sort by net amount (Shopify native).
    Amount,
    /// Sort by gross charges (Shopify native).
    ChargeGross,
    /// Sort by fees (Shopify native).
    FeeAmount,
    /// Sort by ID (Shopify native).
    Id,

    // === Client-side sorting (Rust) ===
    /// Sort by transaction type (client-side).
    TransactionType,
}

impl PayoutSortKey {
    /// Parse a sort key from a URL parameter string.
    #[must_use]
    pub fn from_str_param(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "issued_at" | "date" => Some(Self::IssuedAt),
            "status" => Some(Self::Status),
            "amount" | "net" => Some(Self::Amount),
            "gross" | "charge_gross" => Some(Self::ChargeGross),
            "fees" | "fee_amount" => Some(Self::FeeAmount),
            "id" | "trace_id" => Some(Self::Id),
            "type" | "transaction_type" => Some(Self::TransactionType),
            _ => None,
        }
    }

    /// Get the URL parameter string for this sort key.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IssuedAt => "issued_at",
            Self::Status => "status",
            Self::Amount => "net",
            Self::ChargeGross => "gross",
            Self::FeeAmount => "fees",
            Self::Id => "trace_id",
            Self::TransactionType => "type",
        }
    }

    /// Whether this sort key is supported natively by Shopify API.
    #[must_use]
    pub const fn is_shopify_native(self) -> bool {
        matches!(
            self,
            Self::IssuedAt
                | Self::Status
                | Self::Amount
                | Self::ChargeGross
                | Self::FeeAmount
                | Self::Id
        )
    }
}

/// Payout summary breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutSummary {
    /// Gross charges amount.
    pub charges_gross: Money,
    /// Charges fees.
    pub charges_fee: Money,
    /// Refunds fee gross (total refund amount).
    pub refunds_fee_gross: Money,
    /// Refunds fees.
    pub refunds_fee: Money,
    /// Gross adjustments amount.
    pub adjustments_gross: Money,
    /// Adjustments fees.
    pub adjustments_fee: Money,
    /// Reserved funds gross.
    pub reserved_funds_gross: Money,
    /// Reserved funds fee.
    pub reserved_funds_fee: Money,
    /// Retried payouts gross.
    pub retried_payouts_gross: Money,
    /// Retried payouts fee.
    pub retried_payouts_fee: Money,
}

/// Detailed payout information including summary breakdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutDetail {
    /// Payout ID.
    pub id: String,
    /// Legacy resource ID.
    pub legacy_resource_id: Option<String>,
    /// Payout status.
    pub status: PayoutStatus,
    /// Transaction type.
    pub transaction_type: PayoutTransactionType,
    /// Net amount (the payout amount).
    pub net: Money,
    /// Gross amount.
    pub gross: Money,
    /// When the payout was issued.
    pub issued_at: Option<String>,
    /// Summary breakdown.
    pub summary: Option<PayoutSummary>,
}

// =============================================================================
// Balance Transaction Types
// =============================================================================

/// Balance transaction type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BalanceTransactionType {
    /// Charge transaction.
    Charge,
    /// Refund transaction.
    Refund,
    /// Adjustment transaction.
    Adjustment,
    /// Dispute transaction.
    Dispute,
    /// Payout transaction.
    Payout,
    /// Reserved funds transaction.
    ReservedFunds,
}

impl std::fmt::Display for BalanceTransactionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Charge => write!(f, "Charge"),
            Self::Refund => write!(f, "Refund"),
            Self::Adjustment => write!(f, "Adjustment"),
            Self::Dispute => write!(f, "Dispute"),
            Self::Payout => write!(f, "Payout"),
            Self::ReservedFunds => write!(f, "Reserved Funds"),
        }
    }
}

/// Source type for balance transactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BalanceTransactionSourceType {
    /// Charge source.
    Charge,
    /// Refund source.
    Refund,
    /// Adjustment source.
    Adjustment,
    /// Dispute source.
    Dispute,
    /// Payout source.
    Payout,
    /// Reserved funds source.
    ReservedFunds,
    /// Unknown source type.
    Unknown,
}

impl std::fmt::Display for BalanceTransactionSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Charge => write!(f, "Charge"),
            Self::Refund => write!(f, "Refund"),
            Self::Adjustment => write!(f, "Adjustment"),
            Self::Dispute => write!(f, "Dispute"),
            Self::Payout => write!(f, "Payout"),
            Self::ReservedFunds => write!(f, "Reserved Funds"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// A balance transaction in the Shopify Payments account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceTransaction {
    /// Transaction ID.
    pub id: String,
    /// Transaction amount.
    pub amount: Money,
    /// Transaction fee.
    pub fee: Money,
    /// Net amount.
    pub net: Money,
    /// Transaction date.
    pub transaction_date: String,
    /// Transaction type.
    pub transaction_type: BalanceTransactionType,
    /// Source type.
    pub source_type: BalanceTransactionSourceType,
    /// Associated order ID (if any).
    pub order_id: Option<String>,
    /// Associated order name (if any).
    pub order_name: Option<String>,
}

/// Connection type for paginated balance transactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BalanceTransactionConnection {
    /// List of transactions.
    pub transactions: Vec<BalanceTransaction>,
    /// Pagination info.
    pub page_info: PageInfo,
}

// =============================================================================
// Dispute Types
// =============================================================================

/// Dispute status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisputeStatus {
    /// Needs a response from the merchant.
    NeedsResponse,
    /// Under review by the card network.
    UnderReview,
    /// Charge has been refunded.
    ChargeRefunded,
    /// Merchant accepted the dispute.
    Accepted,
    /// Dispute was won.
    Won,
    /// Dispute was lost.
    Lost,
}

impl std::fmt::Display for DisputeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NeedsResponse => write!(f, "Needs Response"),
            Self::UnderReview => write!(f, "Under Review"),
            Self::ChargeRefunded => write!(f, "Charge Refunded"),
            Self::Accepted => write!(f, "Accepted"),
            Self::Won => write!(f, "Won"),
            Self::Lost => write!(f, "Lost"),
        }
    }
}

/// Dispute type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DisputeType {
    /// Chargeback dispute.
    Chargeback,
    /// Inquiry dispute.
    Inquiry,
}

impl std::fmt::Display for DisputeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Chargeback => write!(f, "Chargeback"),
            Self::Inquiry => write!(f, "Inquiry"),
        }
    }
}

/// Dispute reason details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeReasonDetails {
    /// Reason for the dispute.
    pub reason: String,
    /// Network reason code.
    pub network_reason_code: Option<String>,
}

/// A Shopify Payments dispute.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dispute {
    /// Dispute ID.
    pub id: String,
    /// Legacy resource ID.
    pub legacy_resource_id: Option<String>,
    /// Dispute status.
    pub status: DisputeStatus,
    /// Type of dispute (chargeback or inquiry).
    pub kind: DisputeType,
    /// Disputed amount.
    pub amount: Money,
    /// When the dispute was initiated.
    pub initiated_at: String,
    /// Evidence due date.
    pub evidence_due_by: Option<String>,
    /// When the dispute was finalized.
    pub finalized_on: Option<String>,
    /// Reason details.
    pub reason_details: Option<DisputeReasonDetails>,
    /// Associated order ID.
    pub order_id: Option<String>,
    /// Associated order name.
    pub order_name: Option<String>,
}

/// Connection type for paginated disputes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeConnection {
    /// List of disputes.
    pub disputes: Vec<Dispute>,
    /// Pagination info.
    pub page_info: PageInfo,
}

/// Address for dispute evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeAddress {
    /// Address line 1.
    pub address1: Option<String>,
    /// Address line 2.
    pub address2: Option<String>,
    /// City.
    pub city: Option<String>,
    /// Province/state code.
    pub province_code: Option<String>,
    /// Country code.
    pub country_code: Option<String>,
    /// ZIP/postal code.
    pub zip: Option<String>,
}

/// Dispute file upload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeFileUpload {
    /// File ID.
    pub id: String,
    /// Evidence type.
    pub evidence_type: String,
    /// File size in bytes.
    pub file_size: Option<i64>,
    /// Original file name.
    pub original_file_name: Option<String>,
    /// File MIME type.
    pub file_type: Option<String>,
    /// File URL.
    pub url: Option<String>,
}

/// Dispute fulfillment evidence (shipping information).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeFulfillment {
    /// Fulfillment ID.
    pub id: String,
    /// Carrier name.
    pub carrier: Option<String>,
    /// Ship date.
    pub date: Option<String>,
    /// Tracking number.
    pub tracking_number: Option<String>,
}

/// Dispute evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeEvidence {
    /// Product description.
    pub product_description: Option<String>,
    /// Customer first name.
    pub customer_first_name: Option<String>,
    /// Customer last name.
    pub customer_last_name: Option<String>,
    /// Customer email address.
    pub customer_email: Option<String>,
    /// Shipping carrier.
    pub shipping_carrier: Option<String>,
    /// Shipping tracking number.
    pub shipping_tracking_number: Option<String>,
    /// Uncategorized text.
    pub uncategorized_text: Option<String>,
    /// Whether evidence has been submitted.
    pub submitted: bool,
    /// Access activity log.
    pub access_activity_log: Option<String>,
    /// Cancellation policy disclosure.
    pub cancellation_policy_disclosure: Option<String>,
    /// Cancellation rebuttal.
    pub cancellation_rebuttal: Option<String>,
    /// Refund policy disclosure.
    pub refund_policy_disclosure: Option<String>,
    /// Refund refusal explanation.
    pub refund_refusal_explanation: Option<String>,
    /// Billing address.
    pub billing_address: Option<DisputeAddress>,
    /// Shipping address.
    pub shipping_address: Option<DisputeAddress>,
    /// File uploads.
    pub file_uploads: Vec<DisputeFileUpload>,
    /// Fulfillments.
    pub fulfillments: Vec<DisputeFulfillment>,
}

/// Detailed dispute information including evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisputeDetail {
    /// The dispute.
    pub dispute: Dispute,
    /// Customer email.
    pub customer_email: Option<String>,
    /// Customer name.
    pub customer_name: Option<String>,
    /// Order total.
    pub order_total: Option<Money>,
    /// Evidence.
    pub evidence: Option<DisputeEvidence>,
    /// When evidence was submitted.
    pub evidence_sent_on: Option<String>,
}

/// Input for updating dispute evidence.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DisputeEvidenceUpdateInput {
    /// Product description.
    pub product_description: Option<String>,
    /// Customer first name.
    pub customer_first_name: Option<String>,
    /// Customer last name.
    pub customer_last_name: Option<String>,
    /// Customer email.
    pub customer_email: Option<String>,
    /// Shipping carrier.
    pub shipping_carrier: Option<String>,
    /// Tracking number.
    pub shipping_tracking_number: Option<String>,
    /// Uncategorized text for additional context.
    pub uncategorized_text: Option<String>,
    /// Access activity log.
    pub access_activity_log: Option<String>,
    /// Cancellation policy disclosure.
    pub cancellation_policy_disclosure: Option<String>,
    /// Cancellation rebuttal.
    pub cancellation_rebuttal: Option<String>,
    /// Refund policy disclosure.
    pub refund_policy_disclosure: Option<String>,
    /// Refund refusal explanation.
    pub refund_refusal_explanation: Option<String>,
    /// Whether to submit the evidence.
    pub submit_evidence: bool,
}

// =============================================================================
// Bank Account Types
// =============================================================================

/// Bank account status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BankAccountStatus {
    /// Bank account is pending verification.
    Pending,
    /// Bank account is verified.
    Verified,
    /// Bank account has been deleted.
    Deleted,
}

impl std::fmt::Display for BankAccountStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "Pending"),
            Self::Verified => write!(f, "Verified"),
            Self::Deleted => write!(f, "Deleted"),
        }
    }
}

/// Payout schedule interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PayoutScheduleInterval {
    /// Daily payouts.
    Daily,
    /// Weekly payouts.
    Weekly,
    /// Monthly payouts.
    Monthly,
    /// Manual payouts.
    Manual,
}

impl std::fmt::Display for PayoutScheduleInterval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Daily => write!(f, "Daily"),
            Self::Weekly => write!(f, "Weekly"),
            Self::Monthly => write!(f, "Monthly"),
            Self::Manual => write!(f, "Manual"),
        }
    }
}

/// Payout schedule configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PayoutSchedule {
    /// Schedule interval.
    pub interval: PayoutScheduleInterval,
    /// Day of month for monthly payouts (1-31).
    pub monthly_anchor: Option<i64>,
    /// Day of week for weekly payouts (MONDAY-SUNDAY).
    pub weekly_anchor: Option<String>,
}

/// A connected bank account for Shopify Payments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BankAccount {
    /// Bank account ID.
    pub id: String,
    /// Last digits of account number.
    pub account_number_last_digits: String,
    /// Bank name.
    pub bank_name: Option<String>,
    /// Country code.
    pub country: String,
    /// Currency code.
    pub currency: String,
    /// Account status.
    pub status: BankAccountStatus,
    /// When the account was created.
    pub created_at: Option<String>,
}

/// Staged upload target for file uploads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedUploadTarget {
    /// The URL to upload the file to.
    pub url: String,
    /// The resource URL after upload completes.
    pub resource_url: String,
    /// Form parameters to include with the upload.
    pub parameters: Vec<(String, String)>,
}
