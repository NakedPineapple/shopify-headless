//! Low stock alert workflow.
//!
//! Runs on a schedule (default: every hour) and performs:
//!
//! 1. **Fetch**: Query Shopify for all active products with inventory levels.
//! 2. **Filter**: Identify products below the configured threshold.
//! 3. **Deduplicate**: Check `automation_log` to avoid alerting for the same
//!    product within 24 hours.
//! 4. **Alert**: Send a Slack notification for each newly low-stock product.
//! 5. **Email**: Optionally send email alerts to configured recipients.
//! 6. **Log**: Record the run in `automation_log` with product details.

use naked_pineapple_services::slack::{Block, ContextElement, PlainText, SlackClient, Text};
use sqlx::PgPool;
use tracing::{debug, error, info, instrument, warn};

use crate::db::automation_log;
use crate::outbound;
use crate::shopify::ShopifyClient;
use crate::shopify::inventory;

/// Service references needed by the low stock workflow.
pub struct LowStockClients<'a> {
    /// Database connection pool.
    pub pool: &'a PgPool,
    /// Shopify Admin API client.
    pub shopify: &'a ShopifyClient,
    /// Slack client for sending alerts.
    pub slack: &'a SlackClient,
    /// Inventory threshold below which an alert is triggered.
    pub threshold: i32,
    /// Email recipients for low stock alerts (empty = no email alerts).
    pub email_recipients: &'a [String],
}

/// Run the complete low stock monitoring workflow.
#[instrument(skip(clients), fields(threshold = clients.threshold))]
pub async fn run(clients: &LowStockClients<'_>) {
    let start = std::time::Instant::now();

    let log_id = match automation_log::start_run(clients.pool, "low_stock").await {
        Ok(id) => id,
        Err(e) => {
            error!(error = %e, "failed to start automation log");
            return;
        }
    };

    match check_inventory(clients).await {
        Ok((processed, alerted)) => {
            let duration = i64::try_from(start.elapsed().as_millis()).unwrap_or(0);
            if let Err(e) = automation_log::complete_run(
                clients.pool,
                log_id,
                processed,
                alerted,
                None,
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

/// Check inventory levels and send alerts. Returns (processed, alerted) counts.
async fn check_inventory(clients: &LowStockClients<'_>) -> Result<(i32, i32), String> {
    let products = inventory::fetch_all_inventory(clients.shopify)
        .await
        .map_err(|e| format!("failed to fetch inventory: {e}"))?;

    let low_stock: Vec<_> = products
        .iter()
        .filter(|p| p.total_inventory < clients.threshold && p.total_inventory >= 0)
        .collect();

    let total = i32::try_from(products.len()).unwrap_or(0);

    if low_stock.is_empty() {
        debug!(products_checked = total, "no low stock products found");
        return Ok((total, 0));
    }

    debug!(
        count = low_stock.len(),
        threshold = clients.threshold,
        "found low stock products"
    );

    let mut alerted = 0i32;
    for product in &low_stock {
        // Deduplicate: skip if we already alerted for this product in the last 24h
        match automation_log::has_recent_alert(clients.pool, "low_stock_alert", &product.id, 24)
            .await
        {
            Ok(true) => {
                debug!(
                    product = %product.title,
                    "skipping low stock alert (already alerted within 24h)"
                );
                continue;
            }
            Ok(false) => {}
            Err(e) => {
                warn!(
                    product = %product.title,
                    error = %e,
                    "failed to check recent alerts, sending alert anyway"
                );
            }
        }

        send_slack_alert(clients, product).await;
        send_email_alerts(clients, product).await;
        log_alert(clients.pool, product).await;
        alerted += 1;
    }

    if alerted > 0 {
        info!(
            alerted = alerted,
            total_low = low_stock.len(),
            "low stock alerts sent"
        );
    }

    Ok((total, alerted))
}

/// Send a Slack notification for a low stock product.
async fn send_slack_alert(clients: &LowStockClients<'_>, product: &inventory::ProductInventory) {
    let variant_details = format_variant_details(product);

    let blocks = vec![
        Block::Header {
            text: PlainText::new("Low Stock Alert"),
        },
        Block::Section {
            text: Text::mrkdwn(format!(
                "*{}*\nTotal inventory: *{}* units (threshold: {})",
                product.title, product.total_inventory, clients.threshold,
            )),
            accessory: None,
        },
        Block::Section {
            text: Text::mrkdwn(variant_details),
            accessory: None,
        },
        Block::Context {
            elements: vec![ContextElement::Mrkdwn {
                text: "Automated low stock alert from email-automation service".to_string(),
            }],
        },
    ];

    let channel = clients.slack.default_channel();
    let fallback = format!(
        "Low stock: {} ({} units remaining)",
        product.title, product.total_inventory
    );

    if let Err(e) = clients
        .slack
        .post_message(channel, blocks, Some(&fallback))
        .await
    {
        warn!(
            product = %product.title,
            error = %e,
            "failed to send low stock Slack alert"
        );
    }
}

/// Format variant-level inventory details for the Slack message.
fn format_variant_details(product: &inventory::ProductInventory) -> String {
    use std::fmt::Write;

    if product.variants.is_empty() {
        return "No variant data available".to_string();
    }

    let mut out = String::from("*Variant breakdown:*\n");
    for variant in &product.variants {
        let sku_part = variant
            .sku
            .as_deref()
            .map_or(String::new(), |s| format!(" (SKU: {s})"));
        let _ = writeln!(
            out,
            "  {} {} — {} units",
            if variant.inventory_quantity <= 0 {
                ":red_circle:"
            } else {
                ":large_yellow_circle:"
            },
            variant.title,
            variant.inventory_quantity
        );
        if !sku_part.is_empty() {
            // Trim the newline we just added and append SKU
            if out.ends_with('\n') {
                out.pop();
            }
            let _ = writeln!(out, "{sku_part}");
        }
    }

    out
}

/// Send low stock alert emails to configured recipients.
async fn send_email_alerts(clients: &LowStockClients<'_>, product: &inventory::ProductInventory) {
    if clients.email_recipients.is_empty() {
        return;
    }

    let data = outbound::LowStockAlertData {
        product_title: product.title.clone(),
        total_inventory: product.total_inventory,
        threshold: clients.threshold,
        variants: product
            .variants
            .iter()
            .map(|v| outbound::LowStockVariantData {
                title: v.title.clone(),
                sku: v.sku.clone(),
                inventory_quantity: v.inventory_quantity,
            })
            .collect(),
    };

    for recipient in clients.email_recipients {
        if let Err(e) =
            outbound::enqueue_low_stock_alert(clients.pool, recipient, &product.id, &data).await
        {
            warn!(
                product = %product.title,
                recipient = %recipient,
                error = %e,
                "failed to enqueue low stock alert email"
            );
        }
    }
}

/// Log an individual product alert to `automation_log` for deduplication.
async fn log_alert(pool: &PgPool, product: &inventory::ProductInventory) {
    let metadata = serde_json::json!({
        "product_id": product.id,
        "product_title": product.title,
        "total_inventory": product.total_inventory,
    });

    let log_id = match automation_log::start_run(pool, "low_stock_alert").await {
        Ok(id) => id,
        Err(e) => {
            warn!(error = %e, "failed to log low stock alert");
            return;
        }
    };

    if let Err(e) = automation_log::complete_run(pool, log_id, 1, 1, Some(&metadata), 0).await {
        warn!(error = %e, "failed to complete low stock alert log");
    }
}
