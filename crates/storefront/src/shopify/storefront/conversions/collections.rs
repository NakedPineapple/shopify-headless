//! Collection type conversion functions.

use crate::shopify::types::{
    Collection, CollectionConnection, Image, Money, PageInfo, PriceRange, Product, ProductVariant,
    Seo,
};

use super::super::queries::{get_collection_by_handle, get_collections};

/// Convert a `CurrencyCode` enum to string.
fn currency_code_to_string<T: std::fmt::Debug>(code: T) -> String {
    format!("{code:?}")
}

// =============================================================================
// get_collection_by_handle conversions
// =============================================================================

pub fn convert_collection(
    collection: get_collection_by_handle::GetCollectionByHandleCollection,
) -> Collection {
    let fields = collection.collection_fields;

    Collection {
        id: fields.id,
        handle: fields.handle,
        title: fields.title,
        description: fields.description,
        description_html: fields.description_html,
        updated_at: Some(fields.updated_at),
        online_store_url: fields.online_store_url,
        seo: Some(Seo {
            title: fields.seo.title,
            description: fields.seo.description,
        }),
        image: fields.image.map(convert_image_collection),
        products: collection
            .products
            .edges
            .into_iter()
            .map(|e| convert_collection_product(e.node))
            .collect(),
    }
}

fn convert_image_collection(i: get_collection_by_handle::CollectionImageFields) -> Image {
    Image {
        id: i.id,
        url: i.url,
        alt_text: i.alt_text,
        width: i.width,
        height: i.height,
    }
}

fn convert_collection_product(
    product: get_collection_by_handle::CollectionProductFields,
) -> Product {
    Product {
        id: product.id,
        handle: product.handle,
        title: product.title,
        description: product.description,
        description_html: String::new(),
        available_for_sale: product.available_for_sale,
        kind: product.product_type,
        vendor: product.vendor,
        tags: product.tags,
        created_at: None,
        updated_at: None,
        online_store_url: None,
        seo: None,
        price_range: PriceRange {
            min_variant_price: Money {
                amount: product.price_range.min_variant_price.amount,
                currency_code: currency_code_to_string(
                    product.price_range.min_variant_price.currency_code,
                ),
            },
            max_variant_price: Money {
                amount: product.price_range.max_variant_price.amount,
                currency_code: currency_code_to_string(
                    product.price_range.max_variant_price.currency_code,
                ),
            },
        },
        compare_at_price_range: Some(PriceRange {
            min_variant_price: Money {
                amount: product.compare_at_price_range.min_variant_price.amount,
                currency_code: currency_code_to_string(
                    product
                        .compare_at_price_range
                        .min_variant_price
                        .currency_code,
                ),
            },
            max_variant_price: Money {
                amount: product.compare_at_price_range.max_variant_price.amount,
                currency_code: currency_code_to_string(
                    product
                        .compare_at_price_range
                        .max_variant_price
                        .currency_code,
                ),
            },
        }),
        featured_image: product.featured_image.map(convert_image_collection),
        images: product
            .images
            .edges
            .into_iter()
            .map(|e| convert_image_collection(e.node))
            .collect(),
        options: vec![],
        variants: product
            .variants
            .edges
            .into_iter()
            .map(|v| ProductVariant {
                id: v.node.id,
                title: String::new(),
                available_for_sale: v.node.available_for_sale,
                quantity_available: None,
                sku: None,
                barcode: None,
                price: Money {
                    amount: v.node.price.amount,
                    currency_code: currency_code_to_string(v.node.price.currency_code),
                },
                compare_at_price: v.node.compare_at_price.map(|p| Money {
                    amount: p.amount,
                    currency_code: currency_code_to_string(p.currency_code),
                }),
                selected_options: vec![],
                image: None,
                shop_pay_installments: None,
            })
            .collect(),
        rating: None,
        ingredients: None,
        directions: None,
        warning: None,
        promotes: Vec::new(),
        benefits: None,
        free_from: Vec::new(),
        requires_selling_plan: false,
        selling_plan_groups: Vec::new(),
    }
}

// =============================================================================
// get_collections conversions
// =============================================================================

pub fn convert_collection_connection(
    conn: get_collections::GetCollectionsCollections,
) -> CollectionConnection {
    CollectionConnection {
        collections: conn
            .edges
            .into_iter()
            .map(|e| convert_collections_list_collection(e.node))
            .collect(),
        page_info: PageInfo {
            has_next_page: conn.page_info.has_next_page,
            has_previous_page: conn.page_info.has_previous_page,
            start_cursor: conn.page_info.start_cursor,
            end_cursor: conn.page_info.end_cursor,
        },
    }
}

fn convert_collections_list_collection(
    collection: get_collections::GetCollectionsCollectionsEdgesNode,
) -> Collection {
    let fields = collection.collection_fields;

    Collection {
        id: fields.id,
        handle: fields.handle,
        title: fields.title,
        description: fields.description,
        description_html: fields.description_html,
        updated_at: Some(fields.updated_at),
        online_store_url: fields.online_store_url,
        seo: Some(Seo {
            title: fields.seo.title,
            description: fields.seo.description,
        }),
        image: fields.image.map(convert_image_list),
        products: collection
            .products
            .edges
            .into_iter()
            .map(|p| Product {
                id: p.node.id,
                handle: p.node.handle,
                title: p.node.title,
                description: String::new(),
                description_html: String::new(),
                available_for_sale: true,
                kind: String::new(),
                vendor: String::new(),
                tags: vec![],
                created_at: None,
                updated_at: None,
                online_store_url: None,
                seo: None,
                price_range: PriceRange {
                    min_variant_price: Money {
                        amount: "0".to_string(),
                        currency_code: "USD".to_string(),
                    },
                    max_variant_price: Money {
                        amount: "0".to_string(),
                        currency_code: "USD".to_string(),
                    },
                },
                compare_at_price_range: None,
                featured_image: p.node.featured_image.map(convert_image_list),
                images: vec![],
                options: vec![],
                variants: vec![],
                rating: None,
                ingredients: None,
                directions: None,
                warning: None,
                promotes: Vec::new(),
                benefits: None,
                free_from: Vec::new(),
                requires_selling_plan: false,
                selling_plan_groups: Vec::new(),
            })
            .collect(),
    }
}

fn convert_image_list(i: get_collections::CollectionImageFields) -> Image {
    Image {
        id: i.id,
        url: i.url,
        alt_text: i.alt_text,
        width: i.width,
        height: i.height,
    }
}
