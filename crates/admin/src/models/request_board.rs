//! Request board domain models for bug reports and feature requests.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use naked_pineapple_core::{AdminUserId, RequestBoardCommentId, RequestBoardItemId};

/// Type of request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "request_type", rename_all = "snake_case")]
pub enum RequestType {
    /// A bug report.
    BugReport,
    /// A feature request.
    FeatureRequest,
}

impl RequestType {
    /// Display label for templates.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::BugReport => "Bug Report",
            Self::FeatureRequest => "Feature Request",
        }
    }

    /// Slug for form values and HTML attributes.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::BugReport => "bug_report",
            Self::FeatureRequest => "feature_request",
        }
    }
}

/// Priority level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "request_priority", rename_all = "snake_case")]
pub enum RequestPriority {
    /// Low priority.
    Low,
    /// Medium priority.
    Medium,
    /// High priority.
    High,
    /// Critical priority.
    Critical,
}

impl RequestPriority {
    /// Display label for templates.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Medium",
            Self::High => "High",
            Self::Critical => "Critical",
        }
    }

    /// Slug for form values and HTML attributes.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

/// Kanban column status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "request_status", rename_all = "snake_case")]
pub enum RequestStatus {
    /// Newly submitted.
    New,
    /// Being reviewed.
    UnderReview,
    /// Work in progress.
    InProgress,
    /// Completed.
    Done,
    /// Closed / won't fix.
    Closed,
}

impl RequestStatus {
    /// Display label for templates.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::New => "New",
            Self::UnderReview => "Under Review",
            Self::InProgress => "In Progress",
            Self::Done => "Done",
            Self::Closed => "Closed",
        }
    }

    /// Slug for form values and HTML attributes.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::New => "new",
            Self::UnderReview => "under_review",
            Self::InProgress => "in_progress",
            Self::Done => "done",
            Self::Closed => "closed",
        }
    }

    /// All statuses in column order.
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::New,
            Self::UnderReview,
            Self::InProgress,
            Self::Done,
            Self::Closed,
        ]
    }
}

/// A request board item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestBoardItem {
    /// Unique item ID.
    pub id: RequestBoardItemId,
    /// Request title.
    pub title: String,
    /// Request description (markdown).
    pub description: String,
    /// Type of request.
    pub request_type: RequestType,
    /// Priority level.
    pub priority: RequestPriority,
    /// Current Kanban status.
    pub status: RequestStatus,
    /// Position within the status column.
    pub position: i32,
    /// Admin user who created this request.
    pub created_by: AdminUserId,
    /// When the request was created.
    pub created_at: DateTime<Utc>,
    /// When the request was last updated.
    pub updated_at: DateTime<Utc>,
}

/// A request board item with creator name and comment count (for board view).
#[derive(Debug, Clone)]
pub struct RequestBoardItemSummary {
    /// The item data.
    pub item: RequestBoardItem,
    /// Name of the admin who created it.
    pub creator_name: String,
    /// Number of comments on this item.
    pub comment_count: i64,
}

/// A comment on a request board item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestBoardComment {
    /// Unique comment ID.
    pub id: RequestBoardCommentId,
    /// Item this comment belongs to.
    pub item_id: RequestBoardItemId,
    /// Admin user who wrote the comment.
    pub admin_user_id: AdminUserId,
    /// Comment body (markdown).
    pub body: String,
    /// When the comment was created.
    pub created_at: DateTime<Utc>,
    /// When the comment was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Comment with the author name for display.
#[derive(Debug, Clone)]
pub struct RequestBoardCommentWithAuthor {
    /// The comment data.
    pub comment: RequestBoardComment,
    /// Name of the admin who wrote it.
    pub author_name: String,
}

/// Input for creating a new request.
#[derive(Debug, Clone)]
pub struct CreateRequestInput {
    /// Request title.
    pub title: String,
    /// Request description (markdown).
    pub description: String,
    /// Type of request.
    pub request_type: RequestType,
    /// Priority level.
    pub priority: RequestPriority,
    /// Admin user creating the request.
    pub created_by: AdminUserId,
}

/// Input for updating a request.
#[derive(Debug, Clone)]
pub struct UpdateRequestInput {
    /// Updated title.
    pub title: String,
    /// Updated description (markdown).
    pub description: String,
    /// Updated type.
    pub request_type: RequestType,
    /// Updated priority.
    pub priority: RequestPriority,
}

/// Input for moving a card (drag-and-drop status change).
#[derive(Debug, Clone)]
pub struct MoveRequestInput {
    /// Target status column.
    pub status: RequestStatus,
    /// Target position within the column.
    pub position: i32,
}

/// Input for adding a comment.
#[derive(Debug, Clone)]
pub struct CreateCommentInput {
    /// Item this comment belongs to.
    pub item_id: RequestBoardItemId,
    /// Admin user writing the comment.
    pub admin_user_id: AdminUserId,
    /// Comment body (markdown).
    pub body: String,
}
