-- TikTok Shop returns and refunds.

SET search_path TO admin, public;

CREATE TABLE admin.tiktok_returns (
    id                      SERIAL PRIMARY KEY,
    return_id               TEXT NOT NULL UNIQUE,
    tiktok_order_id         TEXT,
    return_status           TEXT NOT NULL DEFAULT 'pending',
    return_type             TEXT NOT NULL DEFAULT 'return',
    reason                  TEXT,
    buyer_note              TEXT,
    refund_amount           TEXT,
    currency                TEXT,
    decision_deadline       TIMESTAMPTZ,
    return_tracking_number  TEXT,
    raw_json                JSONB,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_tiktok_returns_order ON admin.tiktok_returns(tiktok_order_id) WHERE tiktok_order_id IS NOT NULL;
CREATE INDEX idx_tiktok_returns_status ON admin.tiktok_returns(return_status);

CREATE TRIGGER tiktok_returns_updated_at
    BEFORE UPDATE ON admin.tiktok_returns
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();
