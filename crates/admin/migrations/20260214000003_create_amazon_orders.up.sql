-- Amazon orders cached locally (Orders API rate limit: 1 req/60s).

SET search_path TO admin, public;

-- Amazon orders
CREATE TABLE admin.amazon_orders (
    id                      SERIAL PRIMARY KEY,
    amazon_order_id         TEXT NOT NULL UNIQUE,
    shopify_order_id        TEXT,
    purchase_date           TIMESTAMPTZ,
    last_update_date        TIMESTAMPTZ,
    order_status            TEXT NOT NULL DEFAULT 'Unknown',
    fulfillment_channel     TEXT,
    sales_channel           TEXT,
    order_type              TEXT,
    order_total_amount      TEXT,
    order_total_currency    TEXT,
    number_of_items_shipped   INTEGER DEFAULT 0,
    number_of_items_unshipped INTEGER DEFAULT 0,
    is_business_order       BOOLEAN DEFAULT FALSE,
    is_prime                BOOLEAN DEFAULT FALSE,
    marketplace_id          TEXT,
    ship_name               TEXT,
    ship_city               TEXT,
    ship_state              TEXT,
    ship_postal_code        TEXT,
    ship_country            TEXT,
    buyer_email             TEXT,
    buyer_name              TEXT,
    raw_json                JSONB,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_amazon_orders_status ON admin.amazon_orders(order_status);
CREATE INDEX idx_amazon_orders_purchase_date ON admin.amazon_orders(purchase_date DESC);
CREATE INDEX idx_amazon_orders_shopify ON admin.amazon_orders(shopify_order_id) WHERE shopify_order_id IS NOT NULL;

CREATE TRIGGER amazon_orders_updated_at
    BEFORE UPDATE ON admin.amazon_orders
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();

-- Amazon order line items
CREATE TABLE admin.amazon_order_items (
    id                  SERIAL PRIMARY KEY,
    amazon_order_id     TEXT NOT NULL REFERENCES admin.amazon_orders(amazon_order_id) ON DELETE CASCADE,
    order_item_id       TEXT NOT NULL,
    asin                TEXT NOT NULL,
    seller_sku          TEXT,
    title               TEXT,
    quantity_ordered     INTEGER NOT NULL DEFAULT 0,
    quantity_shipped     INTEGER DEFAULT 0,
    item_price_amount   TEXT,
    item_price_currency TEXT,
    item_tax_amount     TEXT,
    item_tax_currency   TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    UNIQUE(amazon_order_id, order_item_id)
);

CREATE INDEX idx_amazon_order_items_order ON admin.amazon_order_items(amazon_order_id);
CREATE INDEX idx_amazon_order_items_asin ON admin.amazon_order_items(asin);

-- Sync state for tracking high-water-mark
CREATE TABLE admin.amazon_sync_state (
    id              SERIAL PRIMARY KEY,
    sync_type       TEXT NOT NULL UNIQUE,
    last_sync_at    TIMESTAMPTZ NOT NULL,
    items_synced    INTEGER DEFAULT 0,
    error           TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER amazon_sync_state_updated_at
    BEFORE UPDATE ON admin.amazon_sync_state
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();
