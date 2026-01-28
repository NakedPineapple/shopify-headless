//! User preferences API handlers.

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::post,
};
use serde::{Deserialize, Serialize};

use crate::{db::settings, middleware::auth::RequireAdminAuth, state::AppState};

/// Build the preferences router.
pub fn router() -> Router<AppState> {
    Router::new().route("/api/preferences/table/{table_id}", post(save_table_prefs))
}

/// Request for saving table preferences.
#[derive(Debug, Deserialize)]
pub struct TablePrefsRequest {
    pub columns: Vec<String>,
}

/// Response for table preferences.
#[derive(Debug, Serialize)]
pub struct TablePrefsResponse {
    pub success: bool,
}

/// Save table column preferences.
///
/// # Errors
///
/// Returns an error if the request body is invalid or the database operation fails.
pub async fn save_table_prefs(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(table_id): Path<String>,
    Json(body): Json<TablePrefsRequest>,
) -> Result<Json<TablePrefsResponse>, StatusCode> {
    // Build the settings key
    let key = format!("table.{table_id}.columns");
    let value = serde_json::to_value(&body.columns).map_err(|_| StatusCode::BAD_REQUEST)?;

    // Save to database using settings module
    match settings::set_user_setting(state.pool(), admin.id.into(), &key, &value).await {
        Ok(()) => Ok(Json(TablePrefsResponse { success: true })),
        Err(e) => {
            tracing::error!("Failed to save table preferences: {e}");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
