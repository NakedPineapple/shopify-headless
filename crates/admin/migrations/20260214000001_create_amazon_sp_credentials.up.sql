-- Amazon SP-API credentials for direct Selling Partner API access.
-- Stores Login with Amazon (LWA) OAuth tokens and AWS IAM credentials.

SET search_path TO admin, public;

CREATE TABLE admin.amazon_sp_credentials (
    id SERIAL PRIMARY KEY,
    account_name TEXT NOT NULL UNIQUE DEFAULT 'default',
    lwa_client_id TEXT NOT NULL,
    lwa_client_secret TEXT NOT NULL,
    lwa_refresh_token TEXT NOT NULL,
    aws_access_key_id TEXT NOT NULL,
    aws_secret_access_key TEXT NOT NULL,
    seller_id TEXT NOT NULL,
    marketplace_id TEXT NOT NULL DEFAULT 'ATVPDKIKX0DER',
    access_token TEXT,
    access_token_expires_at BIGINT,
    connected_by INTEGER REFERENCES admin.admin_user(id),
    connected_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER amazon_sp_credentials_updated_at
    BEFORE UPDATE ON admin.amazon_sp_credentials
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();
