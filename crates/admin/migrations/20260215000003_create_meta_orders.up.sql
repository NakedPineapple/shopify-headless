-- Meta Commerce orders cached locally from Facebook Shop and Instagram Shopping.

SET search_path TO admin, public;

-- Meta orders
CREATE TABLE admin.meta_orders (
    id                          SERIAL PRIMARY KEY,
    facebook_order_id           TEXT NOT NULL UNIQUE,
    shopify_order_id            TEXT,
    created_time                TIMESTAMPTZ,
    last_updated_time           TIMESTAMPTZ,
    order_status                TEXT NOT NULL DEFAULT 'CREATED',
    channel                     TEXT NOT NULL DEFAULT 'facebook',
    buyer_name                  TEXT,
    buyer_email                 TEXT,
    ship_name                   TEXT,
    ship_street1                TEXT,
    ship_street2                TEXT,
    ship_city                   TEXT,
    ship_state                  TEXT,
    ship_postal_code            TEXT,
    ship_country                TEXT,
    estimated_payment_amount    TEXT,
    estimated_payment_currency  TEXT,
    raw_json                    JSONB,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_meta_orders_status ON admin.meta_orders(order_status);
CREATE INDEX idx_meta_orders_channel ON admin.meta_orders(channel);
CREATE INDEX idx_meta_orders_created ON admin.meta_orders(created_time DESC);
CREATE INDEX idx_meta_orders_shopify ON admin.meta_orders(shopify_order_id) WHERE shopify_order_id IS NOT NULL;

CREATE TRIGGER meta_orders_updated_at
    BEFORE UPDATE ON admin.meta_orders
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();

-- Meta order line items
CREATE TABLE admin.meta_order_items (
    id                  SERIAL PRIMARY KEY,
    facebook_order_id   TEXT NOT NULL REFERENCES admin.meta_orders(facebook_order_id) ON DELETE CASCADE,
    product_id          TEXT NOT NULL,
    retailer_id         TEXT,
    quantity            INTEGER NOT NULL DEFAULT 1,
    price_per_unit      TEXT,
    currency            TEXT,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    UNIQUE(facebook_order_id, product_id)
);

CREATE INDEX idx_meta_order_items_order ON admin.meta_order_items(facebook_order_id);

-- Sync state for tracking high-water-mark
CREATE TABLE admin.meta_sync_state (
    id              SERIAL PRIMARY KEY,
    sync_type       TEXT NOT NULL UNIQUE,
    last_sync_at    TIMESTAMPTZ NOT NULL,
    items_synced    INTEGER DEFAULT 0,
    error           TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER meta_sync_state_updated_at
    BEFORE UPDATE ON admin.meta_sync_state
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();
