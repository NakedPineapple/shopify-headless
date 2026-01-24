-- Create settings table for application configuration
-- Key-value store with JSONB values for flexibility

SET search_path TO admin, public;

CREATE TABLE admin.settings (
    id SERIAL PRIMARY KEY,
    key TEXT NOT NULL UNIQUE,
    value JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_settings_key ON admin.settings(key);

CREATE TRIGGER settings_updated_at
    BEFORE UPDATE ON admin.settings
    FOR EACH ROW
    EXECUTE FUNCTION admin.update_updated_at_column();
