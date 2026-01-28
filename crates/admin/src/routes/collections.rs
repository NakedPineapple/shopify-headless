//! Collections management route handlers.

use std::collections::HashSet;

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
    shopify::types::{
        Collection, CollectionProduct, CollectionRuleSet, CollectionSeo, ResourcePublication,
    },
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
    /// Sort order for products in the collection.
    pub sort_order: Option<String>,
    /// SEO title for search engines.
    pub seo_title: Option<String>,
    /// SEO meta description.
    pub seo_description: Option<String>,
    /// Publication IDs as comma-separated string (parsed in handler).
    #[serde(default)]
    pub publication_ids: Option<String>,
}

impl CollectionFormInput {
    /// Parse `publication_ids` from comma-separated string into a Vec.
    #[must_use]
    pub fn parse_publication_ids(&self) -> Vec<String> {
        self.publication_ids
            .as_deref()
            .unwrap_or("")
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }
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
    pub publications: Vec<ResourcePublication>,
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
            publications: collection.publications.clone(),
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

/// View for a publication/sales channel.
#[derive(Debug, Clone)]
pub struct PublicationView {
    pub id: String,
    pub name: String,
    pub is_published: bool,
}

/// Collection edit form template.
#[derive(Template)]
#[template(path = "collections/edit.html")]
pub struct CollectionEditTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub collection: CollectionDetailView,
    pub description_html: String,
    pub all_publications: Vec<PublicationView>,
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

    // Fetch collection and all available publications in parallel
    let (collection_result, publications_result) = tokio::join!(
        state.shopify().get_collection(&collection_id),
        state.shopify().get_publications()
    );

    let collection = match collection_result {
        Ok(Some(c)) => c,
        Ok(None) => return (StatusCode::NOT_FOUND, "Collection not found").into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch collection: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch collection",
            )
                .into_response();
        }
    };

    let all_pubs = publications_result.unwrap_or_default();

    // Build publication views with current published status
    let published_ids: std::collections::HashSet<&str> = collection
        .publications
        .iter()
        .filter(|p| p.is_published)
        .map(|p| p.publication.id.as_str())
        .collect();

    let all_publications: Vec<PublicationView> = all_pubs
        .into_iter()
        .map(|p| PublicationView {
            is_published: published_ids.contains(p.id.as_str()),
            id: p.id,
            name: p.name,
        })
        .collect();

    let description_html = collection.description_html.clone().unwrap_or_default();
    let template = CollectionEditTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/collections".to_string(),
        collection: CollectionDetailView::from(&collection),
        description_html,
        all_publications,
        error: None,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
    .into_response()
}

/// Handle collection publication changes across multiple sales channels.
async fn handle_publication_changes(
    state: &AppState,
    collection_id: &str,
    current_publications: &[crate::shopify::types::ResourcePublication],
    desired_publication_ids: &[String],
) {
    use std::collections::HashSet;

    // Get currently published channel IDs
    let currently_published: HashSet<&str> = current_publications
        .iter()
        .filter(|p| p.is_published)
        .map(|p| p.publication.id.as_str())
        .collect();

    // Get desired published channel IDs
    let desired: HashSet<&str> = desired_publication_ids.iter().map(String::as_str).collect();

    // Channels to publish to (in desired but not currently published)
    let to_publish: Vec<String> = desired
        .difference(&currently_published)
        .map(|s| (*s).to_string())
        .collect();

    // Channels to unpublish from (currently published but not in desired)
    let to_unpublish: Vec<String> = currently_published
        .difference(&desired)
        .map(|s| (*s).to_string())
        .collect();

    // Publish to new channels
    if !to_publish.is_empty() {
        match state
            .shopify()
            .publish_collection(collection_id, &to_publish)
            .await
        {
            Ok(()) => {
                tracing::info!(
                    collection_id = %collection_id,
                    channels = ?to_publish,
                    "Collection published to channels"
                );
            }
            Err(e) => {
                tracing::error!(
                    collection_id = %collection_id,
                    channels = ?to_publish,
                    error = %e,
                    "Failed to publish collection"
                );
            }
        }
    }

    // Unpublish from removed channels
    if !to_unpublish.is_empty() {
        match state
            .shopify()
            .unpublish_collection(collection_id, &to_unpublish)
            .await
        {
            Ok(()) => {
                tracing::info!(
                    collection_id = %collection_id,
                    channels = ?to_unpublish,
                    "Collection unpublished from channels"
                );
            }
            Err(e) => {
                tracing::error!(
                    collection_id = %collection_id,
                    channels = ?to_unpublish,
                    error = %e,
                    "Failed to unpublish collection"
                );
            }
        }
    }
}

/// Check if user is attempting to unpublish from any channel they don't have permission for.
fn is_trying_to_unpublish(
    current_publications: &[ResourcePublication],
    desired_ids: &[String],
) -> bool {
    let currently_published_ids: HashSet<&str> = current_publications
        .iter()
        .filter(|p| p.is_published)
        .map(|p| p.publication.id.as_str())
        .collect();

    let desired_set: HashSet<&str> = desired_ids.iter().map(String::as_str).collect();

    // If any currently published channel is not in desired, they're trying to unpublish
    currently_published_ids
        .difference(&desired_set)
        .next()
        .is_some()
}

/// Render collection edit form with an error message.
async fn render_edit_error(
    state: &AppState,
    admin: &crate::models::session::CurrentAdmin,
    collection: &Collection,
    error: String,
) -> axum::response::Response {
    let all_pubs = state.shopify().get_publications().await.unwrap_or_default();
    let published_ids: HashSet<&str> = collection
        .publications
        .iter()
        .filter(|p| p.is_published)
        .map(|p| p.publication.id.as_str())
        .collect();

    let all_publications: Vec<PublicationView> = all_pubs
        .into_iter()
        .map(|p| PublicationView {
            is_published: published_ids.contains(p.id.as_str()),
            id: p.id,
            name: p.name,
        })
        .collect();

    let description_html = collection.description_html.clone().unwrap_or_default();
    let template = CollectionEditTemplate {
        admin_user: AdminUserView::from(admin),
        current_path: "/collections".to_string(),
        collection: CollectionDetailView::from(collection),
        description_html,
        all_publications,
        error: Some(error),
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
    .into_response()
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

    // Get current collection to check publication changes
    let current_collection = match state.shopify().get_collection(&collection_id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, "Collection not found").into_response();
        }
        Err(e) => {
            tracing::error!(collection_id = %collection_id, error = %e, "Failed to fetch collection");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to fetch collection",
            )
                .into_response();
        }
    };

    // Parse publication IDs from comma-separated hidden field
    let publication_ids = input.parse_publication_ids();

    // Check if non-super-admin is trying to unpublish
    if admin.role != AdminRole::SuperAdmin
        && is_trying_to_unpublish(&current_collection.publications, &publication_ids)
    {
        tracing::warn!(
            collection_id = %collection_id,
            admin_id = %admin.id,
            "Non-super-admin attempted to unpublish collection from channels"
        );
        return (
            StatusCode::FORBIDDEN,
            "Only super admins can unpublish collections from sales channels",
        )
            .into_response();
    }

    // Merge form input with current values to avoid sending nulls
    // (workaround for graphql_client skip_none bug)
    let current_seo = current_collection.seo.as_ref();
    let title = input.title.clone();
    let description_html = input
        .description_html
        .clone()
        .or_else(|| current_collection.description_html.clone())
        .unwrap_or_default();
    let sort_order = input
        .sort_order
        .clone()
        .or_else(|| current_collection.sort_order.clone())
        .unwrap_or_else(|| "MANUAL".to_string());
    let seo_title = input
        .seo_title
        .clone()
        .or_else(|| current_seo.and_then(|s| s.title.clone()))
        .unwrap_or_default();
    let seo_description = input
        .seo_description
        .clone()
        .or_else(|| current_seo.and_then(|s| s.description.clone()))
        .unwrap_or_default();

    match state
        .shopify()
        .update_collection(
            &collection_id,
            Some(&title),
            Some(&description_html),
            Some(&sort_order),
            Some(&seo_title),
            Some(&seo_description),
        )
        .await
    {
        Ok(_) => {
            handle_publication_changes(
                &state,
                &collection_id,
                &current_collection.publications,
                &publication_ids,
            )
            .await;

            tracing::info!(collection_id = %collection_id, "Collection updated");
            let numeric_id = collection_id
                .split('/')
                .next_back()
                .unwrap_or(&collection_id);
            Redirect::to(&format!("/collections/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(collection_id = %collection_id, error = %e, "Failed to update collection");
            render_edit_error(&state, &admin, &current_collection, e.to_string()).await
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

// ============================================================================
// Sort Order Update
// ============================================================================

/// Input for updating sort order.
#[derive(Debug, Deserialize)]
pub struct SortOrderInput {
    pub sort_order: String,
}

/// Update collection sort order handler (JSON).
#[instrument(skip(_admin, state))]
pub async fn update_sort_order(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<SortOrderInput>,
) -> impl IntoResponse {
    let collection_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Collection/{id}")
    };

    match state
        .shopify()
        .update_collection_sort_order(&collection_id, &input.sort_order)
        .await
    {
        Ok(()) => {
            tracing::info!(collection_id = %collection_id, sort_order = %input.sort_order, "Collection sort order updated");
            (
                StatusCode::OK,
                Json(serde_json::json!({"success": true, "sort_order": input.sort_order})),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(collection_id = %collection_id, error = %e, "Failed to update sort order");
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Failed to update sort order: {e}")})),
            )
                .into_response()
        }
    }
}

// ============================================================================
// Product Reordering
// ============================================================================

/// Input for reordering products in a collection.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductMoveInput {
    /// The product ID (full GID or numeric).
    pub id: String,
    /// The new position (0-indexed).
    pub new_position: i64,
}

/// Reorder products in a collection handler (HTMX/JSON).
#[instrument(skip(_admin, state))]
pub async fn reorder_products(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(moves): Json<Vec<ProductMoveInput>>,
) -> impl IntoResponse {
    let collection_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Collection/{id}")
    };

    // Convert moves to the format expected by the API
    let move_tuples: Vec<(String, i64)> = moves
        .into_iter()
        .map(|m| {
            let product_id = if m.id.starts_with("gid://") {
                m.id
            } else {
                format!("gid://shopify/Product/{}", m.id)
            };
            (product_id, m.new_position)
        })
        .collect();

    match state
        .shopify()
        .reorder_collection_products(&collection_id, move_tuples)
        .await
    {
        Ok(()) => {
            tracing::info!(collection_id = %collection_id, "Products reordered in collection");
            (StatusCode::OK, Json(serde_json::json!({"success": true}))).into_response()
        }
        Err(e) => {
            tracing::error!(collection_id = %collection_id, error = %e, "Failed to reorder products");
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Failed to reorder products: {e}")})),
            )
                .into_response()
        }
    }
}

// ============================================================================
// Image Management
// ============================================================================

/// Extracted file from multipart upload.
struct ExtractedFile {
    filename: String,
    content_type: String,
    bytes: Vec<u8>,
}

/// Extract file from multipart form data.
async fn extract_file_from_multipart(
    mut multipart: axum::extract::Multipart,
) -> Result<ExtractedFile, &'static str> {
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            let filename = field.file_name().unwrap_or("image.jpg").to_string();
            let content_type = field.content_type().unwrap_or("image/jpeg").to_string();

            match field.bytes().await {
                Ok(bytes) => {
                    return Ok(ExtractedFile {
                        filename,
                        content_type,
                        bytes: bytes.to_vec(),
                    });
                }
                Err(e) => {
                    tracing::error!(error = %e, "Failed to read file bytes");
                    return Err("Failed to read file");
                }
            }
        }
    }
    Err("No file provided")
}

/// Upload file bytes to Shopify's staged upload target.
async fn upload_to_staged_target(
    staged_target: &crate::shopify::StagedUploadTarget,
    filename: &str,
    content_type: &str,
    bytes: Vec<u8>,
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let mut form = reqwest::multipart::Form::new();

    // Add parameters from staged upload
    for (name, value) in &staged_target.parameters {
        form = form.text(name.clone(), value.clone());
    }

    // Add the file
    let file_part = reqwest::multipart::Part::bytes(bytes)
        .file_name(filename.to_string())
        .mime_str(content_type)
        .unwrap_or_else(|_| {
            reqwest::multipart::Part::bytes(vec![]).file_name(filename.to_string())
        });
    form = form.part("file", file_part);

    match client.post(&staged_target.url).multipart(form).send().await {
        Ok(response) => {
            if response.status().is_success() {
                Ok(())
            } else {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                tracing::error!(status = %status, body = %body, "Staged upload failed");
                Err(format!("Upload failed with status {status}"))
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to upload to staged target");
            Err(format!("Upload failed: {e}"))
        }
    }
}

/// Upload collection image handler.
#[instrument(skip(_admin, state))]
pub async fn upload_image(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    multipart: axum::extract::Multipart,
) -> impl IntoResponse {
    let collection_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Collection/{id}")
    };

    // Step 1: Extract file from multipart
    let file = match extract_file_from_multipart(multipart).await {
        Ok(f) => f,
        Err(msg) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": msg})),
            )
                .into_response();
        }
    };

    let file_size = i64::try_from(file.bytes.len()).unwrap_or(i64::MAX);

    // Step 2: Create staged upload target
    let staged_target = match state
        .shopify()
        .create_staged_upload(&file.filename, &file.content_type, file_size, "IMAGE")
        .await
    {
        Ok(target) => target,
        Err(e) => {
            tracing::error!(error = %e, "Failed to create staged upload");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to create upload: {e}")})),
            )
                .into_response();
        }
    };

    // Step 3: Upload file to staged target
    if let Err(msg) = upload_to_staged_target(
        &staged_target,
        &file.filename,
        &file.content_type,
        file.bytes,
    )
    .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": msg})),
        )
            .into_response();
    }

    // Step 4: Update collection with new image
    match state
        .shopify()
        .update_collection_image(&collection_id, &staged_target.resource_url, None)
        .await
    {
        Ok(()) => {
            tracing::info!(collection_id = %collection_id, filename = %file.filename, "Collection image uploaded");
            (
                StatusCode::OK,
                Json(serde_json::json!({"success": true, "filename": file.filename})),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to update collection image");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to update image: {e}")})),
            )
                .into_response()
        }
    }
}

/// Delete collection image handler.
#[instrument(skip(_admin, state))]
pub async fn delete_image(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let collection_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Collection/{id}")
    };

    match state
        .shopify()
        .delete_collection_image(&collection_id)
        .await
    {
        Ok(()) => {
            tracing::info!(collection_id = %collection_id, "Collection image deleted");
            (StatusCode::OK, Json(serde_json::json!({"success": true}))).into_response()
        }
        Err(e) => {
            tracing::error!(collection_id = %collection_id, error = %e, "Failed to delete image");
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": format!("Failed to delete image: {e}")})),
            )
                .into_response()
        }
    }
}
