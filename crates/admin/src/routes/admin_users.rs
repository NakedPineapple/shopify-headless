//! Admin users management route handler.

use askama::Template;
use axum::{
    Form,
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use secrecy::SecretString;
use serde::Deserialize;
use tracing::{debug, info, instrument, warn};

use naked_pineapple_core::{AdminRole, AdminUserId};

use crate::{
    db::{AdminInvite, AdminInviteRepository, AdminUserRepository, RepositoryError},
    filters,
    middleware::auth::RequireSuperAdmin,
    services::AdminAuthService,
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
    pub has_password: bool,
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

/// Form input for setting a break-glass password.
#[derive(Debug, Deserialize)]
pub struct SetPasswordForm {
    pub password: String,
    pub password_confirm: String,
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
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
pub async fn index(
    RequireSuperAdmin(admin): RequireSuperAdmin,
    State(state): State<AppState>,
) -> Html<String> {
    debug!("Listing all admin users and pending invites");

    let user_repo = AdminUserRepository::new(state.pool());
    let invite_repo = AdminInviteRepository::new(state.pool());
    let current_user_id = admin.id.as_i32();

    // Fetch all users
    let users: Vec<AdminUserListItem> = match user_repo.list_all().await {
        Ok(users) => {
            debug!(user_count = users.len(), "Fetched admin users");
            users
                .iter()
                .map(|u| AdminUserListItem {
                    id: u.id.as_i32(),
                    email: u.email.to_string(),
                    name: u.name.clone(),
                    role: format!("{}", u.role),
                    has_password: u.has_password,
                    created_at: u.created_at,
                    is_current_user: u.id == admin.id,
                })
                .collect()
        }
        Err(e) => {
            tracing::error!("Failed to fetch admin users: {e}");
            vec![]
        }
    };

    // Fetch pending invites (not used, not expired)
    let pending_invites: Vec<InviteListItem> = match invite_repo.list_all().await {
        Ok(invites) => {
            let pending: Vec<InviteListItem> = invites
                .iter()
                .filter(|i| !i.is_used())
                .map(InviteListItem::from)
                .collect();
            debug!(
                total_invites = invites.len(),
                pending_invites = pending.len(),
                "Fetched admin invites"
            );
            pending
        }
        Err(e) => {
            tracing::error!("Failed to fetch invites: {e}");
            vec![]
        }
    };

    info!(
        user_count = users.len(),
        pending_invite_count = pending_invites.len(),
        "Rendered admin users index page"
    );

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
#[instrument(skip(state, form), fields(admin_id = %admin.id.as_i32(), target_user_id = %id))]
pub async fn update_role(
    RequireSuperAdmin(admin): RequireSuperAdmin,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Form(form): Form<UpdateRoleForm>,
) -> Response {
    debug!(new_role = %form.role, "Updating admin user role");

    let target_id = AdminUserId::new(id);

    // Cannot modify yourself
    if target_id == admin.id {
        warn!("Admin attempted to change their own role");
        return error_response(StatusCode::FORBIDDEN, "You cannot change your own role");
    }

    // Parse role
    let new_role = match form.role.as_str() {
        "admin" => AdminRole::Admin,
        "super_admin" => AdminRole::SuperAdmin,
        _ => {
            warn!(role = %form.role, "Invalid role specified");
            return error_response(StatusCode::BAD_REQUEST, "Invalid role");
        }
    };

    let user_repo = AdminUserRepository::new(state.pool());

    // If demoting from super_admin, check we won't remove the last one
    if new_role == AdminRole::Admin
        && let Ok(Some(target_user)) = user_repo.get_by_id(target_id).await
        && target_user.role == AdminRole::SuperAdmin
        && let Ok(count) = user_repo.count_by_role(AdminRole::SuperAdmin).await
        && count <= 1
    {
        warn!("Attempted to demote the last super admin");
        return error_response(StatusCode::FORBIDDEN, "Cannot demote the last super admin");
    }

    // Update role
    let updated_user = match user_repo.update_role(target_id, new_role).await {
        Ok(user) => user,
        Err(RepositoryError::NotFound) => {
            warn!("Target user not found for role update");
            return error_response(StatusCode::NOT_FOUND, "User not found");
        }
        Err(e) => {
            tracing::error!("Failed to update role: {e}");
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to update role");
        }
    };

    info!(
        target_user_id = %updated_user.id.as_i32(),
        new_role = %updated_user.role,
        "Successfully updated admin user role"
    );

    // Return updated row
    let template = AdminUserRowTemplate {
        user: AdminUserListItem {
            id: updated_user.id.as_i32(),
            email: updated_user.email.to_string(),
            name: updated_user.name,
            role: format!("{}", updated_user.role),
            has_password: updated_user.has_password,
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
#[instrument(skip(state, form), fields(admin_id = %admin.id.as_i32(), target_user_id = %id))]
pub async fn delete_user(
    RequireSuperAdmin(admin): RequireSuperAdmin,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Form(form): Form<DeleteUserForm>,
) -> Response {
    debug!("Deleting admin user");

    let target_id = AdminUserId::new(id);

    // Cannot delete yourself
    if target_id == admin.id {
        warn!("Admin attempted to delete their own account");
        return error_response(StatusCode::FORBIDDEN, "You cannot delete your own account");
    }

    let user_repo = AdminUserRepository::new(state.pool());

    // Get the user to verify email confirmation
    let target_user = match user_repo.get_by_id(target_id).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            warn!("Target user not found for deletion");
            return error_response(StatusCode::NOT_FOUND, "User not found");
        }
        Err(e) => {
            tracing::error!("Failed to get user: {e}");
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to get user");
        }
    };

    // Verify email confirmation
    if form.confirm_email.trim().to_lowercase() != target_user.email.as_str().to_lowercase() {
        warn!("Email confirmation did not match for user deletion");
        return error_response(StatusCode::BAD_REQUEST, "Email confirmation does not match");
    }

    // If deleting a super_admin, check we won't remove the last one
    if target_user.role == AdminRole::SuperAdmin
        && let Ok(count) = user_repo.count_by_role(AdminRole::SuperAdmin).await
        && count <= 1
    {
        warn!("Attempted to delete the last super admin");
        return error_response(StatusCode::FORBIDDEN, "Cannot delete the last super admin");
    }

    // Delete user
    if let Err(e) = user_repo.delete(target_id).await {
        tracing::error!("Failed to delete user: {e}");
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to delete user");
    }

    info!(
        deleted_user_email = %target_user.email,
        "Successfully deleted admin user"
    );

    // Return empty response with hx-swap delete
    StatusCode::OK.into_response()
}

/// Create a new admin invite.
///
/// POST /admin-users/invites
#[instrument(skip(state, form), fields(admin_id = %admin.id.as_i32()))]
pub async fn create_invite(
    RequireSuperAdmin(admin): RequireSuperAdmin,
    State(state): State<AppState>,
    Form(form): Form<CreateInviteForm>,
) -> Response {
    debug!(
        email = %form.email,
        name = %form.name,
        role = %form.role,
        "Creating admin invite"
    );

    let email = form.email.trim().to_lowercase();
    let name = form.name.trim();

    // Validate inputs
    if email.is_empty() {
        warn!("Invite creation failed: email is empty");
        return error_response(StatusCode::BAD_REQUEST, "Email is required");
    }
    if name.is_empty() {
        warn!("Invite creation failed: name is empty");
        return error_response(StatusCode::BAD_REQUEST, "Name is required");
    }

    // Parse role (only admin or super_admin allowed)
    let role = match form.role.as_str() {
        "admin" => AdminRole::Admin,
        "super_admin" => AdminRole::SuperAdmin,
        _ => {
            warn!(role = %form.role, "Invalid role specified for invite");
            return error_response(StatusCode::BAD_REQUEST, "Invalid role");
        }
    };

    let expires_in_days = form.expires_in_days.unwrap_or(7);
    if !(1..=30).contains(&expires_in_days) {
        warn!(expires_in_days, "Invalid expiration days for invite");
        return error_response(
            StatusCode::BAD_REQUEST,
            "Expiration must be between 1 and 30 days",
        );
    }

    let invite_repo = AdminInviteRepository::new(state.pool());

    // Check for existing valid invite
    if matches!(invite_repo.is_valid_invite(&email).await, Ok(true)) {
        warn!(email = %email, "Valid invite already exists for this email");
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
        warn!(email = %email, "Admin with this email already exists");
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
            warn!(email = %email, "Invite creation conflict: {msg}");
            return error_response(StatusCode::CONFLICT, &msg);
        }
        Err(e) => {
            tracing::error!("Failed to create invite: {e}");
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to create invite");
        }
    };

    info!(
        invite_id = invite.id,
        email = %email,
        role = %role,
        expires_in_days,
        "Successfully created admin invite"
    );

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
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32(), invite_id = %id))]
pub async fn delete_invite(
    RequireSuperAdmin(admin): RequireSuperAdmin,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Response {
    debug!("Deleting admin invite");

    let invite_repo = AdminInviteRepository::new(state.pool());

    if let Err(e) = invite_repo.delete(id).await {
        if matches!(e, RepositoryError::NotFound) {
            warn!("Invite not found for deletion");
            return error_response(StatusCode::NOT_FOUND, "Invite not found");
        }
        tracing::error!("Failed to delete invite: {e}");
        return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to delete invite");
    }

    info!("Successfully deleted admin invite");

    // Return empty response with hx-swap delete
    StatusCode::OK.into_response()
}

// =============================================================================
// Password Management
// =============================================================================

/// Set a break-glass password for an admin user.
///
/// POST /admin-users/{id}/password
#[instrument(skip(state, form), fields(admin_id = %admin.id.as_i32(), target_user_id = %id))]
pub async fn set_password(
    RequireSuperAdmin(admin): RequireSuperAdmin,
    State(state): State<AppState>,
    Path(id): Path<i32>,
    Form(form): Form<SetPasswordForm>,
) -> Response {
    debug!("Setting break-glass password for admin user");

    let target_id = AdminUserId::new(id);

    if form.password != form.password_confirm {
        warn!("Password confirmation did not match");
        return error_response(StatusCode::BAD_REQUEST, "Passwords do not match");
    }

    let password = SecretString::from(form.password);
    let auth = AdminAuthService::new(state.pool(), state.webauthn());

    if let Err(e) = auth.set_password(target_id, &password).await {
        warn!(error = %e, "Failed to set break-glass password");
        return error_response(StatusCode::BAD_REQUEST, &e.to_string());
    }

    info!(
        target_user_id = %target_id.as_i32(),
        set_by = %admin.id.as_i32(),
        "Break-glass password set by super admin"
    );

    // Return updated user row
    render_user_row(&state, target_id, admin.id).await
}

/// Clear a break-glass password for an admin user.
///
/// POST /admin-users/{id}/password/clear
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32(), target_user_id = %id))]
pub async fn clear_password(
    RequireSuperAdmin(admin): RequireSuperAdmin,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Response {
    debug!("Clearing break-glass password for admin user");

    let target_id = AdminUserId::new(id);
    let auth = AdminAuthService::new(state.pool(), state.webauthn());

    if let Err(e) = auth.clear_password(target_id).await {
        warn!(error = %e, "Failed to clear break-glass password");
        return error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to clear password",
        );
    }

    info!(
        target_user_id = %target_id.as_i32(),
        cleared_by = %admin.id.as_i32(),
        "Break-glass password cleared by super admin"
    );

    // Return updated user row
    render_user_row(&state, target_id, admin.id).await
}

/// Render a single user row for HTMX responses.
async fn render_user_row(
    state: &AppState,
    target_id: AdminUserId,
    current_admin_id: AdminUserId,
) -> Response {
    let user_repo = AdminUserRepository::new(state.pool());
    let user = match user_repo.get_by_id(target_id).await {
        Ok(Some(user)) => user,
        Ok(None) => return error_response(StatusCode::NOT_FOUND, "User not found"),
        Err(e) => {
            tracing::error!("Failed to fetch user: {e}");
            return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch user");
        }
    };

    let template = AdminUserRowTemplate {
        user: AdminUserListItem {
            id: user.id.as_i32(),
            email: user.email.to_string(),
            name: user.name,
            role: format!("{}", user.role),
            has_password: user.has_password,
            created_at: user.created_at,
            is_current_user: user.id == current_admin_id,
        },
        current_user_id: current_admin_id.as_i32(),
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_owned()
    }))
    .into_response()
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
