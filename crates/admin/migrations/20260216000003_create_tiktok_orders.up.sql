-- TikTok Shop orders cached locally from the TikTok Shop Open API.
-- Includes TikTok-native fields: content attribution, creator/affiliate,
-- FBT fulfillment, and shipping tracking.

SET search_path TO admin, public;

-- TikTok orders
CREATE TABLE admin.tiktok_orders (
    id                  SERIAL PRIMARY KEY,
    tiktok_order_id     TEXT NOT NULL UNIQUE,
    shopify_order_id    TEXT,
    order_status        TEXT NOT NULL DEFAULT 'UNPAID',
    created_time        TIMESTAMPTZ,
    last_updated_time   TIMESTAMPTZ,
    -- Buyer / shipping
    buyer_name          TEXT,
    buyer_email         TEXT,
    buyer_phone         TEXT,
    ship_name           TEXT,
    ship_street1        TEXT,
    ship_street2        TEXT,
    ship_city           TEXT,
    ship_state          TEXT,
    ship_postal_code    TEXT,
    ship_country        TEXT,
    -- Payment
    payment_amount      TEXT,
    payment_currency    TEXT,
    shipping_amount     TEXT,
    platform_discount   TEXT,
    -- Content attribution (TikTok-specific)
    source_type         TEXT,
    -- Creator / affiliate (TikTok-specific)
    creator_username    TEXT,
    creator_id          TEXT,
    is_affiliate_order  BOOLEAN NOT NULL DEFAULT FALSE,
    commission_rate     NUMERIC,
    commission_amount   TEXT,
    commission_status   TEXT,
    -- Fulfillment (TikTok-specific)
    is_fbt              BOOLEAN NOT NULL DEFAULT FALSE,
    fbt_warehouse_id    TEXT,
    shipping_provider_id TEXT,
    tracking_number     TEXT,
    shipping_status     TEXT,
    -- Raw data
    raw_json            JSONB,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_tiktok_orders_status ON admin.tiktok_orders(order_status);
CREATE INDEX idx_tiktok_orders_created ON admin.tiktok_orders(created_time DESC);
CREATE INDEX idx_tiktok_orders_shopify ON admin.tiktok_orders(shopify_order_id) WHERE shopify_order_id IS NOT NULL;
CREATE INDEX idx_tiktok_orders_source ON admin.tiktok_orders(source_type) WHERE source_type IS NOT NULL;
CREATE INDEX idx_tiktok_orders_creator ON admin.tiktok_orders(creator_username) WHERE creator_username IS NOT NULL;
CREATE INDEX idx_tiktok_orders_affiliate ON admin.tiktok_orders(is_affiliate_order) WHERE is_affiliate_order = TRUE;

CREATE TRIGGER tiktok_orders_updated_at
    BEFORE UPDATE ON admin.tiktok_orders
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();

-- TikTok order line items
CREATE TABLE admin.tiktok_order_items (
    id                  SERIAL PRIMARY KEY,
    tiktok_order_id     TEXT NOT NULL REFERENCES admin.tiktok_orders(tiktok_order_id) ON DELETE CASCADE,
    product_id          TEXT NOT NULL,
    sku_id              TEXT,
    product_name        TEXT,
    quantity            INTEGER NOT NULL DEFAULT 1,
    sale_price          TEXT,
    original_price      TEXT,
    currency            TEXT,
    seller_discount     TEXT,
    platform_discount   TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    UNIQUE(tiktok_order_id, product_id)
);

CREATE INDEX idx_tiktok_order_items_order ON admin.tiktok_order_items(tiktok_order_id);

-- Sync state for tracking high-water-mark
CREATE TABLE admin.tiktok_sync_state (
    id              SERIAL PRIMARY KEY,
    sync_type       TEXT NOT NULL UNIQUE,
    last_sync_at    TIMESTAMPTZ NOT NULL,
    items_synced    INTEGER DEFAULT 0,
    error           TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER tiktok_sync_state_updated_at
    BEFORE UPDATE ON admin.tiktok_sync_state
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();
