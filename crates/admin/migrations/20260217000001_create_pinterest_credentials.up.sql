SET search_path TO admin, public;

CREATE TABLE admin.pinterest_credentials (
    id SERIAL PRIMARY KEY,
    account_name TEXT NOT NULL UNIQUE DEFAULT 'default',
    app_id TEXT NOT NULL,
    app_secret TEXT NOT NULL,
    access_token TEXT NOT NULL,
    refresh_token TEXT NOT NULL,
    ad_account_id TEXT NOT NULL,
    token_expires_at BIGINT,
    connected_by INTEGER REFERENCES admin.admin_user(id),
    connected_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER pinterest_credentials_updated_at
    BEFORE UPDATE ON admin.pinterest_credentials
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();
