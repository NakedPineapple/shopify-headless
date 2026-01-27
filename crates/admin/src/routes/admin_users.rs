//! Admin users management route handler.

use askama::Template;
use axum::extract::State;
use axum::response::Html;
use chrono::{DateTime, Utc};
use tracing::instrument;

use naked_pineapple_core::AdminRole;

use crate::{
    db::{AdminInvite, AdminInviteRepository, AdminUserRepository},
    filters,
    middleware::auth::RequireSuperAdmin,
    models::CurrentAdmin,
    state::AppState,
};

use super::dashboard::AdminUserView;

/// Admin user view for templates.
#[derive(Debug, Clone)]
pub struct AdminUserListItem {
    pub id: i32,
    pub email: String,
    pub name: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
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

/// Admin users page template.
#[derive(Template)]
#[template(path = "admin_users/index.html")]
pub struct AdminUsersIndexTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub users: Vec<AdminUserListItem>,
    pub pending_invites: Vec<InviteListItem>,
}

/// Admin users list page handler (`super_admin` only).
#[instrument(skip(admin, state))]
pub async fn index(
    RequireSuperAdmin(admin): RequireSuperAdmin,
    State(state): State<AppState>,
) -> Html<String> {
    let user_repo = AdminUserRepository::new(state.pool());
    let invite_repo = AdminInviteRepository::new(state.pool());

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
        current_path: "/admin-users".to_string(),
        users,
        pending_invites,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}
