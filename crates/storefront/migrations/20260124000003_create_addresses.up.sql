-- Create addresses table for user shipping/billing addresses
-- Follows Shopify address format for compatibility

SET search_path TO storefront, public;

CREATE TABLE storefront.address (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES storefront.user(id) ON DELETE CASCADE,
    first_name TEXT NOT NULL,
    last_name TEXT NOT NULL,
    address1 TEXT NOT NULL,
    address2 TEXT,
    city TEXT NOT NULL,
    province TEXT NOT NULL,
    province_code TEXT NOT NULL,
    country TEXT NOT NULL,
    country_code TEXT NOT NULL,
    zip TEXT NOT NULL,
    phone TEXT,
    is_default BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc'),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT (CURRENT_TIMESTAMP AT TIME ZONE 'utc')
);

CREATE INDEX idx_address_user_id ON storefront.address(user_id);

CREATE TRIGGER address_updated_at
    BEFORE UPDATE ON storefront.address
    FOR EACH ROW
    EXECUTE FUNCTION storefront.update_updated_at_column();
