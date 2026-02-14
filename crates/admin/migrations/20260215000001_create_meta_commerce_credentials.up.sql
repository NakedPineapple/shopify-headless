-- Meta Commerce credentials for Graph API access.
-- Stores Facebook App credentials and Page Access Token.

SET search_path TO admin, public;

CREATE TABLE admin.meta_commerce_credentials (
    id SERIAL PRIMARY KEY,
    account_name TEXT NOT NULL UNIQUE DEFAULT 'default',
    app_id TEXT NOT NULL,
    app_secret TEXT NOT NULL,
    page_access_token TEXT NOT NULL,
    page_id TEXT NOT NULL,
    commerce_account_id TEXT NOT NULL,
    catalog_id TEXT NOT NULL,
    token_expires_at BIGINT,
    connected_by INTEGER REFERENCES admin.admin_user(id),
    connected_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER meta_commerce_credentials_updated_at
    BEFORE UPDATE ON admin.meta_commerce_credentials
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();
