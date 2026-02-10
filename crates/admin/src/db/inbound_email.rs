//! Database queries for the email inbox.
//!
//! Read-oriented queries against `admin.inbound_email` for the admin UI.
//! Write operations (classify, route, etc.) live in the automations crate.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use super::RepositoryError;

// =============================================================================
// Row Types
// =============================================================================

/// Summary row for the email list view.
#[derive(Debug)]
pub struct InboundEmailSummary {
    pub id: i32,
    pub from_address: String,
    pub from_name: Option<String>,
    pub subject: String,
    pub mailbox: String,
    pub classification: Option<String>,
    pub confidence: Option<f32>,
    pub status: String,
    pub received_at: DateTime<Utc>,
    pub has_draft: bool,
    pub reviewed_by: Option<String>,
    pub error: Option<String>,
}

/// Internal row for the list query (matches sqlx column mapping).
#[derive(Debug, sqlx::FromRow)]
struct SummaryRow {
    id: i32,
    from_address: String,
    from_name: Option<String>,
    subject: String,
    mailbox: String,
    classification: Option<String>,
    confidence: Option<f32>,
    status: String,
    received_at: DateTime<Utc>,
    has_draft: bool,
    reviewed_by: Option<String>,
    error: Option<String>,
}

impl From<SummaryRow> for InboundEmailSummary {
    fn from(row: SummaryRow) -> Self {
        Self {
            id: row.id,
            from_address: row.from_address,
            from_name: row.from_name,
            subject: row.subject,
            mailbox: row.mailbox,
            classification: row.classification,
            confidence: row.confidence,
            status: row.status,
            received_at: row.received_at,
            has_draft: row.has_draft,
            reviewed_by: row.reviewed_by,
            error: row.error,
        }
    }
}

impl InboundEmailSummary {
    /// True when AI classification confidence is below the review threshold.
    #[must_use]
    pub fn is_low_confidence(&self) -> bool {
        self.confidence.is_some_and(|c| c < 0.7)
    }
}

/// Full detail for a single email.
#[derive(Debug)]
pub struct InboundEmailDetail {
    pub id: i32,
    pub m365_message_id: String,
    pub conversation_id: String,
    pub mailbox: String,
    pub from_address: String,
    pub from_name: Option<String>,
    pub to_addresses: serde_json::Value,
    pub subject: String,
    pub body_preview: Option<String>,
    pub body_text: Option<String>,
    pub received_at: DateTime<Utc>,
    pub classification: Option<String>,
    pub sub_category: Option<String>,
    pub confidence: Option<f32>,
    pub reasoning: Option<String>,
    pub status: String,
    pub routed_to: Option<String>,
    pub klaviyo_ticket_id: Option<String>,
    pub response_draft: Option<String>,
    pub response_approved: bool,
    pub response_sent_at: Option<DateTime<Utc>>,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// An entity extracted from classification reasoning or email body.
pub struct ExtractedEntity {
    /// Phosphor icon class (e.g. "ph-user").
    pub icon: &'static str,
    /// Display label (e.g. "Customer").
    pub label: &'static str,
    /// The extracted value.
    pub value: String,
}

impl InboundEmailDetail {
    /// Confidence as a rounded integer string (e.g. "85") for display.
    #[must_use]
    pub fn confidence_pct(&self) -> String {
        self.confidence.map_or_else(
            || "0".to_string(),
            |c| format!("{:.0}", f64::from(c) * 100.0),
        )
    }

    /// "high" (>=80%), "medium" (>=50%), or "low" for CSS class selection.
    #[must_use]
    pub fn confidence_level(&self) -> &str {
        match self.confidence {
            Some(c) if c >= 0.8 => "high",
            Some(c) if c >= 0.5 => "medium",
            _ => "low",
        }
    }

    /// Format `to_addresses` JSON array as a comma-separated string.
    #[must_use]
    pub fn to_addresses_display(&self) -> String {
        self.to_addresses
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(serde_json::Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default()
    }

    /// Extract structured entities from reasoning text and email body.
    ///
    /// The AI classifier returns order numbers, product names, tracking numbers,
    /// and customer name as structured fields, but only the reasoning text is
    /// persisted to the DB. This method re-extracts common patterns.
    #[must_use]
    pub fn extracted_entities(&self) -> Vec<ExtractedEntity> {
        let mut entities = Vec::new();

        if let Some(name) = &self.from_name {
            entities.push(ExtractedEntity {
                icon: "ph-user",
                label: "Customer",
                value: name.clone(),
            });
        }

        let search_text = format!(
            "{} {}",
            self.reasoning.as_deref().unwrap_or(""),
            self.body_text
                .as_deref()
                .or(self.body_preview.as_deref())
                .unwrap_or(""),
        );

        extract_order_numbers(&search_text, &mut entities);
        extract_tracking_numbers(&search_text, &mut entities);

        entities
    }
}

/// Find order-number patterns: `#` followed by 4+ digits.
fn extract_order_numbers(text: &str, entities: &mut Vec<ExtractedEntity>) {
    for word in text.split_whitespace() {
        let cleaned = word.trim_matches(|c: char| c == ',' || c == '.' || c == ')' || c == ']');
        if let Some(num) = cleaned.strip_prefix('#')
            && num.len() >= 4
            && num.chars().all(|c| c.is_ascii_digit())
        {
            let value = cleaned.to_string();
            if !entities
                .iter()
                .any(|e| e.label == "Order" && e.value == value)
            {
                entities.push(ExtractedEntity {
                    icon: "ph-hash",
                    label: "Order",
                    value,
                });
            }
        }
    }
}

/// Find tracking-number patterns: 1Z (UPS) or 15+ digit strings (USPS/FedEx).
fn extract_tracking_numbers(text: &str, entities: &mut Vec<ExtractedEntity>) {
    for word in text.split_whitespace() {
        let cleaned = word.trim_matches(|c: char| c == ',' || c == '.' || c == ')' || c == ']');
        let is_tracking = (cleaned.starts_with("1Z") && cleaned.len() >= 18)
            || (cleaned.len() >= 15 && cleaned.chars().all(|c| c.is_ascii_digit()));
        if is_tracking {
            let value = cleaned.to_string();
            if !entities
                .iter()
                .any(|e| e.label == "Tracking" && e.value == value)
            {
                entities.push(ExtractedEntity {
                    icon: "ph-package",
                    label: "Tracking",
                    value,
                });
            }
        }
    }
}

/// Internal row for the detail query.
#[derive(Debug, sqlx::FromRow)]
struct DetailRow {
    id: i32,
    m365_message_id: String,
    conversation_id: String,
    mailbox: String,
    from_address: String,
    from_name: Option<String>,
    to_addresses: serde_json::Value,
    subject: String,
    body_preview: Option<String>,
    body_text: Option<String>,
    received_at: DateTime<Utc>,
    classification: Option<String>,
    sub_category: Option<String>,
    confidence: Option<f32>,
    reasoning: Option<String>,
    status: String,
    routed_to: Option<String>,
    klaviyo_ticket_id: Option<String>,
    response_draft: Option<String>,
    response_approved: bool,
    response_sent_at: Option<DateTime<Utc>>,
    reviewed_by: Option<String>,
    reviewed_at: Option<DateTime<Utc>>,
    error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<DetailRow> for InboundEmailDetail {
    fn from(row: DetailRow) -> Self {
        Self {
            id: row.id,
            m365_message_id: row.m365_message_id,
            conversation_id: row.conversation_id,
            mailbox: row.mailbox,
            from_address: row.from_address,
            from_name: row.from_name,
            to_addresses: row.to_addresses,
            subject: row.subject,
            body_preview: row.body_preview,
            body_text: row.body_text,
            received_at: row.received_at,
            classification: row.classification,
            sub_category: row.sub_category,
            confidence: row.confidence,
            reasoning: row.reasoning,
            status: row.status,
            routed_to: row.routed_to,
            klaviyo_ticket_id: row.klaviyo_ticket_id,
            response_draft: row.response_draft,
            response_approved: row.response_approved,
            response_sent_at: row.response_sent_at,
            reviewed_by: row.reviewed_by,
            reviewed_at: row.reviewed_at,
            error: row.error,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// A message in the same conversation thread.
#[derive(Debug)]
pub struct ThreadMessage {
    pub id: i32,
    pub from_address: String,
    pub from_name: Option<String>,
    pub subject: String,
    pub body_preview: Option<String>,
    pub received_at: DateTime<Utc>,
    pub status: String,
}

/// Status count for tab badges.
#[derive(Debug)]
pub struct StatusCount {
    pub status: String,
    pub count: i64,
}

// =============================================================================
// Queries
// =============================================================================

/// List emails with optional status and classification filters.
///
/// # Errors
///
/// Returns `RepositoryError::Database` if the query fails.
pub async fn list(
    pool: &PgPool,
    status_filter: Option<&str>,
    classification_filter: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<InboundEmailSummary>, RepositoryError> {
    let rows = sqlx::query_as!(
        SummaryRow,
        r#"
        SELECT
            id,
            from_address,
            from_name,
            subject,
            mailbox,
            classification,
            confidence,
            status,
            received_at as "received_at: DateTime<Utc>",
            (response_draft IS NOT NULL) as "has_draft!: bool",
            reviewed_by,
            error
        FROM admin.inbound_email
        WHERE ($1::text IS NULL OR status = $1)
          AND ($2::text IS NULL OR classification = $2)
        ORDER BY received_at DESC
        LIMIT $3 OFFSET $4
        "#,
        status_filter,
        classification_filter,
        limit,
        offset,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(Into::into).collect())
}

/// Get a single email by ID with all fields.
///
/// # Errors
///
/// Returns `RepositoryError::Database` if the query fails.
pub async fn get_by_id(
    pool: &PgPool,
    id: i32,
) -> Result<Option<InboundEmailDetail>, RepositoryError> {
    let row = sqlx::query_as!(
        DetailRow,
        r#"
        SELECT
            id,
            m365_message_id,
            conversation_id,
            mailbox,
            from_address,
            from_name,
            to_addresses,
            subject,
            body_preview,
            body_text,
            received_at as "received_at: DateTime<Utc>",
            classification,
            sub_category,
            confidence,
            reasoning,
            status,
            routed_to,
            klaviyo_ticket_id,
            response_draft,
            response_approved,
            response_sent_at as "response_sent_at: DateTime<Utc>",
            reviewed_by,
            reviewed_at as "reviewed_at: DateTime<Utc>",
            error,
            created_at as "created_at: DateTime<Utc>",
            updated_at as "updated_at: DateTime<Utc>"
        FROM admin.inbound_email
        WHERE id = $1
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(Into::into))
}

/// Get other emails in the same conversation thread.
///
/// # Errors
///
/// Returns `RepositoryError::Database` if the query fails.
pub async fn get_thread(
    pool: &PgPool,
    conversation_id: &str,
    exclude_id: i32,
) -> Result<Vec<ThreadMessage>, RepositoryError> {
    let rows = sqlx::query_as!(
        ThreadMessage,
        r#"
        SELECT
            id,
            from_address,
            from_name,
            subject,
            body_preview,
            received_at as "received_at: DateTime<Utc>",
            status
        FROM admin.inbound_email
        WHERE conversation_id = $1 AND id != $2
        ORDER BY received_at ASC
        "#,
        conversation_id,
        exclude_id,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Count emails by status (for tab badges).
///
/// # Errors
///
/// Returns `RepositoryError::Database` if the query fails.
pub async fn count_by_status(pool: &PgPool) -> Result<Vec<StatusCount>, RepositoryError> {
    let rows = sqlx::query_as!(
        StatusCount,
        r#"
        SELECT
            status as "status!",
            COUNT(*) as "count!"
        FROM admin.inbound_email
        GROUP BY status
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Count emails in `pending_review` status.
///
/// # Errors
///
/// Returns `RepositoryError::Database` if the query fails.
pub async fn count_pending_review(pool: &PgPool) -> Result<i64, RepositoryError> {
    let count = sqlx::query_scalar!(
        r#"
        SELECT COUNT(*) as "count!"
        FROM admin.inbound_email
        WHERE status = 'pending_review'
        "#,
    )
    .fetch_one(pool)
    .await?;

    Ok(count)
}

/// Approve a draft response: mark as approved, set reviewer, update status.
///
/// # Errors
///
/// Returns `RepositoryError::Database` if the query fails.
/// Returns `RepositoryError::NotFound` if the email does not exist.
pub async fn approve_draft(pool: &PgPool, id: i32, reviewer: &str) -> Result<(), RepositoryError> {
    let result = sqlx::query!(
        r#"
        UPDATE admin.inbound_email
        SET response_approved = TRUE,
            reviewed_by = $2,
            reviewed_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
            status = 'responded'
        WHERE id = $1
        "#,
        id,
        reviewer,
    )
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(RepositoryError::NotFound);
    }

    Ok(())
}

/// Mark the response as sent.
///
/// # Errors
///
/// Returns `RepositoryError::Database` if the query fails.
pub async fn mark_response_sent(pool: &PgPool, id: i32) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        UPDATE admin.inbound_email
        SET response_sent_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
        WHERE id = $1
        "#,
        id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Reject a draft response: mark as rejected, set reviewer, update status.
///
/// # Errors
///
/// Returns `RepositoryError::Database` if the query fails.
/// Returns `RepositoryError::NotFound` if the email does not exist.
pub async fn reject_draft(pool: &PgPool, id: i32, reviewer: &str) -> Result<(), RepositoryError> {
    let result = sqlx::query!(
        r#"
        UPDATE admin.inbound_email
        SET response_approved = FALSE,
            reviewed_by = $2,
            reviewed_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
            status = 'routed'
        WHERE id = $1
        "#,
        id,
        reviewer,
    )
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(RepositoryError::NotFound);
    }

    Ok(())
}

/// Update the draft response text.
///
/// # Errors
///
/// Returns `RepositoryError::Database` if the query fails.
/// Returns `RepositoryError::NotFound` if the email does not exist.
pub async fn update_draft(pool: &PgPool, id: i32, draft_text: &str) -> Result<(), RepositoryError> {
    let result = sqlx::query!(
        r#"
        UPDATE admin.inbound_email
        SET response_draft = $2
        WHERE id = $1
        "#,
        id,
        draft_text,
    )
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(RepositoryError::NotFound);
    }

    Ok(())
}

/// Archive an email: set status to archived and record reviewer.
///
/// # Errors
///
/// Returns `RepositoryError::Database` if the query fails.
/// Returns `RepositoryError::NotFound` if the email does not exist.
pub async fn archive_email(pool: &PgPool, id: i32, reviewer: &str) -> Result<(), RepositoryError> {
    let result = sqlx::query!(
        r#"
        UPDATE admin.inbound_email
        SET status = 'archived',
            reviewed_by = $2,
            reviewed_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
        WHERE id = $1
        "#,
        id,
        reviewer,
    )
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(RepositoryError::NotFound);
    }

    Ok(())
}
