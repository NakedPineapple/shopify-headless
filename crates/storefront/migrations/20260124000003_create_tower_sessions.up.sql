-- Create sessions table for tower-sessions PostgresStore

SET search_path TO storefront, public;

CREATE TABLE storefront.sessions (
    id TEXT PRIMARY KEY,
    data BYTEA NOT NULL,
    expiry_date TIMESTAMPTZ NOT NULL
);

CREATE INDEX idx_sessions_expiry_date ON storefront.sessions(expiry_date);
