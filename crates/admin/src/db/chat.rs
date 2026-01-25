//! Database operations for chat sessions and messages.
//!
//! All queries use sqlx macros for compile-time verification.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use naked_pineapple_core::{AdminUserId, ChatMessageId, ChatRole, ChatSessionId};

use super::RepositoryError;
use crate::models::chat::{ChatMessage, ChatSession};

// =============================================================================
// Internal Row Types
// =============================================================================

/// Internal row type for `PostgreSQL` chat session queries.
#[derive(Debug, sqlx::FromRow)]
struct ChatSessionRow {
    id: i32,
    admin_user_id: i32,
    title: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<ChatSessionRow> for ChatSession {
    type Error = RepositoryError;

    fn try_from(row: ChatSessionRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: ChatSessionId::new(row.id),
            admin_user_id: AdminUserId::new(row.admin_user_id),
            title: row.title,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
    }
}

/// Internal row type for `PostgreSQL` chat message queries.
#[derive(Debug, sqlx::FromRow)]
struct ChatMessageRow {
    id: i32,
    chat_session_id: i32,
    role: ChatRole,
    content: serde_json::Value,
    created_at: DateTime<Utc>,
}

impl From<ChatMessageRow> for ChatMessage {
    fn from(row: ChatMessageRow) -> Self {
        Self {
            id: ChatMessageId::new(row.id),
            chat_session_id: ChatSessionId::new(row.chat_session_id),
            role: row.role,
            content: row.content,
            created_at: row.created_at,
        }
    }
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for chat database operations.
pub struct ChatRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> ChatRepository<'a> {
    /// Create a new chat repository.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Create a new chat session.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn create_session(
        &self,
        admin_user_id: AdminUserId,
    ) -> Result<ChatSession, RepositoryError> {
        let row = sqlx::query_as!(
            ChatSessionRow,
            r#"
            INSERT INTO admin.chat_session (admin_user_id)
            VALUES ($1)
            RETURNING id, admin_user_id, title,
                      created_at as "created_at: DateTime<Utc>",
                      updated_at as "updated_at: DateTime<Utc>"
            "#,
            admin_user_id.as_i32()
        )
        .fetch_one(self.pool)
        .await?;

        row.try_into()
    }

    /// Get a chat session by ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn get_session(
        &self,
        id: ChatSessionId,
    ) -> Result<Option<ChatSession>, RepositoryError> {
        let row = sqlx::query_as!(
            ChatSessionRow,
            r#"
            SELECT id, admin_user_id, title,
                   created_at as "created_at: DateTime<Utc>",
                   updated_at as "updated_at: DateTime<Utc>"
            FROM admin.chat_session
            WHERE id = $1
            "#,
            id.as_i32()
        )
        .fetch_optional(self.pool)
        .await?;

        match row {
            Some(r) => Ok(Some(r.try_into()?)),
            None => Ok(None),
        }
    }

    /// List chat sessions for an admin user.
    ///
    /// Returns sessions ordered by last update (most recent first).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn list_sessions(
        &self,
        admin_user_id: AdminUserId,
    ) -> Result<Vec<ChatSession>, RepositoryError> {
        let rows = sqlx::query_as!(
            ChatSessionRow,
            r#"
            SELECT id, admin_user_id, title,
                   created_at as "created_at: DateTime<Utc>",
                   updated_at as "updated_at: DateTime<Utc>"
            FROM admin.chat_session
            WHERE admin_user_id = $1
            ORDER BY updated_at DESC
            "#,
            admin_user_id.as_i32()
        )
        .fetch_all(self.pool)
        .await?;

        rows.into_iter().map(TryInto::try_into).collect()
    }

    /// Update a session's title.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the session doesn't exist.
    /// Returns `RepositoryError::Database` for other database errors.
    pub async fn update_session_title(
        &self,
        id: ChatSessionId,
        title: &str,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query!(
            r#"
            UPDATE admin.chat_session
            SET title = $1
            WHERE id = $2
            "#,
            title,
            id.as_i32()
        )
        .execute(self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        Ok(())
    }

    /// Add a message to a chat session.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn add_message(
        &self,
        chat_session_id: ChatSessionId,
        role: ChatRole,
        content: serde_json::Value,
    ) -> Result<ChatMessage, RepositoryError> {
        let row = sqlx::query_as!(
            ChatMessageRow,
            r#"
            INSERT INTO admin.chat_message (chat_session_id, role, content)
            VALUES ($1, $2, $3)
            RETURNING id, chat_session_id, role as "role: ChatRole",
                      content, created_at as "created_at: DateTime<Utc>"
            "#,
            chat_session_id.as_i32(),
            role as ChatRole,
            content
        )
        .fetch_one(self.pool)
        .await?;

        Ok(row.into())
    }

    /// Get all messages for a chat session.
    ///
    /// Returns messages ordered by creation time (oldest first).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn get_messages(
        &self,
        chat_session_id: ChatSessionId,
    ) -> Result<Vec<ChatMessage>, RepositoryError> {
        let rows = sqlx::query_as!(
            ChatMessageRow,
            r#"
            SELECT id, chat_session_id, role as "role: ChatRole",
                   content, created_at as "created_at: DateTime<Utc>"
            FROM admin.chat_message
            WHERE chat_session_id = $1
            ORDER BY created_at ASC
            "#,
            chat_session_id.as_i32()
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Delete a chat session and all its messages.
    ///
    /// # Returns
    ///
    /// Returns `true` if the session was deleted, `false` if it didn't exist.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    pub async fn delete_session(&self, id: ChatSessionId) -> Result<bool, RepositoryError> {
        let result = sqlx::query!(
            r#"
            DELETE FROM admin.chat_session
            WHERE id = $1
            "#,
            id.as_i32()
        )
        .execute(self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }
}
