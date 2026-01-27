//! Gift card management route handlers.

use askama::Template;
use axum::{
    Form,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use serde::Deserialize;
use tracing::instrument;

use crate::{
    filters, middleware::auth::RequireAdminAuth, shopify::types::GiftCard, state::AppState,
};

use super::dashboard::AdminUserView;

/// Pagination query parameters.
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub cursor: Option<String>,
    pub query: Option<String>,
}

/// Gift card view for templates.
#[derive(Debug, Clone)]
pub struct GiftCardView {
    pub id: String,
    pub last_characters: String,
    pub balance: String,
    pub initial_value: String,
    pub expires_on: Option<String>,
    pub enabled: bool,
    pub created_at: String,
    pub customer_name: Option<String>,
    pub customer_email: Option<String>,
}

impl From<&GiftCard> for GiftCardView {
    fn from(gc: &GiftCard) -> Self {
        Self {
            id: gc.id.clone(),
            last_characters: gc.last_characters.clone(),
            balance: format!("${}", gc.balance.amount),
            initial_value: format!("${}", gc.initial_value.amount),
            expires_on: gc.expires_on.clone(),
            enabled: gc.enabled,
            created_at: gc.created_at.clone(),
            customer_name: gc.customer_name.clone(),
            customer_email: gc.customer_email.clone(),
        }
    }
}

/// Gift cards list page template.
#[derive(Template)]
#[template(path = "gift_cards/index.html")]
pub struct GiftCardsIndexTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub gift_cards: Vec<GiftCardView>,
    pub has_next_page: bool,
    pub next_cursor: Option<String>,
    pub search_query: Option<String>,
}

/// Gift card create form template.
#[derive(Template)]
#[template(path = "gift_cards/new.html")]
pub struct GiftCardNewTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub error: Option<String>,
    pub success_code: Option<String>,
}

/// Form input for creating gift cards.
#[derive(Debug, Deserialize)]
pub struct GiftCardFormInput {
    pub initial_value: String,
    pub customer_id: Option<String>,
    pub expires_on: Option<String>,
    pub note: Option<String>,
}

/// Gift cards list page handler.
#[instrument(skip(admin, state))]
pub async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Html<String> {
    let result = state
        .shopify()
        .get_gift_cards(25, query.cursor.clone(), query.query.clone())
        .await;

    let (gift_cards, has_next_page, next_cursor) = match result {
        Ok(conn) => {
            let gift_cards: Vec<GiftCardView> =
                conn.gift_cards.iter().map(GiftCardView::from).collect();
            (
                gift_cards,
                conn.page_info.has_next_page,
                conn.page_info.end_cursor,
            )
        }
        Err(e) => {
            tracing::error!("Failed to fetch gift cards: {e}");
            (vec![], false, None)
        }
    };

    let template = GiftCardsIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/gift-cards".to_string(),
        gift_cards,
        has_next_page,
        next_cursor,
        search_query: query.query,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// New gift card form handler.
#[instrument(skip(admin))]
pub async fn new_gift_card(RequireAdminAuth(admin): RequireAdminAuth) -> Html<String> {
    let template = GiftCardNewTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/gift-cards".to_string(),
        error: None,
        success_code: None,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// Create gift card handler.
#[instrument(skip(admin, state))]
pub async fn create(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<GiftCardFormInput>,
) -> impl IntoResponse {
    match state
        .shopify()
        .create_gift_card(
            &input.initial_value,
            input.customer_id.as_deref(),
            input.expires_on.as_deref(),
            input.note.as_deref(),
        )
        .await
    {
        Ok((gift_card_id, code)) => {
            tracing::info!(gift_card_id = %gift_card_id, initial_value = %input.initial_value, "Gift card created");
            // Show success with the gift card code
            let template = GiftCardNewTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/gift-cards".to_string(),
                error: None,
                success_code: Some(code),
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
        Err(e) => {
            tracing::error!(initial_value = %input.initial_value, error = %e, "Failed to create gift card");
            let template = GiftCardNewTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/gift-cards".to_string(),
                error: Some(e.to_string()),
                success_code: None,
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
    }
}

/// Disable gift card handler (HTMX).
#[instrument(skip(_admin, state))]
pub async fn disable(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Ensure ID has the proper Shopify format
    let gift_card_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/GiftCard/{id}")
    };

    match state.shopify().disable_gift_card(&gift_card_id).await {
        Ok(()) => {
            tracing::info!(gift_card_id = %gift_card_id, "Gift card disabled");
            (
                StatusCode::OK,
                [("HX-Trigger", "gift-card-disabled")],
                Html(
                    "<span class=\"text-yellow-600 dark:text-yellow-400\">Disabled</span>"
                        .to_string(),
                ),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(gift_card_id = %gift_card_id, error = %e, "Failed to disable gift card");
            (
                StatusCode::BAD_REQUEST,
                Html(format!(
                    "<span class=\"text-red-600 dark:text-red-400\">Error: {e}</span>"
                )),
            )
                .into_response()
        }
    }
}
