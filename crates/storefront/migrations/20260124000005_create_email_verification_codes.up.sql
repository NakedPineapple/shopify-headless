-- Create email_verification_codes table for email verification flow

SET search_path TO storefront, public;

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
