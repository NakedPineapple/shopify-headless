-- Admin authentication tables
SET search_path TO admin, public;

-- Session table for tower-sessions PostgresStore
CREATE TABLE admin.session (
    id TEXT PRIMARY KEY,
    data BYTEA NOT NULL,
    expiry_date TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_admin_session_expiry ON admin.session(expiry_date);
