//! Collections management route handlers.

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
    filters, middleware::auth::RequireAdminAuth, shopify::types::Collection, state::AppState,
};

use super::dashboard::AdminUserView;

/// Pagination query parameters.
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub cursor: Option<String>,
    pub query: Option<String>,
}

/// Form input for creating/updating collections.
#[derive(Debug, Deserialize)]
pub struct CollectionFormInput {
    pub title: String,
    pub description_html: Option<String>,
}

/// Collection view for templates.
#[derive(Debug, Clone)]
pub struct CollectionView {
    pub id: String,
    pub title: String,
    pub handle: String,
    pub description: String,
    pub products_count: i64,
    pub image_url: Option<String>,
}

impl From<&Collection> for CollectionView {
    fn from(collection: &Collection) -> Self {
        Self {
            id: collection.id.clone(),
            title: collection.title.clone(),
            handle: collection.handle.clone(),
            description: collection.description.clone(),
            products_count: collection.products_count,
            image_url: collection.image.as_ref().map(|img| img.url.clone()),
        }
    }
}

/// Collections list page template.
#[derive(Template)]
#[template(path = "collections/index.html")]
pub struct CollectionsIndexTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub collections: Vec<CollectionView>,
    pub has_next_page: bool,
    pub next_cursor: Option<String>,
    pub search_query: Option<String>,
}

/// Collection create form template.
#[derive(Template)]
#[template(path = "collections/new.html")]
pub struct CollectionNewTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub error: Option<String>,
}

/// Collection edit form template.
#[derive(Template)]
#[template(path = "collections/edit.html")]
pub struct CollectionEditTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub collection: CollectionView,
    pub description_html: String,
    pub error: Option<String>,
}

/// Collections list page handler.
#[instrument(skip(admin, state))]
pub async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Html<String> {
    let result = state
        .shopify()
        .get_collections(25, query.cursor.clone(), query.query.clone())
        .await;

    let (collections, has_next_page, next_cursor) = match result {
        Ok(conn) => {
            let collections: Vec<CollectionView> =
                conn.collections.iter().map(CollectionView::from).collect();
            (
                collections,
                conn.page_info.has_next_page,
                conn.page_info.end_cursor,
            )
        }
        Err(e) => {
            tracing::error!("Failed to fetch collections: {e}");
            (vec![], false, None)
        }
    };

    let template = CollectionsIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/collections".to_string(),
        collections,
        has_next_page,
        next_cursor,
        search_query: query.query,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// New collection form handler.
#[instrument(skip(admin))]
pub async fn new_collection(RequireAdminAuth(admin): RequireAdminAuth) -> Html<String> {
    let template = CollectionNewTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/collections".to_string(),
        error: None,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// Create collection handler.
#[instrument(skip(admin, state))]
pub async fn create(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<CollectionFormInput>,
) -> impl IntoResponse {
    match state
        .shopify()
        .create_collection(&input.title, input.description_html.as_deref())
        .await
    {
        Ok(collection_id) => {
            tracing::info!(collection_id = %collection_id, title = %input.title, "Collection created");
            let numeric_id = collection_id
                .split('/')
                .next_back()
                .unwrap_or(&collection_id);
            Redirect::to(&format!("/collections/{numeric_id}/edit")).into_response()
        }
        Err(e) => {
            tracing::error!(title = %input.title, error = %e, "Failed to create collection");
            let template = CollectionNewTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/collections".to_string(),
                error: Some(e.to_string()),
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
    }
}

/// Edit collection form handler.
#[instrument(skip(admin, state))]
pub async fn edit(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let collection_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/Collection/{id}")
    };

    match state.shopify().get_collection(&collection_id).await {
        Ok(Some(collection)) => {
            let description_html = collection.description_html.clone().unwrap_or_default();
            let template = CollectionEditTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/collections".to_string(),
                collection: CollectionView::from(&collection),
                description_html,
                error: None,
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Collection not found").into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch collection: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch collection",
            )
                .into_response()
        }
    }
}

/// Update collection handler.
#[instrument(skip(admin, state))]
pub async fn update(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<CollectionFormInput>,
) -> impl IntoResponse {
    let collection_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Collection/{id}")
    };

    match state
        .shopify()
        .update_collection(
            &collection_id,
            Some(&input.title),
            input.description_html.as_deref(),
        )
        .await
    {
        Ok(_) => {
            tracing::info!(collection_id = %collection_id, "Collection updated");
            Redirect::to("/collections").into_response()
        }
        Err(e) => {
            tracing::error!(collection_id = %collection_id, error = %e, "Failed to update collection");
            match state.shopify().get_collection(&collection_id).await {
                Ok(Some(collection)) => {
                    let description_html = collection.description_html.clone().unwrap_or_default();
                    let template = CollectionEditTemplate {
                        admin_user: AdminUserView::from(&admin),
                        current_path: "/collections".to_string(),
                        collection: CollectionView::from(&collection),
                        description_html,
                        error: Some(e.to_string()),
                    };

                    Html(template.render().unwrap_or_else(|e| {
                        tracing::error!("Template render error: {}", e);
                        "Internal Server Error".to_string()
                    }))
                    .into_response()
                }
                _ => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to update collection",
                )
                    .into_response(),
            }
        }
    }
}

/// Delete collection handler.
#[instrument(skip(_admin, state))]
pub async fn delete(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let collection_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Collection/{id}")
    };

    match state.shopify().delete_collection(&collection_id).await {
        Ok(_) => {
            tracing::info!(collection_id = %collection_id, "Collection deleted");
            Redirect::to("/collections").into_response()
        }
        Err(e) => {
            tracing::error!(collection_id = %collection_id, error = %e, "Failed to delete collection");
            (StatusCode::BAD_REQUEST, format!("Failed to delete: {e}")).into_response()
        }
    }
}
