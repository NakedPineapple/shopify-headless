-- Shopify OAuth tokens for Admin API access

SET search_path TO admin, public;

CREATE TABLE admin.shopify_token (
    id SERIAL PRIMARY KEY,
    shop TEXT NOT NULL UNIQUE,
    access_token TEXT NOT NULL,
    scope TEXT NOT NULL,
    obtained_at BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_shopify_token_shop ON admin.shopify_token(shop);

CREATE TRIGGER shopify_token_updated_at
    BEFORE UPDATE ON admin.shopify_token
    FOR EACH ROW
    EXECUTE FUNCTION admin.update_updated_at_column();
