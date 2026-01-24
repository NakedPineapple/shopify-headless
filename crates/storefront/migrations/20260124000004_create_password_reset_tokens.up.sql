-- Create password_reset_tokens table for email-based password recovery

SET search_path TO storefront, public;

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
