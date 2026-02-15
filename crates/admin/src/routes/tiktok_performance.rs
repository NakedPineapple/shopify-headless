//! TikTok Shop performance dashboard routes.
//!
//! Provides a view of the shop's seller performance metrics
//! (delivery rates, cancellation rates, health scores) synced
//! from the TikTok Shop Open API. Only `super_admin` users can
//! access these features.

use askama::Template;
use axum::{
    Router,
    extract::State,
    response::{Html, IntoResponse, Redirect, Response},
    routing::get,
};
use rust_decimal::Decimal;
use tower_sessions::Session;
use tracing::instrument;

use crate::db::{TikTokPerformanceRepository, TikTokPerformanceSnapshot};
use crate::filters;
use crate::middleware::require_super_admin;
use crate::models::CurrentAdmin;
use crate::state::AppState;

use super::dashboard::AdminUserView;

// =============================================================================
// View Types
// =============================================================================

/// Pre-computed display values for the latest performance snapshot.
///
/// Askama templates cannot compare `Decimal` to integer literals, so we
/// convert the raw snapshot into strings and CSS colour classes on the
/// Rust side.
struct PerformanceMetricDisplay {
    health: String,
    snapshot_date: String,
    otd_rate: Option<String>,
    otd_color: String,
    ld_rate: Option<String>,
    ld_color: String,
    sfc_rate: Option<String>,
    sfc_color: String,
    cs_rate: Option<String>,
    cs_color: String,
}

// =============================================================================
// Templates
// =============================================================================

/// TikTok performance dashboard template.
#[derive(Template)]
#[template(path = "tiktok/performance.html")]
struct PerformanceTemplate {
    admin_user: AdminUserView,
    current_path: String,
    connected: bool,
    latest_display: Option<PerformanceMetricDisplay>,
    history: Vec<TikTokPerformanceSnapshot>,
    history_labels: String,
    otd_data: String,
    ld_data: String,
    sync_error: Option<String>,
}

// =============================================================================
// Router
// =============================================================================

/// Build the TikTok performance router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/tiktok/performance", get(performance_dashboard))
        .route("/tiktok/performance/sync", get(performance_sync))
}

// =============================================================================
// Route Handlers
// =============================================================================

/// GET /tiktok/performance -- Performance dashboard.
#[instrument(skip(state, session))]
async fn performance_dashboard(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(admin) = get_admin(&session).await else {
        return Redirect::to("/auth/login").into_response();
    };

    let connected = state.tiktok().is_some();
    if !connected {
        return render(PerformanceTemplate {
            admin_user: AdminUserView::from(&admin),
            current_path: "/tiktok/performance".to_string(),
            connected: false,
            latest_display: None,
            history: vec![],
            history_labels: "[]".to_string(),
            otd_data: "[]".to_string(),
            ld_data: "[]".to_string(),
            sync_error: None,
        });
    }

    let repo = TikTokPerformanceRepository::new(state.pool());
    let latest = repo.get_latest().await.ok().flatten();
    let latest_display = latest.as_ref().map(snapshot_to_display);
    let history = repo.get_history(30).await.unwrap_or_default();
    let (history_labels, otd_data, ld_data) = build_history_chart_json(&history);

    render(PerformanceTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/tiktok/performance".to_string(),
        connected: true,
        latest_display,
        history,
        history_labels,
        otd_data,
        ld_data,
        sync_error: None,
    })
}

/// GET /tiktok/performance/sync -- Trigger manual performance sync.
#[instrument(skip(state, session))]
async fn performance_sync(State(state): State<AppState>, session: Session) -> Response {
    if let Err(response) = require_super_admin(&state, &session).await {
        return response;
    }

    let Some(client) = state.tiktok() else {
        return Redirect::to("/tiktok/performance").into_response();
    };

    match client.get_performance_metrics().await {
        Ok(metrics) => {
            let repo = TikTokPerformanceRepository::new(state.pool());
            let params = crate::db::tiktok_performance::UpsertTikTokPerformanceParams {
                snapshot_date: chrono::Utc::now().date_naive(),
                on_time_delivery_rate: metrics
                    .on_time_delivery_rate
                    .and_then(|v| Decimal::try_from(v).ok()),
                late_dispatch_rate: metrics
                    .late_dispatch_rate
                    .and_then(|v| Decimal::try_from(v).ok()),
                seller_fault_cancel_rate: metrics
                    .seller_fault_cancel_rate
                    .and_then(|v| Decimal::try_from(v).ok()),
                customer_satisfaction_rate: metrics
                    .customer_satisfaction_rate
                    .and_then(|v| Decimal::try_from(v).ok()),
                overall_health: metrics
                    .overall_health
                    .unwrap_or_else(|| "UNKNOWN".to_string()),
            };

            match repo.upsert_snapshot(&params).await {
                Ok(_) => tracing::info!("TikTok performance snapshot synced"),
                Err(e) => tracing::error!(error = %e, "Failed to save performance snapshot"),
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to fetch TikTok performance metrics");
        }
    }

    Redirect::to("/tiktok/performance").into_response()
}

// =============================================================================
// Helpers
// =============================================================================

/// Convert a raw `TikTokPerformanceSnapshot` into display-ready values.
fn snapshot_to_display(snap: &TikTokPerformanceSnapshot) -> PerformanceMetricDisplay {
    let health = snap.overall_health.to_lowercase();
    let snapshot_date = snap.snapshot_date.format("%b %d, %Y").to_string();

    let (otd_rate, otd_color) = format_rate_and_color(snap.on_time_delivery_rate, true);
    let (ld_rate, ld_color) = format_rate_and_color(snap.late_dispatch_rate, false);
    let (sfc_rate, sfc_color) = format_rate_and_color(snap.seller_fault_cancel_rate, false);
    let (cs_rate, cs_color) = format_rate_and_color(snap.customer_satisfaction_rate, true);

    PerformanceMetricDisplay {
        health,
        snapshot_date,
        otd_rate,
        otd_color,
        ld_rate,
        ld_color,
        sfc_rate,
        sfc_color,
        cs_rate,
        cs_color,
    }
}

/// Format a rate as a percentage string and pick a colour class.
///
/// For `higher_is_better` metrics (OTD, satisfaction): >=95 green, >=90 amber, else red.
/// For `lower_is_better` metrics (late dispatch, cancellation): <=2 green, <=5 amber, else red.
fn format_rate_and_color(
    rate: Option<Decimal>,
    higher_is_better: bool,
) -> (Option<String>, String) {
    let Some(value) = rate else {
        return (None, "text-muted-foreground".to_string());
    };

    let display = format!("{value:.1}");

    let color = if higher_is_better {
        let threshold_green = Decimal::new(95, 0);
        let threshold_amber = Decimal::new(90, 0);
        if value >= threshold_green {
            "text-green-600 dark:text-green-400"
        } else if value >= threshold_amber {
            "text-amber-600 dark:text-amber-400"
        } else {
            "text-red-600 dark:text-red-400"
        }
    } else {
        let threshold_green = Decimal::new(2, 0);
        let threshold_amber = Decimal::new(5, 0);
        if value <= threshold_green {
            "text-green-600 dark:text-green-400"
        } else if value <= threshold_amber {
            "text-amber-600 dark:text-amber-400"
        } else {
            "text-red-600 dark:text-red-400"
        }
    };

    (Some(display), color.to_string())
}

/// Build Chart.js JSON arrays for the performance history chart.
fn build_history_chart_json(history: &[TikTokPerformanceSnapshot]) -> (String, String, String) {
    if history.is_empty() {
        return ("[]".to_string(), "[]".to_string(), "[]".to_string());
    }

    // History is most-recent-first, so reverse for chronological chart order
    let labels: Vec<String> = history
        .iter()
        .rev()
        .map(|s| s.snapshot_date.format("%b %d").to_string())
        .collect();

    let otd: Vec<String> = history
        .iter()
        .rev()
        .map(|s| {
            s.on_time_delivery_rate
                .map_or_else(|| "null".to_string(), |v| v.to_string())
        })
        .collect();

    let ld: Vec<String> = history
        .iter()
        .rev()
        .map(|s| {
            s.late_dispatch_rate
                .map_or_else(|| "null".to_string(), |v| v.to_string())
        })
        .collect();

    let labels_json = serde_json::to_string(&labels).unwrap_or_else(|_| "[]".to_string());
    let otd_json = format!("[{}]", otd.join(","));
    let ld_json = format!("[{}]", ld.join(","));

    (labels_json, otd_json, ld_json)
}

/// Get the current admin from the session.
async fn get_admin(session: &Session) -> Option<CurrentAdmin> {
    session
        .get::<CurrentAdmin>(crate::models::session_keys::CURRENT_ADMIN)
        .await
        .ok()
        .flatten()
}

/// Render an Askama template into an HTML response.
fn render(template: impl Template) -> Response {
    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
    .into_response()
}
