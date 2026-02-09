//! Shopify product lookups for email triage context.
//!
//! Provides product data to the response composer so draft replies can
//! reference real product availability, sizing, and pricing.

use serde_json::json;
use tracing::{debug, instrument};

use super::client::{ShopifyClient, ShopifyError};

/// Summary of a Shopify product for inclusion in response prompts.
#[derive(Debug)]
pub struct ProductSummary {
    /// Product title.
    pub title: String,
    /// Product description (plain text, truncated).
    pub description: String,
    /// Whether the product is currently available.
    pub available: bool,
    /// Variant summaries (size/color + price + in-stock).
    pub variants: Vec<VariantSummary>,
}

/// Summary of a product variant.
#[derive(Debug)]
pub struct VariantSummary {
    /// Variant title (e.g., "Small / Blue").
    pub title: String,
    /// Price with currency.
    pub price: String,
    /// Whether this variant is in stock.
    pub available: bool,
}

const PRODUCT_SEARCH_QUERY: &str = r"
query ProductSearch($query: String!) {
    products(first: 3, query: $query) {
        nodes {
            title
            description(truncateAt: 500)
            status
            totalInventory
            variants(first: 20) {
                nodes {
                    title
                    price
                    inventoryQuantity
                    availableForSale
                }
            }
        }
    }
}
";

/// Search for products by name or keyword.
#[instrument(skip(client))]
pub async fn search(
    client: &ShopifyClient,
    query: &str,
) -> Result<Vec<ProductSummary>, ShopifyError> {
    debug!(query = %query, "searching products");

    let data = client
        .graphql(PRODUCT_SEARCH_QUERY, json!({ "query": query }))
        .await?;

    Ok(parse_products(&data))
}

/// Parse product nodes from the GraphQL response.
fn parse_products(data: &serde_json::Value) -> Vec<ProductSummary> {
    let Some(nodes) = data
        .get("products")
        .and_then(|p| p.get("nodes"))
        .and_then(|n| n.as_array())
    else {
        return Vec::new();
    };

    nodes.iter().filter_map(parse_single_product).collect()
}

/// Parse a single product node.
fn parse_single_product(node: &serde_json::Value) -> Option<ProductSummary> {
    let title = node.get("title")?.as_str()?.to_string();

    let description = node
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let status = node
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("DRAFT");
    let total_inventory = node
        .get("totalInventory")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);
    let available = status == "ACTIVE" && total_inventory > 0;

    let variants = node
        .get("variants")
        .and_then(|v| v.get("nodes"))
        .and_then(|n| n.as_array())
        .map(|items| items.iter().map(parse_variant).collect())
        .unwrap_or_default();

    Some(ProductSummary {
        title,
        description,
        available,
        variants,
    })
}

/// Parse a single variant node.
fn parse_variant(node: &serde_json::Value) -> VariantSummary {
    let title = node
        .get("title")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Default")
        .to_string();

    let price = node
        .get("price")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("0.00")
        .to_string();

    let available = node
        .get("availableForSale")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    VariantSummary {
        title,
        price,
        available,
    }
}

/// Format product summaries as context text for the response prompt.
#[must_use]
pub fn format_products_for_prompt(products: &[ProductSummary]) -> String {
    use std::fmt::Write;

    if products.is_empty() {
        return String::new();
    }

    let mut out = String::with_capacity(1000);
    out.push_str("Shopify Product Data:\n");

    for product in products {
        let status = if product.available {
            "in stock"
        } else {
            "out of stock"
        };
        let _ = writeln!(out, "- {} ({})", product.title, status);

        if !product.description.is_empty() {
            let _ = writeln!(out, "  Description: {}", product.description);
        }

        let available_variants: Vec<&VariantSummary> =
            product.variants.iter().filter(|v| v.available).collect();
        if !available_variants.is_empty() {
            let variant_strs: Vec<String> = available_variants
                .iter()
                .map(|v| format!("{} (${v})", v.title, v = v.price))
                .collect();
            let _ = writeln!(
                out,
                "  Available sizes/options: {}",
                variant_strs.join(", ")
            );
        }
    }

    out
}
