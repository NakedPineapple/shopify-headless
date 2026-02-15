-- Faire wholesale payout and settlement data.

SET search_path TO admin, public;

CREATE TABLE admin.faire_payouts (
    id                          SERIAL PRIMARY KEY,
    faire_payout_token          TEXT NOT NULL UNIQUE,
    payout_period_start         TIMESTAMPTZ,
    payout_period_end           TIMESTAMPTZ,
    total_revenue               TEXT,
    total_refunds               TEXT,
    total_commission            TEXT,
    total_shipping_fees         TEXT,
    net_payout                  TEXT,
    currency                    TEXT,
    payout_status               TEXT NOT NULL DEFAULT 'PENDING',
    payout_date                 TIMESTAMPTZ,
    raw_json                    JSONB,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER faire_payouts_updated_at
    BEFORE UPDATE ON admin.faire_payouts
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();

CREATE TABLE admin.faire_payout_line_items (
    id                      SERIAL PRIMARY KEY,
    faire_payout_token      TEXT NOT NULL REFERENCES admin.faire_payouts(faire_payout_token) ON DELETE CASCADE,
    faire_order_token       TEXT NOT NULL,
    order_amount            TEXT,
    refund_amount           TEXT,
    commission_amount       TEXT,
    shipping_fee            TEXT,
    net_amount              TEXT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    UNIQUE(faire_payout_token, faire_order_token)
);

CREATE INDEX idx_faire_payout_line_items_payout ON admin.faire_payout_line_items(faire_payout_token);
