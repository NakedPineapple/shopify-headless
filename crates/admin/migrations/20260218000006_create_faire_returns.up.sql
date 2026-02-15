-- Faire wholesale returns and refunds.

SET search_path TO admin, public;

CREATE TABLE admin.faire_returns (
    id                      SERIAL PRIMARY KEY,
    faire_return_token      TEXT NOT NULL UNIQUE,
    faire_order_token       TEXT,
    return_status           TEXT NOT NULL DEFAULT 'PENDING',
    return_reason           TEXT,
    retailer_note           TEXT,
    refund_amount           TEXT,
    currency                TEXT,
    decision_deadline       TIMESTAMPTZ,
    raw_json                JSONB,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_faire_returns_order ON admin.faire_returns(faire_order_token) WHERE faire_order_token IS NOT NULL;
CREATE INDEX idx_faire_returns_status ON admin.faire_returns(return_status);

CREATE TRIGGER faire_returns_updated_at
    BEFORE UPDATE ON admin.faire_returns
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();
