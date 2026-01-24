-- Create user_credentials table for WebAuthn passkeys
-- Users can have multiple passkeys (phone, laptop, security key, etc.)

SET search_path TO storefront, public;

CREATE TABLE storefront.user_credential (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES storefront.user(id) ON DELETE CASCADE,
    credential_id BYTEA NOT NULL UNIQUE,
    public_key BYTEA NOT NULL,
    counter INTEGER NOT NULL DEFAULT 0,
    name TEXT NOT NULL DEFAULT 'Passkey',
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_user_credential_user_id ON storefront.user_credential(user_id);
CREATE INDEX idx_user_credential_credential_id ON storefront.user_credential(credential_id);
