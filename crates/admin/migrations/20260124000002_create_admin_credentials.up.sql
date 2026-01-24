-- Create admin_credential table for WebAuthn passkeys
-- Admins can have multiple passkeys (phone, laptop, security key, etc.)

SET search_path TO admin, public;

CREATE TABLE admin.admin_credential (
    id SERIAL PRIMARY KEY,
    admin_user_id INTEGER NOT NULL REFERENCES admin.admin_user(id) ON DELETE CASCADE,
    credential_id BYTEA NOT NULL UNIQUE,
    public_key BYTEA NOT NULL,
    counter INTEGER NOT NULL DEFAULT 0,
    name TEXT NOT NULL DEFAULT 'Passkey',
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_admin_credential_admin_user_id ON admin.admin_credential(admin_user_id);
CREATE INDEX idx_admin_credential_credential_id ON admin.admin_credential(credential_id);
