//! Shopify inventory level queries for low stock monitoring.
//!
//! Fetches all active products with their inventory quantities so the
//! low stock workflow can compare against the configured threshold.

use serde_json::json;
use tracing::{debug, instrument, warn};

use super::client::{ShopifyClient, ShopifyError};

/// A product with its current inventory level.
#[derive(Debug)]
pub struct ProductInventory {
    /// Shopify product global ID.
    pub id: String,
    /// Product title.
    pub title: String,
    /// Total inventory across all variants and locations.
    pub total_inventory: i32,
    /// Variants with their individual inventory quantities.
    pub variants: Vec<VariantInventory>,
}

/// A product variant with its inventory quantity.
#[derive(Debug)]
pub struct VariantInventory {
    /// Variant title (e.g., "Small / Blue").
    pub title: String,
    /// SKU (if set).
    pub sku: Option<String>,
    /// Current inventory quantity.
    pub inventory_quantity: i32,
}

const INVENTORY_QUERY: &str = r"
query InventoryLevels($first: Int!, $after: String, $query: String) {
    products(first: $first, after: $after, query: $query) {
        nodes {
            id
            title
            totalInventory
            variants(first: 50) {
                nodes {
                    title
                    sku
                    inventoryQuantity
                }
            }
        }
        pageInfo {
            hasNextPage
            endCursor
        }
    }
}
";

/// Fetch all active products with their inventory levels.
///
/// Paginates through all products in batches of 50.
#[instrument(skip(client))]
pub async fn fetch_all_inventory(
    client: &ShopifyClient,
) -> Result<Vec<ProductInventory>, ShopifyError> {
    let mut all_products = Vec::new();
    let mut cursor: Option<String> = None;
    let page_size = 50;

    loop {
        let variables = cursor.as_ref().map_or_else(
            || json!({ "first": page_size, "query": "status:active" }),
            |c| json!({ "first": page_size, "after": c, "query": "status:active" }),
        );

        let data = client.graphql(INVENTORY_QUERY, variables).await?;

        let products_data = data.get("products");
        let nodes = products_data
            .and_then(|p| p.get("nodes"))
            .and_then(|n| n.as_array());

        let Some(nodes) = nodes else {
            warn!("no products found in inventory response");
            break;
        };

        for node in nodes {
            if let Some(product) = parse_product_inventory(node) {
                all_products.push(product);
            }
        }

        let has_next = products_data
            .and_then(|p| p.get("pageInfo"))
            .and_then(|pi| pi.get("hasNextPage"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        if !has_next {
            break;
        }

        cursor = products_data
            .and_then(|p| p.get("pageInfo"))
            .and_then(|pi| pi.get("endCursor"))
            .and_then(|c| c.as_str())
            .map(String::from);
    }

    debug!(count = all_products.len(), "fetched product inventory");
    Ok(all_products)
}

/// Parse a single product node into a `ProductInventory`.
fn parse_product_inventory(node: &serde_json::Value) -> Option<ProductInventory> {
    let id = node.get("id")?.as_str()?.to_string();
    let title = node.get("title")?.as_str()?.to_string();

    let total_inventory = node
        .get("totalInventory")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    let total_inventory = i32::try_from(total_inventory).unwrap_or(0);

    let variants = node
        .get("variants")
        .and_then(|v| v.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|items| items.iter().filter_map(parse_variant_inventory).collect())
        .unwrap_or_default();

    Some(ProductInventory {
        id,
        title,
        total_inventory,
        variants,
    })
}

/// Parse a single variant node into a `VariantInventory`.
fn parse_variant_inventory(node: &serde_json::Value) -> Option<VariantInventory> {
    let title = node.get("title")?.as_str()?.to_string();

    let sku = node
        .get("sku")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from);

    let inventory_quantity = node
        .get("inventoryQuantity")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    let inventory_quantity = i32::try_from(inventory_quantity).unwrap_or(0);

    Some(VariantInventory {
        title,
        sku,
        inventory_quantity,
    })
}
