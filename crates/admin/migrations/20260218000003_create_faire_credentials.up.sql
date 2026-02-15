SET search_path TO admin, public;

CREATE TABLE admin.faire_credentials (
    id SERIAL PRIMARY KEY,
    account_name TEXT NOT NULL UNIQUE DEFAULT 'default',
    brand_id TEXT NOT NULL,
    api_token TEXT NOT NULL,
    connected_by INTEGER REFERENCES admin.admin_user(id),
    connected_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER faire_credentials_updated_at
    BEFORE UPDATE ON admin.faire_credentials
    FOR EACH ROW EXECUTE FUNCTION admin.update_updated_at_column();
