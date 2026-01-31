//! Inventory query methods for `ShipHero` API.
//!
//! Provides methods to fetch product inventory, warehouse stock levels,
//! bin locations, and lot/expiration data from the warehouse.

use serde::{Deserialize, Serialize};
use tracing::instrument;

use super::ShipHeroError;
use super::client::ShipHeroClient;
use super::queries::{
    GetExpirationLots, GetInventoryChanges, GetLocations, GetProductBySku, GetProducts,
    GetWarehouseProducts,
};

// =============================================================================
// Domain Types
// =============================================================================

/// A product in the `ShipHero` warehouse system.
#[derive(Debug, Clone, Serialize)]
pub struct Product {
    /// `ShipHero` product ID.
    pub id: String,
    /// Legacy numeric ID.
    pub legacy_id: Option<i64>,
    /// Product SKU.
    pub sku: Option<String>,
    /// Product name.
    pub name: Option<String>,
    /// Barcode.
    pub barcode: Option<String>,
    /// Country of manufacture.
    pub country_of_manufacture: Option<String>,
    /// Whether this is a kit.
    pub kit: Option<bool>,
    /// Whether the kit is built on demand.
    pub kit_build: Option<bool>,
    /// Whether air shipping is prohibited.
    pub no_air: Option<bool>,
    /// Whether this is a final sale item.
    pub final_sale: Option<bool>,
    /// Whether the product is virtual (no physical inventory).
    pub is_virtual: Option<bool>,
    /// Warehouse-specific inventory data.
    pub warehouse_products: Vec<WarehouseProduct>,
    /// Product images.
    pub images: Vec<ProductImage>,
    /// Created timestamp.
    pub created_at: Option<String>,
    /// Updated timestamp.
    pub updated_at: Option<String>,
}

/// Warehouse-specific inventory for a product.
#[derive(Debug, Clone, Serialize)]
pub struct WarehouseProduct {
    /// Warehouse ID.
    pub warehouse_id: Option<String>,
    /// Quantity on hand.
    pub on_hand: Option<i64>,
    /// Quantity allocated to orders.
    pub allocated: Option<i64>,
    /// Quantity available for new orders.
    pub available: Option<i64>,
    /// Quantity on backorder.
    pub backorder: Option<i64>,
    /// Primary bin location.
    pub inventory_bin: Option<String>,
    /// Overstock bin location.
    pub inventory_overstock_bin: Option<String>,
    /// Reserved inventory.
    pub reserve_inventory: Option<i64>,
    /// Replenishment level.
    pub replenishment_level: Option<i64>,
    /// Reorder level (low stock threshold).
    pub reorder_level: Option<i64>,
    /// Quantity to reorder.
    pub reorder_amount: Option<i64>,
    /// Unit price.
    pub price: Option<String>,
    /// Total value.
    pub value: Option<String>,
    /// Currency for value.
    pub value_currency: Option<String>,
}

/// Product image.
#[derive(Debug, Clone, Serialize)]
pub struct ProductImage {
    /// Image URL.
    pub src: Option<String>,
    /// Image position/order.
    pub position: Option<i64>,
}

/// A bin location in the warehouse.
#[derive(Debug, Clone, Serialize)]
pub struct Location {
    /// Location ID.
    pub id: String,
    /// Legacy numeric ID.
    pub legacy_id: Option<i64>,
    /// Location name (e.g., "A-01-02").
    pub name: Option<String>,
    /// Zone within warehouse.
    pub zone: Option<String>,
    /// Whether items can be picked from this location.
    pub pickable: Option<bool>,
    /// Whether items are sellable from this location.
    pub sellable: Option<bool>,
    /// Whether this is a cart location.
    pub is_cart: Option<bool>,
    /// Pick priority.
    pub pick_priority: Option<i64>,
    /// Location type (e.g., "shelf", "floor").
    pub kind: Option<String>,
    /// Warehouse ID.
    pub warehouse_id: Option<String>,
}

/// An inventory change/adjustment record.
#[derive(Debug, Clone, Serialize)]
pub struct InventoryChange {
    /// Change ID.
    pub id: String,
    /// Product SKU.
    pub sku: Option<String>,
    /// Previous on-hand quantity.
    pub previous_on_hand: Option<i64>,
    /// Change in on-hand quantity.
    pub change_in_on_hand: Option<i64>,
    /// Reason for the change.
    pub reason: Option<String>,
    /// Whether this was a cycle count.
    pub cycle_counted: Option<bool>,
    /// Location ID where change occurred.
    pub location_id: Option<String>,
    /// Location name.
    pub location_name: Option<String>,
    /// Timestamp of the change.
    pub created_at: Option<String>,
    /// User who made the change.
    pub user_id: Option<String>,
}

/// A lot/batch with expiration date.
#[derive(Debug, Clone, Serialize)]
pub struct ExpirationLot {
    /// Lot ID.
    pub id: String,
    /// Legacy numeric ID.
    pub legacy_id: Option<i64>,
    /// Product SKU.
    pub sku: Option<String>,
    /// Lot name/number.
    pub name: Option<String>,
    /// Expiration date.
    pub expires_at: Option<String>,
    /// Whether the lot is active.
    pub is_active: Option<bool>,
    /// Created timestamp.
    pub created_at: Option<String>,
    /// Updated timestamp.
    pub updated_at: Option<String>,
}

/// Paginated list of products.
#[derive(Debug, Clone, Serialize)]
pub struct ProductConnection {
    /// Products in this page.
    pub products: Vec<Product>,
    /// Whether there are more pages.
    pub has_next_page: bool,
    /// Cursor for the next page.
    pub end_cursor: Option<String>,
}

/// Paginated list of warehouse products.
#[derive(Debug, Clone, Serialize)]
pub struct WarehouseProductConnection {
    /// Warehouse products in this page.
    pub products: Vec<WarehouseProduct>,
    /// Whether there are more pages.
    pub has_next_page: bool,
    /// Cursor for the next page.
    pub end_cursor: Option<String>,
}

/// Paginated list of locations.
#[derive(Debug, Clone, Serialize)]
pub struct LocationConnection {
    /// Locations in this page.
    pub locations: Vec<Location>,
    /// Whether there are more pages.
    pub has_next_page: bool,
    /// Cursor for the next page.
    pub end_cursor: Option<String>,
}

// =============================================================================
// ShipHeroClient Inventory Methods
// =============================================================================

impl ShipHeroClient {
    /// Get products with inventory levels.
    ///
    /// # Errors
    ///
    /// Returns `ShipHeroError` if the API call fails.
    #[instrument(skip(self))]
    pub async fn get_products(
        &self,
        first: Option<i64>,
        after: Option<String>,
        sku: Option<String>,
    ) -> Result<ProductConnection, ShipHeroError> {
        use super::queries::get_products::Variables;

        let variables = Variables { first, after, sku };
        let response = self.execute_query::<GetProducts>(variables).await?;

        let Some(products_result) = response.products else {
            return Ok(ProductConnection {
                products: Vec::new(),
                has_next_page: false,
                end_cursor: None,
            });
        };

        let Some(data) = products_result.data else {
            return Ok(ProductConnection {
                products: Vec::new(),
                has_next_page: false,
                end_cursor: None,
            });
        };

        let has_next_page = data.page_info.has_next_page;
        let end_cursor = data.page_info.end_cursor;

        let products: Vec<Product> = data
            .edges
            .into_iter()
            .flatten()
            .filter_map(|edge| {
                let node = edge.node?;
                Some(Product {
                    id: node.id?,
                    legacy_id: node.legacy_id,
                    sku: node.sku,
                    name: node.name,
                    barcode: node.barcode,
                    country_of_manufacture: node.country_of_manufacture,
                    kit: node.kit,
                    kit_build: node.kit_build,
                    no_air: node.no_air,
                    final_sale: node.final_sale,
                    is_virtual: node.virtual_,
                    warehouse_products: convert_warehouse_products(node.warehouse_products),
                    images: convert_images(node.images),
                    created_at: node.created_at,
                    updated_at: node.updated_at,
                })
            })
            .collect();

        Ok(ProductConnection {
            products,
            has_next_page,
            end_cursor,
        })
    }

    /// Get a single product by SKU.
    ///
    /// # Errors
    ///
    /// Returns `ShipHeroError` if the API call fails.
    #[instrument(skip(self), fields(sku = %sku))]
    pub async fn get_product_by_sku(&self, sku: &str) -> Result<Option<Product>, ShipHeroError> {
        use super::queries::get_product_by_sku::Variables;

        let variables = Variables {
            sku: sku.to_string(),
        };
        let response = self.execute_query::<GetProductBySku>(variables).await?;

        let product = response
            .products
            .and_then(|result| result.data)
            .and_then(|data| data.edges.into_iter().flatten().next())
            .and_then(|edge| edge.node)
            .map(|node| Product {
                id: node.id.unwrap_or_default(),
                legacy_id: node.legacy_id,
                sku: node.sku,
                name: node.name,
                barcode: node.barcode,
                country_of_manufacture: node.country_of_manufacture,
                kit: node.kit,
                kit_build: node.kit_build,
                no_air: None,
                final_sale: None,
                is_virtual: None,
                warehouse_products: convert_product_by_sku_warehouse_products(
                    node.warehouse_products,
                ),
                images: Vec::new(),
                created_at: None,
                updated_at: None,
            });

        Ok(product)
    }

    /// Get warehouse products with detailed inventory.
    ///
    /// # Errors
    ///
    /// Returns `ShipHeroError` if the API call fails.
    #[instrument(skip(self), fields(warehouse_id = %warehouse_id))]
    pub async fn get_warehouse_products(
        &self,
        warehouse_id: &str,
        first: Option<i64>,
        after: Option<String>,
    ) -> Result<WarehouseProductConnection, ShipHeroError> {
        use super::queries::get_warehouse_products::Variables;

        let variables = Variables {
            warehouse_id: warehouse_id.to_string(),
            first,
            after,
        };
        let response = self
            .execute_query::<GetWarehouseProducts>(variables)
            .await?;

        let Some(result) = response.warehouse_products else {
            return Ok(WarehouseProductConnection {
                products: Vec::new(),
                has_next_page: false,
                end_cursor: None,
            });
        };

        let Some(data) = result.data else {
            return Ok(WarehouseProductConnection {
                products: Vec::new(),
                has_next_page: false,
                end_cursor: None,
            });
        };

        let has_next_page = data.page_info.has_next_page;
        let end_cursor = data.page_info.end_cursor;

        let products: Vec<WarehouseProduct> = data
            .edges
            .into_iter()
            .flatten()
            .filter_map(|edge| {
                let node = edge.node?;
                Some(WarehouseProduct {
                    warehouse_id: node.warehouse_id,
                    on_hand: node.on_hand,
                    allocated: node.allocated,
                    available: node.available,
                    backorder: node.backorder,
                    inventory_bin: node.inventory_bin,
                    inventory_overstock_bin: node.inventory_overstock_bin,
                    reserve_inventory: node.reserve_inventory,
                    replenishment_level: node.replenishment_level,
                    reorder_level: node.reorder_level,
                    reorder_amount: node.reorder_amount,
                    price: node.price,
                    value: node.value,
                    value_currency: None,
                })
            })
            .collect();

        Ok(WarehouseProductConnection {
            products,
            has_next_page,
            end_cursor,
        })
    }

    /// Get locations (bins) in a warehouse.
    ///
    /// # Errors
    ///
    /// Returns `ShipHeroError` if the API call fails.
    #[instrument(skip(self), fields(warehouse_id = %warehouse_id))]
    pub async fn get_locations(
        &self,
        warehouse_id: &str,
        first: Option<i64>,
    ) -> Result<LocationConnection, ShipHeroError> {
        use super::queries::get_locations::Variables;

        let variables = Variables {
            warehouse_id: warehouse_id.to_string(),
            first,
        };
        let response = self.execute_query::<GetLocations>(variables).await?;

        let Some(result) = response.locations else {
            return Ok(LocationConnection {
                locations: Vec::new(),
                has_next_page: false,
                end_cursor: None,
            });
        };

        let Some(data) = result.data else {
            return Ok(LocationConnection {
                locations: Vec::new(),
                has_next_page: false,
                end_cursor: None,
            });
        };

        let has_next_page = data.page_info.has_next_page;
        let end_cursor = data.page_info.end_cursor;

        let locations: Vec<Location> = data
            .edges
            .into_iter()
            .flatten()
            .filter_map(|edge| {
                let node = edge.node?;
                Some(Location {
                    id: node.id?,
                    legacy_id: node.legacy_id,
                    name: node.name,
                    zone: node.zone,
                    pickable: node.pickable,
                    sellable: node.sellable,
                    is_cart: node.is_cart,
                    pick_priority: node.pick_priority,
                    kind: node.type_.map(|t| format!("{t:?}")),
                    warehouse_id: node.warehouse_id,
                })
            })
            .collect();

        Ok(LocationConnection {
            locations,
            has_next_page,
            end_cursor,
        })
    }

    /// Get inventory changes for a SKU.
    ///
    /// # Errors
    ///
    /// Returns `ShipHeroError` if the API call fails.
    #[instrument(skip(self), fields(sku = %sku))]
    pub async fn get_inventory_changes(
        &self,
        sku: &str,
        date_from: Option<String>,
        date_to: Option<String>,
        first: Option<i64>,
    ) -> Result<Vec<InventoryChange>, ShipHeroError> {
        use super::queries::get_inventory_changes::Variables;

        let variables = Variables {
            sku: sku.to_string(),
            date_from,
            date_to,
            first,
        };
        let response = self.execute_query::<GetInventoryChanges>(variables).await?;

        let changes = response
            .inventory_changes
            .and_then(|result| result.data)
            .map(|data| {
                data.edges
                    .into_iter()
                    .flatten()
                    .filter_map(|edge| {
                        let node = edge.node?;
                        Some(InventoryChange {
                            id: node.id?,
                            sku: node.sku,
                            previous_on_hand: node.previous_on_hand,
                            change_in_on_hand: node.change_in_on_hand,
                            reason: node.reason,
                            cycle_counted: node.cycle_counted,
                            location_id: node.location_id,
                            location_name: node.location.and_then(|l| l.name),
                            created_at: node.created_at,
                            user_id: node.user_id,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(changes)
    }

    /// Get expiration lots for a SKU or warehouse.
    ///
    /// # Errors
    ///
    /// Returns `ShipHeroError` if the API call fails.
    #[instrument(skip(self))]
    pub async fn get_expiration_lots(
        &self,
        sku: Option<String>,
        warehouse_id: Option<String>,
        first: Option<i64>,
    ) -> Result<Vec<ExpirationLot>, ShipHeroError> {
        use super::queries::get_expiration_lots::Variables;

        let variables = Variables {
            sku,
            warehouse_id,
            first,
        };
        let response = self.execute_query::<GetExpirationLots>(variables).await?;

        let lots = response
            .expiration_lots
            .and_then(|result| result.data)
            .map(|data| {
                data.edges
                    .into_iter()
                    .flatten()
                    .filter_map(|edge| {
                        let node = edge.node?;
                        Some(ExpirationLot {
                            id: node.id?,
                            legacy_id: node.legacy_id,
                            sku: node.sku,
                            name: node.name,
                            expires_at: node.expires_at,
                            is_active: node.is_active,
                            created_at: node.created_at,
                            updated_at: node.updated_at,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(lots)
    }
}

// =============================================================================
// Conversion Helper Functions
// =============================================================================

fn convert_warehouse_products(
    wps: Option<
        Vec<
            Option<super::queries::get_products::GetProductsProductsDataEdgesNodeWarehouseProducts>,
        >,
    >,
) -> Vec<WarehouseProduct> {
    let Some(wps) = wps else {
        return Vec::new();
    };
    wps.into_iter()
        .flatten()
        .map(|wp| WarehouseProduct {
            warehouse_id: wp.warehouse_id,
            on_hand: wp.on_hand,
            allocated: None,
            available: None,
            backorder: None,
            inventory_bin: wp.inventory_bin,
            inventory_overstock_bin: wp.inventory_overstock_bin,
            reserve_inventory: wp.reserve_inventory,
            replenishment_level: wp.replenishment_level,
            reorder_level: wp.reorder_level,
            reorder_amount: wp.reorder_amount,
            price: wp.price,
            value: wp.value,
            value_currency: wp.value_currency,
        })
        .collect()
}

fn convert_product_by_sku_warehouse_products(
    wps: Option<Vec<Option<super::queries::get_product_by_sku::GetProductBySkuProductsDataEdgesNodeWarehouseProducts>>>,
) -> Vec<WarehouseProduct> {
    let Some(wps) = wps else {
        return Vec::new();
    };
    wps.into_iter()
        .flatten()
        .map(|wp| WarehouseProduct {
            warehouse_id: wp.warehouse_id,
            on_hand: wp.on_hand,
            allocated: None,
            available: None,
            backorder: None,
            inventory_bin: wp.inventory_bin,
            inventory_overstock_bin: wp.inventory_overstock_bin,
            reserve_inventory: wp.reserve_inventory,
            replenishment_level: wp.replenishment_level,
            reorder_level: wp.reorder_level,
            reorder_amount: wp.reorder_amount,
            price: wp.price,
            value: wp.value,
            value_currency: wp.value_currency,
        })
        .collect()
}

fn convert_images(
    images: Option<
        Vec<Option<super::queries::get_products::GetProductsProductsDataEdgesNodeImages>>,
    >,
) -> Vec<ProductImage> {
    let Some(images) = images else {
        return Vec::new();
    };
    images
        .into_iter()
        .flatten()
        .map(|img| ProductImage {
            src: img.src,
            position: img.position,
        })
        .collect()
}
