//! Inventory type conversion functions.

use crate::shopify::types::{InventoryLevel, InventoryLevelConnection, PageInfo};

use super::super::queries::get_inventory_levels;

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
