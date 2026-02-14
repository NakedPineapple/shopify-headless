//! Amazon SP-API Product Pricing API (v0).
//!
//! Provides competitive pricing intelligence for ASINs, including buy box
//! prices, offer counts, and sales rankings.

use serde::Deserialize;
use tracing::instrument;

use super::AmazonSpError;
use super::client::AmazonSpClient;

/// Maximum ASINs per request (API limit).
const MAX_ASINS_PER_REQUEST: usize = 20;

// =============================================================================
// Response Types (PascalCase JSON)
// =============================================================================

/// Top-level response wrapper for a single ASIN's competitive pricing result.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PricingResultPayload {
    pub status: String,
    #[serde(rename = "ASIN")]
    pub asin: Option<String>,
    pub product: Option<PricingProduct>,
}

/// Product pricing data.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PricingProduct {
    pub competitive_pricing: Option<CompetitivePricingData>,
    pub sales_rankings: Option<Vec<SalesRanking>>,
}

/// Competitive pricing data for a product.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CompetitivePricingData {
    pub competitive_prices: Option<Vec<CompetitivePrice>>,
    pub number_of_offer_listings: Option<Vec<OfferListingCount>>,
}

/// A single competitive price entry.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CompetitivePrice {
    #[serde(rename = "CompetitivePriceId")]
    pub price_id: Option<String>,
    pub price: Option<PriceBreakdown>,
    pub belongs_to_requester: Option<bool>,
}

/// Price breakdown (only landed price used for buy box comparison).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PriceBreakdown {
    pub landed_price: Option<MoneyType>,
}

/// Monetary amount with currency code.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct MoneyType {
    pub currency_code: Option<String>,
    pub amount: Option<f64>,
}

/// Number of offers at a given condition.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct OfferListingCount {
    pub count: i32,
}

/// Sales ranking in a product category.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SalesRanking {
    pub rank: i32,
}

/// Query parameters for the competitive pricing endpoint.
#[derive(serde::Serialize)]
#[serde(rename_all = "PascalCase")]
struct CompetitivePricingQuery {
    marketplace_id: String,
    #[serde(rename = "Asins")]
    asins: String,
    item_type: String,
}

/// Response from `GET /products/pricing/v0/competitivePrice`.
#[derive(Debug, Deserialize)]
struct CompetitivePricingResponse {
    pub payload: Option<Vec<PricingResultPayload>>,
    pub errors: Option<Vec<super::types::SpApiError>>,
}

// =============================================================================
// Public Result Type
// =============================================================================

/// Competitive pricing result for a single ASIN.
#[derive(Debug, Clone)]
pub struct PricingResult {
    pub status: String,
    pub asin: Option<String>,
    pub buy_box_price: Option<f64>,
    pub buy_box_currency: Option<String>,
    pub belongs_to_requester: Option<bool>,
    pub num_offers: i32,
    pub sales_rank: Option<i32>,
}

/// Convert raw API payload into the public result type.
fn to_pricing_result(payload: PricingResultPayload) -> PricingResult {
    let (buy_box_price, buy_box_currency, belongs_to_requester) =
        extract_buy_box(payload.product.as_ref());
    let num_offers = extract_offer_count(payload.product.as_ref());
    let sales_rank = extract_sales_rank(payload.product.as_ref());

    PricingResult {
        status: payload.status,
        asin: payload.asin,
        buy_box_price,
        buy_box_currency,
        belongs_to_requester,
        num_offers,
        sales_rank,
    }
}

/// Extract buy box price info from the product data.
fn extract_buy_box(
    product: Option<&PricingProduct>,
) -> (Option<f64>, Option<String>, Option<bool>) {
    let Some(product) = product else {
        return (None, None, None);
    };
    let Some(cp) = &product.competitive_pricing else {
        return (None, None, None);
    };
    let Some(prices) = &cp.competitive_prices else {
        return (None, None, None);
    };

    // Find the "1" (Buy Box) competitive price
    let buy_box = prices.iter().find(|p| p.price_id.as_deref() == Some("1"));

    buy_box.map_or((None, None, None), |bb| {
        let price = bb
            .price
            .as_ref()
            .and_then(|p| p.landed_price.as_ref())
            .and_then(|lp| lp.amount);
        let currency = bb
            .price
            .as_ref()
            .and_then(|p| p.landed_price.as_ref())
            .and_then(|lp| lp.currency_code.clone());
        (price, currency, bb.belongs_to_requester)
    })
}

/// Extract total offer count from competitive pricing data.
fn extract_offer_count(product: Option<&PricingProduct>) -> i32 {
    product
        .and_then(|p| p.competitive_pricing.as_ref())
        .and_then(|cp| cp.number_of_offer_listings.as_ref())
        .map_or(0, |listings| listings.iter().map(|l| l.count).sum())
}

/// Extract the best sales rank from rankings data.
fn extract_sales_rank(product: Option<&PricingProduct>) -> Option<i32> {
    product
        .and_then(|p| p.sales_rankings.as_ref())
        .and_then(|rankings| rankings.first())
        .map(|r| r.rank)
}

// =============================================================================
// Client Implementation
// =============================================================================

impl AmazonSpClient {
    /// Get competitive pricing for a list of ASINs.
    ///
    /// Automatically batches requests when more than 20 ASINs are provided.
    /// Rate limit: 10 requests/second.
    ///
    /// # Errors
    ///
    /// Returns error if any batch request fails.
    #[instrument(skip(self), fields(asin_count = asins.len()))]
    pub async fn get_competitive_pricing(
        &self,
        asins: &[String],
    ) -> Result<Vec<PricingResult>, AmazonSpError> {
        let mut all_results = Vec::with_capacity(asins.len());

        for chunk in asins.chunks(MAX_ASINS_PER_REQUEST) {
            let results = self.fetch_competitive_pricing_batch(chunk).await?;
            all_results.extend(results);
        }

        Ok(all_results)
    }

    /// Fetch competitive pricing for a single batch of ASINs (max 20).
    async fn fetch_competitive_pricing_batch(
        &self,
        asins: &[String],
    ) -> Result<Vec<PricingResult>, AmazonSpError> {
        let query = CompetitivePricingQuery {
            marketplace_id: self.marketplace_id().to_string(),
            asins: asins.join(","),
            item_type: "Asin".to_string(),
        };

        let response: CompetitivePricingResponse = self
            .execute("/products/pricing/v0/competitivePrice", Some(&query))
            .await?;

        if let Some(errors) = response.errors
            && let Some(first) = errors.first()
        {
            return Err(AmazonSpError::Api {
                status: 400,
                message: first.message.clone(),
            });
        }

        let results = response
            .payload
            .unwrap_or_default()
            .into_iter()
            .map(to_pricing_result)
            .collect();

        Ok(results)
    }
}
