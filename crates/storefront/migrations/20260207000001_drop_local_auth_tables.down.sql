-- Recreate local auth tables (reverses drop_local_auth_tables).
-- This restores the tables to their final state (including all ALTER TABLE additions).

SET search_path TO storefront, public;

-- 1. user (parent table)
CREATE TABLE storefront.user (
    id SERIAL PRIMARY KEY,
    email CITEXT NOT NULL UNIQUE,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_user_email ON storefront.user(email);

CREATE TRIGGER user_updated_at
    BEFORE UPDATE ON storefront.user
    FOR EACH ROW
    EXECUTE FUNCTION storefront.update_updated_at_column();

-- 2. user_password
CREATE TABLE storefront.user_password (
    user_id INTEGER PRIMARY KEY REFERENCES storefront.user(id) ON DELETE CASCADE,
    password_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE TRIGGER user_password_updated_at
    BEFORE UPDATE ON storefront.user_password
    FOR EACH ROW
    EXECUTE FUNCTION storefront.update_updated_at_column();

-- 3. user_credential (with shopify_customer_id and email columns)
CREATE TABLE storefront.user_credential (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES storefront.user(id) ON DELETE CASCADE,
    credential_id BYTEA NOT NULL UNIQUE,
    public_key BYTEA NOT NULL,
    counter INTEGER NOT NULL DEFAULT 0,
    name TEXT NOT NULL DEFAULT 'Passkey',
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    shopify_customer_id TEXT,
    email TEXT
);

CREATE INDEX idx_user_credential_user_id ON storefront.user_credential(user_id);
CREATE INDEX idx_user_credential_credential_id ON storefront.user_credential(credential_id);
CREATE INDEX idx_user_credential_shopify_customer_id
    ON storefront.user_credential(shopify_customer_id)
    WHERE shopify_customer_id IS NOT NULL;
CREATE INDEX idx_user_credential_email ON storefront.user_credential(email)
    WHERE email IS NOT NULL;

-- 4. password_reset_token
CREATE TABLE storefront.password_reset_token (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES storefront.user(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_password_reset_token_user_id ON storefront.password_reset_token(user_id);
CREATE INDEX idx_password_reset_token_expires_at ON storefront.password_reset_token(expires_at);

-- 5. email_verification_code
CREATE TABLE storefront.email_verification_code (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES storefront.user(id) ON DELETE CASCADE,
    code_hash TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_email_verification_code_user_id ON storefront.email_verification_code(user_id);
CREATE INDEX idx_email_verification_code_expires_at ON storefront.email_verification_code(expires_at);
