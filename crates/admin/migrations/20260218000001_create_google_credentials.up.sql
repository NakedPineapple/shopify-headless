SET search_path TO admin, public;

CREATE TABLE admin.google_credentials (
    id SERIAL PRIMARY KEY,
    account_name TEXT NOT NULL UNIQUE DEFAULT 'default',
    merchant_id TEXT NOT NULL,
    client_id TEXT NOT NULL,
    client_secret TEXT NOT NULL,
    access_token TEXT NOT NULL,
    refresh_token TEXT NOT NULL,
    token_expires_at BIGINT,
    connected_by INTEGER REFERENCES admin.admin_user(id),
    connected_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER google_credentials_updated_at
    BEFORE UPDATE ON admin.google_credentials
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();
