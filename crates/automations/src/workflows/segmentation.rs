//! Customer segmentation workflow.
//!
//! Runs on a schedule (default: daily) and performs:
//!
//! 1. **Fetch**: Query Shopify for all enabled customers with order history.
//! 2. **Classify**: Assign each customer to a segment based on order count,
//!    lifetime value, and recency of last purchase.
//! 3. **Tag**: Apply `segment:*` tags in Shopify (removing stale ones).
//! 4. **Sync**: Upsert profile properties in Klaviyo for flow targeting.
//! 5. **Log**: Record the run in `automation_log`.

use naked_pineapple_services::klaviyo::KlaviyoClient;
use naked_pineapple_services::klaviyo::profiles::ProfileUpsertParams;
use sqlx::PgPool;
use tracing::{debug, error, info, instrument, warn};

use crate::db::automation_log;
use crate::shopify::ShopifyClient;
use crate::shopify::customers::{self, CustomerProfile};

/// Service references needed by the segmentation workflow.
pub struct SegmentationClients<'a> {
    /// Database connection pool.
    pub pool: &'a PgPool,
    /// Shopify Admin API client.
    pub shopify: &'a ShopifyClient,
    /// Klaviyo client for profile upserts.
    pub klaviyo: &'a KlaviyoClient,
}

/// Customer segment classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Segment {
    /// 1 order.
    FirstTimeBuyer,
    /// 2-4 orders.
    RepeatCustomer,
    /// 5+ orders or $500+ lifetime value.
    Vip,
    /// No order in 60-89 days after a previous purchase.
    AtRisk,
    /// No order in 90+ days after a previous purchase.
    Lapsed,
}

impl Segment {
    /// The Shopify tag name for this segment.
    const fn tag(self) -> &'static str {
        match self {
            Self::FirstTimeBuyer => "segment:first_time_buyer",
            Self::RepeatCustomer => "segment:repeat_customer",
            Self::Vip => "segment:vip",
            Self::AtRisk => "segment:at_risk",
            Self::Lapsed => "segment:lapsed",
        }
    }

    /// The Klaviyo property value for this segment.
    const fn value(self) -> &'static str {
        match self {
            Self::FirstTimeBuyer => "first_time_buyer",
            Self::RepeatCustomer => "repeat_customer",
            Self::Vip => "vip",
            Self::AtRisk => "at_risk",
            Self::Lapsed => "lapsed",
        }
    }
}

/// Run the complete customer segmentation workflow.
#[instrument(skip(clients))]
pub async fn run(clients: &SegmentationClients<'_>) {
    let start = std::time::Instant::now();

    let log_id = match automation_log::start_run(clients.pool, "segmentation").await {
        Ok(id) => id,
        Err(e) => {
            error!(error = %e, "failed to start automation log");
            return;
        }
    };

    match segment_customers(clients).await {
        Ok((processed, updated)) => {
            let duration = i64::try_from(start.elapsed().as_millis()).unwrap_or(0);
            let metadata = serde_json::json!({
                "customers_processed": processed,
                "customers_updated": updated,
            });
            if let Err(e) = automation_log::complete_run(
                clients.pool,
                log_id,
                processed,
                updated,
                Some(&metadata),
                duration,
            )
            .await
            {
                warn!(error = %e, "failed to complete automation log");
            }
        }
        Err(msg) => {
            let duration = i64::try_from(start.elapsed().as_millis()).unwrap_or(0);
            if let Err(e) = automation_log::fail_run(clients.pool, log_id, &msg, duration).await {
                warn!(error = %e, "failed to record automation failure");
            }
        }
    }
}

/// Fetch, classify, tag, and sync customers. Returns (processed, updated).
async fn segment_customers(clients: &SegmentationClients<'_>) -> Result<(i32, i32), String> {
    let all_customers = customers::fetch_all_customers(clients.shopify)
        .await
        .map_err(|e| format!("failed to fetch customers: {e}"))?;

    let total = i32::try_from(all_customers.len()).unwrap_or(0);

    if all_customers.is_empty() {
        debug!("no customers found for segmentation");
        return Ok((0, 0));
    }

    debug!(count = total, "classifying customers");

    let mut updated = 0i32;
    for customer in &all_customers {
        // Skip customers without email (can't sync to Klaviyo)
        if customer.email.is_none() {
            continue;
        }

        // Skip customers with no orders (nothing to segment)
        if customer.order_count == 0 {
            continue;
        }

        let segment = classify(customer);
        let tag = segment.tag();

        // Check if the tag is already applied
        if customer.tags.iter().any(|t| t == tag) {
            continue;
        }

        // Apply the Shopify tag
        if let Err(e) =
            customers::apply_segment_tag(clients.shopify, &customer.id, &customer.tags, tag).await
        {
            warn!(
                customer_id = %customer.id,
                error = %e,
                "failed to apply segment tag in Shopify"
            );
            continue;
        }

        // Sync to Klaviyo
        sync_to_klaviyo(clients.klaviyo, customer, segment).await;

        updated += 1;
    }

    if updated > 0 {
        info!(
            total = total,
            updated = updated,
            "customer segmentation complete"
        );
    }

    Ok((total, updated))
}

/// Classify a customer into a segment based on their order history.
fn classify(customer: &CustomerProfile) -> Segment {
    let days_since_last_order = customer
        .last_order_at
        .as_deref()
        .and_then(|d| chrono::DateTime::parse_from_rfc3339(d).ok())
        .map(|last| {
            let now = chrono::Utc::now();
            (now - last.with_timezone(&chrono::Utc)).num_days()
        });

    // Check recency first (at_risk / lapsed override other segments)
    if let Some(days) = days_since_last_order {
        if days >= 90 {
            return Segment::Lapsed;
        }
        if days >= 60 {
            return Segment::AtRisk;
        }
    }

    // VIP: 5+ orders or $500+ lifetime value
    let lifetime_value: f64 = customer.amount_spent.parse().unwrap_or(0.0);
    if customer.order_count >= 5 || lifetime_value >= 500.0 {
        return Segment::Vip;
    }

    // Repeat customer: 2-4 orders
    if customer.order_count >= 2 {
        return Segment::RepeatCustomer;
    }

    Segment::FirstTimeBuyer
}

/// Sync customer segment data to Klaviyo profile.
async fn sync_to_klaviyo(klaviyo: &KlaviyoClient, customer: &CustomerProfile, segment: Segment) {
    let Some(email) = &customer.email else {
        return;
    };

    let params = ProfileUpsertParams {
        email,
        first_name: customer.first_name.as_deref(),
        last_name: customer.last_name.as_deref(),
        segment: segment.value(),
        order_count: customer.order_count,
        lifetime_value: &customer.amount_spent,
        last_order_at: customer.last_order_at.as_deref(),
    };

    if let Err(e) = klaviyo.upsert_profile(&params).await {
        warn!(
            email = %email,
            error = %e,
            "failed to upsert profile in Klaviyo"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_customer(
        order_count: i32,
        amount: &str,
        last_order_days_ago: Option<i64>,
    ) -> CustomerProfile {
        let last_order_at = last_order_days_ago.map(|days| {
            let dt = chrono::Utc::now() - chrono::Duration::days(days);
            dt.to_rfc3339()
        });

        CustomerProfile {
            id: "gid://shopify/Customer/1".to_string(),
            email: Some("test@example.com".to_string()),
            first_name: Some("Test".to_string()),
            last_name: Some("User".to_string()),
            order_count,
            amount_spent: amount.to_string(),
            tags: vec![],
            last_order_at,
        }
    }

    #[test]
    fn test_classify_first_time_buyer() {
        let customer = make_customer(1, "49.99", Some(5));
        assert_eq!(classify(&customer), Segment::FirstTimeBuyer);
    }

    #[test]
    fn test_classify_repeat_customer() {
        let customer = make_customer(3, "149.99", Some(10));
        assert_eq!(classify(&customer), Segment::RepeatCustomer);
    }

    #[test]
    fn test_classify_vip_by_orders() {
        let customer = make_customer(5, "200.00", Some(10));
        assert_eq!(classify(&customer), Segment::Vip);
    }

    #[test]
    fn test_classify_vip_by_spend() {
        let customer = make_customer(2, "550.00", Some(10));
        assert_eq!(classify(&customer), Segment::Vip);
    }

    #[test]
    fn test_classify_at_risk() {
        let customer = make_customer(3, "200.00", Some(75));
        assert_eq!(classify(&customer), Segment::AtRisk);
    }

    #[test]
    fn test_classify_lapsed() {
        let customer = make_customer(3, "200.00", Some(100));
        assert_eq!(classify(&customer), Segment::Lapsed);
    }

    #[test]
    fn test_lapsed_overrides_vip() {
        let customer = make_customer(10, "1000.00", Some(120));
        assert_eq!(classify(&customer), Segment::Lapsed);
    }
}
