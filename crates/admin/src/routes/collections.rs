//! Collections management route handlers.

use askama::Template;
use axum::{
    Form, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use serde::Deserialize;
use tracing::instrument;

use naked_pineapple_core::AdminRole;

use crate::{
    filters,
    middleware::auth::{RequireAdminAuth, RequireSuperAdmin},
    shopify::types::{Collection, CollectionProduct, CollectionRuleSet, CollectionSeo},
    state::AppState,
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
    /// Whether the collection should be published (visible to customers).
    pub published: Option<String>,
    /// Sort order for products in the collection.
    pub sort_order: Option<String>,
    /// SEO title for search engines.
    pub seo_title: Option<String>,
    /// SEO meta description.
    pub seo_description: Option<String>,
}

/// Form input for adding/removing products.
#[derive(Debug, Deserialize)]
pub struct ProductsFormInput {
    pub product_ids: Vec<String>,
}

/// Collection view for templates (list view).
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

/// Collection image view for templates.
#[derive(Debug, Clone)]
pub struct CollectionImageView {
    pub id: String,
    pub url: String,
    pub alt_text: Option<String>,
}

/// Collection detail view for show template.
#[derive(Debug, Clone)]
pub struct CollectionDetailView {
    pub id: String,
    pub title: String,
    pub handle: String,
    pub description: String,
    pub description_html: String,
    pub products_count: i64,
    pub image: Option<CollectionImageView>,
    pub updated_at: Option<String>,
    pub rule_set: Option<CollectionRuleSet>,
    pub sort_order: String,
    pub seo: CollectionSeo,
    pub published_on_current_publication: bool,
}

impl From<&Collection> for CollectionDetailView {
    fn from(collection: &Collection) -> Self {
        Self {
            id: collection.id.clone(),
            title: collection.title.clone(),
            handle: collection.handle.clone(),
            description: collection.description.clone(),
            description_html: collection.description_html.clone().unwrap_or_default(),
            products_count: collection.products_count,
            image: collection.image.as_ref().map(|img| CollectionImageView {
                id: img.id.clone().unwrap_or_default(),
                url: img.url.clone(),
                alt_text: img.alt_text.clone(),
            }),
            updated_at: collection.updated_at.clone(),
            rule_set: collection.rule_set.clone(),
            sort_order: collection
                .sort_order
                .clone()
                .unwrap_or_else(|| "MANUAL".to_string()),
            seo: collection.seo.clone().unwrap_or_default(),
            published_on_current_publication: collection
                .published_on_current_publication
                .unwrap_or(false),
        }
    }
}

/// Product view for collection show template.
#[derive(Debug, Clone)]
pub struct CollectionProductView {
    pub id: String,
    pub title: String,
    pub handle: String,
    pub status: String,
    pub status_class: String,
    pub image_url: Option<String>,
    pub inventory: i64,
    pub price: String,
}

impl From<&CollectionProduct> for CollectionProductView {
    fn from(product: &CollectionProduct) -> Self {
        let status_class = match product.status.as_str() {
            "Active" | "ACTIVE" => "bg-emerald-500/20 text-emerald-400",
            "Draft" | "DRAFT" => "bg-amber-500/20 text-amber-400",
            _ => "bg-zinc-500/20 text-zinc-400",
        };

        Self {
            id: product.id.clone(),
            title: product.title.clone(),
            handle: product.handle.clone(),
            status: product.status.clone(),
            status_class: status_class.to_string(),
            image_url: product.image_url.clone(),
            inventory: product.total_inventory,
            price: format!("${:.2}", product.price.parse::<f64>().unwrap_or(0.0)),
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
    pub collection: CollectionDetailView,
    pub description_html: String,
    pub error: Option<String>,
}

/// Collection show page template.
#[derive(Template)]
#[template(path = "collections/show.html")]
pub struct CollectionShowTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub collection: CollectionDetailView,
    pub products: Vec<CollectionProductView>,
    pub has_more_products: bool,
    pub end_cursor: Option<String>,
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

/// Show collection details page handler.
#[instrument(skip(admin, state))]
pub async fn show(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let collection_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/Collection/{id}")
    };

    match state
        .shopify()
        .get_collection_with_products(&collection_id, 20, None)
        .await
    {
        Ok(Some(data)) => {
            let products: Vec<CollectionProductView> = data
                .products
                .iter()
                .map(CollectionProductView::from)
                .collect();

            let template = CollectionShowTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/collections".to_string(),
                collection: CollectionDetailView::from(&data.collection),
                products,
                has_more_products: data.has_next_page,
                end_cursor: data.end_cursor,
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
            // Redirect to show page instead of edit
            Redirect::to(&format!("/collections/{numeric_id}")).into_response()
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
                collection: CollectionDetailView::from(&collection),
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

/// Handle collection visibility changes (publish/unpublish).
async fn handle_visibility_change(state: &AppState, collection_id: &str, published_str: &str) {
    let should_be_published = published_str == "true";

    // Get current collection to check if visibility changed
    let Ok(Some(collection)) = state.shopify().get_collection(collection_id).await else {
        return;
    };

    let is_currently_published = collection.published_on_current_publication.unwrap_or(true);
    if should_be_published == is_currently_published {
        return;
    }

    // Visibility change requested
    if should_be_published {
        match state.shopify().publish_collection(collection_id).await {
            Ok(()) => tracing::info!(collection_id = %collection_id, "Collection published"),
            Err(e) => {
                tracing::error!(collection_id = %collection_id, error = %e, "Failed to publish collection");
            }
        }
    } else {
        match state.shopify().unpublish_collection(collection_id).await {
            Ok(()) => tracing::info!(collection_id = %collection_id, "Collection unpublished"),
            Err(e) => {
                tracing::error!(collection_id = %collection_id, error = %e, "Failed to unpublish collection");
            }
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

    // Check if user is trying to unpublish without super_admin privileges
    let wants_unpublished = input.published.as_deref() == Some("false");
    let is_super_admin = admin.role == AdminRole::SuperAdmin;
    if wants_unpublished && !is_super_admin {
        tracing::warn!(
            collection_id = %collection_id,
            admin_id = %admin.id,
            "Non-super-admin attempted to unpublish collection"
        );
        return (
            StatusCode::FORBIDDEN,
            "Only super admins can unpublish collections",
        )
            .into_response();
    }

    match state
        .shopify()
        .update_collection(
            &collection_id,
            Some(&input.title),
            input.description_html.as_deref(),
            input.sort_order.as_deref(),
            input.seo_title.as_deref(),
            input.seo_description.as_deref(),
        )
        .await
    {
        Ok(_) => {
            // Handle visibility change if specified
            if let Some(published_str) = &input.published {
                handle_visibility_change(&state, &collection_id, published_str).await;
            }

            tracing::info!(collection_id = %collection_id, "Collection updated");
            // Redirect to show page
            let numeric_id = collection_id
                .split('/')
                .next_back()
                .unwrap_or(&collection_id);
            Redirect::to(&format!("/collections/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(collection_id = %collection_id, error = %e, "Failed to update collection");
            match state.shopify().get_collection(&collection_id).await {
                Ok(Some(collection)) => {
                    let description_html = collection.description_html.clone().unwrap_or_default();
                    let template = CollectionEditTemplate {
                        admin_user: AdminUserView::from(&admin),
                        current_path: "/collections".to_string(),
                        collection: CollectionDetailView::from(&collection),
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

/// Delete collection handler (`super_admin` only).
#[instrument(skip(_admin, state))]
pub async fn delete(
    RequireSuperAdmin(_admin): RequireSuperAdmin,
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

/// Add products to collection handler (HTMX).
#[instrument(skip(admin, state))]
pub async fn add_products(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<ProductsFormInput>,
) -> impl IntoResponse {
    let collection_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Collection/{id}")
    };

    // Ensure product IDs are in GID format
    let product_ids: Vec<String> = input
        .product_ids
        .into_iter()
        .map(|pid| {
            if pid.starts_with("gid://") {
                pid
            } else {
                format!("gid://shopify/Product/{pid}")
            }
        })
        .collect();

    match state
        .shopify()
        .add_products_to_collection(&collection_id, product_ids)
        .await
    {
        Ok(()) => {
            tracing::info!(collection_id = %collection_id, "Products added to collection");
            // Re-fetch and return updated products table
            match state
                .shopify()
                .get_collection_with_products(&collection_id, 20, None)
                .await
            {
                Ok(Some(data)) => {
                    let products: Vec<CollectionProductView> = data
                        .products
                        .iter()
                        .map(CollectionProductView::from)
                        .collect();

                    let template = CollectionShowTemplate {
                        admin_user: AdminUserView::from(&admin),
                        current_path: "/collections".to_string(),
                        collection: CollectionDetailView::from(&data.collection),
                        products,
                        has_more_products: data.has_next_page,
                        end_cursor: data.end_cursor,
                    };

                    Html(template.render().unwrap_or_else(|e| {
                        tracing::error!("Template render error: {}", e);
                        "Internal Server Error".to_string()
                    }))
                    .into_response()
                }
                _ => StatusCode::OK.into_response(),
            }
        }
        Err(e) => {
            tracing::error!(collection_id = %collection_id, error = %e, "Failed to add products");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to add products: {e}"),
            )
                .into_response()
        }
    }
}

/// Remove products from collection handler (HTMX).
#[instrument(skip(admin, state))]
pub async fn remove_products(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<ProductsFormInput>,
) -> impl IntoResponse {
    let collection_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Collection/{id}")
    };

    // Ensure product IDs are in GID format
    let product_ids: Vec<String> = input
        .product_ids
        .into_iter()
        .map(|pid| {
            if pid.starts_with("gid://") {
                pid
            } else {
                format!("gid://shopify/Product/{pid}")
            }
        })
        .collect();

    match state
        .shopify()
        .remove_products_from_collection(&collection_id, product_ids)
        .await
    {
        Ok(()) => {
            tracing::info!(collection_id = %collection_id, "Products removed from collection");
            // Re-fetch and return updated products table
            match state
                .shopify()
                .get_collection_with_products(&collection_id, 20, None)
                .await
            {
                Ok(Some(data)) => {
                    let products: Vec<CollectionProductView> = data
                        .products
                        .iter()
                        .map(CollectionProductView::from)
                        .collect();

                    let template = CollectionShowTemplate {
                        admin_user: AdminUserView::from(&admin),
                        current_path: "/collections".to_string(),
                        collection: CollectionDetailView::from(&data.collection),
                        products,
                        has_more_products: data.has_next_page,
                        end_cursor: data.end_cursor,
                    };

                    Html(template.render().unwrap_or_else(|e| {
                        tracing::error!("Template render error: {}", e);
                        "Internal Server Error".to_string()
                    }))
                    .into_response()
                }
                _ => StatusCode::OK.into_response(),
            }
        }
        Err(e) => {
            tracing::error!(collection_id = %collection_id, error = %e, "Failed to remove products");
            (
                StatusCode::BAD_REQUEST,
                format!("Failed to remove products: {e}"),
            )
                .into_response()
        }
    }
}
