-- TikTok Shop credentials for Open API v2 access.
-- Stores app credentials, OAuth tokens, and shop identity.

SET search_path TO admin, public;

CREATE TABLE admin.tiktok_shop_credentials (
    id SERIAL PRIMARY KEY,
    account_name TEXT NOT NULL UNIQUE DEFAULT 'default',
    app_key TEXT NOT NULL,
    app_secret TEXT NOT NULL,
    access_token TEXT NOT NULL,
    refresh_token TEXT NOT NULL,
    shop_id TEXT NOT NULL,
    shop_cipher TEXT NOT NULL,
    token_expires_at BIGINT,
    connected_by INTEGER REFERENCES admin.admin_user(id),
    connected_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER tiktok_shop_credentials_updated_at
    BEFORE UPDATE ON admin.tiktok_shop_credentials
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();
