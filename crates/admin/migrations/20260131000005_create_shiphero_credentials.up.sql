-- ShipHero API credentials (JWT tokens from email/password authentication)

SET search_path TO admin, public;

CREATE TABLE admin.shiphero_credentials (
    id SERIAL PRIMARY KEY,
    -- Only one ShipHero connection per installation
    account_name TEXT NOT NULL UNIQUE DEFAULT 'default',
    -- Email used for authentication (for display only, not re-auth)
    email TEXT NOT NULL,
    -- JWT access token from ShipHero auth endpoint
    access_token TEXT NOT NULL,
    -- Refresh token for obtaining new access tokens (if provided by ShipHero)
    refresh_token TEXT,
    -- Unix timestamp when access token expires
    access_token_expires_at BIGINT NOT NULL,
    -- Unix timestamp when refresh token expires (if applicable)
    refresh_token_expires_at BIGINT,
    -- Who connected the account (audit trail)
    connected_by INTEGER REFERENCES admin.admin_user(id),
    connected_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    -- Last successful API call (for health monitoring)
    last_used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER shiphero_credentials_updated_at
    BEFORE UPDATE ON admin.shiphero_credentials
    FOR EACH ROW
    EXECUTE FUNCTION admin.update_updated_at_column();
