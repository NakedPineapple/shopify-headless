//! Admin users management route handler.

use askama::Template;
use axum::{
    Form,
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use tracing::instrument;

use naked_pineapple_core::{AdminRole, AdminUserId};

use crate::{
    db::{AdminInvite, AdminInviteRepository, AdminUserRepository, RepositoryError},
    filters,
    middleware::auth::RequireSuperAdmin,
    state::AppState,
};

use super::dashboard::AdminUserView;

// =============================================================================
// View Models
// =============================================================================

/// Admin user view for templates.
#[derive(Debug, Clone)]
pub struct AdminUserListItem {
    pub id: i32,
    pub email: String,
    pub name: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub is_current_user: bool,
}

/// Invite view for templates.
#[derive(Debug, Clone)]
pub struct InviteListItem {
    pub id: i32,
    pub email: String,
    pub name: String,
    pub role: String,
    pub expires_at: DateTime<Utc>,
    pub is_expired: bool,
    pub is_used: bool,
    pub created_at: DateTime<Utc>,
}

impl From<&AdminInvite> for InviteListItem {
    fn from(invite: &AdminInvite) -> Self {
        Self {
            id: invite.id,
            email: invite.email.to_string(),
            name: invite.name.clone(),
            role: format!("{}", invite.role),
            expires_at: invite.expires_at,
            is_expired: invite.is_expired(),
            is_used: invite.is_used(),
            created_at: invite.created_at,
        }
    }
}

// =============================================================================
// Form Inputs
// =============================================================================

/// Form input for updating admin role.
#[derive(Debug, Deserialize)]
pub struct UpdateRoleForm {
    pub role: String,
}

/// Form input for deleting an admin user.
#[derive(Debug, Deserialize)]
pub struct DeleteUserForm {
    pub confirm_email: String,
}

/// Form input for creating an invite.
#[derive(Debug, Deserialize)]
pub struct CreateInviteForm {
    pub email: String,
    pub name: String,
    pub role: String,
    pub expires_in_days: Option<i32>,
}

// =============================================================================
// Templates
// =============================================================================

/// Admin users page template.
#[derive(Template)]
#[template(path = "admin_users/index.html")]
pub struct AdminUsersIndexTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub users: Vec<AdminUserListItem>,
    pub pending_invites: Vec<InviteListItem>,
    pub current_user_id: i32,
}

/// Single admin user row template for HTMX updates.
#[derive(Template)]
#[template(path = "admin_users/_user_row.html")]
pub struct AdminUserRowTemplate {
    pub user: AdminUserListItem,
    pub current_user_id: i32,
}

/// Single invite row template for HTMX updates.
#[derive(Template)]
#[template(path = "admin_users/_invite_row.html")]
pub struct InviteRowTemplate {
    pub invite: InviteListItem,
}

/// Error response template for HTMX.
#[derive(Template)]
#[template(path = "admin_users/_error.html")]
pub struct ErrorTemplate {
    pub message: String,
}

// =============================================================================
// Handlers
// =============================================================================

/// Admin users list page handler (`super_admin` only).
#[instrument(skip(admin, state))]
pub async fn index(
    RequireSuperAdmin(admin): RequireSuperAdmin,
    State(state): State<AppState>,
) -> Html<String> {
    let user_repo = AdminUserRepository::new(state.pool());
    let invite_repo = AdminInviteRepository::new(state.pool());
    let current_user_id = admin.id.as_i32();

    // Fetch all users
    let users = match user_repo.list_all().await {
        Ok(users) => users
            .iter()
            .map(|u| AdminUserListItem {
                id: u.id.as_i32(),
                email: u.email.to_string(),
                name: u.name.clone(),
                role: format!("{}", u.role),
                created_at: u.created_at,
                is_current_user: u.id == admin.id,
            })
            .collect(),
        Err(e) => {
            tracing::error!("Failed to fetch admin users: {e}");
            vec![]
        }
    };

    // Fetch pending invites (not used, not expired)
    let pending_invites = match invite_repo.list_all().await {
        Ok(invites) => invites
            .iter()
            .filter(|i| !i.is_used())
            .map(InviteListItem::from)
            .collect(),
        Err(e) => {
            tracing::error!("Failed to fetch invites: {e}");
            vec![]
        }
    };

    let template = AdminUsersIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/admin-users".to_owned(),
        users,
        pending_invites,
        current_user_id,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_owned()
    }))
}

/// Update an admin user's role.
///
/// POST /admin-users/{id}/role
#[instrument(skip(admin, state))]
pub async fn update_role(
    RequireSuperAdmin(admin): RequireSuperAdmin,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Form(form): Form<UpdateRoleForm>,
) -> Response {
    let target_id = AdminUserId::new(id);

    // Cannot modify yourself
    if target_id == admin.id {
        return error_response(StatusCode::FORBIDDEN, "You cannot change your own role");
    }

    // Parse role
    let new_role = match form.role.as_str() {
        "admin" => AdminRole::Admin,
        "super_admin" => AdminRole::SuperAdmin,
        _ => return error_response(StatusCode::BAD_REQUEST, "Invalid role"),
    };

    let user_repo = AdminUserRepository::new(state.pool());

    // If demoting from super_admin, check we won't remove the last one
    if new_role == AdminRole::Admin
        && let Ok(Some(target_user)) = user_repo.get_by_id(target_id).await
        && target_user.role == AdminRole::SuperAdmin
        && let Ok(count) = user_repo.count_by_role(AdminRole::SuperAdmin).await
        && count <= 1
    {
        return error_response(StatusCode::FORBIDDEN, "Cannot demote the last super admin");
    }

    // Update role
    let updated_user = match user_repo.update_role(target_id, new_role).await {
        Ok(user) => user,
        Err(RepositoryError::NotFound) => {
            return error_response(StatusCode::NOT_FOUND, "User not found");
        }
        Err(e) => {
            tracing::error!("Failed to update role: {e}");
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to update role");
        }
    };

    // Return updated row
    let template = AdminUserRowTemplate {
        user: AdminUserListItem {
            id: updated_user.id.as_i32(),
            email: updated_user.email.to_string(),
            name: updated_user.name,
            role: format!("{}", updated_user.role),
            created_at: updated_user.created_at,
            is_current_user: false,
        },
        current_user_id: admin.id.as_i32(),
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_owned()
    }))
    .into_response()
}

/// Delete an admin user.
///
/// POST /admin-users/{id}/delete
#[instrument(skip(admin, state))]
pub async fn delete_user(
    RequireSuperAdmin(admin): RequireSuperAdmin,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Form(form): Form<DeleteUserForm>,
) -> Response {
    let target_id = AdminUserId::new(id);

    // Cannot delete yourself
    if target_id == admin.id {
        return error_response(StatusCode::FORBIDDEN, "You cannot delete your own account");
    }

    let user_repo = AdminUserRepository::new(state.pool());

    // Get the user to verify email confirmation
    let target_user = match user_repo.get_by_id(target_id).await {
        Ok(Some(user)) => user,
        Ok(None) => return error_response(StatusCode::NOT_FOUND, "User not found"),
        Err(e) => {
            tracing::error!("Failed to get user: {e}");
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to get user");
        }
    };

    // Verify email confirmation
    if form.confirm_email.trim().to_lowercase() != target_user.email.as_str().to_lowercase() {
        return error_response(StatusCode::BAD_REQUEST, "Email confirmation does not match");
    }

    // If deleting a super_admin, check we won't remove the last one
    if target_user.role == AdminRole::SuperAdmin
        && let Ok(count) = user_repo.count_by_role(AdminRole::SuperAdmin).await
        && count <= 1
    {
        return error_response(StatusCode::FORBIDDEN, "Cannot delete the last super admin");
    }

    // Delete user
    if let Err(e) = user_repo.delete(target_id).await {
        tracing::error!("Failed to delete user: {e}");
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to delete user");
    }

    // Return empty response with hx-swap delete
    StatusCode::OK.into_response()
}

/// Create a new admin invite.
///
/// POST /admin-users/invites
#[instrument(skip(admin, state))]
pub async fn create_invite(
    RequireSuperAdmin(admin): RequireSuperAdmin,
    State(state): State<AppState>,
    Form(form): Form<CreateInviteForm>,
) -> Response {
    let email = form.email.trim().to_lowercase();
    let name = form.name.trim();

    // Validate inputs
    if email.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "Email is required");
    }
    if name.is_empty() {
        return error_response(StatusCode::BAD_REQUEST, "Name is required");
    }

    // Parse role (only admin or super_admin allowed)
    let role = match form.role.as_str() {
        "admin" => AdminRole::Admin,
        "super_admin" => AdminRole::SuperAdmin,
        _ => return error_response(StatusCode::BAD_REQUEST, "Invalid role"),
    };

    let expires_in_days = form.expires_in_days.unwrap_or(7);
    if !(1..=30).contains(&expires_in_days) {
        return error_response(
            StatusCode::BAD_REQUEST,
            "Expiration must be between 1 and 30 days",
        );
    }

    let invite_repo = AdminInviteRepository::new(state.pool());

    // Check for existing valid invite
    if matches!(invite_repo.is_valid_invite(&email).await, Ok(true)) {
        return error_response(
            StatusCode::CONFLICT,
            "A valid invite already exists for this email",
        );
    }

    // Check if email is already an admin
    let user_repo = AdminUserRepository::new(state.pool());
    if let Ok(parsed_email) = naked_pineapple_core::Email::parse(&email)
        && let Ok(Some(_)) = user_repo.get_by_email(&parsed_email).await
    {
        return error_response(
            StatusCode::CONFLICT,
            "An admin with this email already exists",
        );
    }

    // Create invite
    let invite = match invite_repo
        .create(&email, name, role, Some(admin.id), expires_in_days)
        .await
    {
        Ok(invite) => invite,
        Err(RepositoryError::Conflict(msg)) => {
            return error_response(StatusCode::CONFLICT, &msg);
        }
        Err(e) => {
            tracing::error!("Failed to create invite: {e}");
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to create invite");
        }
    };

    // Return the new invite row
    let template = InviteRowTemplate {
        invite: InviteListItem::from(&invite),
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_owned()
    }))
    .into_response()
}

/// Delete an admin invite.
///
/// POST /admin-users/invites/{id}/delete
#[instrument(skip(_admin, state))]
pub async fn delete_invite(
    RequireSuperAdmin(_admin): RequireSuperAdmin,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Response {
    let invite_repo = AdminInviteRepository::new(state.pool());

    if let Err(e) = invite_repo.delete(id).await {
        if matches!(e, RepositoryError::NotFound) {
            return error_response(StatusCode::NOT_FOUND, "Invite not found");
        }
        tracing::error!("Failed to delete invite: {e}");
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to delete invite");
    }

    // Return empty response with hx-swap delete
    StatusCode::OK.into_response()
}

// =============================================================================
// Helpers
// =============================================================================

/// Create an error response for HTMX requests.
fn error_response(status: StatusCode, message: &str) -> Response {
    let template = ErrorTemplate {
        message: message.to_owned(),
    };

    let html = template.render().unwrap_or_else(|_| message.to_owned());

    (
        status,
        [
            ("HX-Retarget", "#error-container"),
            ("HX-Reswap", "innerHTML"),
        ],
        Html(html),
    )
        .into_response()
}
