//! Database operations for chat sessions and messages.
//!
//! All queries use sqlx macros for compile-time verification.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::{debug, info, instrument, warn};

use naked_pineapple_core::{AdminUserId, ChatMessageId, ChatRole, ChatSessionId};

use super::RepositoryError;
use crate::models::chat::{ApiInteraction, ChatMessage, ChatSession};

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
    api_interaction: Option<serde_json::Value>,
    created_at: DateTime<Utc>,
}

impl From<ChatMessageRow> for ChatMessage {
    fn from(row: ChatMessageRow) -> Self {
        let api_interaction = row
            .api_interaction
            .and_then(|v| serde_json::from_value::<ApiInteraction>(v).ok());

        Self {
            id: ChatMessageId::new(row.id),
            chat_session_id: ChatSessionId::new(row.chat_session_id),
            role: row.role,
            content: row.content,
            api_interaction,
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
    #[instrument(skip(self), fields(admin_user_id = %admin_user_id.as_i32()), level = "debug")]
    pub async fn create_session(
        &self,
        admin_user_id: AdminUserId,
    ) -> Result<ChatSession, RepositoryError> {
        debug!("Creating chat session");
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

        let session: ChatSession = row.try_into()?;
        info!(session_id = %session.id.as_i32(), "Chat session created");
        Ok(session)
    }

    /// Get a chat session by ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(session_id = %id.as_i32()), level = "debug")]
    pub async fn get_session(
        &self,
        id: ChatSessionId,
    ) -> Result<Option<ChatSession>, RepositoryError> {
        debug!("Fetching chat session");
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

        if row.is_none() {
            debug!("Chat session not found");
        }
        row.map(TryInto::try_into).transpose()
    }

    /// List chat sessions for an admin user.
    ///
    /// Returns sessions ordered by last update (most recent first).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(admin_user_id = %admin_user_id.as_i32()), level = "debug")]
    pub async fn list_sessions(
        &self,
        admin_user_id: AdminUserId,
    ) -> Result<Vec<ChatSession>, RepositoryError> {
        debug!("Listing chat sessions for admin user");
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

        let sessions: Vec<ChatSession> = rows
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;
        debug!(count = sessions.len(), "Retrieved chat sessions");
        Ok(sessions)
    }

    /// Update a session's title.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the session doesn't exist.
    /// Returns `RepositoryError::Database` for other database errors.
    #[instrument(skip(self), fields(session_id = %id.as_i32()), level = "debug")]
    pub async fn update_session_title(
        &self,
        id: ChatSessionId,
        title: &str,
    ) -> Result<(), RepositoryError> {
        debug!("Updating chat session title");
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
            warn!("Chat session not found for title update");
            return Err(RepositoryError::NotFound);
        }

        info!("Chat session title updated");
        Ok(())
    }

    /// Add a message to a chat session.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, content), fields(session_id = %chat_session_id.as_i32(), role = ?role), level = "debug")]
    pub async fn add_message(
        &self,
        chat_session_id: ChatSessionId,
        role: ChatRole,
        content: serde_json::Value,
    ) -> Result<ChatMessage, RepositoryError> {
        debug!("Adding message to chat session");
        self.add_message_with_interaction(chat_session_id, role, content, None)
            .await
    }

    /// Add a message to a chat session with API interaction metadata.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, content, api_interaction), fields(session_id = %chat_session_id.as_i32(), role = ?role, has_interaction = api_interaction.is_some()), level = "debug")]
    pub async fn add_message_with_interaction(
        &self,
        chat_session_id: ChatSessionId,
        role: ChatRole,
        content: serde_json::Value,
        api_interaction: Option<&ApiInteraction>,
    ) -> Result<ChatMessage, RepositoryError> {
        debug!("Adding message with interaction to chat session");
        let interaction_json = api_interaction
            .map(serde_json::to_value)
            .transpose()
            .map_err(|e| RepositoryError::Serialization(e.to_string()))?;

        let row = sqlx::query_as!(
            ChatMessageRow,
            r#"
            INSERT INTO admin.chat_message (chat_session_id, role, content, api_interaction)
            VALUES ($1, $2, $3, $4)
            RETURNING id, chat_session_id, role as "role: ChatRole",
                      content, api_interaction, created_at as "created_at: DateTime<Utc>"
            "#,
            chat_session_id.as_i32(),
            role as ChatRole,
            content,
            interaction_json
        )
        .fetch_one(self.pool)
        .await?;

        let message: ChatMessage = row.into();
        info!(message_id = %message.id.as_i32(), "Chat message added");
        Ok(message)
    }

    /// Get all messages for a chat session.
    ///
    /// Returns messages ordered by creation time (oldest first).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(session_id = %chat_session_id.as_i32()), level = "debug")]
    pub async fn get_messages(
        &self,
        chat_session_id: ChatSessionId,
    ) -> Result<Vec<ChatMessage>, RepositoryError> {
        debug!("Fetching messages for chat session");
        let rows = sqlx::query_as!(
            ChatMessageRow,
            r#"
            SELECT id, chat_session_id, role as "role: ChatRole",
                   content, api_interaction, created_at as "created_at: DateTime<Utc>"
            FROM admin.chat_message
            WHERE chat_session_id = $1
            ORDER BY created_at ASC
            "#,
            chat_session_id.as_i32()
        )
        .fetch_all(self.pool)
        .await?;

        let messages: Vec<ChatMessage> = rows.into_iter().map(Into::into).collect();
        debug!(count = messages.len(), "Retrieved chat messages");
        Ok(messages)
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
    #[instrument(skip(self), fields(session_id = %id.as_i32()), level = "debug")]
    pub async fn delete_session(&self, id: ChatSessionId) -> Result<bool, RepositoryError> {
        debug!("Deleting chat session");
        let result = sqlx::query!(
            r#"
            DELETE FROM admin.chat_session
            WHERE id = $1
            "#,
            id.as_i32()
        )
        .execute(self.pool)
        .await?;

        let deleted = result.rows_affected() > 0;
        if deleted {
            info!("Chat session deleted");
        } else {
            debug!("Chat session not found for deletion");
        }
        Ok(deleted)
    }

    /// List all chat sessions (for super admin).
    ///
    /// Returns sessions ordered by last update (most recent first).
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn list_all_sessions(&self) -> Result<Vec<ChatSession>, RepositoryError> {
        debug!("Listing all chat sessions");
        let rows = sqlx::query_as!(
            ChatSessionRow,
            r#"
            SELECT id, admin_user_id, title,
                   created_at as "created_at: DateTime<Utc>",
                   updated_at as "updated_at: DateTime<Utc>"
            FROM admin.chat_session
            ORDER BY updated_at DESC
            "#
        )
        .fetch_all(self.pool)
        .await?;

        let sessions: Vec<ChatSession> = rows
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;
        debug!(count = sessions.len(), "Retrieved all chat sessions");
        Ok(sessions)
    }

    /// List chat sessions with pagination.
    ///
    /// Returns sessions ordered by last update (most recent first).
    ///
    /// # Arguments
    ///
    /// * `admin_user_id` - If provided, filter to this admin's sessions only
    /// * `limit` - Maximum number of sessions to return
    /// * `offset` - Number of sessions to skip
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(admin_user_id = ?admin_user_id.map(|id| id.as_i32()), limit, offset), level = "debug")]
    pub async fn list_sessions_paginated(
        &self,
        admin_user_id: Option<AdminUserId>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<ChatSession>, RepositoryError> {
        debug!("Listing chat sessions with pagination");
        let rows = match admin_user_id {
            Some(uid) => {
                sqlx::query_as!(
                    ChatSessionRow,
                    r#"
                    SELECT id, admin_user_id, title,
                           created_at as "created_at: DateTime<Utc>",
                           updated_at as "updated_at: DateTime<Utc>"
                    FROM admin.chat_session
                    WHERE admin_user_id = $1
                    ORDER BY updated_at DESC
                    LIMIT $2 OFFSET $3
                    "#,
                    uid.as_i32(),
                    limit,
                    offset
                )
                .fetch_all(self.pool)
                .await?
            }
            None => {
                sqlx::query_as!(
                    ChatSessionRow,
                    r#"
                    SELECT id, admin_user_id, title,
                           created_at as "created_at: DateTime<Utc>",
                           updated_at as "updated_at: DateTime<Utc>"
                    FROM admin.chat_session
                    ORDER BY updated_at DESC
                    LIMIT $1 OFFSET $2
                    "#,
                    limit,
                    offset
                )
                .fetch_all(self.pool)
                .await?
            }
        };

        let sessions: Vec<ChatSession> = rows
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<_, _>>()?;
        debug!(count = sessions.len(), "Retrieved paginated chat sessions");
        Ok(sessions)
    }

    /// Count total chat sessions.
    ///
    /// # Arguments
    ///
    /// * `admin_user_id` - If provided, count only this admin's sessions
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(admin_user_id = ?admin_user_id.map(|id| id.as_i32())), level = "debug")]
    pub async fn count_sessions(
        &self,
        admin_user_id: Option<AdminUserId>,
    ) -> Result<i64, RepositoryError> {
        debug!("Counting chat sessions");
        let count = match admin_user_id {
            Some(uid) => {
                sqlx::query_scalar!(
                    r#"
                    SELECT COUNT(*) as "count!"
                    FROM admin.chat_session
                    WHERE admin_user_id = $1
                    "#,
                    uid.as_i32()
                )
                .fetch_one(self.pool)
                .await?
            }
            None => {
                sqlx::query_scalar!(
                    r#"
                    SELECT COUNT(*) as "count!"
                    FROM admin.chat_session
                    "#
                )
                .fetch_one(self.pool)
                .await?
            }
        };

        debug!(count, "Chat session count retrieved");
        Ok(count)
    }

    /// Check if an admin owns a session.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), fields(session_id = %session_id.as_i32(), admin_user_id = %admin_user_id.as_i32()), level = "debug")]
    pub async fn session_belongs_to_admin(
        &self,
        session_id: ChatSessionId,
        admin_user_id: AdminUserId,
    ) -> Result<bool, RepositoryError> {
        debug!("Checking session ownership");
        let exists = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM admin.chat_session
                WHERE id = $1 AND admin_user_id = $2
            ) as "exists!"
            "#,
            session_id.as_i32(),
            admin_user_id.as_i32()
        )
        .fetch_one(self.pool)
        .await?;

        debug!(belongs = exists, "Session ownership check complete");
        Ok(exists)
    }
}
