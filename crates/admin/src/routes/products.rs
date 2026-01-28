//! Products list and management route handlers.

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
        .update_product(
            &product_id,
            ProductUpdateInput {
                title: Some(&input.title),
                description_html: input.description_html.as_deref(),
                vendor: input.vendor.as_deref(),
                product_type: input.product_type.as_deref(),
                tags: Some(tags),
                status: Some(&input.status),
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
            // Re-fetch product for the form
            match state.shopify().get_product(&product_id).await {
                Ok(Some(product)) => {
                    let template = ProductEditTemplate {
                        admin_user: AdminUserView::from(&admin),
                        current_path: "/products".to_string(),
                        product: ProductDetailView::from(&product),
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
                    "Failed to update product",
                )
                    .into_response(),
            }
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

    match state
        .shopify()
        .update_product(
            &product_id,
            ProductUpdateInput {
                status: Some("ARCHIVED"),
                ..Default::default()
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
