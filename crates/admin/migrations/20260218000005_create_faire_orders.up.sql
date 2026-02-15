-- Faire wholesale orders cached locally from the Faire Brand API.
-- Includes wholesale-specific fields: retailer info, payment terms,
-- first-order tracking, and Faire commission/payout.

SET search_path TO admin, public;

-- Faire orders
CREATE TABLE admin.faire_orders (
    id                  SERIAL PRIMARY KEY,
    faire_order_token   TEXT NOT NULL UNIQUE,
    shopify_order_id    TEXT,
    order_state         TEXT NOT NULL DEFAULT 'NEW',
    created_at_faire    TIMESTAMPTZ,
    last_updated_at     TIMESTAMPTZ,
    -- Retailer info (wholesale-specific)
    retailer_id         TEXT,
    retailer_name       TEXT,
    retailer_email      TEXT,
    retailer_phone      TEXT,
    -- Shipping address
    ship_name           TEXT,
    ship_street1        TEXT,
    ship_street2        TEXT,
    ship_city           TEXT,
    ship_state          TEXT,
    ship_postal_code    TEXT,
    ship_country        TEXT,
    -- Financial
    order_total         TEXT,
    currency            TEXT,
    shipping_cost       TEXT,
    faire_commission    TEXT,
    net_payout          TEXT,
    -- Wholesale-specific
    is_first_order      BOOLEAN NOT NULL DEFAULT FALSE,
    payment_terms       TEXT,
    -- Raw data
    raw_json            JSONB,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_faire_orders_state ON admin.faire_orders(order_state);
CREATE INDEX idx_faire_orders_created ON admin.faire_orders(created_at_faire DESC);
CREATE INDEX idx_faire_orders_shopify ON admin.faire_orders(shopify_order_id) WHERE shopify_order_id IS NOT NULL;
CREATE INDEX idx_faire_orders_retailer ON admin.faire_orders(retailer_id) WHERE retailer_id IS NOT NULL;
CREATE INDEX idx_faire_orders_first ON admin.faire_orders(is_first_order) WHERE is_first_order = TRUE;

CREATE TRIGGER faire_orders_updated_at
    BEFORE UPDATE ON admin.faire_orders
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();

-- Faire order line items
CREATE TABLE admin.faire_order_items (
    id                      SERIAL PRIMARY KEY,
    faire_order_token       TEXT NOT NULL REFERENCES admin.faire_orders(faire_order_token) ON DELETE CASCADE,
    product_token           TEXT NOT NULL,
    product_option_token    TEXT,
    product_name            TEXT,
    quantity                INTEGER NOT NULL DEFAULT 1,
    unit_price              TEXT,
    total_price             TEXT,
    sku                     TEXT,
    currency                TEXT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    UNIQUE(faire_order_token, product_token)
);

CREATE INDEX idx_faire_order_items_order ON admin.faire_order_items(faire_order_token);

-- Sync state for tracking high-water-mark
CREATE TABLE admin.faire_sync_state (
    id              SERIAL PRIMARY KEY,
    sync_type       TEXT NOT NULL UNIQUE,
    last_sync_at    TIMESTAMPTZ NOT NULL,
    items_synced    INTEGER DEFAULT 0,
    error           TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER faire_sync_state_updated_at
    BEFORE UPDATE ON admin.faire_sync_state
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();
