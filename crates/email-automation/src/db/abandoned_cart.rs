//! Database operations for the `abandoned_cart` table.
//!
//! Tracks abandoned Shopify checkouts and their recovery status
//! through the lifecycle: `detected` → `first_email_sent` → `recovered`.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use tracing::instrument;

use super::{RepositoryError, to_time_offset};

/// Parameters for inserting a new abandoned cart record.
pub struct InsertParams<'a> {
    /// Shopify checkout global ID.
    pub shopify_checkout_id: &'a str,
    /// Customer email address.
    pub customer_email: Option<&'a str>,
    /// Cart total amount.
    pub cart_total: Option<Decimal>,
    /// Line items as JSON array.
    pub line_items: &'a serde_json::Value,
    /// When the cart was abandoned.
    pub abandoned_at: DateTime<Utc>,
}

/// An abandoned cart row for recovery trigger processing.
pub struct DetectedCartRow {
    /// Row ID.
    pub id: i32,
    /// Shopify checkout global ID.
    pub shopify_checkout_id: String,
    /// Customer email address.
    pub customer_email: Option<String>,
    /// Cart total amount.
    pub cart_total: Option<Decimal>,
    /// Line items as JSON.
    pub line_items: serde_json::Value,
}

/// An abandoned cart row for recovery detection.
pub struct PendingRecoveryRow {
    /// Row ID.
    pub id: i32,
    /// Customer email address.
    pub customer_email: Option<String>,
    /// When the cart was abandoned, as an ISO 8601 string.
    pub abandoned_at_str: String,
}

/// Check if an abandoned cart with this Shopify checkout ID already exists.
#[instrument(skip(pool))]
pub async fn exists_by_checkout_id(
    pool: &PgPool,
    shopify_checkout_id: &str,
) -> Result<bool, RepositoryError> {
    let exists = sqlx::query_scalar!(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM admin.abandoned_cart
            WHERE shopify_checkout_id = $1
        ) AS "exists!"
        "#,
        shopify_checkout_id,
    )
    .fetch_one(pool)
    .await?;

    Ok(exists)
}

/// Insert a new abandoned cart record. Returns the new row's ID.
#[instrument(skip(pool, params), fields(checkout_id = %params.shopify_checkout_id))]
pub async fn insert(pool: &PgPool, params: &InsertParams<'_>) -> Result<i32, RepositoryError> {
    let id = sqlx::query_scalar!(
        r#"
        INSERT INTO admin.abandoned_cart (
            shopify_checkout_id, customer_email, cart_total,
            line_items, abandoned_at
        )
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id
        "#,
        params.shopify_checkout_id,
        params.customer_email,
        params.cart_total,
        params.line_items,
        to_time_offset(params.abandoned_at),
    )
    .fetch_one(pool)
    .await?;

    Ok(id)
}

/// Fetch abandoned carts in `detected` status, ready for recovery trigger.
#[instrument(skip(pool))]
pub async fn fetch_detected(pool: &PgPool) -> Result<Vec<DetectedCartRow>, RepositoryError> {
    let rows = sqlx::query_as!(
        DetectedCartRow,
        r#"
        SELECT id, shopify_checkout_id,
               customer_email,
               cart_total,
               line_items
        FROM admin.abandoned_cart
        WHERE recovery_status = 'detected'
        ORDER BY abandoned_at ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Update an abandoned cart's status to `first_email_sent` after triggering
/// the Klaviyo recovery flow.
#[instrument(skip(pool), fields(%cart_id))]
pub async fn mark_recovery_triggered(pool: &PgPool, cart_id: i32) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        UPDATE admin.abandoned_cart
        SET recovery_status = 'first_email_sent',
            first_email_at = NOW()
        WHERE id = $1
        "#,
        cart_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Fetch abandoned carts in `first_email_sent` status for recovery detection.
///
/// Returns the customer email and abandoned timestamp as a string so we can
/// query Shopify for orders placed after the cart was abandoned.
#[instrument(skip(pool))]
pub async fn fetch_pending_recovery(
    pool: &PgPool,
) -> Result<Vec<PendingRecoveryRow>, RepositoryError> {
    let rows = sqlx::query_as!(
        PendingRecoveryRow,
        r#"
        SELECT id,
               customer_email,
               to_char(abandoned_at AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24:MI:SS"Z"') AS "abandoned_at_str!"
        FROM admin.abandoned_cart
        WHERE recovery_status = 'first_email_sent'
        ORDER BY abandoned_at ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

/// Mark an abandoned cart as recovered when the customer completes a purchase.
#[instrument(skip(pool), fields(%cart_id))]
pub async fn mark_recovered(
    pool: &PgPool,
    cart_id: i32,
    order_id: &str,
) -> Result<(), RepositoryError> {
    sqlx::query!(
        r#"
        UPDATE admin.abandoned_cart
        SET recovery_status = 'recovered',
            recovered_at = NOW(),
            recovery_order_id = $1
        WHERE id = $2
        "#,
        order_id,
        cart_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}
