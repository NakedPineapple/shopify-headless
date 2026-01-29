//! Inventory type conversion functions.

use crate::shopify::types::{
    Image, InventoryItem, InventoryItemConnection, InventoryItemProduct, InventoryItemVariant,
    InventoryLevel, InventoryLevelConnection, Location, LocationAddress, LocationConnection, Money,
    PageInfo, ProductStatus,
};

use super::super::queries::{
    get_inventory_item, get_inventory_items, get_inventory_levels, get_locations,
};

// =============================================================================
// GetInventoryLevels conversions
// =============================================================================

pub fn convert_inventory_level_connection(
    location: get_inventory_levels::GetInventoryLevelsLocation,
) -> InventoryLevelConnection {
    let location_id = location.id.clone();
    let location_name = location.name.clone();

    InventoryLevelConnection {
        inventory_levels: location
            .inventory_levels
            .edges
            .into_iter()
            .map(|e| convert_inventory_level(e.node, &location_id, &location_name))
            .collect(),
        page_info: PageInfo {
            has_next_page: location.inventory_levels.page_info.has_next_page,
            has_previous_page: location.inventory_levels.page_info.has_previous_page,
            start_cursor: location.inventory_levels.page_info.start_cursor,
            end_cursor: location.inventory_levels.page_info.end_cursor,
        },
    }
}

fn convert_inventory_level(
    level: get_inventory_levels::GetInventoryLevelsLocationInventoryLevelsEdgesNode,
    location_id: &str,
    location_name: &str,
) -> InventoryLevel {
    // Extract quantities by name
    let mut available: i64 = 0;
    let mut on_hand: i64 = 0;
    let mut incoming: i64 = 0;

    for qty in &level.quantities {
        match qty.name.as_str() {
            "available" => available = qty.quantity,
            "on_hand" => on_hand = qty.quantity,
            "incoming" => incoming = qty.quantity,
            _ => {}
        }
    }

    InventoryLevel {
        inventory_item_id: level.item.id,
        location_id: location_id.to_string(),
        location_name: Some(location_name.to_string()),
        available,
        on_hand,
        incoming,
        updated_at: Some(level.updated_at),
    }
}

// =============================================================================
// GetLocations conversions
// =============================================================================

pub fn convert_location_connection(
    locations: get_locations::GetLocationsLocations,
) -> LocationConnection {
    LocationConnection {
        locations: locations
            .edges
            .into_iter()
            .map(|e| convert_location(e.node))
            .collect(),
        page_info: PageInfo {
            has_next_page: locations.page_info.has_next_page,
            has_previous_page: false,
            start_cursor: None,
            end_cursor: locations.page_info.end_cursor,
        },
    }
}

fn convert_location(location: get_locations::GetLocationsLocationsEdgesNode) -> Location {
    let address = location.address;
    Location {
        id: location.id,
        name: location.name,
        is_active: location.is_active,
        fulfills_online_orders: location.fulfills_online_orders,
        address: Some(LocationAddress {
            address1: address.address1,
            city: address.city,
            province_code: address.province_code,
            country_code: address.country_code,
            zip: address.zip,
        }),
    }
}

// =============================================================================
// GetInventoryItems conversions
// =============================================================================

/// Convert the `GetInventoryItems` response to our domain type.
pub fn convert_inventory_item_connection(
    response: get_inventory_items::ResponseData,
) -> InventoryItemConnection {
    InventoryItemConnection {
        items: response
            .inventory_items
            .edges
            .into_iter()
            .map(|e| convert_inventory_item_from_list(e.node))
            .collect(),
        page_info: PageInfo {
            has_next_page: response.inventory_items.page_info.has_next_page,
            has_previous_page: false,
            start_cursor: None,
            end_cursor: response.inventory_items.page_info.end_cursor,
        },
    }
}

fn convert_inventory_item_from_list(
    item: get_inventory_items::GetInventoryItemsInventoryItemsEdgesNode,
) -> InventoryItem {
    // Convert inventory levels
    let inventory_levels: Vec<InventoryLevel> = item
        .inventory_levels
        .edges
        .into_iter()
        .map(|e| {
            let level = e.node;
            let mut available: i64 = 0;
            let mut on_hand: i64 = 0;
            let mut incoming: i64 = 0;

            for qty in &level.quantities {
                match qty.name.as_str() {
                    "available" => available = qty.quantity,
                    "on_hand" => on_hand = qty.quantity,
                    "incoming" => incoming = qty.quantity,
                    // committed tracked but not stored in InventoryLevel
                    _ => {}
                }
            }

            InventoryLevel {
                inventory_item_id: item.id.clone(),
                location_id: level.location.id,
                location_name: Some(level.location.name),
                available,
                on_hand,
                incoming,
                updated_at: None,
            }
        })
        .collect();

    // Convert variant and product info from variants connection (first item)
    let variant = item.variants.and_then(|variants| {
        variants.edges.into_iter().next().map(|edge| {
            let v = edge.node;
            // product is a required field in this query, so convert directly
            let p = v.product;
            let product = Some(InventoryItemProduct {
                id: p.id,
                title: p.title,
                handle: p.handle,
                status: convert_product_status(&p.status),
                featured_image: p
                    .featured_media
                    .and_then(|m| m.preview)
                    .and_then(|preview| preview.image)
                    .map(|img| Image {
                        id: None,
                        url: img.url,
                        alt_text: img.alt_text,
                        width: None,
                        height: None,
                    }),
            });
            InventoryItemVariant {
                id: v.id,
                title: v.title,
                display_name: None,
                price: None,
                image: None,
                product,
            }
        })
    });

    InventoryItem {
        id: item.id,
        sku: item.sku,
        tracked: item.tracked,
        requires_shipping: item.requires_shipping,
        unit_cost: item.unit_cost.map(|c| Money {
            amount: c.amount,
            currency_code: format!("{:?}", c.currency_code),
        }),
        harmonized_system_code: item.harmonized_system_code,
        country_code_of_origin: item.country_code_of_origin.map(|c| format!("{c:?}")),
        province_code_of_origin: None,
        inventory_levels,
        variant,
    }
}

const fn convert_product_status(status: &get_inventory_items::ProductStatus) -> ProductStatus {
    match status {
        get_inventory_items::ProductStatus::ACTIVE => ProductStatus::Active,
        get_inventory_items::ProductStatus::ARCHIVED => ProductStatus::Archived,
        get_inventory_items::ProductStatus::UNLISTED => ProductStatus::Unlisted,
        get_inventory_items::ProductStatus::DRAFT
        | get_inventory_items::ProductStatus::Other(_) => ProductStatus::Draft,
    }
}

// =============================================================================
// GetInventoryItem (single) conversions
// =============================================================================

/// Convert the `GetInventoryItem` response to our domain type.
pub fn convert_single_inventory_item(
    item: get_inventory_item::GetInventoryItemInventoryItem,
) -> InventoryItem {
    // Convert inventory levels
    let inventory_levels: Vec<InventoryLevel> = item
        .inventory_levels
        .edges
        .into_iter()
        .map(|e| {
            let level = e.node;
            let mut available: i64 = 0;
            let mut on_hand: i64 = 0;
            let mut incoming: i64 = 0;

            for qty in &level.quantities {
                match qty.name.as_str() {
                    "available" => available = qty.quantity,
                    "on_hand" => on_hand = qty.quantity,
                    "incoming" => incoming = qty.quantity,
                    // committed, reserved, damaged tracked but not stored in InventoryLevel
                    _ => {}
                }
            }

            InventoryLevel {
                inventory_item_id: item.id.clone(),
                location_id: level.location.id,
                location_name: Some(level.location.name),
                available,
                on_hand,
                incoming,
                updated_at: Some(level.updated_at),
            }
        })
        .collect();

    // Convert variant and product info from variants connection (first item)
    let variant = item.variants.and_then(|variants| {
        variants.edges.into_iter().next().map(|edge| {
            let v = edge.node;
            // product is a required field in this query, so convert directly
            let p = v.product;
            let product = Some(InventoryItemProduct {
                id: p.id,
                title: p.title,
                handle: p.handle,
                status: convert_single_product_status(&p.status),
                featured_image: p
                    .featured_media
                    .and_then(|m| m.preview)
                    .and_then(|preview| preview.image)
                    .map(|img| Image {
                        id: None,
                        url: img.url,
                        alt_text: img.alt_text,
                        width: None,
                        height: None,
                    }),
            });
            InventoryItemVariant {
                id: v.id,
                title: v.title,
                display_name: None,
                price: None,
                image: None,
                product,
            }
        })
    });

    InventoryItem {
        id: item.id,
        sku: item.sku,
        tracked: item.tracked,
        requires_shipping: item.requires_shipping,
        unit_cost: item.unit_cost.map(|c| Money {
            amount: c.amount,
            currency_code: format!("{:?}", c.currency_code),
        }),
        harmonized_system_code: item.harmonized_system_code,
        country_code_of_origin: item.country_code_of_origin.map(|c| format!("{c:?}")),
        province_code_of_origin: item.province_code_of_origin.map(|p| format!("{p:?}")),
        inventory_levels,
        variant,
    }
}

const fn convert_single_product_status(
    status: &get_inventory_item::ProductStatus,
) -> ProductStatus {
    match status {
        get_inventory_item::ProductStatus::ACTIVE => ProductStatus::Active,
        get_inventory_item::ProductStatus::ARCHIVED => ProductStatus::Archived,
        get_inventory_item::ProductStatus::UNLISTED => ProductStatus::Unlisted,
        get_inventory_item::ProductStatus::DRAFT | get_inventory_item::ProductStatus::Other(_) => {
            ProductStatus::Draft
        }
    }
}
