//! Dashboard route handler.

use askama::Template;
use axum::{extract::State, response::Html};

use crate::{middleware::auth::RequireAdminAuth, models::CurrentAdmin, state::AppState};

use naked_pineapple_core::AdminRole;

/// Admin user view for templates.
#[derive(Debug, Clone)]
pub struct AdminUserView {
    pub name: String,
    pub email: String,
    pub is_super_admin: bool,
}

impl From<&CurrentAdmin> for AdminUserView {
    fn from(admin: &CurrentAdmin) -> Self {
        Self {
            name: admin.name.clone(),
            email: admin.email.to_string(),
            is_super_admin: admin.role == AdminRole::SuperAdmin,
        }
    }
}

/// Dashboard metrics.
#[derive(Debug, Clone)]
pub struct DashboardMetrics {
    pub total_orders: String,
    pub total_revenue: String,
    pub total_customers: String,
    pub total_products: String,
}

impl Default for DashboardMetrics {
    fn default() -> Self {
        Self {
            total_orders: "0".to_string(),
            total_revenue: "$0.00".to_string(),
            total_customers: "0".to_string(),
            total_products: "0".to_string(),
        }
    }
}

/// Recent order view for dashboard.
#[derive(Debug, Clone)]
pub struct RecentOrderView {
    pub number: String,
    pub customer_name: String,
    pub total: String,
    pub status: String,
}

/// Activity item for dashboard.
#[derive(Debug, Clone)]
pub struct ActivityView {
    pub activity_type: String,
    pub icon: String,
    pub description: String,
    pub time_ago: String,
}

/// Dashboard template.
#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub metrics: DashboardMetrics,
    pub recent_orders: Vec<RecentOrderView>,
    pub recent_activity: Vec<ActivityView>,
}

/// Dashboard page handler.
pub async fn dashboard(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(_state): State<AppState>,
) -> Html<String> {
    // TODO: Fetch real metrics from Shopify Admin API
    let metrics = DashboardMetrics::default();

    // TODO: Fetch recent orders from Shopify
    let recent_orders: Vec<RecentOrderView> = vec![];

    // TODO: Build activity feed from events
    let recent_activity: Vec<ActivityView> = vec![];

    let template = DashboardTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/".to_string(),
        metrics,
        recent_orders,
        recent_activity,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}
