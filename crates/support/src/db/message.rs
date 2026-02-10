//! CRUD operations for `storefront.support_message`.

use chrono::{DateTime, Utc};
use naked_pineapple_core::{SupportConversationId, SupportMessageId, SupportMessageRole};
use sqlx::PgPool;

use crate::error::SupportError;
use crate::models::{CreateMessageParams, SupportMessage};

#[derive(Debug, sqlx::FromRow)]
struct MessageRow {
    id: i32,
    support_conversation_id: i32,
    role: SupportMessageRole,
    content: serde_json::Value,
    api_interaction: Option<serde_json::Value>,
    admin_user_id: Option<i32>,
    created_at: DateTime<Utc>,
}

impl From<MessageRow> for SupportMessage {
    fn from(row: MessageRow) -> Self {
        Self {
            id: SupportMessageId::new(row.id),
            support_conversation_id: SupportConversationId::new(row.support_conversation_id),
            role: row.role,
            content: row.content,
            api_interaction: row.api_interaction,
            admin_user_id: row.admin_user_id,
            created_at: row.created_at,
        }
    }
}

/// Repository for support message operations.
pub struct MessageRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> MessageRepository<'a> {
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Create a new message.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn create(
        &self,
        params: &CreateMessageParams,
    ) -> Result<SupportMessage, SupportError> {
        #[cfg(feature = "sqlx-macros")]
        let row = sqlx::query_as!(
            MessageRow,
            r#"
            INSERT INTO storefront.support_message
                (support_conversation_id, role, content, api_interaction, admin_user_id)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING
                id, support_conversation_id,
                role as "role: SupportMessageRole",
                content, api_interaction, admin_user_id,
                created_at as "created_at: DateTime<Utc>"
            "#,
            params.support_conversation_id.as_i32(),
            params.role as SupportMessageRole,
            params.content,
            params.api_interaction,
            params.admin_user_id,
        )
        .fetch_one(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        let row = sqlx::query_as::<_, MessageRow>(
            "
            INSERT INTO storefront.support_message
                (support_conversation_id, role, content, api_interaction, admin_user_id)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING
                id, support_conversation_id,
                role, content, api_interaction, admin_user_id,
                created_at
            ",
        )
        .bind(params.support_conversation_id.as_i32())
        .bind(params.role as SupportMessageRole)
        .bind(&params.content)
        .bind(&params.api_interaction)
        .bind(params.admin_user_id)
        .fetch_one(self.pool)
        .await?;

        Ok(row.into())
    }

    /// Get all messages for a conversation, ordered by creation time.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn list_by_conversation(
        &self,
        conversation_id: SupportConversationId,
    ) -> Result<Vec<SupportMessage>, SupportError> {
        #[cfg(feature = "sqlx-macros")]
        let rows = sqlx::query_as!(
            MessageRow,
            r#"
            SELECT
                id, support_conversation_id,
                role as "role: SupportMessageRole",
                content, api_interaction, admin_user_id,
                created_at as "created_at: DateTime<Utc>"
            FROM storefront.support_message
            WHERE support_conversation_id = $1
            ORDER BY created_at ASC
            "#,
            conversation_id.as_i32(),
        )
        .fetch_all(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        let rows = sqlx::query_as::<_, MessageRow>(
            "
            SELECT
                id, support_conversation_id,
                role, content, api_interaction, admin_user_id,
                created_at
            FROM storefront.support_message
            WHERE support_conversation_id = $1
            ORDER BY created_at ASC
            ",
        )
        .bind(conversation_id.as_i32())
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Count messages in a conversation.
    ///
    /// # Errors
    ///
    /// Returns `SupportError` if the database query fails.
    pub async fn count_by_conversation(
        &self,
        conversation_id: SupportConversationId,
    ) -> Result<i64, SupportError> {
        #[cfg(feature = "sqlx-macros")]
        let count = sqlx::query_scalar!(
            r#"
            SELECT count(*) as "count!"
            FROM storefront.support_message
            WHERE support_conversation_id = $1
            "#,
            conversation_id.as_i32(),
        )
        .fetch_one(self.pool)
        .await?;

        #[cfg(not(feature = "sqlx-macros"))]
        let count: i64 = sqlx::query_scalar(
            r#"
            SELECT count(*) as "count"
            FROM storefront.support_message
            WHERE support_conversation_id = $1
            "#,
        )
        .bind(conversation_id.as_i32())
        .fetch_one(self.pool)
        .await?;

        Ok(count)
    }
}
