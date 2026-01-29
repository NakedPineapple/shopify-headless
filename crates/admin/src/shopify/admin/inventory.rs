//! Location and inventory management operations for the Admin API.

use tracing::instrument;

use super::{
    AdminClient, AdminShopifyError,
    conversions::{
        convert_inventory_item_connection, convert_inventory_level_connection,
        convert_location_connection, convert_single_inventory_item,
    },
    queries::{
        ActivateInventory, DeactivateInventory, GetInventoryItem, GetInventoryItems,
        GetInventoryLevels, GetLocations, InventoryAdjustQuantities, InventorySetQuantities,
        MoveInventory, UpdateInventoryItem,
    },
};
use crate::shopify::types::{
    InventoryItem, InventoryItemConnection, InventoryItemUpdateInput, InventoryLevelConnection,
    LocationConnection,
};

impl AdminClient {
    /// Get all locations.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_locations(&self) -> Result<LocationConnection, AdminShopifyError> {
        let variables = super::queries::get_locations::Variables { first: Some(50) };

        let response = self.execute::<GetLocations>(variables).await?;

        Ok(convert_location_connection(response.locations))
    }

    /// Get inventory levels at a location.
    ///
    /// # Arguments
    ///
    /// * `location_id` - Shopify location ID (e.g., `gid://shopify/Location/123`)
    /// * `first` - Number of inventory levels to return
    /// * `after` - Cursor for pagination
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or the location is not found.
    #[instrument(skip(self), fields(location_id = %location_id))]
    pub async fn get_inventory_levels(
        &self,
        location_id: &str,
        first: i64,
        after: Option<String>,
    ) -> Result<InventoryLevelConnection, AdminShopifyError> {
        let variables = super::queries::get_inventory_levels::Variables {
            location_id: location_id.to_string(),
            first: Some(first),
            after,
        };

        let response = self.execute::<GetInventoryLevels>(variables).await?;

        response
            .location
            .map(convert_inventory_level_connection)
            .ok_or_else(|| AdminShopifyError::NotFound(format!("Location {location_id} not found")))
    }

    /// Get inventory items with pagination.
    ///
    /// # Arguments
    ///
    /// * `first` - Number of items to return
    /// * `after` - Cursor for pagination
    /// * `query` - Optional search query
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails.
    #[instrument(skip(self))]
    pub async fn get_inventory_items(
        &self,
        first: i64,
        after: Option<String>,
        query: Option<String>,
    ) -> Result<InventoryItemConnection, AdminShopifyError> {
        let variables = super::queries::get_inventory_items::Variables {
            first: Some(first),
            after,
            query,
        };

        let response = self.execute::<GetInventoryItems>(variables).await?;

        Ok(convert_inventory_item_connection(response))
    }

    /// Get a single inventory item by ID.
    ///
    /// # Arguments
    ///
    /// * `id` - Shopify inventory item ID (e.g., `gid://shopify/InventoryItem/123`)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or the item is not found.
    #[instrument(skip(self), fields(id = %id))]
    pub async fn get_inventory_item(&self, id: &str) -> Result<InventoryItem, AdminShopifyError> {
        let variables = super::queries::get_inventory_item::Variables { id: id.to_string() };

        let response = self.execute::<GetInventoryItem>(variables).await?;

        response
            .inventory_item
            .map(convert_single_inventory_item)
            .ok_or_else(|| AdminShopifyError::NotFound(format!("Inventory item {id} not found")))
    }

    /// Adjust inventory quantity (delta adjustment).
    ///
    /// # Arguments
    ///
    /// * `inventory_item_id` - Shopify inventory item ID
    /// * `location_id` - Shopify location ID
    /// * `delta` - Amount to adjust (positive or negative)
    /// * `reason` - Optional reason for adjustment
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(inventory_item_id = %inventory_item_id, location_id = %location_id, delta = %delta))]
    pub async fn adjust_inventory(
        &self,
        inventory_item_id: &str,
        location_id: &str,
        delta: i64,
        reason: Option<&str>,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::inventory_adjust_quantities::{
            InventoryAdjustQuantitiesInput, InventoryChangeInput,
        };

        let variables = super::queries::inventory_adjust_quantities::Variables {
            input: InventoryAdjustQuantitiesInput {
                name: "available".to_string(),
                reason: reason.unwrap_or("Manual adjustment").to_string(),
                reference_document_uri: None,
                changes: vec![InventoryChangeInput {
                    inventory_item_id: inventory_item_id.to_string(),
                    location_id: location_id.to_string(),
                    delta,
                    change_from_quantity: None,
                    ledger_document_uri: None,
                }],
            },
        };

        let response = self.execute::<InventoryAdjustQuantities>(variables).await?;

        if let Some(payload) = response.inventory_adjust_quantities
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| e.message.clone())
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Set inventory quantity to an absolute value.
    ///
    /// # Arguments
    ///
    /// * `inventory_item_id` - Shopify inventory item ID
    /// * `location_id` - Shopify location ID
    /// * `quantity` - Quantity to set
    /// * `reason` - Optional reason for adjustment
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self), fields(inventory_item_id = %inventory_item_id, location_id = %location_id, quantity = %quantity))]
    pub async fn set_inventory(
        &self,
        inventory_item_id: &str,
        location_id: &str,
        quantity: i64,
        reason: Option<&str>,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::inventory_set_quantities::{
            InventoryQuantityInput, InventorySetQuantitiesInput,
        };

        let variables = super::queries::inventory_set_quantities::Variables {
            input: InventorySetQuantitiesInput {
                name: "on_hand".to_string(),
                reason: reason.unwrap_or("Manual adjustment").to_string(),
                reference_document_uri: None,
                quantities: vec![InventoryQuantityInput {
                    inventory_item_id: inventory_item_id.to_string(),
                    location_id: location_id.to_string(),
                    quantity,
                    change_from_quantity: None,
                }],
            },
        };

        let response = self.execute::<InventorySetQuantities>(variables).await?;

        if let Some(payload) = response.inventory_set_quantities
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| e.message.clone())
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Update inventory item properties.
    ///
    /// # Arguments
    ///
    /// * `id` - Inventory item ID
    /// * `input` - Fields to update
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self, input), fields(id = %id))]
    pub async fn update_inventory_item(
        &self,
        id: &str,
        input: &InventoryItemUpdateInput,
    ) -> Result<InventoryItem, AdminShopifyError> {
        use super::queries::update_inventory_item::{CountryCode, InventoryItemInput};

        let country_code = input
            .country_code_of_origin
            .as_ref()
            .map(|code| match code.as_str() {
                "US" => CountryCode::US,
                "CN" => CountryCode::CN,
                "VN" => CountryCode::VN,
                "BD" => CountryCode::BD,
                "IN" => CountryCode::IN,
                "ID" => CountryCode::ID,
                "TH" => CountryCode::TH,
                "PK" => CountryCode::PK,
                "TR" => CountryCode::TR,
                "KH" => CountryCode::KH,
                "MX" => CountryCode::MX,
                "IT" => CountryCode::IT,
                "PT" => CountryCode::PT,
                "ES" => CountryCode::ES,
                "GB" => CountryCode::GB,
                "CA" => CountryCode::CA,
                "AU" => CountryCode::AU,
                "JP" => CountryCode::JP,
                "KR" => CountryCode::KR,
                "TW" => CountryCode::TW,
                _ => CountryCode::Other(code.clone()),
            });

        let variables = super::queries::update_inventory_item::Variables {
            id: id.to_string(),
            input: InventoryItemInput {
                tracked: input.tracked,
                country_code_of_origin: country_code,
                province_code_of_origin: input.province_code_of_origin.clone(),
                harmonized_system_code: input.harmonized_system_code.clone(),
                cost: None,
                country_harmonized_system_codes: None,
                measurement: None,
                requires_shipping: input.requires_shipping,
                sku: None,
            },
        };

        let response = self.execute::<UpdateInventoryItem>(variables).await?;

        if let Some(ref payload) = response.inventory_item_update
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| e.message.clone())
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        self.get_inventory_item(id).await
    }

    /// Move inventory from one location to another.
    ///
    /// # Arguments
    ///
    /// * `inventory_item_id` - The inventory item ID
    /// * `from_location_id` - Source location ID
    /// * `to_location_id` - Destination location ID
    /// * `quantity` - Quantity to move
    /// * `reason` - Reason for the move
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn move_inventory(
        &self,
        inventory_item_id: &str,
        from_location_id: &str,
        to_location_id: &str,
        quantity: i64,
        reason: Option<&str>,
    ) -> Result<(), AdminShopifyError> {
        use super::queries::move_inventory::{
            InventoryMoveQuantitiesInput, InventoryMoveQuantityChange,
            InventoryMoveQuantityTerminalInput,
        };

        let variables = super::queries::move_inventory::Variables {
            input: InventoryMoveQuantitiesInput {
                changes: vec![InventoryMoveQuantityChange {
                    inventory_item_id: inventory_item_id.to_string(),
                    quantity,
                    from: InventoryMoveQuantityTerminalInput {
                        location_id: from_location_id.to_string(),
                        name: "available".to_string(),
                        ledger_document_uri: None,
                        change_from_quantity: None,
                    },
                    to: InventoryMoveQuantityTerminalInput {
                        location_id: to_location_id.to_string(),
                        name: "available".to_string(),
                        ledger_document_uri: None,
                        change_from_quantity: None,
                    },
                }],
                reason: reason.unwrap_or("Stock transfer").to_string(),
                reference_document_uri: String::new(),
            },
        };

        let response = self.execute::<MoveInventory>(variables).await?;

        if let Some(ref payload) = response.inventory_move_quantities
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| e.message.clone())
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Activate inventory tracking at a location.
    ///
    /// # Arguments
    ///
    /// * `inventory_item_id` - The inventory item ID
    /// * `location_id` - The location to activate at
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn activate_inventory(
        &self,
        inventory_item_id: &str,
        location_id: &str,
    ) -> Result<(), AdminShopifyError> {
        let variables = super::queries::activate_inventory::Variables {
            inventory_item_id: inventory_item_id.to_string(),
            location_id: location_id.to_string(),
        };

        let response = self.execute::<ActivateInventory>(variables).await?;

        if let Some(ref payload) = response.inventory_activate
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| e.message.clone())
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }

    /// Deactivate inventory tracking at a location.
    ///
    /// # Arguments
    ///
    /// * `inventory_level_id` - The inventory level ID (not item ID)
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or returns user errors.
    #[instrument(skip(self))]
    pub async fn deactivate_inventory(
        &self,
        inventory_level_id: &str,
    ) -> Result<(), AdminShopifyError> {
        let variables = super::queries::deactivate_inventory::Variables {
            inventory_level_id: inventory_level_id.to_string(),
        };

        let response = self.execute::<DeactivateInventory>(variables).await?;

        if let Some(ref payload) = response.inventory_deactivate
            && !payload.user_errors.is_empty()
        {
            let error_messages: Vec<String> = payload
                .user_errors
                .iter()
                .map(|e| e.message.clone())
                .collect();
            return Err(AdminShopifyError::UserError(error_messages.join("; ")));
        }

        Ok(())
    }
}
