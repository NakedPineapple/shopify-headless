-- Create users table for storefront authentication
-- Separate from Shopify customers - local auth only

SET search_path TO storefront, public;

CREATE TABLE storefront.user (
    id SERIAL PRIMARY KEY,
    email CITEXT NOT NULL UNIQUE,
    password_hash TEXT,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_user_email ON storefront.user(email);

-- Trigger to auto-update updated_at
CREATE OR REPLACE FUNCTION storefront.update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = (CURRENT_TIMESTAMP AT TIME ZONE 'utc');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER user_updated_at
    BEFORE UPDATE ON storefront.user
    FOR EACH ROW
    EXECUTE FUNCTION storefront.update_updated_at_column();
