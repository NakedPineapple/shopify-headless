//! Products list and management route handlers.

use askama::Template;
use axum::{
    Form, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
};
use serde::Deserialize;
use tracing::instrument;

use crate::{
    filters,
    middleware::auth::{RequireAdminAuth, RequireSuperAdmin},
    models::CurrentAdmin,
    shopify::{
        ProductUpdateInput,
        types::{AdminProduct, Money, ProductStatus},
    },
    state::AppState,
};

use naked_pineapple_core::AdminRole;

use super::dashboard::AdminUserView;

/// Pagination query parameters.
#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub cursor: Option<String>,
    pub query: Option<String>,
}

/// Product view for templates.
#[derive(Debug, Clone)]
pub struct ProductView {
    pub id: String,
    pub title: String,
    pub status: String,
    pub status_class: String,
    pub inventory: i64,
    pub price: String,
    pub image_url: Option<String>,
    pub handle: String,
}

// =============================================================================
// Type Conversions
// =============================================================================

/// Format a Shopify Money type as a price string.
fn format_price(money: &Money) -> String {
    money.amount.parse::<f64>().map_or_else(
        |_| format!("${}", money.amount),
        |amount| format!("${amount:.2}"),
    )
}

impl From<&AdminProduct> for ProductView {
    fn from(product: &AdminProduct) -> Self {
        let (status, status_class) = match product.status {
            ProductStatus::Active => (
                "Active",
                "bg-green-500/10 text-green-600 dark:text-green-400 ring-1 ring-inset ring-green-500/20",
            ),
            ProductStatus::Draft => (
                "Draft",
                "bg-yellow-500/10 text-yellow-600 dark:text-yellow-400 ring-1 ring-inset ring-yellow-500/20",
            ),
            ProductStatus::Archived => (
                "Archived",
                "bg-zinc-500/10 text-zinc-600 dark:text-zinc-400 ring-1 ring-inset ring-zinc-500/20",
            ),
            ProductStatus::Unlisted => (
                "Unlisted",
                "bg-blue-500/10 text-blue-600 dark:text-blue-400 ring-1 ring-inset ring-blue-500/20",
            ),
        };

        // Get price from first variant
        let price = product
            .variants
            .first()
            .map_or_else(|| "$0.00".to_string(), |v| format_price(&v.price));

        Self {
            id: product.id.clone(),
            title: product.title.clone(),
            status: status.to_string(),
            status_class: status_class.to_string(),
            inventory: product.total_inventory,
            price,
            image_url: product.featured_image.as_ref().map(|img| img.url.clone()),
            handle: product.handle.clone(),
        }
    }
}

/// Form input for creating/updating products.
#[derive(Debug, Deserialize)]
pub struct ProductFormInput {
    pub title: String,
    pub description_html: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub tags: Option<String>, // comma-separated
    pub status: String,
}

/// Products list page template.
#[derive(Template)]
#[template(path = "products/index.html")]
pub struct ProductsIndexTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub products: Vec<ProductView>,
    pub has_next_page: bool,
    pub next_cursor: Option<String>,
    pub search_query: Option<String>,
}

/// Product detail page template.
#[derive(Template)]
#[template(path = "products/show.html")]
pub struct ProductShowTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub product: ProductDetailView,
}

/// Product create form template.
#[derive(Template)]
#[template(path = "products/new.html")]
pub struct ProductNewTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub error: Option<String>,
}

/// Product edit form template.
#[derive(Template)]
#[template(path = "products/edit.html")]
pub struct ProductEditTemplate {
    pub admin_user: AdminUserView,
    pub current_path: String,
    pub product: ProductDetailView,
    pub error: Option<String>,
}

/// Detailed product view for detail/edit pages.
#[derive(Debug, Clone)]
pub struct ProductDetailView {
    pub id: String,
    pub title: String,
    pub description_html: String,
    pub status: String,
    pub status_class: String,
    pub vendor: String,
    pub product_type: String,
    pub tags: String,
    pub inventory: i64,
    pub price: String,
    pub image_url: Option<String>,
    pub images: Vec<ImageView>,
    pub handle: String,
    pub variants: Vec<VariantView>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// Image view for templates.
#[derive(Debug, Clone)]
pub struct ImageView {
    pub id: String,
    pub url: String,
    pub alt: Option<String>,
}

/// Variant view for templates.
#[derive(Debug, Clone)]
pub struct VariantView {
    pub id: String,
    pub title: String,
    pub sku: Option<String>,
    pub barcode: Option<String>,
    pub price: String,
    pub compare_at_price: Option<String>,
    pub inventory_quantity: i64,
}

impl From<&AdminProduct> for ProductDetailView {
    fn from(product: &AdminProduct) -> Self {
        let (status, status_class) = match product.status {
            ProductStatus::Active => (
                "ACTIVE",
                "bg-green-500/10 text-green-600 dark:text-green-400 ring-1 ring-inset ring-green-500/20",
            ),
            ProductStatus::Draft => (
                "DRAFT",
                "bg-yellow-500/10 text-yellow-600 dark:text-yellow-400 ring-1 ring-inset ring-yellow-500/20",
            ),
            ProductStatus::Archived => (
                "ARCHIVED",
                "bg-zinc-500/10 text-zinc-600 dark:text-zinc-400 ring-1 ring-inset ring-zinc-500/20",
            ),
            ProductStatus::Unlisted => (
                "UNLISTED",
                "bg-blue-500/10 text-blue-600 dark:text-blue-400 ring-1 ring-inset ring-blue-500/20",
            ),
        };

        let price = product
            .variants
            .first()
            .map_or_else(|| "$0.00".to_string(), |v| format_price(&v.price));

        let variants: Vec<VariantView> = product
            .variants
            .iter()
            .map(|v| VariantView {
                id: v.id.clone(),
                title: v.title.clone(),
                sku: v.sku.clone(),
                barcode: v.barcode.clone(),
                price: v.price.amount.clone(),
                compare_at_price: v.compare_at_price.as_ref().map(|p| p.amount.clone()),
                inventory_quantity: v.inventory_quantity,
            })
            .collect();

        let images: Vec<ImageView> = product
            .images
            .iter()
            .map(|img| ImageView {
                id: img.id.clone().unwrap_or_default(),
                url: img.url.clone(),
                alt: img.alt_text.clone(),
            })
            .collect();

        Self {
            id: product.id.clone(),
            title: product.title.clone(),
            description_html: product.description_html.clone(),
            status: status.to_string(),
            status_class: status_class.to_string(),
            vendor: product.vendor.clone(),
            product_type: product.kind.clone(),
            tags: product.tags.join(", "),
            inventory: product.total_inventory,
            price,
            image_url: product.featured_image.as_ref().map(|img| img.url.clone()),
            images,
            handle: product.handle.clone(),
            variants,
            created_at: product.created_at.clone(),
            updated_at: product.updated_at.clone(),
        }
    }
}

/// Products list page handler.
#[instrument(skip(admin, state))]
pub async fn index(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Query(query): Query<PaginationQuery>,
) -> Html<String> {
    let result = state
        .shopify()
        .get_products(25, query.cursor.clone(), query.query.clone())
        .await;

    let (products, has_next_page, next_cursor) = match result {
        Ok(conn) => {
            let products: Vec<ProductView> = conn.products.iter().map(ProductView::from).collect();
            (
                products,
                conn.page_info.has_next_page,
                conn.page_info.end_cursor,
            )
        }
        Err(e) => {
            tracing::error!("Failed to fetch products: {e}");
            (vec![], false, None)
        }
    };

    let template = ProductsIndexTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/products".to_string(),
        products,
        has_next_page,
        next_cursor,
        search_query: query.query,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// Product detail page handler.
#[instrument(skip(admin, state))]
pub async fn show(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Ensure ID has the proper Shopify format
    let product_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/Product/{id}")
    };

    match state.shopify().get_product(&product_id).await {
        Ok(Some(product)) => {
            let template = ProductShowTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/products".to_string(),
                product: ProductDetailView::from(&product),
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Product not found").into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch product: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch product").into_response()
        }
    }
}

/// New product form handler.
#[instrument(skip(admin))]
pub async fn new_product(RequireAdminAuth(admin): RequireAdminAuth) -> Html<String> {
    let template = ProductNewTemplate {
        admin_user: AdminUserView::from(&admin),
        current_path: "/products".to_string(),
        error: None,
    };

    Html(template.render().unwrap_or_else(|e| {
        tracing::error!("Template render error: {}", e);
        "Internal Server Error".to_string()
    }))
}

/// Create product handler.
#[instrument(skip(admin, state))]
pub async fn create(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Form(input): Form<ProductFormInput>,
) -> impl IntoResponse {
    // Parse tags from comma-separated string
    let tags: Vec<String> = input
        .tags
        .as_deref()
        .unwrap_or("")
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    match state
        .shopify()
        .create_product(
            &input.title,
            input.description_html.as_deref(),
            input.vendor.as_deref(),
            input.product_type.as_deref(),
            tags,
            &input.status,
        )
        .await
    {
        Ok(product_id) => {
            tracing::info!(product_id = %product_id, title = %input.title, "Product created");
            // Extract numeric ID for redirect
            let numeric_id = product_id.split('/').next_back().unwrap_or(&product_id);
            Redirect::to(&format!("/products/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(title = %input.title, error = %e, "Failed to create product");
            let template = ProductNewTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/products".to_string(),
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

/// Edit product form handler.
#[instrument(skip(admin, state))]
pub async fn edit(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let product_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/Product/{id}")
    };

    match state.shopify().get_product(&product_id).await {
        Ok(Some(product)) => {
            let template = ProductEditTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/products".to_string(),
                product: ProductDetailView::from(&product),
                error: None,
            };

            Html(template.render().unwrap_or_else(|e| {
                tracing::error!("Template render error: {}", e);
                "Internal Server Error".to_string()
            }))
            .into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Product not found").into_response(),
        Err(e) => {
            tracing::error!("Failed to fetch product: {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch product").into_response()
        }
    }
}

/// Update product handler.
#[instrument(skip(admin, state))]
pub async fn update(
    RequireAdminAuth(admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(input): Form<ProductFormInput>,
) -> impl IntoResponse {
    let product_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Product/{id}")
    };

    // Fetch current product to merge values (workaround for graphql_client skip_none bug)
    let current_product = match state.shopify().get_product(&product_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, "Product not found").into_response();
        }
        Err(e) => {
            tracing::error!(product_id = %product_id, error = %e, "Failed to fetch product");
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch product").into_response();
        }
    };

    // Parse tags from comma-separated string, fall back to current tags
    let tags: Vec<String> = if let Some(ref tag_str) = input.tags {
        tag_str
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        current_product.tags.clone()
    };

    // Merge form input with current values - always send values, never nulls
    let title = input.title.clone();
    let description_html = input
        .description_html
        .clone()
        .unwrap_or_else(|| current_product.description_html.clone());
    let vendor = input
        .vendor
        .clone()
        .unwrap_or_else(|| current_product.vendor.clone());
    let product_type = input
        .product_type
        .clone()
        .unwrap_or_else(|| current_product.kind.clone());
    let status = input.status.clone();

    match state
        .shopify()
        .update_product(
            &product_id,
            ProductUpdateInput {
                title: Some(&title),
                description_html: Some(&description_html),
                vendor: Some(&vendor),
                product_type: Some(&product_type),
                tags: Some(tags),
                status: Some(&status),
            },
        )
        .await
    {
        Ok(_) => {
            tracing::info!(product_id = %product_id, "Product updated");
            let numeric_id = id.split('/').next_back().unwrap_or(&id);
            Redirect::to(&format!("/products/{numeric_id}")).into_response()
        }
        Err(e) => {
            tracing::error!(product_id = %product_id, error = %e, "Failed to update product");
            let template = ProductEditTemplate {
                admin_user: AdminUserView::from(&admin),
                current_path: "/products".to_string(),
                product: ProductDetailView::from(&current_product),
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

/// Archive product handler (HTMX).
/// Requires `super_admin` role.
#[instrument(skip(_admin, state))]
pub async fn archive(
    RequireSuperAdmin(_admin): RequireSuperAdmin,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let product_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Product/{id}")
    };

    // Fetch current product to merge values (workaround for graphql_client skip_none bug)
    let current_product = match state.shopify().get_product(&product_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, Html("Product not found".to_string())).into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html(format!("Failed to fetch product: {e}")),
            )
                .into_response();
        }
    };

    match state
        .shopify()
        .update_product(
            &product_id,
            ProductUpdateInput {
                title: Some(&current_product.title),
                description_html: Some(&current_product.description_html),
                vendor: Some(&current_product.vendor),
                product_type: Some(&current_product.kind),
                tags: Some(current_product.tags.clone()),
                status: Some("ARCHIVED"),
            },
        )
        .await
    {
        Ok(_) => {
            tracing::info!(product_id = %product_id, "Product archived");
            (
                StatusCode::OK,
                [("HX-Trigger", "product-archived")],
                Html(
                    r#"<span class="text-green-600 dark:text-green-400">Archived</span>"#
                        .to_string(),
                ),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(product_id = %product_id, error = %e, "Failed to archive product");
            (
                StatusCode::BAD_REQUEST,
                Html(format!(
                    r#"<span class="text-red-600 dark:text-red-400">Error: {e}</span>"#
                )),
            )
                .into_response()
        }
    }
}

/// Delete product handler.
/// Requires `super_admin` role.
#[instrument(skip(_admin, state))]
pub async fn delete(
    RequireSuperAdmin(_admin): RequireSuperAdmin,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let product_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Product/{id}")
    };

    match state.shopify().delete_product(&product_id).await {
        Ok(_) => {
            tracing::info!(product_id = %product_id, "Product deleted");
            Redirect::to("/products").into_response()
        }
        Err(e) => {
            tracing::error!(product_id = %product_id, error = %e, "Failed to delete product");
            (StatusCode::BAD_REQUEST, format!("Failed to delete: {e}")).into_response()
        }
    }
}

// ============================================================================
// Variant Update
// ============================================================================

/// Form data for variant update.
#[derive(Debug, Clone, Deserialize)]
pub struct VariantFormInput {
    pub price: Option<String>,
    pub compare_at_price: Option<String>,
    pub sku: Option<String>,
    pub barcode: Option<String>,
}

/// Update variant handler (HTMX).
#[instrument(skip(_admin, state))]
pub async fn update_variant(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path((product_id, variant_id)): Path<(String, String)>,
    Form(input): Form<VariantFormInput>,
) -> impl IntoResponse {
    let full_product_id = if product_id.starts_with("gid://") {
        product_id.clone()
    } else {
        format!("gid://shopify/Product/{product_id}")
    };

    let full_variant_id = if variant_id.starts_with("gid://") {
        variant_id.clone()
    } else {
        format!("gid://shopify/ProductVariant/{variant_id}")
    };

    match state
        .shopify()
        .update_variant(
            &full_product_id,
            &full_variant_id,
            input.price.as_deref(),
            input.compare_at_price.as_deref(),
            input.sku.as_deref(),
            input.barcode.as_deref(),
        )
        .await
    {
        Ok(variant) => {
            tracing::info!(variant_id = %full_variant_id, "Variant updated");
            // Return a success message with updated values
            let html = format!(
                r#"<div class="p-4 bg-green-50 dark:bg-green-900/20 border border-green-200 dark:border-green-800 rounded-lg">
                    <div class="flex items-center gap-2 text-green-600 dark:text-green-400 text-sm">
                        <i class="ph ph-check-circle"></i>
                        <span>Variant updated successfully</span>
                    </div>
                    <div class="mt-2 text-xs text-gray-600 dark:text-gray-400">
                        <div>Price: ${}</div>
                        {}
                        {}
                        {}
                    </div>
                </div>"#,
                variant.price.amount,
                variant
                    .compare_at_price
                    .as_ref()
                    .map_or_else(String::new, |p| format!(
                        "<div>Compare at: ${}</div>",
                        p.amount
                    )),
                variant
                    .sku
                    .as_ref()
                    .map_or_else(String::new, |s| format!("<div>SKU: {s}</div>")),
                variant
                    .barcode
                    .as_ref()
                    .map_or_else(String::new, |b| format!("<div>Barcode: {b}</div>")),
            );
            (StatusCode::OK, Html(html)).into_response()
        }
        Err(e) => {
            tracing::error!(variant_id = %full_variant_id, error = %e, "Failed to update variant");
            let html = format!(
                r#"<div class="p-4 bg-red-50 dark:bg-red-900/20 border border-red-200 dark:border-red-800 rounded-lg">
                    <div class="flex items-center gap-2 text-red-600 dark:text-red-400 text-sm">
                        <i class="ph ph-warning-circle"></i>
                        <span>Error: {e}</span>
                    </div>
                </div>"#
            );
            (StatusCode::BAD_REQUEST, Html(html)).into_response()
        }
    }
}

// ============================================================================
// Image Management
// ============================================================================

/// Delete image from product handler (HTMX).
#[instrument(skip(_admin, state))]
pub async fn delete_image(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path((_, media_id)): Path<(String, String)>,
) -> impl IntoResponse {
    // File IDs use MediaImage prefix
    let full_media_id = if media_id.starts_with("gid://") {
        media_id.clone()
    } else {
        format!("gid://shopify/MediaImage/{media_id}")
    };

    match state
        .shopify()
        .delete_files(vec![full_media_id.clone()])
        .await
    {
        Ok(deleted_ids) => {
            tracing::info!(media_id = %full_media_id, "Image deleted");
            let html = if deleted_ids.is_empty() {
                r#"<div class="text-red-600 dark:text-red-400 text-sm">No images were deleted</div>"#.to_string()
            } else {
                // Return empty string to remove the element
                String::new()
            };
            (
                StatusCode::OK,
                [("HX-Trigger", "image-deleted")],
                Html(html),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(media_id = %full_media_id, error = %e, "Failed to delete image");
            let html =
                format!(r#"<div class="text-red-600 dark:text-red-400 text-sm">Error: {e}</div>"#);
            (StatusCode::BAD_REQUEST, Html(html)).into_response()
        }
    }
}

/// Image move input for reordering.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageMoveInput {
    /// The media ID (full GID or numeric).
    pub id: String,
    /// The new position (0-indexed).
    pub new_position: i64,
}

/// Reorder product images handler.
#[instrument(skip(_admin, state))]
pub async fn reorder_images(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(moves): Json<Vec<ImageMoveInput>>,
) -> impl IntoResponse {
    let product_id = if id.starts_with("gid://") {
        id
    } else {
        format!("gid://shopify/Product/{id}")
    };

    // Convert moves to the format expected by the Shopify client
    let move_tuples: Vec<(String, i64)> = moves
        .into_iter()
        .map(|m| {
            let media_id = if m.id.starts_with("gid://") {
                m.id
            } else {
                format!("gid://shopify/MediaImage/{}", m.id)
            };
            (media_id, m.new_position)
        })
        .collect();

    match state
        .shopify()
        .reorder_product_media(&product_id, move_tuples)
        .await
    {
        Ok(()) => {
            tracing::info!(product_id = %product_id, "Images reordered");
            (StatusCode::OK, Json(serde_json::json!({"success": true}))).into_response()
        }
        Err(e) => {
            tracing::error!(product_id = %product_id, error = %e, "Failed to reorder images");
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

/// Alt text form input.
#[derive(Debug, Deserialize)]
pub struct AltTextInput {
    /// The alt text for the image.
    pub alt_text: String,
}

/// Update image alt text handler.
#[instrument(skip(_admin, state))]
pub async fn update_image_alt(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path((_, media_id)): Path<(String, String)>,
    Form(input): Form<AltTextInput>,
) -> impl IntoResponse {
    // File IDs use MediaImage prefix
    let full_media_id = if media_id.starts_with("gid://") {
        media_id.clone()
    } else {
        format!("gid://shopify/MediaImage/{media_id}")
    };

    match state
        .shopify()
        .update_media_alt_text(&full_media_id, &input.alt_text)
        .await
    {
        Ok(()) => {
            tracing::info!(media_id = %full_media_id, alt = %input.alt_text, "Image alt text updated");
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": true,
                    "media_id": full_media_id,
                    "alt_text": input.alt_text
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(media_id = %full_media_id, error = %e, "Failed to update image alt text");
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

/// Extracted file data from multipart form.
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
                Err("Failed to upload to staging".to_string())
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to upload to staged target");
            Err(format!("Upload failed: {e}"))
        }
    }
}

/// Upload image to product handler.
#[instrument(skip(_admin, state, multipart))]
pub async fn upload_image(
    RequireAdminAuth(_admin): RequireAdminAuth,
    State(state): State<AppState>,
    Path(id): Path<String>,
    multipart: axum::extract::Multipart,
) -> impl IntoResponse {
    let product_id = if id.starts_with("gid://") {
        id.clone()
    } else {
        format!("gid://shopify/Product/{id}")
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

    // Step 4: Attach media to product
    match state
        .shopify()
        .attach_media_to_product(&product_id, &staged_target.resource_url, None)
        .await
    {
        Ok(()) => {
            tracing::info!(product_id = %product_id, filename = %file.filename, "Image uploaded");
            (
                StatusCode::OK,
                Json(serde_json::json!({"success": true, "filename": file.filename})),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, "Failed to attach media to product");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to attach image: {e}")})),
            )
                .into_response()
        }
    }
}
