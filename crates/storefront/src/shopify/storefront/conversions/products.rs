//! Product type conversion functions.

use crate::shopify::types::{
    Image, Money, PageInfo, PriceRange, Product, ProductConnection, ProductOption, ProductVariant,
    SelectedOption, Seo,
};

use super::super::queries::{get_product_by_handle, get_product_recommendations, get_products};

/// Convert a `CurrencyCode` enum to string.
fn currency_code_to_string<T: std::fmt::Debug>(code: T) -> String {
    format!("{code:?}")
}

// =============================================================================
// get_product_by_handle conversions
// =============================================================================

pub fn convert_product(product: get_product_by_handle::GetProductByHandleProduct) -> Product {
    let fields = product.product_fields;

    Product {
        id: fields.id,
        handle: fields.handle,
        title: fields.title,
        description: fields.description,
        description_html: fields.description_html,
        available_for_sale: fields.available_for_sale,
        kind: fields.product_type,
        vendor: fields.vendor,
        tags: fields.tags,
        created_at: Some(fields.created_at),
        updated_at: Some(fields.updated_at),
        online_store_url: fields.online_store_url,
        seo: Some(Seo {
            title: fields.seo.title,
            description: fields.seo.description,
        }),
        price_range: convert_price_range_handle(fields.price_range),
        compare_at_price_range: Some(convert_compare_at_price_range_handle(
            fields.compare_at_price_range,
        )),
        featured_image: fields.featured_image.map(convert_image_handle),
        images: product
            .images
            .edges
            .into_iter()
            .map(|e| convert_image_handle(e.node))
            .collect(),
        options: fields
            .options
            .into_iter()
            .map(convert_option_handle)
            .collect(),
        variants: product
            .variants
            .edges
            .into_iter()
            .map(|e| convert_variant_handle(e.node))
            .collect(),
    }
}

fn convert_image_handle(i: get_product_by_handle::ImageFields) -> Image {
    Image {
        id: i.id,
        url: i.url,
        alt_text: i.alt_text,
        width: i.width,
        height: i.height,
    }
}

fn convert_money_handle(m: get_product_by_handle::MoneyFields) -> Money {
    Money {
        amount: m.amount,
        currency_code: currency_code_to_string(m.currency_code),
    }
}

fn convert_price_range_handle(r: get_product_by_handle::ProductFieldsPriceRange) -> PriceRange {
    PriceRange {
        min_variant_price: convert_money_handle(r.min_variant_price),
        max_variant_price: convert_money_handle(r.max_variant_price),
    }
}

fn convert_compare_at_price_range_handle(
    r: get_product_by_handle::ProductFieldsCompareAtPriceRange,
) -> PriceRange {
    PriceRange {
        min_variant_price: Money {
            amount: r.min_variant_price.amount,
            currency_code: currency_code_to_string(r.min_variant_price.currency_code),
        },
        max_variant_price: Money {
            amount: r.max_variant_price.amount,
            currency_code: currency_code_to_string(r.max_variant_price.currency_code),
        },
    }
}

fn convert_option_handle(o: get_product_by_handle::ProductFieldsOptions) -> ProductOption {
    ProductOption {
        id: o.id,
        name: o.name,
        values: o.option_values.into_iter().map(|v| v.name).collect(),
    }
}

fn convert_variant_handle(v: get_product_by_handle::ProductVariantFields) -> ProductVariant {
    ProductVariant {
        id: v.id,
        title: v.title,
        available_for_sale: v.available_for_sale,
        quantity_available: v.quantity_available,
        sku: v.sku,
        barcode: v.barcode,
        price: Money {
            amount: v.price.amount,
            currency_code: currency_code_to_string(v.price.currency_code),
        },
        compare_at_price: v.compare_at_price.map(|p| Money {
            amount: p.amount,
            currency_code: currency_code_to_string(p.currency_code),
        }),
        selected_options: v
            .selected_options
            .into_iter()
            .map(|o| SelectedOption {
                name: o.name,
                value: o.value,
            })
            .collect(),
        image: v.image.map(|i| Image {
            id: i.id,
            url: i.url,
            alt_text: i.alt_text,
            width: i.width,
            height: i.height,
        }),
    }
}

// =============================================================================
// get_products conversions
// =============================================================================

pub fn convert_product_connection(conn: get_products::GetProductsProducts) -> ProductConnection {
    ProductConnection {
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

fn convert_products_list_product(product: get_products::GetProductsProductsEdgesNode) -> Product {
    let fields = product.product_fields;

    Product {
        id: fields.id,
        handle: fields.handle,
        title: fields.title,
        description: fields.description,
        description_html: fields.description_html,
        available_for_sale: fields.available_for_sale,
        kind: fields.product_type,
        vendor: fields.vendor,
        tags: fields.tags,
        created_at: Some(fields.created_at),
        updated_at: Some(fields.updated_at),
        online_store_url: fields.online_store_url,
        seo: Some(Seo {
            title: fields.seo.title,
            description: fields.seo.description,
        }),
        price_range: convert_price_range_list(fields.price_range),
        compare_at_price_range: Some(convert_compare_at_price_range_list(
            fields.compare_at_price_range,
        )),
        featured_image: fields.featured_image.map(convert_image_list),
        images: product
            .images
            .edges
            .into_iter()
            .map(|e| convert_image_list(e.node))
            .collect(),
        options: fields
            .options
            .into_iter()
            .map(convert_option_list)
            .collect(),
        variants: product
            .variants
            .edges
            .into_iter()
            .map(|e| convert_variant_list(e.node))
            .collect(),
    }
}

fn convert_image_list(i: get_products::ImageFields) -> Image {
    Image {
        id: i.id,
        url: i.url,
        alt_text: i.alt_text,
        width: i.width,
        height: i.height,
    }
}

fn convert_money_list(m: get_products::MoneyFields) -> Money {
    Money {
        amount: m.amount,
        currency_code: currency_code_to_string(m.currency_code),
    }
}

fn convert_price_range_list(r: get_products::ProductFieldsPriceRange) -> PriceRange {
    PriceRange {
        min_variant_price: convert_money_list(r.min_variant_price),
        max_variant_price: convert_money_list(r.max_variant_price),
    }
}

fn convert_compare_at_price_range_list(
    r: get_products::ProductFieldsCompareAtPriceRange,
) -> PriceRange {
    PriceRange {
        min_variant_price: Money {
            amount: r.min_variant_price.amount,
            currency_code: currency_code_to_string(r.min_variant_price.currency_code),
        },
        max_variant_price: Money {
            amount: r.max_variant_price.amount,
            currency_code: currency_code_to_string(r.max_variant_price.currency_code),
        },
    }
}

fn convert_option_list(o: get_products::ProductFieldsOptions) -> ProductOption {
    ProductOption {
        id: o.id,
        name: o.name,
        values: o.option_values.into_iter().map(|v| v.name).collect(),
    }
}

fn convert_variant_list(v: get_products::ProductVariantFields) -> ProductVariant {
    ProductVariant {
        id: v.id,
        title: v.title,
        available_for_sale: v.available_for_sale,
        quantity_available: v.quantity_available,
        sku: v.sku,
        barcode: v.barcode,
        price: Money {
            amount: v.price.amount,
            currency_code: currency_code_to_string(v.price.currency_code),
        },
        compare_at_price: v.compare_at_price.map(|p| Money {
            amount: p.amount,
            currency_code: currency_code_to_string(p.currency_code),
        }),
        selected_options: v
            .selected_options
            .into_iter()
            .map(|o| SelectedOption {
                name: o.name,
                value: o.value,
            })
            .collect(),
        image: v.image.map(|i| Image {
            id: i.id,
            url: i.url,
            alt_text: i.alt_text,
            width: i.width,
            height: i.height,
        }),
    }
}

// =============================================================================
// get_product_recommendations conversions
// =============================================================================

pub fn convert_product_recommendation(
    product: get_product_recommendations::GetProductRecommendationsProductRecommendations,
) -> Product {
    let fields = product.product_fields;

    Product {
        id: fields.id,
        handle: fields.handle,
        title: fields.title,
        description: fields.description,
        description_html: fields.description_html,
        available_for_sale: fields.available_for_sale,
        kind: fields.product_type,
        vendor: fields.vendor,
        tags: fields.tags,
        created_at: Some(fields.created_at),
        updated_at: Some(fields.updated_at),
        online_store_url: fields.online_store_url,
        seo: Some(Seo {
            title: fields.seo.title,
            description: fields.seo.description,
        }),
        price_range: convert_price_range_rec(fields.price_range),
        compare_at_price_range: Some(convert_compare_at_price_range_rec(
            fields.compare_at_price_range,
        )),
        featured_image: fields.featured_image.map(convert_image_rec),
        images: product
            .images
            .edges
            .into_iter()
            .map(|e| convert_image_rec(e.node))
            .collect(),
        options: fields.options.into_iter().map(convert_option_rec).collect(),
        variants: product
            .variants
            .edges
            .into_iter()
            .map(|e| convert_variant_rec(e.node))
            .collect(),
    }
}

fn convert_image_rec(i: get_product_recommendations::ImageFields) -> Image {
    Image {
        id: i.id,
        url: i.url,
        alt_text: i.alt_text,
        width: i.width,
        height: i.height,
    }
}

fn convert_money_rec(m: get_product_recommendations::MoneyFields) -> Money {
    Money {
        amount: m.amount,
        currency_code: currency_code_to_string(m.currency_code),
    }
}

fn convert_price_range_rec(r: get_product_recommendations::ProductFieldsPriceRange) -> PriceRange {
    PriceRange {
        min_variant_price: convert_money_rec(r.min_variant_price),
        max_variant_price: convert_money_rec(r.max_variant_price),
    }
}

fn convert_compare_at_price_range_rec(
    r: get_product_recommendations::ProductFieldsCompareAtPriceRange,
) -> PriceRange {
    PriceRange {
        min_variant_price: Money {
            amount: r.min_variant_price.amount,
            currency_code: currency_code_to_string(r.min_variant_price.currency_code),
        },
        max_variant_price: Money {
            amount: r.max_variant_price.amount,
            currency_code: currency_code_to_string(r.max_variant_price.currency_code),
        },
    }
}

fn convert_option_rec(o: get_product_recommendations::ProductFieldsOptions) -> ProductOption {
    ProductOption {
        id: o.id,
        name: o.name,
        values: o.option_values.into_iter().map(|v| v.name).collect(),
    }
}

fn convert_variant_rec(v: get_product_recommendations::ProductVariantFields) -> ProductVariant {
    ProductVariant {
        id: v.id,
        title: v.title,
        available_for_sale: v.available_for_sale,
        quantity_available: v.quantity_available,
        sku: v.sku,
        barcode: v.barcode,
        price: Money {
            amount: v.price.amount,
            currency_code: currency_code_to_string(v.price.currency_code),
        },
        compare_at_price: v.compare_at_price.map(|p| Money {
            amount: p.amount,
            currency_code: currency_code_to_string(p.currency_code),
        }),
        selected_options: v
            .selected_options
            .into_iter()
            .map(|o| SelectedOption {
                name: o.name,
                value: o.value,
            })
            .collect(),
        image: v.image.map(|i| Image {
            id: i.id,
            url: i.url,
            alt_text: i.alt_text,
            width: i.width,
            height: i.height,
        }),
    }
}
