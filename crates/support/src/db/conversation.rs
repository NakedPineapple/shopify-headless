//! CRUD operations for `storefront.support_conversation`.

use chrono::{DateTime, Utc};
use naked_pineapple_core::{SupportConversationId, SupportConversationStatus};
use sqlx::PgPool;

use crate::error::SupportError;
use crate::models::{ConversationSummary, CreateConversationParams, SupportConversation};

use super::to_time_offset;

// Internal row type for query_as!
#[derive(Debug, sqlx::FromRow)]
struct ConversationRow {
    id: i32,
    session_token: String,
    shopify_customer_id: Option<String>,
    customer_email: Option<String>,
    customer_name: Option<String>,
    status: SupportConversationStatus,
    assigned_admin_id: Option<i32>,
    escalated_at: Option<DateTime<Utc>>,
    escalation_reason: Option<String>,
    title: Option<String>,
    source: String,
    is_authenticated: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    resolved_at: Option<DateTime<Utc>>,
    last_customer_message_at: Option<DateTime<Utc>>,
    last_agent_message_at: Option<DateTime<Utc>>,
}

impl From<ConversationRow> for SupportConversation {
    fn from(row: ConversationRow) -> Self {
        Self {
            id: SupportConversationId::new(row.id),
            session_token: row.session_token,
            shopify_customer_id: row.shopify_customer_id,
            customer_email: row.customer_email,
            customer_name: row.customer_name,
            status: row.status,
            assigned_admin_id: row.assigned_admin_id,
            escalated_at: row.escalated_at,
            escalation_reason: row.escalation_reason,
            title: row.title,
            source: row.source,
            is_authenticated: row.is_authenticated,
            created_at: row.created_at,
            updated_at: row.updated_at,
            resolved_at: row.resolved_at,
            last_customer_message_at: row.last_customer_message_at,
            last_agent_message_at: row.last_agent_message_at,
        }
    }
}

/// Repository for support conversation operations.
pub struct ConversationRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> ConversationRepository<'a> {
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Create a new conversation.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn create(
        &self,
        params: &CreateConversationParams,
    ) -> Result<SupportConversation, SupportError> {
        let source = params.source.as_deref().unwrap_or("chat");

        #[cfg(feature = "sqlx-macros")]
        let row = sqlx::query_as!(
            ConversationRow,
            r#"
            INSERT INTO storefront.support_conversation
                (session_token, shopify_customer_id, customer_email, customer_name, is_authenticated, source)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING
                id, session_token, shopify_customer_id, customer_email, customer_name,
                status as "status: SupportConversationStatus",
                assigned_admin_id,
                escalated_at as "escalated_at: DateTime<Utc>",
                escalation_reason, title, source, is_authenticated,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>",
                resolved_at as "resolved_at: DateTime<Utc>",
                last_customer_message_at as "last_customer_message_at: DateTime<Utc>",
                last_agent_message_at as "last_agent_message_at: DateTime<Utc>"
            "#,
            params.session_token,
            params.shopify_customer_id,
            params.customer_email,
            params.customer_name,
            params.is_authenticated,
            source,
        )
        .fetch_one(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        let row = sqlx::query_as::<_, ConversationRow>(
            "
            INSERT INTO storefront.support_conversation
                (session_token, shopify_customer_id, customer_email, customer_name, is_authenticated, source)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING
                id, session_token, shopify_customer_id, customer_email, customer_name,
                status,
                assigned_admin_id,
                escalated_at,
                escalation_reason, title, source, is_authenticated,
                created_at,
                updated_at,
                resolved_at,
                last_customer_message_at,
                last_agent_message_at
            ",
        )
        .bind(&params.session_token)
        .bind(&params.shopify_customer_id)
        .bind(&params.customer_email)
        .bind(&params.customer_name)
        .bind(params.is_authenticated)
        .bind(source)
        .fetch_one(self.pool)
        .await?;

        Ok(row.into())
    }

    /// Get a conversation by ID.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn get_by_id(
        &self,
        id: SupportConversationId,
    ) -> Result<SupportConversation, SupportError> {
        #[cfg(feature = "sqlx-macros")]
        let row = sqlx::query_as!(
            ConversationRow,
            r#"
            SELECT
                id, session_token, shopify_customer_id, customer_email, customer_name,
                status as "status: SupportConversationStatus",
                assigned_admin_id,
                escalated_at as "escalated_at: DateTime<Utc>",
                escalation_reason, title, source, is_authenticated,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>",
                resolved_at as "resolved_at: DateTime<Utc>",
                last_customer_message_at as "last_customer_message_at: DateTime<Utc>",
                last_agent_message_at as "last_agent_message_at: DateTime<Utc>"
            FROM storefront.support_conversation
            WHERE id = $1
            "#,
            id.as_i32(),
        )
        .fetch_optional(self.pool)
        .await?
        .ok_or(SupportError::ConversationNotFound)?;

        #[cfg(not(feature = "sqlx-macros"))]
        let row = sqlx::query_as::<_, ConversationRow>(
            "
            SELECT
                id, session_token, shopify_customer_id, customer_email, customer_name,
                status,
                assigned_admin_id,
                escalated_at,
                escalation_reason, title, source, is_authenticated,
                created_at,
                updated_at,
                resolved_at,
                last_customer_message_at,
                last_agent_message_at
            FROM storefront.support_conversation
            WHERE id = $1
            ",
        )
        .bind(id.as_i32())
        .fetch_optional(self.pool)
        .await?
        .ok_or(SupportError::ConversationNotFound)?;

        Ok(row.into())
    }

    /// Find an active conversation for a session token.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn find_active_by_session(
        &self,
        session_token: &str,
    ) -> Result<Option<SupportConversation>, SupportError> {
        #[cfg(feature = "sqlx-macros")]
        let row = sqlx::query_as!(
            ConversationRow,
            r#"
            SELECT
                id, session_token, shopify_customer_id, customer_email, customer_name,
                status as "status: SupportConversationStatus",
                assigned_admin_id,
                escalated_at as "escalated_at: DateTime<Utc>",
                escalation_reason, title, source, is_authenticated,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>",
                resolved_at as "resolved_at: DateTime<Utc>",
                last_customer_message_at as "last_customer_message_at: DateTime<Utc>",
                last_agent_message_at as "last_agent_message_at: DateTime<Utc>"
            FROM storefront.support_conversation
            WHERE session_token = $1 AND status IN ('active', 'escalated', 'waiting')
            ORDER BY updated_at DESC
            LIMIT 1
            "#,
            session_token,
        )
        .fetch_optional(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        let row = sqlx::query_as::<_, ConversationRow>(
            "
            SELECT
                id, session_token, shopify_customer_id, customer_email, customer_name,
                status,
                assigned_admin_id,
                escalated_at,
                escalation_reason, title, source, is_authenticated,
                created_at,
                updated_at,
                resolved_at,
                last_customer_message_at,
                last_agent_message_at
            FROM storefront.support_conversation
            WHERE session_token = $1 AND status IN ('active', 'escalated', 'waiting')
            ORDER BY updated_at DESC
            LIMIT 1
            ",
        )
        .bind(session_token)
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    /// Update conversation status.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn update_status(
        &self,
        id: SupportConversationId,
        status: SupportConversationStatus,
    ) -> Result<(), SupportError> {
        #[cfg(feature = "sqlx-macros")]
        sqlx::query!(
            r#"
            UPDATE storefront.support_conversation
            SET status = $2
            WHERE id = $1
            "#,
            id.as_i32(),
            status as SupportConversationStatus,
        )
        .execute(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        sqlx::query(
            "
            UPDATE storefront.support_conversation
            SET status = $2
            WHERE id = $1
            ",
        )
        .bind(id.as_i32())
        .bind(status)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Mark a conversation as escalated.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn escalate(
        &self,
        id: SupportConversationId,
        reason: &str,
    ) -> Result<(), SupportError> {
        let now = to_time_offset(Utc::now());

        #[cfg(feature = "sqlx-macros")]
        sqlx::query!(
            r#"
            UPDATE storefront.support_conversation
            SET status = 'escalated', escalated_at = $2, escalation_reason = $3
            WHERE id = $1
            "#,
            id.as_i32(),
            now,
            reason,
        )
        .execute(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        sqlx::query(
            "
            UPDATE storefront.support_conversation
            SET status = 'escalated', escalated_at = $2, escalation_reason = $3
            WHERE id = $1
            ",
        )
        .bind(id.as_i32())
        .bind(now)
        .bind(reason)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Update the last customer message timestamp.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn touch_customer_message(
        &self,
        id: SupportConversationId,
    ) -> Result<(), SupportError> {
        let now = to_time_offset(Utc::now());

        #[cfg(feature = "sqlx-macros")]
        sqlx::query!(
            r#"
            UPDATE storefront.support_conversation
            SET last_customer_message_at = $2
            WHERE id = $1
            "#,
            id.as_i32(),
            now,
        )
        .execute(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        sqlx::query(
            "
            UPDATE storefront.support_conversation
            SET last_customer_message_at = $2
            WHERE id = $1
            ",
        )
        .bind(id.as_i32())
        .bind(now)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Update the last agent message timestamp.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn touch_agent_message(&self, id: SupportConversationId) -> Result<(), SupportError> {
        let now = to_time_offset(Utc::now());

        #[cfg(feature = "sqlx-macros")]
        sqlx::query!(
            r#"
            UPDATE storefront.support_conversation
            SET last_agent_message_at = $2
            WHERE id = $1
            "#,
            id.as_i32(),
            now,
        )
        .execute(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        sqlx::query(
            "
            UPDATE storefront.support_conversation
            SET last_agent_message_at = $2
            WHERE id = $1
            ",
        )
        .bind(id.as_i32())
        .bind(now)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Assign a conversation to an admin user.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn assign(
        &self,
        id: SupportConversationId,
        admin_user_id: i32,
    ) -> Result<(), SupportError> {
        #[cfg(feature = "sqlx-macros")]
        sqlx::query!(
            r#"
            UPDATE storefront.support_conversation
            SET assigned_admin_id = $2
            WHERE id = $1
            "#,
            id.as_i32(),
            admin_user_id,
        )
        .execute(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        sqlx::query(
            "
            UPDATE storefront.support_conversation
            SET assigned_admin_id = $2
            WHERE id = $1
            ",
        )
        .bind(id.as_i32())
        .bind(admin_user_id)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// Resolve a conversation.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn resolve(&self, id: SupportConversationId) -> Result<(), SupportError> {
        let now = to_time_offset(Utc::now());

        #[cfg(feature = "sqlx-macros")]
        sqlx::query!(
            r#"
            UPDATE storefront.support_conversation
            SET status = 'resolved', resolved_at = $2
            WHERE id = $1
            "#,
            id.as_i32(),
            now,
        )
        .execute(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        sqlx::query(
            "
            UPDATE storefront.support_conversation
            SET status = 'resolved', resolved_at = $2
            WHERE id = $1
            ",
        )
        .bind(id.as_i32())
        .bind(now)
        .execute(self.pool)
        .await?;

        Ok(())
    }

    /// List conversations with filters (for admin inbox).
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn list(
        &self,
        status_filter: Option<SupportConversationStatus>,
        assigned_to: Option<i32>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ConversationSummary>, SupportError> {
        #[cfg(feature = "sqlx-macros")]
        let rows = sqlx::query_as!(
            ConversationSummaryRow,
            r#"
            SELECT
                c.id, c.session_token, c.shopify_customer_id, c.customer_email,
                c.customer_name,
                c.status as "status: SupportConversationStatus",
                c.assigned_admin_id,
                c.escalated_at as "escalated_at: DateTime<Utc>",
                c.escalation_reason, c.title, c.source, c.is_authenticated,
                c.created_at as "created_at: DateTime<Utc>",
                c.updated_at as "updated_at: DateTime<Utc>",
                c.resolved_at as "resolved_at: DateTime<Utc>",
                c.last_customer_message_at as "last_customer_message_at: DateTime<Utc>",
                c.last_agent_message_at as "last_agent_message_at: DateTime<Utc>",
                (SELECT count(*) FROM storefront.support_message m WHERE m.support_conversation_id = c.id) as "message_count!",
                (SELECT content->>'text'
                 FROM storefront.support_message m
                 WHERE m.support_conversation_id = c.id
                 ORDER BY m.created_at DESC LIMIT 1) as last_message_preview
            FROM storefront.support_conversation c
            WHERE ($1::storefront.support_conversation_status IS NULL OR c.status = $1)
              AND ($2::integer IS NULL OR c.assigned_admin_id = $2)
            ORDER BY c.updated_at DESC
            LIMIT $3 OFFSET $4
            "#,
            status_filter as Option<SupportConversationStatus>,
            assigned_to,
            limit,
            offset,
        )
        .fetch_all(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        let rows = sqlx::query_as::<_, ConversationSummaryRow>(
            "
            SELECT
                c.id, c.session_token, c.shopify_customer_id, c.customer_email,
                c.customer_name,
                c.status,
                c.assigned_admin_id,
                c.escalated_at,
                c.escalation_reason, c.title, c.source, c.is_authenticated,
                c.created_at,
                c.updated_at,
                c.resolved_at,
                c.last_customer_message_at,
                c.last_agent_message_at,
                (SELECT count(*) FROM storefront.support_message m WHERE m.support_conversation_id = c.id) as message_count,
                (SELECT content->>'text'
                 FROM storefront.support_message m
                 WHERE m.support_conversation_id = c.id
                 ORDER BY m.created_at DESC LIMIT 1) as last_message_preview
            FROM storefront.support_conversation c
            WHERE ($1::storefront.support_conversation_status IS NULL OR c.status = $1)
              AND ($2::integer IS NULL OR c.assigned_admin_id = $2)
            ORDER BY c.updated_at DESC
            LIMIT $3 OFFSET $4
            ",
        )
        .bind(status_filter)
        .bind(assigned_to)
        .bind(limit)
        .bind(offset)
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Count conversations by status (for inbox badges).
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn count_by_status(&self) -> Result<Vec<StatusCount>, SupportError> {
        #[cfg(feature = "sqlx-macros")]
        let rows = sqlx::query_as!(
            StatusCount,
            r#"
            SELECT
                status as "status: SupportConversationStatus",
                count(*) as "count!"
            FROM storefront.support_conversation
            WHERE status NOT IN ('closed')
            GROUP BY status
            "#,
        )
        .fetch_all(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        let rows = sqlx::query_as::<_, StatusCount>(
            r#"
            SELECT
                status,
                count(*) as "count"
            FROM storefront.support_conversation
            WHERE status NOT IN ('closed')
            GROUP BY status
            "#,
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }

    /// List conversations for a specific customer.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn list_by_customer(
        &self,
        shopify_customer_id: &str,
        limit: i64,
    ) -> Result<Vec<SupportConversation>, SupportError> {
        #[cfg(feature = "sqlx-macros")]
        let rows = sqlx::query_as!(
            ConversationRow,
            r#"
            SELECT
                id, session_token, shopify_customer_id, customer_email, customer_name,
                status as "status: SupportConversationStatus",
                assigned_admin_id,
                escalated_at as "escalated_at: DateTime<Utc>",
                escalation_reason, title, source, is_authenticated,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>",
                resolved_at as "resolved_at: DateTime<Utc>",
                last_customer_message_at as "last_customer_message_at: DateTime<Utc>",
                last_agent_message_at as "last_agent_message_at: DateTime<Utc>"
            FROM storefront.support_conversation
            WHERE shopify_customer_id = $1
            ORDER BY updated_at DESC
            LIMIT $2
            "#,
            shopify_customer_id,
            limit,
        )
        .fetch_all(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        let rows = sqlx::query_as::<_, ConversationRow>(
            "
            SELECT
                id, session_token, shopify_customer_id, customer_email, customer_name,
                status,
                assigned_admin_id,
                escalated_at,
                escalation_reason, title, source, is_authenticated,
                created_at,
                updated_at,
                resolved_at,
                last_customer_message_at,
                last_agent_message_at
            FROM storefront.support_conversation
            WHERE shopify_customer_id = $1
            ORDER BY updated_at DESC
            LIMIT $2
            ",
        )
        .bind(shopify_customer_id)
        .bind(limit)
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }
}

/// Status count for inbox badges.
#[derive(Debug, sqlx::FromRow)]
pub struct StatusCount {
    pub status: SupportConversationStatus,
    pub count: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct ConversationSummaryRow {
    id: i32,
    session_token: String,
    shopify_customer_id: Option<String>,
    customer_email: Option<String>,
    customer_name: Option<String>,
    status: SupportConversationStatus,
    assigned_admin_id: Option<i32>,
    escalated_at: Option<DateTime<Utc>>,
    escalation_reason: Option<String>,
    title: Option<String>,
    source: String,
    is_authenticated: bool,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    resolved_at: Option<DateTime<Utc>>,
    last_customer_message_at: Option<DateTime<Utc>>,
    last_agent_message_at: Option<DateTime<Utc>>,
    message_count: i64,
    last_message_preview: Option<String>,
}

impl From<ConversationSummaryRow> for ConversationSummary {
    fn from(row: ConversationSummaryRow) -> Self {
        Self {
            conversation: SupportConversation {
                id: SupportConversationId::new(row.id),
                session_token: row.session_token,
                shopify_customer_id: row.shopify_customer_id,
                customer_email: row.customer_email,
                customer_name: row.customer_name,
                status: row.status,
                assigned_admin_id: row.assigned_admin_id,
                escalated_at: row.escalated_at,
                escalation_reason: row.escalation_reason,
                title: row.title,
                source: row.source,
                is_authenticated: row.is_authenticated,
                created_at: row.created_at,
                updated_at: row.updated_at,
                resolved_at: row.resolved_at,
                last_customer_message_at: row.last_customer_message_at,
                last_agent_message_at: row.last_agent_message_at,
            },
            last_message_preview: row.last_message_preview,
            message_count: row.message_count,
        }
    }
}
