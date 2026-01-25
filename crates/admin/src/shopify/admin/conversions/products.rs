//! Product type conversion functions.

use crate::shopify::types::{
    AdminProduct, AdminProductConnection, AdminProductVariant, Image, Money, PageInfo,
    ProductStatus,
};

use super::super::queries::{get_product, get_products};
use super::currency_code_to_string;

// =============================================================================
// GetProduct conversions
// =============================================================================

pub fn convert_product(product: get_product::GetProductProduct) -> AdminProduct {
    let status = match product.status {
        get_product::ProductStatus::ACTIVE => ProductStatus::Active,
        get_product::ProductStatus::ARCHIVED => ProductStatus::Archived,
        get_product::ProductStatus::UNLISTED => ProductStatus::Unlisted,
        get_product::ProductStatus::DRAFT | get_product::ProductStatus::Other(_) => {
            ProductStatus::Draft
        }
    };

    AdminProduct {
        id: product.id,
        handle: product.handle,
        title: product.title,
        description: product.description,
        description_html: product.description_html,
        status,
        kind: product.product_type,
        vendor: product.vendor,
        tags: product.tags,
        total_inventory: product.total_inventory,
        created_at: Some(product.created_at),
        updated_at: Some(product.updated_at),
        featured_image: product
            .featured_media
            .and_then(|m| m.preview)
            .and_then(|p| p.image)
            .map(|i| Image {
                id: i.id,
                url: i.url,
                alt_text: i.alt_text,
                width: i.width,
                height: i.height,
            }),
        images: product
            .media
            .edges
            .into_iter()
            .filter_map(|e| {
                e.node.preview.and_then(|p| p.image).map(|i| Image {
                    id: i.id,
                    url: i.url,
                    alt_text: i.alt_text,
                    width: i.width,
                    height: i.height,
                })
            })
            .collect(),
        variants: product
            .variants
            .edges
            .into_iter()
            .map(|e| convert_variant_single(e.node))
            .collect(),
    }
}

fn convert_variant_single(
    v: get_product::GetProductProductVariantsEdgesNode,
) -> AdminProductVariant {
    // Extract image from first media item's preview
    let image = v
        .media
        .edges
        .into_iter()
        .next()
        .and_then(|e| e.node.preview)
        .and_then(|p| p.image)
        .map(|i| Image {
            id: i.id,
            url: i.url,
            alt_text: i.alt_text,
            width: i.width,
            height: i.height,
        });

    AdminProductVariant {
        id: v.id,
        title: v.title,
        sku: v.sku,
        barcode: v.barcode,
        price: Money {
            amount: v.price,
            currency_code: "USD".to_string(), // Price is Money scalar, no currency info
        },
        compare_at_price: v.compare_at_price.map(|p| Money {
            amount: p,
            currency_code: "USD".to_string(),
        }),
        inventory_quantity: v.inventory_quantity.unwrap_or(0),
        inventory_item_id: v.inventory_item.id.clone(),
        inventory_management: Some(format!("{:?}", v.inventory_policy)),
        weight: None, // Weight not included in query
        weight_unit: None,
        requires_shipping: v.inventory_item.requires_shipping,
        image,
        created_at: Some(v.created_at),
        updated_at: Some(v.updated_at),
    }
}

// =============================================================================
// GetProducts conversions
// =============================================================================

pub fn convert_product_connection(
    conn: get_products::GetProductsProducts,
) -> AdminProductConnection {
    AdminProductConnection {
        products: conn
            .edges
            .into_iter()
            .map(|e| convert_products_list_product(e.node))
            .collect(),
        page_info: PageInfo {
            has_next_page: conn.page_info.has_next_page,
            has_previous_page: conn.page_info.has_previous_page,
            start_cursor: conn.page_info.start_cursor,
            end_cursor: conn.page_info.end_cursor,
        },
    }
}

fn convert_products_list_product(
    product: get_products::GetProductsProductsEdgesNode,
) -> AdminProduct {
    let status = match product.status {
        get_products::ProductStatus::ACTIVE => ProductStatus::Active,
        get_products::ProductStatus::ARCHIVED => ProductStatus::Archived,
        get_products::ProductStatus::UNLISTED => ProductStatus::Unlisted,
        get_products::ProductStatus::DRAFT | get_products::ProductStatus::Other(_) => {
            ProductStatus::Draft
        }
    };

    AdminProduct {
        id: product.id,
        handle: product.handle,
        title: product.title,
        description: product.description,
        description_html: product.description_html,
        status,
        kind: product.product_type,
        vendor: product.vendor,
        tags: product.tags,
        total_inventory: product.total_inventory,
        created_at: Some(product.created_at),
        updated_at: Some(product.updated_at),
        featured_image: product
            .featured_media
            .and_then(|m| m.preview)
            .and_then(|p| p.image)
            .map(|i| Image {
                id: i.id,
                url: i.url,
                alt_text: i.alt_text,
                width: i.width,
                height: i.height,
            }),
        images: product
            .media
            .edges
            .into_iter()
            .filter_map(|e| {
                e.node.preview.and_then(|p| p.image).map(|i| Image {
                    id: i.id,
                    url: i.url,
                    alt_text: i.alt_text,
                    width: i.width,
                    height: i.height,
                })
            })
            .collect(),
        variants: product
            .variants
            .edges
            .into_iter()
            .map(|e| convert_products_list_variant(e.node))
            .collect(),
    }
}

fn convert_products_list_variant(
    v: get_products::GetProductsProductsEdgesNodeVariantsEdgesNode,
) -> AdminProductVariant {
    // Extract image from first media item's preview
    let image = v
        .media
        .edges
        .into_iter()
        .next()
        .and_then(|e| e.node.preview)
        .and_then(|p| p.image)
        .map(|i| Image {
            id: i.id,
            url: i.url,
            alt_text: i.alt_text,
            width: i.width,
            height: i.height,
        });

    AdminProductVariant {
        id: v.id,
        title: v.title,
        sku: v.sku,
        barcode: v.barcode,
        price: Money {
            amount: v.price,
            currency_code: "USD".to_string(),
        },
        compare_at_price: v.compare_at_price.map(|p| Money {
            amount: p,
            currency_code: "USD".to_string(),
        }),
        inventory_quantity: v.inventory_quantity.unwrap_or(0),
        inventory_item_id: v.inventory_item.id.clone(),
        inventory_management: Some(format!("{:?}", v.inventory_policy)),
        weight: None, // Weight not included in query
        weight_unit: None,
        requires_shipping: v.inventory_item.requires_shipping,
        image,
        created_at: Some(v.created_at),
        updated_at: Some(v.updated_at),
    }
}
