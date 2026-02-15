//! Database operations for admin `PostgreSQL`.
//!
//! # Database: `np_admin` (SEPARATE from storefront)
//!
//! ## Tables
//!
//! - `admin_user` - Admin authentication (separate from storefront users)
//! - `admin_session` - Admin session storage
//! - `admin_credential` - Admin `WebAuthn` passkeys
//! - `admin_invite` - Email allowlist for registration
//! - `chat_session` - Claude AI chat sessions
//! - `chat_message` - Chat message history (JSONB content)
//! - `shopify_token` - Encrypted OAuth tokens (if needed)
//! - `settings` - Application settings (JSONB)
//!
//! # Migrations
//!
//! Migrations are stored in `crates/admin/migrations/` and run via:
//! ```bash
//! cargo run -p naked-pineapple-cli -- migrate admin
//! ```

pub mod admin_invites;
pub mod admin_users;
pub mod amazon;
pub mod amazon_orders;
pub mod amazon_products;
pub mod chat;
pub mod documents;
pub mod expense;
pub mod faire_commerce;
pub mod faire_orders;
pub mod faire_payouts;
pub mod faire_products;
pub mod faire_returns;
pub mod gallery;
pub mod google_commerce;
pub mod google_products;
pub mod inbound_email;
pub mod inventory_lot;
pub mod manufacturing;
pub mod meta_commerce;
pub mod meta_orders;
pub mod meta_products;
pub mod pending_actions;
pub mod pinterest_commerce;
pub mod pinterest_products;
pub mod request_board;
pub mod settings;
pub mod shiphero;
pub mod shopify;
pub mod tiktok_commerce;
pub mod tiktok_orders;
pub mod tiktok_performance;
pub mod tiktok_products;
pub mod tiktok_returns;
pub mod tiktok_settlements;
pub mod tool_examples;

use std::time::Duration;

use secrecy::ExposeSecret;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use thiserror::Error;

pub use admin_invites::{AdminInvite, AdminInviteRepository};
pub use admin_users::AdminUserRepository;
pub use amazon::{AmazonSpCredentials, AmazonSpCredentialsRepository, SaveAmazonSpParams};
pub use amazon_orders::{
    AmazonDailyRevenue, AmazonOrderRepository, AmazonRevenueSummary, CachedAmazonOrder,
    CachedAmazonOrderItem, FulfillmentBreakdown, StatusBreakdown,
};
pub use amazon_products::{
    AmazonProductMapping, AmazonProductMappingRepository, CreateMappingParams,
};
pub use chat::ChatRepository;
pub use expense::ExpenseRepository;
pub use faire_commerce::{FaireCredentials, FaireCredentialsRepository, SaveFaireParams};
pub use faire_orders::{
    CachedFaireOrder, CachedFaireOrderItem, FaireDailyRevenue, FaireOrderRepository,
    FaireRevenueSummary, RetailerBreakdown,
};
pub use faire_payouts::{CachedFairePayout, CachedFairePayoutLineItem, FairePayoutRepository};
pub use faire_products::{
    CreateFaireMappingParams, FaireProductMapping, FaireProductMappingRepository,
};
pub use faire_returns::{CachedFaireReturn, FaireReturnRepository};
pub use google_commerce::{GoogleCredentials, GoogleCredentialsRepository, SaveGoogleParams};
pub use google_products::{
    CreateGoogleMappingParams, GoogleProductMapping, GoogleProductMappingRepository,
};
pub use inventory_lot::InventoryLotRepository;
pub use manufacturing::ManufacturingRepository;
pub use meta_commerce::{
    MetaCommerceCredentials, MetaCommerceCredentialsRepository, SaveMetaCommerceParams,
};
pub use meta_orders::{
    CachedMetaOrder, CachedMetaOrderItem, ChannelBreakdown, MetaDailyRevenue, MetaOrderRepository,
    MetaRevenueSummary, MetaStatusBreakdown,
};
pub use meta_products::{
    CreateMetaMappingParams, MetaProductMapping, MetaProductMappingRepository,
};
pub use pinterest_commerce::{
    PinterestCredentials, PinterestCredentialsRepository, SavePinterestParams,
};
pub use pinterest_products::{
    CreatePinterestMappingParams, PinterestProductMapping, PinterestProductMappingRepository,
};
pub use request_board::RequestBoardRepository;
pub use shiphero::{SaveCredentialsParams, ShipHeroCredentials, ShipHeroCredentialsRepository};
pub use shopify::ShopifyTokenRepository;
pub use tiktok_commerce::{
    SaveTikTokShopParams, TikTokShopCredentials, TikTokShopCredentialsRepository,
};
pub use tiktok_orders::{
    AffiliateSummary, CachedTikTokOrder, CachedTikTokOrderItem, CreatorBreakdown, SourceBreakdown,
    TikTokDailyRevenue, TikTokOrderRepository, TikTokRevenueSummary, TikTokStatusBreakdown,
};
pub use tiktok_performance::{TikTokPerformanceRepository, TikTokPerformanceSnapshot};
pub use tiktok_products::{
    CreateTikTokMappingParams, TikTokProductMapping, TikTokProductMappingRepository,
};
pub use tiktok_returns::{CachedTikTokReturn, TikTokReturnRepository};
pub use tiktok_settlements::{
    CachedTikTokSettlement, CachedTikTokSettlementLineItem, TikTokSettlementRepository,
};

/// Errors that can occur during repository operations.
#[derive(Debug, Error)]
pub enum RepositoryError {
    /// Database error from sqlx.
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    /// Data in the database is corrupted or invalid.
    #[error("data corruption: {0}")]
    DataCorruption(String),

    /// Requested entity was not found.
    #[error("not found")]
    NotFound,

    /// Constraint violation (e.g., unique email).
    #[error("constraint violation: {0}")]
    Conflict(String),

    /// Serialization error (e.g., JSON conversion).
    #[error("serialization error: {0}")]
    Serialization(String),
}

/// Create a `PostgreSQL` connection pool with sensible defaults.
///
/// # Arguments
///
/// * `database_url` - `PostgreSQL` connection string (wrapped in `SecretString`)
///
/// # Errors
///
/// Returns `sqlx::Error` if the connection cannot be established.
pub async fn create_pool(database_url: &secrecy::SecretString) -> Result<PgPool, sqlx::Error> {
    let max_conn: u32 = std::env::var("DATABASE_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5);
    let min_conn: u32 = std::env::var("DATABASE_MIN_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);

    PgPoolOptions::new()
        .max_connections(max_conn)
        .min_connections(min_conn)
        .acquire_timeout(Duration::from_secs(10))
        .connect(database_url.expose_secret())
        .await
}
