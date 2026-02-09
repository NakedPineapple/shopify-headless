//! Database operations for the request board.
//!
//! All queries use sqlx macros for compile-time verification.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use tracing::{debug, info, instrument};

use naked_pineapple_core::{AdminUserId, RequestBoardCommentId, RequestBoardItemId};

use super::RepositoryError;
use crate::models::request_board::{
    CreateCommentInput, CreateRequestInput, MoveRequestInput, RequestBoardComment,
    RequestBoardCommentWithAuthor, RequestBoardItem, RequestBoardItemSummary, RequestPriority,
    RequestStatus, RequestType, UpdateRequestInput,
};

// =============================================================================
// Internal Row Types
// =============================================================================

/// Internal row type for request board item queries.
#[derive(Debug, sqlx::FromRow)]
struct ItemRow {
    id: i32,
    title: String,
    description: String,
    request_type: RequestType,
    priority: RequestPriority,
    status: RequestStatus,
    position: i32,
    created_by: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<ItemRow> for RequestBoardItem {
    fn from(row: ItemRow) -> Self {
        Self {
            id: RequestBoardItemId::new(row.id),
            title: row.title,
            description: row.description,
            request_type: row.request_type,
            priority: row.priority,
            status: row.status,
            position: row.position,
            created_by: AdminUserId::new(row.created_by),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Internal row type for the board view (item + creator name + comment count).
#[derive(Debug, sqlx::FromRow)]
struct ItemSummaryRow {
    id: i32,
    title: String,
    description: String,
    request_type: RequestType,
    priority: RequestPriority,
    status: RequestStatus,
    position: i32,
    created_by: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    creator_name: String,
    comment_count: i64,
}

impl From<ItemSummaryRow> for RequestBoardItemSummary {
    fn from(row: ItemSummaryRow) -> Self {
        Self {
            item: RequestBoardItem {
                id: RequestBoardItemId::new(row.id),
                title: row.title,
                description: row.description,
                request_type: row.request_type,
                priority: row.priority,
                status: row.status,
                position: row.position,
                created_by: AdminUserId::new(row.created_by),
                created_at: row.created_at,
                updated_at: row.updated_at,
            },
            creator_name: row.creator_name,
            comment_count: row.comment_count,
        }
    }
}

/// Internal row type for comment queries.
#[derive(Debug, sqlx::FromRow)]
struct CommentRow {
    id: i32,
    item_id: i32,
    admin_user_id: i32,
    body: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl From<CommentRow> for RequestBoardComment {
    fn from(row: CommentRow) -> Self {
        Self {
            id: RequestBoardCommentId::new(row.id),
            item_id: RequestBoardItemId::new(row.item_id),
            admin_user_id: AdminUserId::new(row.admin_user_id),
            body: row.body,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Internal row type for comments with author name.
#[derive(Debug, sqlx::FromRow)]
struct CommentWithAuthorRow {
    id: i32,
    item_id: i32,
    admin_user_id: i32,
    body: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    author_name: String,
}

impl From<CommentWithAuthorRow> for RequestBoardCommentWithAuthor {
    fn from(row: CommentWithAuthorRow) -> Self {
        Self {
            comment: RequestBoardComment {
                id: RequestBoardCommentId::new(row.id),
                item_id: RequestBoardItemId::new(row.item_id),
                admin_user_id: AdminUserId::new(row.admin_user_id),
                body: row.body,
                created_at: row.created_at,
                updated_at: row.updated_at,
            },
            author_name: row.author_name,
        }
    }
}

// =============================================================================
// Repository
// =============================================================================

/// Repository for request board database operations.
pub struct RequestBoardRepository<'a> {
    pool: &'a PgPool,
}

impl<'a> RequestBoardRepository<'a> {
    /// Create a new request board repository.
    #[must_use]
    pub const fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    // =========================================================================
    // Item Operations
    // =========================================================================

    /// List all items with creator names and comment counts, ordered for the board view.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug")]
    pub async fn list_all(&self) -> Result<Vec<RequestBoardItemSummary>, RepositoryError> {
        debug!("Listing all request board items");
        let rows = sqlx::query_as!(
            ItemSummaryRow,
            r#"
            SELECT
                i.id, i.title, i.description,
                i.request_type as "request_type: RequestType",
                i.priority as "priority: RequestPriority",
                i.status as "status: RequestStatus",
                i.position, i.created_by,
                i.created_at as "created_at: DateTime<Utc>",
                i.updated_at as "updated_at: DateTime<Utc>",
                u.name as "creator_name!",
                COALESCE(
                    (SELECT COUNT(*) FROM admin.request_board_comment c WHERE c.item_id = i.id),
                    0
                ) as "comment_count!"
            FROM admin.request_board_item i
            JOIN admin.admin_user u ON u.id = i.created_by
            ORDER BY i.status, i.position, i.created_at DESC
            "#,
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Get a single item by ID.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug", fields(item_id = %id.as_i32()))]
    pub async fn get_item(
        &self,
        id: RequestBoardItemId,
    ) -> Result<Option<RequestBoardItem>, RepositoryError> {
        debug!("Fetching request board item");
        let row = sqlx::query_as!(
            ItemRow,
            r#"
            SELECT
                id, title, description,
                request_type as "request_type: RequestType",
                priority as "priority: RequestPriority",
                status as "status: RequestStatus",
                position, created_by,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            FROM admin.request_board_item
            WHERE id = $1
            "#,
            id.as_i32()
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    /// Get a single item by ID with creator name.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug", fields(item_id = %id.as_i32()))]
    pub async fn get_item_with_creator(
        &self,
        id: RequestBoardItemId,
    ) -> Result<Option<RequestBoardItemSummary>, RepositoryError> {
        debug!("Fetching request board item with creator");
        let row = sqlx::query_as!(
            ItemSummaryRow,
            r#"
            SELECT
                i.id, i.title, i.description,
                i.request_type as "request_type: RequestType",
                i.priority as "priority: RequestPriority",
                i.status as "status: RequestStatus",
                i.position, i.created_by,
                i.created_at as "created_at: DateTime<Utc>",
                i.updated_at as "updated_at: DateTime<Utc>",
                u.name as "creator_name!",
                COALESCE(
                    (SELECT COUNT(*) FROM admin.request_board_comment c WHERE c.item_id = i.id),
                    0
                ) as "comment_count!"
            FROM admin.request_board_item i
            JOIN admin.admin_user u ON u.id = i.created_by
            WHERE i.id = $1
            "#,
            id.as_i32()
        )
        .fetch_optional(self.pool)
        .await?;

        Ok(row.map(Into::into))
    }

    /// Create a new request board item.
    ///
    /// Position is auto-assigned to the end of the "new" column.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, input), level = "debug", fields(title = %input.title))]
    pub async fn create_item(
        &self,
        input: &CreateRequestInput,
    ) -> Result<RequestBoardItem, RepositoryError> {
        debug!("Creating request board item");
        let row = sqlx::query_as!(
            ItemRow,
            r#"
            INSERT INTO admin.request_board_item (
                title, description, request_type, priority, status, position, created_by
            )
            VALUES (
                $1, $2, $3, $4, 'new',
                COALESCE(
                    (SELECT MAX(position) + 1 FROM admin.request_board_item WHERE status = 'new'),
                    0
                ),
                $5
            )
            RETURNING
                id, title, description,
                request_type as "request_type: RequestType",
                priority as "priority: RequestPriority",
                status as "status: RequestStatus",
                position, created_by,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            "#,
            input.title,
            input.description,
            input.request_type as RequestType,
            input.priority as RequestPriority,
            input.created_by.as_i32()
        )
        .fetch_one(self.pool)
        .await?;

        let item: RequestBoardItem = row.into();
        info!(item_id = %item.id.as_i32(), "Request board item created");
        Ok(item)
    }

    /// Update a request board item.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the item does not exist.
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, input), level = "debug", fields(item_id = %id.as_i32()))]
    pub async fn update_item(
        &self,
        id: RequestBoardItemId,
        input: &UpdateRequestInput,
    ) -> Result<RequestBoardItem, RepositoryError> {
        debug!("Updating request board item");
        let row = sqlx::query_as!(
            ItemRow,
            r#"
            UPDATE admin.request_board_item
            SET title = $2, description = $3, request_type = $4, priority = $5
            WHERE id = $1
            RETURNING
                id, title, description,
                request_type as "request_type: RequestType",
                priority as "priority: RequestPriority",
                status as "status: RequestStatus",
                position, created_by,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            "#,
            id.as_i32(),
            input.title,
            input.description,
            input.request_type as RequestType,
            input.priority as RequestPriority,
        )
        .fetch_optional(self.pool)
        .await?
        .ok_or(RepositoryError::NotFound)?;

        let item: RequestBoardItem = row.into();
        info!(item_id = %item.id.as_i32(), "Request board item updated");
        Ok(item)
    }

    /// Move a card to a new status and position (for drag-and-drop).
    ///
    /// Shifts existing items in the target column to make room.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the item does not exist.
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, input), level = "debug", fields(item_id = %id.as_i32()))]
    pub async fn move_item(
        &self,
        id: RequestBoardItemId,
        input: &MoveRequestInput,
    ) -> Result<RequestBoardItem, RepositoryError> {
        debug!(status = %input.status.slug(), position = %input.position, "Moving request board item");

        // Shift items at or after the target position down by 1
        sqlx::query!(
            r#"
            UPDATE admin.request_board_item
            SET position = position + 1
            WHERE status = $1 AND position >= $2 AND id != $3
            "#,
            input.status as RequestStatus,
            input.position,
            id.as_i32()
        )
        .execute(self.pool)
        .await?;

        // Move the item to the new status and position
        let row = sqlx::query_as!(
            ItemRow,
            r#"
            UPDATE admin.request_board_item
            SET status = $2, position = $3
            WHERE id = $1
            RETURNING
                id, title, description,
                request_type as "request_type: RequestType",
                priority as "priority: RequestPriority",
                status as "status: RequestStatus",
                position, created_by,
                created_at as "created_at: DateTime<Utc>",
                updated_at as "updated_at: DateTime<Utc>"
            "#,
            id.as_i32(),
            input.status as RequestStatus,
            input.position,
        )
        .fetch_optional(self.pool)
        .await?
        .ok_or(RepositoryError::NotFound)?;

        let item: RequestBoardItem = row.into();
        info!(item_id = %item.id.as_i32(), status = %item.status.slug(), "Request board item moved");
        Ok(item)
    }

    /// Delete a request board item.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::NotFound` if the item does not exist.
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug", fields(item_id = %id.as_i32()))]
    pub async fn delete_item(&self, id: RequestBoardItemId) -> Result<(), RepositoryError> {
        debug!("Deleting request board item");
        let result = sqlx::query!(
            r#"DELETE FROM admin.request_board_item WHERE id = $1"#,
            id.as_i32()
        )
        .execute(self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        info!(item_id = %id.as_i32(), "Request board item deleted");
        Ok(())
    }

    // =========================================================================
    // Comment Operations
    // =========================================================================

    /// List comments for an item with author names.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self), level = "debug", fields(item_id = %item_id.as_i32()))]
    pub async fn list_comments(
        &self,
        item_id: RequestBoardItemId,
    ) -> Result<Vec<RequestBoardCommentWithAuthor>, RepositoryError> {
        debug!("Listing comments for request board item");
        let rows = sqlx::query_as!(
            CommentWithAuthorRow,
            r#"
            SELECT
                c.id, c.item_id, c.admin_user_id, c.body,
                c.created_at as "created_at: DateTime<Utc>",
                c.updated_at as "updated_at: DateTime<Utc>",
                u.name as "author_name!"
            FROM admin.request_board_comment c
            JOIN admin.admin_user u ON u.id = c.admin_user_id
            WHERE c.item_id = $1
            ORDER BY c.created_at ASC
            "#,
            item_id.as_i32()
        )
        .fetch_all(self.pool)
        .await?;

        Ok(rows.into_iter().map(Into::into).collect())
    }

    /// Create a comment on a request board item.
    ///
    /// # Errors
    ///
    /// Returns `RepositoryError::Database` if the query fails.
    #[instrument(skip(self, input), level = "debug", fields(item_id = %input.item_id.as_i32()))]
    pub async fn create_comment(
        &self,
        input: &CreateCommentInput,
    ) -> Result<RequestBoardCommentWithAuthor, RepositoryError> {
        debug!("Creating comment on request board item");
        let row = sqlx::query_as!(
            CommentWithAuthorRow,
            r#"
            WITH new_comment AS (
                INSERT INTO admin.request_board_comment (item_id, admin_user_id, body)
                VALUES ($1, $2, $3)
                RETURNING id, item_id, admin_user_id, body, created_at, updated_at
            )
            SELECT
                nc.id, nc.item_id, nc.admin_user_id, nc.body,
                nc.created_at as "created_at: DateTime<Utc>",
                nc.updated_at as "updated_at: DateTime<Utc>",
                u.name as "author_name!"
            FROM new_comment nc
            JOIN admin.admin_user u ON u.id = nc.admin_user_id
            "#,
            input.item_id.as_i32(),
            input.admin_user_id.as_i32(),
            input.body,
        )
        .fetch_one(self.pool)
        .await?;

        let comment: RequestBoardCommentWithAuthor = row.into();
        info!(comment_id = %comment.comment.id.as_i32(), "Comment created");
        Ok(comment)
    }
}
