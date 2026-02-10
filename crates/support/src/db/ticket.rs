//! CRUD operations for `storefront.support_ticket`.

use chrono::{DateTime, Utc};
use naked_pineapple_core::{SupportConversationId, SupportTicketId};
use sqlx::PgPool;

use crate::error::SupportError;
use crate::models::{CreateTicketParams, SupportTicket};

use super::to_time_offset;

#[derive(Debug, sqlx::FromRow)]
struct TicketRow {
    id: i32,
    support_conversation_id: i32,
    category: Option<String>,
    priority: String,
    status: String,
    assigned_admin_id: Option<i32>,
    resolution_notes: Option<String>,
    slack_message_ts: Option<String>,
    slack_channel_id: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    resolved_at: Option<DateTime<Utc>>,
}

impl From<TicketRow> for SupportTicket {
    fn from(row: TicketRow) -> Self {
        Self {
            id: SupportTicketId::new(row.id),
            support_conversation_id: SupportConversationId::new(row.support_conversation_id),
            category: row.category,
            priority: row.priority,
            status: row.status,
            assigned_admin_id: row.assigned_admin_id,
            resolution_notes: row.resolution_notes,
            slack_message_ts: row.slack_message_ts,
            slack_channel_id: row.slack_channel_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
            resolved_at: row.resolved_at,
        }
    }
}

/// Repository for support ticket operations.
pub struct TicketRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> TicketRepository<'a> {
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Create a new ticket.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn create(&self, params: &CreateTicketParams) -> Result<SupportTicket, SupportError> {
        #[cfg(feature = "sqlx-macros")]
        let row = sqlx::query_as!(
            TicketRow,
            r#"
            INSERT INTO storefront.support_ticket
                (support_conversation_id, category, priority)
            VALUES ($1, $2, $3)
            RETURNING
                id, support_conversation_id, category, priority, status,
                assigned_admin_id, resolution_notes, slack_message_ts,
                slack_channel_id,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>",
                resolved_at as "resolved_at: DateTime<Utc>"
            "#,
            params.support_conversation_id.as_i32(),
            params.category,
            params.priority,
        )
        .fetch_one(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        let row = sqlx::query_as::<_, TicketRow>(
            r#"
            INSERT INTO storefront.support_ticket
                (support_conversation_id, category, priority)
            VALUES ($1, $2, $3)
            RETURNING
                id, support_conversation_id, category, priority, status,
                assigned_admin_id, resolution_notes, slack_message_ts,
                slack_channel_id,
                created_at, updated_at, resolved_at
            "#,
        )
        .bind(params.support_conversation_id.as_i32())
        .bind(&params.category)
        .bind(&params.priority)
        .fetch_one(self.pool)
        .await?;

        Ok(row.into())
    }

    /// Get a ticket by ID.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn get_by_id(&self, id: SupportTicketId) -> Result<SupportTicket, SupportError> {
        #[cfg(feature = "sqlx-macros")]
        let row = sqlx::query_as!(
            TicketRow,
            r#"
            SELECT
                id, support_conversation_id, category, priority, status,
                assigned_admin_id, resolution_notes, slack_message_ts,
                slack_channel_id,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>",
                resolved_at as "resolved_at: DateTime<Utc>"
            FROM storefront.support_ticket
            WHERE id = $1
            "#,
            id.as_i32(),
        )
        .fetch_optional(self.pool)
        .await?
        .ok_or(SupportError::ConversationNotFound)?;

        #[cfg(not(feature = "sqlx-macros"))]
        let row = sqlx::query_as::<_, TicketRow>(
            r#"
            SELECT
                id, support_conversation_id, category, priority, status,
                assigned_admin_id, resolution_notes, slack_message_ts,
                slack_channel_id,
                created_at, updated_at, resolved_at
            FROM storefront.support_ticket
            WHERE id = $1
            "#,
        )
        .bind(id.as_i32())
        .fetch_optional(self.pool)
        .await?
        .ok_or(SupportError::ConversationNotFound)?;

        Ok(row.into())
    }

    /// Get ticket by conversation ID.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn get_by_conversation(
        &self,
        conversation_id: SupportConversationId,
    ) -> Result<Option<SupportTicket>, SupportError> {
        #[cfg(feature = "sqlx-macros")]
        let row = sqlx::query_as!(
            TicketRow,
            r#"
            SELECT
                id, support_conversation_id, category, priority, status,
                assigned_admin_id, resolution_notes, slack_message_ts,
                slack_channel_id,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>",
                resolved_at as "resolved_at: DateTime<Utc>"
            FROM storefront.support_ticket
            WHERE support_conversation_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
            conversation_id.as_i32(),
        )
        .fetch_optional(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        let row = sqlx::query_as::<_, TicketRow>(
            r#"
            SELECT
                id, support_conversation_id, category, priority, status,
                assigned_admin_id, resolution_notes, slack_message_ts,
                slack_channel_id,
                created_at, updated_at, resolved_at
            FROM storefront.support_ticket
            WHERE support_conversation_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(conversation_id.as_i32())
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    /// Update ticket status and priority.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn update_status(
        &self,
        id: SupportTicketId,
        status: &str,
        priority: &str,
    ) -> Result<(), SupportError> {
        #[cfg(feature = "sqlx-macros")]
        sqlx::query!(
            r#"
            UPDATE storefront.support_ticket
            SET status = $2, priority = $3
            WHERE id = $1
            "#,
            id.as_i32(),
            status,
            priority,
        )
        .execute(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        sqlx::query(
            r#"
            UPDATE storefront.support_ticket
            SET status = $2, priority = $3
            WHERE id = $1
            "#,
        )
        .bind(id.as_i32())
        .bind(status)
        .bind(priority)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Resolve a ticket with notes.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn resolve(
        &self,
        id: SupportTicketId,
        notes: Option<&str>,
    ) -> Result<(), SupportError> {
        let now = to_time_offset(chrono::Utc::now());

        #[cfg(feature = "sqlx-macros")]
        sqlx::query!(
            r#"
            UPDATE storefront.support_ticket
            SET status = 'resolved', resolution_notes = $2, resolved_at = $3
            WHERE id = $1
            "#,
            id.as_i32(),
            notes,
            now,
        )
        .execute(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        sqlx::query(
            r#"
            UPDATE storefront.support_ticket
            SET status = 'resolved', resolution_notes = $2, resolved_at = $3
            WHERE id = $1
            "#,
        )
        .bind(id.as_i32())
        .bind(notes)
        .bind(now)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Update Slack message tracking info.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn set_slack_info(
        &self,
        id: SupportTicketId,
        message_ts: &str,
        channel_id: &str,
    ) -> Result<(), SupportError> {
        #[cfg(feature = "sqlx-macros")]
        sqlx::query!(
            r#"
            UPDATE storefront.support_ticket
            SET slack_message_ts = $2, slack_channel_id = $3
            WHERE id = $1
            "#,
            id.as_i32(),
            message_ts,
            channel_id,
        )
        .execute(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        sqlx::query(
            r#"
            UPDATE storefront.support_ticket
            SET slack_message_ts = $2, slack_channel_id = $3
            WHERE id = $1
            "#,
        )
        .bind(id.as_i32())
        .bind(message_ts)
        .bind(channel_id)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// List tickets with optional status filter.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn list(
        &self,
        status_filter: Option<&str>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<SupportTicket>, SupportError> {
        #[cfg(feature = "sqlx-macros")]
        let rows = sqlx::query_as!(
            TicketRow,
            r#"
            SELECT
                id, support_conversation_id, category, priority, status,
                assigned_admin_id, resolution_notes, slack_message_ts,
                slack_channel_id,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>",
                resolved_at as "resolved_at: DateTime<Utc>"
            FROM storefront.support_ticket
            WHERE ($1::text IS NULL OR status = $1)
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
            status_filter,
            limit,
            offset,
        )
        .fetch_all(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        let rows = sqlx::query_as::<_, TicketRow>(
            r#"
            SELECT
                id, support_conversation_id, category, priority, status,
                assigned_admin_id, resolution_notes, slack_message_ts,
                slack_channel_id,
                created_at, updated_at, resolved_at
            FROM storefront.support_ticket
            WHERE ($1::text IS NULL OR status = $1)
            ORDER BY created_at DESC
            LIMIT $2 OFFSET $3
            "#,
        )
        .bind(status_filter)
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}
