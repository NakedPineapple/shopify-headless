//! Newsletter route handlers for Klaviyo campaign management.
//!
//! Provides HTTP endpoints for viewing and managing email and SMS campaigns.
//! Super admins have full access; non-super admins have read-only access.

use askama::Template;
use axum::{
    Form, Router,
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;
use tracing::{debug, info, instrument, warn};

use crate::filters;
use crate::middleware::{RequireAdminAuth, RequireSuperAdmin};
use crate::routes::dashboard::AdminUserView;
use crate::services::klaviyo::{
    Campaign, CampaignChannel, CampaignStatus, KlaviyoClient, KlaviyoError, SubscriberStats,
};
use crate::state::AppState;

/// Build the newsletter router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/newsletter", get(index))
        .route("/newsletter/campaigns", get(campaigns_list))
        .route(
            "/newsletter/campaigns/new",
            get(campaign_new).post(campaign_create),
        )
        .route("/newsletter/campaigns/{id}", get(campaign_show))
        .route(
            "/newsletter/campaigns/{id}/edit",
            get(campaign_edit).post(campaign_update),
        )
        .route("/newsletter/campaigns/{id}/send", post(campaign_send))
        .route("/newsletter/campaigns/{id}/delete", post(campaign_delete))
        .route("/newsletter/campaigns/{id}/preview", get(campaign_preview))
}

// =============================================================================
// Templates
// =============================================================================

/// Newsletter dashboard template.
#[derive(Template)]
#[template(path = "newsletter/index.html")]
struct NewsletterIndexTemplate {
    admin_user: AdminUserView,
    current_path: String,
    stats: SubscriberStats,
    recent_campaigns: Vec<Campaign>,
}

/// Campaign list template.
#[derive(Template)]
#[template(path = "newsletter/campaigns.html")]
struct CampaignsListTemplate {
    admin_user: AdminUserView,
    current_path: String,
    campaigns: Vec<Campaign>,
    channel_filter: Option<String>,
    status_filter: Option<String>,
}

/// Campaign detail template.
#[derive(Template)]
#[template(path = "newsletter/campaign_show.html")]
struct CampaignShowTemplate {
    admin_user: AdminUserView,
    current_path: String,
    campaign: Campaign,
}

/// Campaign form template (new/edit).
#[derive(Template)]
#[template(path = "newsletter/campaign_form.html")]
struct CampaignFormTemplate {
    admin_user: AdminUserView,
    current_path: String,
    campaign: Option<Campaign>,
    channel: String,
    is_edit: bool,
    error: Option<String>,
}

/// Campaign preview template.
#[derive(Template)]
#[template(path = "newsletter/campaign_preview.html")]
struct CampaignPreviewTemplate {
    admin_user: AdminUserView,
    current_path: String,
    campaign: Campaign,
}

/// Error page template.
#[derive(Template)]
#[template(path = "newsletter/error.html")]
struct NewsletterErrorTemplate {
    admin_user: AdminUserView,
    current_path: String,
    title: String,
    message: String,
    show_config_help: bool,
}

// =============================================================================
// Request Types
// =============================================================================

/// Query parameters for campaign list.
#[derive(Debug, Deserialize)]
pub struct CampaignListQuery {
    channel: Option<String>,
    status: Option<String>,
}

/// Query parameters for new campaign form.
#[derive(Debug, Deserialize)]
pub struct NewCampaignQuery {
    #[serde(default = "default_channel")]
    channel: String,
}

fn default_channel() -> String {
    "email".to_string()
}

/// Form data for creating an email campaign.
#[derive(Debug, Deserialize)]
pub struct CreateEmailCampaignForm {
    name: String,
    subject: String,
    from_email: String,
    from_name: String,
}

/// Form data for updating a campaign.
#[derive(Debug, Deserialize)]
pub struct UpdateCampaignForm {
    name: String,
}

// =============================================================================
// Error Handling
// =============================================================================

/// Newsletter-specific errors.
#[derive(Debug)]
pub enum NewsletterError {
    /// Klaviyo is not configured.
    NotConfigured,
    /// Klaviyo API error.
    Klaviyo(KlaviyoError),
}

impl From<KlaviyoError> for NewsletterError {
    fn from(err: KlaviyoError) -> Self {
        Self::Klaviyo(err)
    }
}

impl IntoResponse for NewsletterError {
    fn into_response(self) -> Response {
        match self {
            Self::NotConfigured => (
                StatusCode::SERVICE_UNAVAILABLE,
                "Newsletter service is not configured. Please set KLAVIYO_API_KEY and KLAVIYO_LIST_ID.",
            )
                .into_response(),
            Self::Klaviyo(err) => {
                let (status, message) = match &err {
                    KlaviyoError::Http(_) => (StatusCode::BAD_GATEWAY, err.to_string()),
                    KlaviyoError::Api { .. } => (StatusCode::BAD_REQUEST, err.to_string()),
                    KlaviyoError::RateLimited(_) => {
                        (StatusCode::TOO_MANY_REQUESTS, err.to_string())
                    }
                    KlaviyoError::NotFound(_) => (StatusCode::NOT_FOUND, err.to_string()),
                    KlaviyoError::Parse(_) => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
                    KlaviyoError::Unauthorized => (StatusCode::UNAUTHORIZED, err.to_string()),
                };
                (status, message).into_response()
            }
        }
    }
}

/// Helper to get the Klaviyo client from app state.
fn get_klaviyo_client(state: &AppState) -> Result<KlaviyoClient, NewsletterError> {
    let config = state
        .config()
        .klaviyo()
        .ok_or(NewsletterError::NotConfigured)?;
    KlaviyoClient::new(config).map_err(NewsletterError::Klaviyo)
}

// =============================================================================
// Route Handlers
// =============================================================================

/// Newsletter dashboard with stats and recent campaigns.
///
/// GET /newsletter
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
async fn index(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
) -> Result<impl IntoResponse, NewsletterError> {
    debug!("Loading newsletter dashboard with stats and recent campaigns");

    let client = match get_klaviyo_client(&state) {
        Ok(c) => c,
        Err(e) => {
            warn!("Klaviyo client not configured");
            return Err(e);
        }
    };

    // Fetch subscriber stats and recent campaigns
    let subscriber_stats = client.get_subscriber_stats().await?;
    let recent_campaigns = client.list_campaigns(None, None).await?;

    // Take only the 5 most recent
    let recent_campaigns: Vec<_> = recent_campaigns.into_iter().take(5).collect();

    info!(
        subscriber_count = subscriber_stats.email_subscribers,
        recent_campaign_count = recent_campaigns.len(),
        "Newsletter dashboard loaded successfully"
    );

    let template = NewsletterIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/newsletter".to_string(),
        stats: subscriber_stats,
        recent_campaigns,
    };

    Ok(Html(template.render().unwrap_or_else(|_| {
        String::from("Error rendering template")
    })))
}

/// List all campaigns with optional filters.
///
/// GET /newsletter/campaigns
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32()))]
async fn campaigns_list(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    axum::extract::Query(query): axum::extract::Query<CampaignListQuery>,
) -> Result<impl IntoResponse, NewsletterError> {
    debug!(
        channel_filter = ?query.channel,
        status_filter = ?query.status,
        "Listing campaigns with filters"
    );

    let client = match get_klaviyo_client(&state) {
        Ok(c) => c,
        Err(e) => {
            warn!("Klaviyo client not configured");
            return Err(e);
        }
    };

    let channel_filter = query.channel.as_deref().and_then(|c| match c {
        "email" => Some(CampaignChannel::Email),
        "sms" => Some(CampaignChannel::Sms),
        _ => None,
    });

    let status_filter = query.status.as_deref().and_then(|s| match s {
        "draft" => Some(CampaignStatus::Draft),
        "scheduled" => Some(CampaignStatus::Scheduled),
        "sending" => Some(CampaignStatus::Sending),
        "sent" => Some(CampaignStatus::Sent),
        "cancelled" => Some(CampaignStatus::Cancelled),
        _ => None,
    });

    let campaigns = client.list_campaigns(status_filter, channel_filter).await?;

    info!(campaign_count = campaigns.len(), "Campaign list loaded");

    let template = CampaignsListTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/newsletter/campaigns".to_string(),
        campaigns,
        channel_filter: query.channel,
        status_filter: query.status,
    };

    Ok(Html(template.render().unwrap_or_else(|_| {
        String::from("Error rendering template")
    })))
}

/// Show campaign creation form.
///
/// GET /newsletter/campaigns/new
#[instrument(fields(admin_id = %admin.id.as_i32()))]
async fn campaign_new(
    RequireSuperAdmin(admin): RequireSuperAdmin,
    axum::extract::Query(query): axum::extract::Query<NewCampaignQuery>,
) -> impl IntoResponse {
    debug!(channel = %query.channel, "Showing new campaign form");

    let template = CampaignFormTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/newsletter/campaigns/new".to_string(),
        campaign: None,
        channel: query.channel,
        is_edit: false,
        error: None,
    };

    Html(
        template
            .render()
            .unwrap_or_else(|_| String::from("Error rendering template")),
    )
}

/// Create a new campaign.
///
/// POST /newsletter/campaigns/new
#[instrument(skip(state, form), fields(admin_id = %admin.id.as_i32()))]
async fn campaign_create(
    State(state): State<AppState>,
    RequireSuperAdmin(admin): RequireSuperAdmin,
    axum::extract::Query(query): axum::extract::Query<NewCampaignQuery>,
    Form(form): Form<CreateEmailCampaignForm>,
) -> Result<Response, NewsletterError> {
    debug!(channel = %query.channel, campaign_name = %form.name, "Creating new campaign");

    let client = match get_klaviyo_client(&state) {
        Ok(c) => c,
        Err(e) => {
            warn!("Klaviyo client not configured");
            return Err(e);
        }
    };

    let campaign = if query.channel == "sms" {
        // For SMS, the subject field is actually the body
        client
            .create_sms_campaign(&form.name, &form.subject)
            .await?
    } else {
        client
            .create_email_campaign(&form.name, &form.subject, &form.from_email, &form.from_name)
            .await?
    };

    info!(
        campaign_id = %campaign.id,
        campaign_name = %form.name,
        channel = %query.channel,
        "Campaign created successfully"
    );

    // Suppress unused variable warning for admin used only in instrument macro
    let _ = &admin;

    Ok(Redirect::to(&format!("/newsletter/campaigns/{}", campaign.id)).into_response())
}

/// Show campaign details.
///
/// GET /newsletter/campaigns/{id}
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32(), campaign_id = %id))]
async fn campaign_show(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, NewsletterError> {
    debug!("Loading campaign details");

    let client = match get_klaviyo_client(&state) {
        Ok(c) => c,
        Err(e) => {
            warn!("Klaviyo client not configured");
            return Err(e);
        }
    };

    let campaign = match client.get_campaign(&id).await {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "Failed to fetch campaign");
            return Err(e.into());
        }
    };

    info!(campaign_name = %campaign.attributes.name, "Campaign details loaded");

    let template = CampaignShowTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: format!("/newsletter/campaigns/{id}"),
        campaign,
    };

    Ok(Html(template.render().unwrap_or_else(|_| {
        String::from("Error rendering template")
    })))
}

/// Show campaign edit form.
///
/// GET /newsletter/campaigns/{id}/edit
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32(), campaign_id = %id))]
async fn campaign_edit(
    State(state): State<AppState>,
    RequireSuperAdmin(admin): RequireSuperAdmin,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, NewsletterError> {
    debug!("Loading campaign edit form");

    let client = match get_klaviyo_client(&state) {
        Ok(c) => c,
        Err(e) => {
            warn!("Klaviyo client not configured");
            return Err(e);
        }
    };

    let campaign = match client.get_campaign(&id).await {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "Failed to fetch campaign for editing");
            return Err(e.into());
        }
    };

    info!(campaign_name = %campaign.attributes.name, "Campaign edit form loaded");

    let template = CampaignFormTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: format!("/newsletter/campaigns/{id}/edit"),
        channel: "email".to_string(), // Will be determined from campaign
        campaign: Some(campaign),
        is_edit: true,
        error: None,
    };

    Ok(Html(template.render().unwrap_or_else(|_| {
        String::from("Error rendering template")
    })))
}

/// Update an existing campaign.
///
/// POST /newsletter/campaigns/{id}/edit
#[instrument(skip(state, form), fields(admin_id = %admin.id.as_i32(), campaign_id = %id))]
async fn campaign_update(
    State(state): State<AppState>,
    RequireSuperAdmin(admin): RequireSuperAdmin,
    Path(id): Path<String>,
    Form(form): Form<UpdateCampaignForm>,
) -> Result<Response, NewsletterError> {
    debug!(new_name = %form.name, "Updating campaign");

    let client = match get_klaviyo_client(&state) {
        Ok(c) => c,
        Err(e) => {
            warn!("Klaviyo client not configured");
            return Err(e);
        }
    };

    if let Err(e) = client.update_campaign(&id, Some(&form.name)).await {
        warn!(error = %e, "Failed to update campaign");
        return Err(e.into());
    }

    info!(new_name = %form.name, "Campaign updated successfully");

    // Suppress unused variable warning for admin used only in instrument macro
    let _ = &admin;

    Ok(Redirect::to(&format!("/newsletter/campaigns/{id}")).into_response())
}

/// Send a campaign immediately.
///
/// POST /newsletter/campaigns/{id}/send
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32(), campaign_id = %id))]
async fn campaign_send(
    State(state): State<AppState>,
    RequireSuperAdmin(admin): RequireSuperAdmin,
    Path(id): Path<String>,
) -> Result<Response, NewsletterError> {
    debug!("Initiating campaign send");

    let client = match get_klaviyo_client(&state) {
        Ok(c) => c,
        Err(e) => {
            warn!("Klaviyo client not configured");
            return Err(e);
        }
    };

    if let Err(e) = client.send_campaign(&id).await {
        warn!(error = %e, "Failed to send campaign");
        return Err(e.into());
    }

    info!("Campaign sent successfully");

    // Suppress unused variable warning for admin used only in instrument macro
    let _ = &admin;

    Ok(Redirect::to(&format!("/newsletter/campaigns/{id}")).into_response())
}

/// Delete a draft campaign.
///
/// POST /newsletter/campaigns/{id}/delete
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32(), campaign_id = %id))]
async fn campaign_delete(
    State(state): State<AppState>,
    RequireSuperAdmin(admin): RequireSuperAdmin,
    Path(id): Path<String>,
) -> Result<Response, NewsletterError> {
    debug!("Deleting campaign");

    let client = match get_klaviyo_client(&state) {
        Ok(c) => c,
        Err(e) => {
            warn!("Klaviyo client not configured");
            return Err(e);
        }
    };

    if let Err(e) = client.delete_campaign(&id).await {
        warn!(error = %e, "Failed to delete campaign");
        return Err(e.into());
    }

    info!("Campaign deleted successfully");

    // Suppress unused variable warning for admin used only in instrument macro
    let _ = &admin;

    Ok(Redirect::to("/newsletter/campaigns").into_response())
}

/// Preview a campaign.
///
/// GET /newsletter/campaigns/{id}/preview
#[instrument(skip(state), fields(admin_id = %admin.id.as_i32(), campaign_id = %id))]
async fn campaign_preview(
    State(state): State<AppState>,
    RequireAdminAuth(admin): RequireAdminAuth,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, NewsletterError> {
    debug!("Loading campaign preview");

    let client = match get_klaviyo_client(&state) {
        Ok(c) => c,
        Err(e) => {
            warn!("Klaviyo client not configured");
            return Err(e);
        }
    };

    let campaign = match client.get_campaign(&id).await {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "Failed to fetch campaign for preview");
            return Err(e.into());
        }
    };

    info!(campaign_name = %campaign.attributes.name, "Campaign preview loaded");

    let template = CampaignPreviewTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: format!("/newsletter/campaigns/{id}/preview"),
        campaign,
    };

    Ok(Html(template.render().unwrap_or_else(|_| {
        String::from("Error rendering template")
    })))
}
