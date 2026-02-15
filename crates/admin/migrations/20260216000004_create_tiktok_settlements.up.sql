-- TikTok Shop settlement and finance data.

SET search_path TO admin, public;

CREATE TABLE admin.tiktok_settlements (
    id                          SERIAL PRIMARY KEY,
    settlement_id               TEXT NOT NULL UNIQUE,
    settlement_period_start     TIMESTAMPTZ,
    settlement_period_end       TIMESTAMPTZ,
    status                      TEXT NOT NULL DEFAULT 'on_hold',
    total_revenue               TEXT,
    total_refunds               TEXT,
    total_platform_fees         TEXT,
    total_affiliate_commission  TEXT,
    net_payout                  TEXT,
    currency                    TEXT,
    payout_date                 TIMESTAMPTZ,
    raw_json                    JSONB,
    created_at                  TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at                  TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER tiktok_settlements_updated_at
    BEFORE UPDATE ON admin.tiktok_settlements
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();

CREATE TABLE admin.tiktok_settlement_line_items (
    id                      SERIAL PRIMARY KEY,
    settlement_id           TEXT NOT NULL REFERENCES admin.tiktok_settlements(settlement_id) ON DELETE CASCADE,
    tiktok_order_id         TEXT,
    order_amount            TEXT,
    refund_amount           TEXT,
    referral_fee            TEXT,
    affiliate_commission    TEXT,
    shipping_fee_subsidy    TEXT,
    net_amount              TEXT,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_tiktok_settlement_line_items_settlement ON admin.tiktok_settlement_line_items(settlement_id);
